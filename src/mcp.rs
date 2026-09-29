//! Stdio MCP proxy. Not a model tool.
//!
//! Newline-delimited JSON-RPC. `tools/call` is admitted before the child sees
//! it. Every other method is forwarded. A deny is a JSON-RPC error whose
//! message is the code token. The child is not invoked.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use crate::{bind_refused, AdmitDecision, Deadbolt, DeadboltError};

/// Run an MCP server as a child. Admit `tools/call` in-process, or over
/// `serve_sock` when set. Does not bind `0.0.0.0`.
pub fn mcp_proxy(
    agent: &str,
    gate: &Deadbolt,
    serve_sock: Option<&Path>,
    argv: &[String],
) -> Result<(), DeadboltError> {
    if argv.is_empty() {
        return Err(DeadboltError::McpSpawn);
    }
    if let Some(sock) = serve_sock {
        let sock = normalize_sock(sock)?;
        if bind_refused(&sock) {
            return Err(DeadboltError::BindRefused);
        }
        if let Err(err) = sock_ensure(&sock, agent) {
            eprintln!("{err}");
        }
        let sock = sock.clone();
        let agent = agent.to_string();
        return spawn_proxy(argv, move |tool| sock_decision(&sock, &agent, tool));
    }
    if let Err(err) = gate.ensure_agent(agent) {
        eprintln!("{err}");
    }
    let gate = gate.clone();
    let agent = agent.to_string();
    spawn_proxy(argv, move |tool| match gate.admit(&agent, tool) {
        AdmitDecision::Allow => None,
        AdmitDecision::Deny { code } => Some(code.as_str().to_string()),
    })
}

fn spawn_proxy(
    argv: &[String],
    admit: impl Fn(&str) -> Option<String>,
) -> Result<(), DeadboltError> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn().map_err(|_| DeadboltError::McpSpawn)?;
    let child_in = child.stdin.take().ok_or(DeadboltError::McpSpawn)?;
    let child_out = child.stdout.take().ok_or(DeadboltError::McpSpawn)?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let ran = proxy_loop(
        stdin.lock(),
        stdout,
        child_in,
        BufReader::new(child_out),
        &admit,
    );
    let _ = child.kill();
    let _ = child.wait();
    ran.map_err(|_| DeadboltError::StoreUnavailable)
}

fn proxy_loop<R, W, CW, CR>(
    mut client_in: R,
    client_out: W,
    mut child_in: CW,
    mut child_out: CR,
    admit: &dyn Fn(&str) -> Option<String>,
) -> std::io::Result<()>
where
    R: BufRead,
    W: Write + Send + 'static,
    CW: Write,
    CR: BufRead + Send + 'static,
{
    let client_out = Arc::new(Mutex::new(client_out));
    let out_for_child = Arc::clone(&client_out);
    let reader = thread::spawn(move || {
        let mut line = String::new();
        loop {
            line.clear();
            match child_out.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let mut w = out_for_child.lock().unwrap_or_else(|e| e.into_inner());
                    if w.write_all(line.as_bytes()).is_err() {
                        break;
                    }
                    let _ = w.flush();
                }
            }
        }
    });
    let mut line = String::new();
    loop {
        line.clear();
        match client_in.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let _ = dispatch(&line, admit, &mut child_in, &client_out);
            }
        }
    }
    drop(child_in);
    let _ = reader.join();
    Ok(())
}

fn dispatch(
    line: &str,
    admit: &dyn Fn(&str) -> Option<String>,
    child: &mut impl Write,
    client: &Mutex<impl Write>,
) -> std::io::Result<()> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let Ok(msg) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(());
    };
    if msg.get("method").and_then(|m| m.as_str()) == Some("tools/call") {
        let Some(name) = msg
            .get("params")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .filter(|s| !s.is_empty())
        else {
            if let Some(id) = request_id(&msg) {
                write_client(client, &error_line(id, -32602, "bad_request"))?;
            }
            return Ok(());
        };
        if let Some(code) = admit(name) {
            if let Some(id) = request_id(&msg) {
                write_client(client, &error_line(id, -32000, &code))?;
            }
            return Ok(());
        }
    }
    child.write_all(forward_line(line).as_bytes())?;
    child.flush()
}

fn request_id(msg: &Value) -> Option<&Value> {
    msg.get("id").filter(|id| !id.is_null())
}

fn error_line(id: &Value, rpc: i64, token: &str) -> String {
    format!(
        "{}\n",
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": rpc, "message": token, "data": {"code": token}}
        })
    )
}

fn forward_line(line: &str) -> String {
    format!("{}\n", line.trim_end_matches(['\r', '\n']))
}

fn write_client(client: &Mutex<impl Write>, line: &str) -> std::io::Result<()> {
    let mut w = client.lock().unwrap_or_else(|e| e.into_inner());
    w.write_all(line.as_bytes())?;
    w.flush()
}

fn normalize_sock(path: &Path) -> Result<PathBuf, DeadboltError> {
    let raw = path.to_string_lossy();
    if raw.starts_with("https://") {
        return Err(DeadboltError::BindRefused);
    }
    let raw = raw.strip_prefix("http://").unwrap_or(&raw);
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return Ok(home.join(rest));
        }
    }
    Ok(PathBuf::from(raw))
}

fn sock_ensure(sock: &Path, agent: &str) -> Result<(), DeadboltError> {
    let body = json!({"agent_id": agent}).to_string();
    match http_json(sock, "POST", "/ensure", Some(&body)) {
        Ok((401, _)) => {
            eprintln!("deadbolt:unauthorized");
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    }
}

fn sock_decision(sock: &Path, agent: &str, tool: &str) -> Option<String> {
    let body = json!({"agent_id": agent, "tool": tool}).to_string();
    match http_json(sock, "POST", "/admit", Some(&body)) {
        Ok((401, _)) => Some("unauthorized".to_string()),
        Ok((_, v)) if v.get("decision").and_then(|d| d.as_str()) == Some("allow") => None,
        Ok((_, v)) => Some(
            v.get("code")
                .and_then(|c| c.as_str())
                .unwrap_or("store_unavailable")
                .to_string(),
        ),
        Err(_) => Some("store_unavailable".to_string()),
    }
}

fn http_json(
    sock: &Path,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, Value), DeadboltError> {
    let body = body.unwrap_or("");
    let token = std::env::var("DEADBOLT_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());
    let token_line = token
        .as_deref()
        .map(|t| format!("x-deadbolt-token: {t}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\n{token_line}content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = dial(sock)?;
    stream
        .write_all(req.as_bytes())
        .map_err(|_| DeadboltError::StoreUnavailable)?;
    let _ = stream.shutdown_write();
    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .map_err(|_| DeadboltError::StoreUnavailable)?;
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let json_body = raw.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
    let value = serde_json::from_str(json_body).unwrap_or(Value::Null);
    Ok((status, value))
}

enum Dial {
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl Dial {
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Unix(s) => s.write_all(bytes),
            Self::Tcp(s) => s.write_all(bytes),
        }
    }

    fn shutdown_write(&mut self) -> std::io::Result<()> {
        match self {
            Self::Unix(s) => s.shutdown(Shutdown::Write),
            Self::Tcp(s) => s.shutdown(Shutdown::Write),
        }
    }

    fn read_to_string(&mut self, out: &mut String) -> std::io::Result<usize> {
        match self {
            Self::Unix(s) => s.read_to_string(out),
            Self::Tcp(s) => s.read_to_string(out),
        }
    }
}

fn dial(path: &Path) -> Result<Dial, DeadboltError> {
    if bind_refused(path) {
        return Err(DeadboltError::BindRefused);
    }
    let raw = path.to_string_lossy();
    if let Some(addr) = tcp_loopback(&raw) {
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        return Ok(Dial::Tcp(stream));
    }
    let stream = UnixStream::connect(path).map_err(|_| DeadboltError::StoreUnavailable)?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    Ok(Dial::Unix(stream))
}

fn tcp_loopback(raw: &str) -> Option<SocketAddr> {
    if raw.contains('/') {
        return None;
    }
    if let Some(rest) = raw.strip_prefix('[') {
        let (host, port) = rest.split_once("]:")?;
        if host == "::1" {
            return Some(SocketAddr::new(
                IpAddr::V6(Ipv6Addr::LOCALHOST),
                port.parse().ok()?,
            ));
        }
        return None;
    }
    let (host, port) = raw.rsplit_once(':')?;
    let port = port.parse().ok()?;
    if host == "127.0.0.1" {
        return Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    if host == "::1" {
        return Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;

    struct ChanWriter {
        tx: Option<mpsc::Sender<String>>,
        buf: Vec<u8>,
    }

    impl ChanWriter {
        fn new(tx: mpsc::Sender<String>) -> Self {
            Self {
                tx: Some(tx),
                buf: Vec::new(),
            }
        }
    }

    impl Write for ChanWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.buf.extend_from_slice(data);
            while let Some(i) = self.buf.iter().position(|&b| b == b'\n') {
                let chunk: Vec<u8> = self.buf.drain(..=i).collect();
                let line = String::from_utf8_lossy(&chunk).into_owned();
                let tx = self
                    .tx
                    .as_ref()
                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed"))?;
                tx.send(line)
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed"))?;
            }
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct ChanReader {
        rx: mpsc::Receiver<String>,
        pending: Vec<u8>,
        pos: usize,
    }

    impl Read for ChanReader {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.pending.len() {
                match self.rx.recv() {
                    Ok(line) => {
                        self.pending = line.into_bytes();
                        self.pos = 0;
                    }
                    Err(_) => return Ok(0),
                }
            }
            let n = (self.pending.len() - self.pos).min(out.len());
            if n == 0 {
                return Ok(0);
            }
            out[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    fn temp_gate() -> (PathBuf, Deadbolt) {
        static N: AtomicU64 = AtomicU64::new(1);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dbmcp{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let gate = Deadbolt::open_at(&dir, true, 60);
        (dir, gate)
    }

    fn drive(gate: &Deadbolt, lines: &[&str]) -> (Vec<String>, Vec<String>) {
        let input = lines.iter().map(|l| format!("{l}\n")).collect::<String>();
        let (child_tx, child_rx) = mpsc::channel::<String>();
        let (resp_tx, resp_rx) = mpsc::channel::<String>();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_bg = Arc::clone(&seen);
        thread::spawn(move || {
            while let Ok(line) = child_rx.recv() {
                seen_bg.lock().unwrap().push(line.clone());
                if let Ok(msg) = serde_json::from_str::<Value>(line.trim()) {
                    if let Some(id) = msg.get("id").filter(|v| !v.is_null()) {
                        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
                        let result = match method {
                            "tools/call" => {
                                json!({"content":[{"type":"text","text":"ran"}],"isError":false})
                            }
                            "tools/list" => json!({"tools":[{"name":"shell"}]}),
                            _ => json!({"ok":true}),
                        };
                        let resp = json!({"jsonrpc":"2.0","id":id,"result":result});
                        let _ = resp_tx.send(format!("{resp}\n"));
                    }
                }
            }
        });
        let (out_tx, out_rx) = mpsc::channel::<String>();
        let gate = gate.clone();
        proxy_loop(
            Cursor::new(input),
            ChanWriter::new(out_tx),
            ChanWriter::new(child_tx),
            BufReader::new(ChanReader {
                rx: resp_rx,
                pending: Vec::new(),
                pos: 0,
            }),
            &move |tool| match gate.admit("shop-bot", tool) {
                AdmitDecision::Allow => None,
                AdmitDecision::Deny { code } => Some(code.as_str().to_string()),
            },
        )
        .unwrap();
        let seen = seen.lock().unwrap().clone();
        let mut out = Vec::new();
        while let Ok(line) = out_rx.try_recv() {
            out.push(line);
        }
        (seen, out)
    }

    #[test]
    fn mcp_proxy_allow_forwards_tools_call() {
        let (dir, gate) = temp_gate();
        gate.ensure_agent("shop-bot").unwrap();
        let (seen, out) = drive(
            &gate,
            &[
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"shell","arguments":{}}}"#,
            ],
        );
        assert!(seen.iter().any(|l| l.contains("\"initialize\"")));
        assert!(seen.iter().any(|l| l.contains("\"tools/call\"")));
        assert!(out.iter().any(|l| l.contains("\"ran\"")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mcp_proxy_killed_does_not_call_child() {
        let (dir, gate) = temp_gate();
        gate.ensure_agent("shop-bot").unwrap();
        gate.kill("shop-bot").unwrap();
        let (seen, out) = drive(
            &gate,
            &[r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"shell"}}"#],
        );
        assert!(
            seen.is_empty(),
            "deny path must not touch the child: {seen:?}"
        );
        assert_eq!(out.len(), 1);
        let v: Value = serde_json::from_str(out[0].trim()).unwrap();
        assert_eq!(v["error"]["message"], "killed");
        assert_eq!(v["error"]["data"]["code"], "killed");
        assert_eq!(v["id"], 7);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mcp_proxy_list_tools_still_works_after_kill() {
        let (dir, gate) = temp_gate();
        gate.ensure_agent("shop-bot").unwrap();
        gate.kill("shop-bot").unwrap();
        let (seen, out) = drive(
            &gate,
            &[
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
                r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
                r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#,
                r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"shell"}}"#,
            ],
        );
        let child = seen.join("\n");
        assert!(child.contains("tools/list"));
        assert!(child.contains("\"ping\""));
        assert!(child.contains("resources/list"));
        assert!(
            !child.contains("tools/call"),
            "killed tools/call must not reach the child: {child}"
        );
        assert!(out.iter().any(|l| l.contains("\"shell\"")));
        assert!(out.iter().any(|l| l.contains("\"killed\"")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
