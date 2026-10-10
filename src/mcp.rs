//! Stdio MCP proxy. Not a model tool.
//!
//! Newline-delimited JSON-RPC. `tools/call` is admitted before the child sees
//! it. Every other method is forwarded. A deny is a JSON-RPC error whose
//! message is the code token. The child is not invoked.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpStream};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::{bind_refused, AdmitDecision, Deadbolt, DeadboltError};

type AdmitFn = dyn Fn(&str, Option<&str>, Option<f64>) -> Option<String>;
const MAX_MCP_FRAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const SHUTDOWN_GRACE: Duration = Duration::from_secs(1);

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
    let scoped = std::env::var_os("DEADBOLT_ADMISSION_TOKEN").is_some();
    if scoped && serve_sock.is_none() {
        return Err(DeadboltError::BadRequest);
    }
    if let Some(sock) = serve_sock {
        let sock = normalize_sock(sock)?;
        if bind_refused(&sock) {
            return Err(DeadboltError::BindRefused);
        }
        if !scoped {
            if let Err(err) = sock_ensure(&sock, agent) {
                eprintln!("{err}");
            }
        }
        let sock = sock.clone();
        let agent = agent.to_string();
        return spawn_proxy(argv, move |tool, dest, usd| {
            if let Some(usd) = usd {
                if sock_spend(&sock, &agent, usd).is_err() {
                    return Some("store_unavailable".into());
                }
            }
            sock_decision(&sock, &agent, tool, dest)
        });
    }
    if let Err(err) = gate.ensure_agent(agent) {
        eprintln!("{err}");
    }
    let gate = gate.clone();
    let agent = agent.to_string();
    spawn_proxy(argv, move |tool, dest, usd| {
        if let Some(usd) = usd {
            if gate.spend_add(&agent, usd).is_err() {
                return Some("store_unavailable".into());
            }
        }
        match gate.admit_dest(&agent, tool, dest) {
            AdmitDecision::Allow => None,
            AdmitDecision::Deny { code } => Some(code.as_str().to_string()),
        }
    })
}

fn spawn_proxy(
    argv: &[String],
    admit: impl Fn(&str, Option<&str>, Option<f64>) -> Option<String> + 'static,
) -> Result<(), DeadboltError> {
    let mut cmd = Command::new(&argv[0]);
    cmd.env_remove("DEADBOLT_TOKEN")
        .env_remove("DEADBOLT_ADMISSION_TOKEN")
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn().map_err(|_| DeadboltError::McpSpawn)?;
    let child_in = child.stdin.take().ok_or(DeadboltError::McpSpawn)?;
    let child_out = child.stdout.take().ok_or(DeadboltError::McpSpawn)?;
    let ran = proxy_loop(
        BufReader::new(std::io::stdin()),
        std::io::stdout(),
        child_in,
        BufReader::new(child_out),
        &admit,
    );
    // The loop already drained final responses after closing child stdin.
    // Always reap the immediate child, including after a read/write failure.
    let stopped = stop_child(&mut child);
    ran.and(stopped)
        .map_err(|_| DeadboltError::StoreUnavailable)
}

fn stop_child(child: &mut Child) -> std::io::Result<()> {
    // EOF can precede availability of the OS exit status. Do not overwrite a
    // natural failure with our own kill while the process is still exiting.
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other("mcp_child_failed"))
            };
        }
        if started.elapsed() >= SHUTDOWN_GRACE {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    // The child can exit between the last poll and kill.
    if let Err(error) = child.kill() {
        match child.try_wait()? {
            Some(status) if status.success() => return Ok(()),
            _ => return Err(error),
        }
    }
    child.wait().map(|_| ())
}

enum ProxyEvent {
    ClientLine(String),
    ChildLine(String),
    ClientEof,
    ChildEof,
    Error(std::io::Error),
}

fn pipe_reader<R: BufRead + Send + 'static>(
    mut input: R,
    events: SyncSender<ProxyEvent>,
    active: Arc<AtomicBool>,
    client: bool,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while active.load(Ordering::Acquire) {
            let mut line = String::new();
            let (event, terminal) = match read_mcp_line(&mut input, &mut line) {
                Ok(0) => (
                    if client {
                        ProxyEvent::ClientEof
                    } else {
                        ProxyEvent::ChildEof
                    },
                    true,
                ),
                Ok(_) => (
                    if client {
                        ProxyEvent::ClientLine(line)
                    } else {
                        ProxyEvent::ChildLine(line)
                    },
                    false,
                ),
                Err(error) => (ProxyEvent::Error(error), true),
            };
            if !active.load(Ordering::Acquire) || events.send(event).is_err() || terminal {
                break;
            }
        }
    })
}

fn read_mcp_line(reader: &mut impl BufRead, line: &mut String) -> std::io::Result<usize> {
    let bytes = reader
        .take(MAX_MCP_FRAME_BYTES as u64 + 1)
        .read_line(line)?;
    if bytes > MAX_MCP_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "mcp_frame_too_large",
        ));
    }
    Ok(bytes)
}

fn proxy_loop<R, W, CW, CR>(
    client_in: R,
    client_out: W,
    child_in: CW,
    child_out: CR,
    admit: &AdmitFn,
) -> std::io::Result<()>
where
    R: BufRead + Send + 'static,
    W: Write,
    CW: Write,
    CR: BufRead + Send + 'static,
{
    // Readers only enqueue bounded frames. This thread alone admits calls and
    // writes output, so neither EOF nor a read failure depends on client input.
    let (events_tx, events_rx) = mpsc::sync_channel(2);
    let active = Arc::new(AtomicBool::new(true));
    let client_reader = pipe_reader(client_in, events_tx.clone(), active.clone(), true);
    let child_reader = pipe_reader(child_out, events_tx, active.clone(), false);
    let client_out = Mutex::new(client_out);
    let mut child_in = Some(child_in);
    let mut deadline: Option<Instant> = None;
    let ran = (|| {
        loop {
            let event = if let Some(end) = deadline {
                let remaining = end.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match events_rx.recv_timeout(remaining) {
                    Ok(event) => event,
                    Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match events_rx.recv() {
                    Ok(event) => event,
                    Err(_) => break,
                }
            };
            match event {
                ProxyEvent::ClientLine(line) => {
                    if let Some(child) = child_in.as_mut() {
                        dispatch(&line, admit, child, &client_out)?;
                    }
                }
                ProxyEvent::ChildLine(line) => write_client(&client_out, &line)?,
                ProxyEvent::ClientEof => {
                    // Closing stdin lets cooperative children finish. Drain
                    // final responses, bounded even if a descendant holds stdout.
                    child_in.take();
                    deadline = Some(Instant::now() + SHUTDOWN_GRACE);
                }
                ProxyEvent::ChildEof => break,
                ProxyEvent::Error(error) => return Err(error),
            }
        }
        Ok(())
    })();
    active.store(false, Ordering::Release);
    drop(events_rx); // Release readers waiting on the bounded queue.
    drop(child_in);
    // A reader blocked in OS input may outlive this call until that pipe closes.
    // It cannot admit or forward anything. Never join such a reader indefinitely.
    for reader in [client_reader, child_reader] {
        if reader.is_finished() && reader.join().is_err() && ran.is_ok() {
            return Err(std::io::Error::other("mcp_reader_thread_failed"));
        }
    }
    ran
}

fn dispatch(
    line: &str,
    admit: &AdmitFn,
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
    // MCP stdio carries individual JSON-RPC objects, not batches. Validate the
    // shape before routing and serialize the same parsed value we admit, so a
    // downstream parser cannot interpret duplicate keys differently.
    let valid_id = |id: &Value| id.is_string() || id.is_number();
    let valid_request = msg.get("method").is_some_and(Value::is_string)
        && msg.get("id").is_none_or(valid_id)
        && msg
            .get("params")
            .is_none_or(|p| p.is_object() || p.is_array())
        && msg.get("result").is_none()
        && msg.get("error").is_none();
    let valid_response = msg.get("method").is_none()
        && msg.get("params").is_none()
        && msg.get("id").is_some_and(valid_id)
        && (msg.get("result").is_some() != msg.get("error").is_some());
    if !msg.is_object()
        || msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !(valid_request || valid_response)
    {
        write_client(client, &error_line(&Value::Null, -32600, "bad_request"))?;
        return Ok(());
    }
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
        let dest = dest_of(&msg);
        let usd = spend_of(&msg);
        if let Some(code) = admit(name, dest.as_deref(), usd) {
            if let Some(id) = request_id(&msg) {
                write_client(client, &error_line(id, -32000, &code))?;
            }
            return Ok(());
        }
    }
    child.write_all(forward_line(&msg.to_string()).as_bytes())?;
    child.flush()
}

fn dest_of(msg: &Value) -> Option<String> {
    msg.get("params")
        .and_then(|p| p.get("arguments"))
        .and_then(find_dest)
}

fn find_dest(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["url", "uri", "href", "endpoint", "host"] {
                if let Some(raw) = map.get(key).and_then(|v| v.as_str()) {
                    if let Some(host) = host_from_field(key, raw) {
                        return Some(host);
                    }
                }
            }
            map.values().find_map(find_dest)
        }
        Value::Array(items) => items.iter().find_map(find_dest),
        _ => None,
    }
}

fn host_from_field(key: &str, raw: &str) -> Option<String> {
    if let Some(host) = host_in(raw) {
        return Some(host);
    }
    if key == "host" {
        bare_host(raw)
    } else {
        None
    }
}

fn bare_host(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.contains('/') || raw.contains(' ') || raw.contains('@') {
        return None;
    }
    let host = raw.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn spend_of(msg: &Value) -> Option<f64> {
    msg.get("params")
        .and_then(|p| p.get("arguments"))
        .and_then(find_spend)
}

fn find_spend(value: &Value) -> Option<f64> {
    match value {
        Value::Object(map) => {
            for key in ["amount", "usd", "cost"] {
                if let Some(n) = map.get(key).and_then(as_usd) {
                    return Some(n);
                }
            }
            map.values().find_map(find_spend)
        }
        Value::Array(items) => items.iter().find_map(find_spend),
        _ => None,
    }
}

fn as_usd(value: &Value) -> Option<f64> {
    if let Some(n) = value.as_f64() {
        return n.is_finite().then_some(n);
    }
    let raw = value.as_str()?.trim();
    let n = raw.parse::<f64>().ok()?;
    n.is_finite().then_some(n)
}

fn host_in(raw: &str) -> Option<String> {
    let rest = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let host = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next()?
    } else {
        host.split(':').next()?
    };
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
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

fn sock_decision(sock: &Path, agent: &str, tool: &str, dest: Option<&str>) -> Option<String> {
    let body = match dest {
        Some(dest) => json!({"agent_id": agent, "tool": tool, "dest": dest}).to_string(),
        None => json!({"agent_id": agent, "tool": tool}).to_string(),
    };
    match http_json(sock, "POST", "/admit", Some(&body)) {
        Ok((401, _)) => Some("unauthorized".to_string()),
        Ok((200..=299, v)) if v.get("decision").and_then(|d| d.as_str()) == Some("allow") => None,
        Ok((_, v)) => Some(
            v.get("code")
                .and_then(|c| c.as_str())
                .unwrap_or("store_unavailable")
                .to_string(),
        ),
        Err(_) => Some("store_unavailable".to_string()),
    }
}

fn sock_spend(sock: &Path, agent: &str, usd: f64) -> Result<(), ()> {
    let body = json!({"agent_id": agent, "usd": usd}).to_string();
    match http_json(sock, "POST", "/spend", Some(&body)) {
        Ok((200, v)) if v.get("ok").and_then(|x| x.as_bool()) == Some(true) => Ok(()),
        _ => Err(()),
    }
}

fn http_json(
    sock: &Path,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, Value), DeadboltError> {
    let body = body.unwrap_or("");
    let token_line = if std::env::var_os("DEADBOLT_ADMISSION_TOKEN").is_some() {
        let secret =
            std::env::var("DEADBOLT_ADMISSION_TOKEN").map_err(|_| DeadboltError::TokenRequired)?;
        if secret.is_empty()
            || !secret
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(DeadboltError::TokenRequired);
        }
        format!("x-deadbolt-admission: {secret}\r\n")
    } else {
        let token = std::env::var("DEADBOLT_TOKEN")
            .ok()
            .filter(|t| !t.is_empty());
        if token
            .as_ref()
            .is_some_and(|t| t.bytes().any(|b| b == b'\r' || b == b'\n'))
        {
            return Err(DeadboltError::TokenRequired);
        }
        token
            .as_deref()
            .map(|t| format!("x-deadbolt-token: {t}\r\n"))
            .unwrap_or_default()
    };
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
        .read_to_string_limited(&mut raw)
        .map_err(|_| DeadboltError::StoreUnavailable)?;
    if raw.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(DeadboltError::StoreUnavailable);
    }
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
    #[cfg(unix)]
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl Dial {
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.write_all(bytes),
            Self::Tcp(s) => s.write_all(bytes),
        }
    }

    fn shutdown_write(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.shutdown(Shutdown::Write),
            Self::Tcp(s) => s.shutdown(Shutdown::Write),
        }
    }

    fn read_to_string_limited(&mut self, out: &mut String) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.take(MAX_RESPONSE_BYTES + 1).read_to_string(out),
            Self::Tcp(s) => s.take(MAX_RESPONSE_BYTES + 1).read_to_string(out),
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
    #[cfg(not(unix))]
    return Err(DeadboltError::BindRefused);
    #[cfg(unix)]
    {
        let stream = UnixStream::connect(path).map_err(|_| DeadboltError::StoreUnavailable)?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        Ok(Dial::Unix(stream))
    }
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

    #[test]
    fn oversized_frame_never_reaches_admission_or_child() {
        let frame = json!({"jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{
            "name":"write_file", "arguments":{"padding":"x".repeat(MAX_MCP_FRAME_BYTES)}
        }})
        .to_string();
        let mut child = Vec::new();
        let (response_tx, response_rx) = mpsc::channel();
        let ran = proxy_loop(
            Cursor::new(frame),
            Vec::new(),
            &mut child,
            BufReader::new(ChanReader {
                rx: response_rx,
                pending: Vec::new(),
                pos: 0,
            }),
            &|_, _, _| panic!("oversized frame reached admission"),
        );
        assert_eq!(ran.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        drop(response_tx);
        assert!(child.is_empty());
    }

    #[test]
    fn retained_child_output_does_not_block_shutdown_or_forward_late_data() {
        let (response_tx, response_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        let started = Instant::now();
        let ran = proxy_loop(
            Cursor::new(Vec::<u8>::new()),
            ChanWriter::new(out_tx),
            Vec::new(),
            BufReader::new(ChanReader {
                rx: response_rx,
                pending: Vec::new(),
                pos: 0,
            }),
            &|_, _, _| panic!("EOF reached admission"),
        );
        ran.unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        // Release the simulated inherited pipe after the bounded drain. Its late bytes
        // must not escape into the client after the proxy has returned.
        let _ = response_tx.send("late output\n".to_string());
        drop(response_tx);
        assert!(matches!(
            out_rx.recv_timeout(SHUTDOWN_GRACE),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn child_eof_and_oversized_output_do_not_wait_for_client_eof() {
        for oversized in [false, true] {
            // Keep client input open while the child exits or fails framing.
            let (input_tx, input_rx) = mpsc::channel();
            let output = if oversized {
                format!("{}\n", "x".repeat(MAX_MCP_FRAME_BYTES + 1))
            } else {
                String::new()
            };
            let (out_tx, out_rx) = mpsc::channel();
            let started = Instant::now();
            let ran = proxy_loop(
                BufReader::new(ChanReader {
                    rx: input_rx,
                    pending: Vec::new(),
                    pos: 0,
                }),
                ChanWriter::new(out_tx),
                Vec::new(),
                Cursor::new(output),
                &|_, _, _| panic!("idle client reached admission"),
            );
            assert!(started.elapsed() < Duration::from_secs(2));
            if oversized {
                assert_eq!(ran.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
            } else {
                ran.unwrap();
            }
            // The detached input reader cannot admit or write after return.
            let _ = input_tx.send("late input\n".into());
            drop(input_tx);
            assert!(matches!(
                out_rx.try_recv(),
                Err(mpsc::TryRecvError::Disconnected)
            ));
        }
    }

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
        let ran = proxy_loop(
            Cursor::new(input),
            ChanWriter::new(out_tx),
            ChanWriter::new(child_tx),
            BufReader::new(ChanReader {
                rx: resp_rx,
                pending: Vec::new(),
                pos: 0,
            }),
            &move |tool, dest, usd| {
                if let Some(usd) = usd {
                    if gate.spend_add("shop-bot", usd).is_err() {
                        return Some("store_unavailable".into());
                    }
                }
                match gate.admit_dest("shop-bot", tool, dest) {
                    AdmitDecision::Allow => None,
                    AdmitDecision::Deny { code } => Some(code.as_str().to_string()),
                }
            },
        );
        ran.unwrap();
        let seen = seen.lock().unwrap().clone();
        let mut out = Vec::new();
        while let Ok(line) = out_rx.try_recv() {
            out.push(line);
        }
        (seen, out)
    }

    #[test]
    fn mcp_sidecar_error_status_cannot_allow_tool() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf).unwrap();
            let body = r#"{"decision":"allow"}"#;
            write!(stream, "HTTP/1.1 503 Service Unavailable\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        assert_eq!(
            sock_decision(Path::new(&addr.to_string()), "A", "shell", None),
            Some("store_unavailable".into())
        );
        server.join().unwrap();
    }

    #[test]
    fn mcp_rejects_batches_and_invalid_envelopes_before_child() {
        let (dir, gate) = temp_gate();
        gate.ensure_agent("shop-bot").unwrap();
        gate.kill("shop-bot").unwrap();
        let (seen, out) = drive(
            &gate,
            &[
                r#"[{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"shell"}}]"#,
                r#"null"#,
                r#"{"jsonrpc":"2.0","id":7,"method":null}"#,
                r#"{"jsonrpc":"1.0","id":7,"method":"tools/call","params":{"name":"shell"}}"#,
            ],
        );
        assert!(seen.is_empty(), "invalid envelopes reached child: {seen:?}");
        assert_eq!(out.len(), 4);
        for line in out {
            let value: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(value["error"]["code"], -32600);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mcp_duplicate_keys_forward_only_the_admitted_interpretation() {
        let (dir, gate) = temp_gate();
        gate.ensure_agent("shop-bot").unwrap();
        gate.kill("shop-bot").unwrap();
        let (seen, out) = drive(
            &gate,
            &[
                r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","method":"ping","params":{"name":"shell"}}"#,
                r#"{"jsonrpc":"2.0","id":9,"method":"ping","method":"tools/call","params":{"name":"shell"}}"#,
                r#"{"jsonrpc":"2.0","id":10,"result":{"ok":true}}"#,
            ],
        );
        assert_eq!(seen.len(), 2);
        assert!(!seen[0].contains("tools/call"));
        assert_eq!(seen[0].matches("\"method\"").count(), 1);
        assert!(seen[1].contains("\"result\""));
        assert!(out.iter().any(|line| line.contains("killed")));
        let _ = std::fs::remove_dir_all(dir);
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

    #[test]
    fn mcp_proxy_dest_policy_denies_foreign_host() {
        let (dir, gate) = temp_gate();
        gate.ensure_agent("shop-bot").unwrap();
        gate.set_policy(
            "shop-bot",
            crate::PolicyPatch {
                dest_allow: Some(vec!["api.stripe.com".into()]),
                ..crate::PolicyPatch::default()
            },
        )
        .unwrap();
        let (seen, out) = drive(
            &gate,
            &[
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"fetch","arguments":{"url":"https://github.com/acme"}}}"#,
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"fetch","arguments":{"note":"no host"}}}"#,
            ],
        );
        assert!(
            seen.is_empty(),
            "deny path must not touch the child: {seen:?}"
        );
        assert_eq!(out.len(), 2);
        for line in &out {
            let v: Value = serde_json::from_str(line.trim()).unwrap();
            assert_eq!(v["error"]["message"], "purpose_exceeded");
            assert_eq!(v["error"]["data"]["code"], "purpose_exceeded");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mcp_proxy_irreversible_does_not_call_child() {
        let (dir, gate) = temp_gate();
        gate.ensure_agent("shop-bot").unwrap();
        gate.set_policy(
            "shop-bot",
            crate::PolicyPatch {
                irreversible: Some(vec!["shell".into()]),
                ..crate::PolicyPatch::default()
            },
        )
        .unwrap();
        let (seen, out) = drive(
            &gate,
            &[
                r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"shell","arguments":{}}}"#,
            ],
        );
        assert!(
            seen.is_empty(),
            "needs_human must not touch the child: {seen:?}"
        );
        let v: Value = serde_json::from_str(out[0].trim()).unwrap();
        assert_eq!(v["error"]["message"], "needs_human");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mcp_proxy_off_list_tool_does_not_call_child() {
        let (dir, gate) = temp_gate();
        gate.ensure_agent("shop-bot").unwrap();
        gate.set_policy(
            "shop-bot",
            crate::PolicyPatch {
                tools_allow: Some(vec!["read_file".into()]),
                ..crate::PolicyPatch::default()
            },
        )
        .unwrap();
        let (seen, out) = drive(
            &gate,
            &[
                r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"shell","arguments":{}}}"#,
            ],
        );
        assert!(
            seen.is_empty(),
            "off-list tool must not touch the child: {seen:?}"
        );
        let v: Value = serde_json::from_str(out[0].trim()).unwrap();
        assert_eq!(v["error"]["message"], "purpose_exceeded");
        let _ = std::fs::remove_dir_all(dir);
    }
}
