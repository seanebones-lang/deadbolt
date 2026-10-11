//! Local sidecar. Not a model tool. Does not change admit semantics.
//!
//! Unix sockets stay mode `0600`. TCP accepts only `127.0.0.1` and `::1`,
//! and only when a token is configured. `kill`, `pause`, `clip`, and `resume`
//! stay on the CLI.

#[cfg(unix)]
use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::{AdmitDecision, Deadbolt, DeadboltConfig, DeadboltError};

const MAX_WORKERS: usize = 64;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REQUEST_BYTES: usize = 65_536;

struct WorkerPermit(Arc<AtomicUsize>);

impl WorkerPermit {
    fn acquire(active: &Arc<AtomicUsize>, limit: usize) -> Option<Self> {
        let mut count = active.load(Ordering::Acquire);
        loop {
            if count >= limit {
                return None;
            }
            match active.compare_exchange_weak(
                count,
                count + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(Self(Arc::clone(active))),
                Err(observed) => count = observed,
            }
        }
    }
}

impl Drop for WorkerPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

trait Connection: Read + Write + Send + 'static {
    fn read_timeout(&self, timeout: Duration) -> std::io::Result<()>;
    fn write_timeout(&self, timeout: Duration) -> std::io::Result<()>;
}

impl Connection for std::net::TcpStream {
    fn read_timeout(&self, t: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(t))
    }
    fn write_timeout(&self, t: Duration) -> std::io::Result<()> {
        self.set_write_timeout(Some(t))
    }
}

#[cfg(unix)]
impl Connection for std::os::unix::net::UnixStream {
    fn read_timeout(&self, t: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(t))
    }
    fn write_timeout(&self, t: Duration) -> std::io::Result<()> {
        self.set_write_timeout(Some(t))
    }
}

fn spawn_worker(
    gate: &Deadbolt,
    stream: impl Connection,
    token: &Option<String>,
    active: &Arc<AtomicUsize>,
) {
    let Some(permit) = WorkerPermit::acquire(active, MAX_WORKERS) else {
        return;
    };
    let gate = gate.clone();
    let token = token.clone();
    // A failed spawn drops the captured permit and connection, without panicking
    // the listener. Saturated listeners close excess sockets immediately.
    let _ = thread::Builder::new()
        .name("deadbolt-http".into())
        .spawn(move || {
            let _permit = permit;
            let _ = handle_io(&gate, stream, token.as_deref(), REQUEST_TIMEOUT);
        });
}

/// Default bind: Unix socket on Unix, loopback TCP on other platforms.
pub fn default_bind_path() -> PathBuf {
    if cfg!(not(unix)) {
        return PathBuf::from("127.0.0.1:9782");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".deadbolt")
        .join("deadbolt.sock")
}

/// True when the bind must be refused.
///
/// Unix paths are allowed. TCP is allowed only for `127.0.0.1` and `::1`.
/// `0.0.0.0`, `[::]`, and any other host are refused.
pub fn bind_refused(path: &Path) -> bool {
    matches!(classify_bind(path), BindKind::Refused)
}

enum BindKind {
    Unix,
    Loopback(SocketAddr),
    Refused,
}

fn classify_bind(path: &Path) -> BindKind {
    let raw = path.to_string_lossy();
    if raw.contains('/') || raw.contains('\\') {
        return BindKind::Unix;
    }
    if raw.contains("0.0.0.0") || raw.contains("[::]") || raw == "::" {
        return BindKind::Refused;
    }
    if let Some(addr) = loopback_addr(&raw) {
        return BindKind::Loopback(addr);
    }
    if looks_like_host_port(&raw) {
        return BindKind::Refused;
    }
    BindKind::Unix
}

fn looks_like_host_port(raw: &str) -> bool {
    if let Some(rest) = raw.strip_prefix('[') {
        return rest.split_once("]:").is_some_and(|(_, port)| port_ok(port));
    }
    raw.rsplit_once(':').is_some_and(|(_, port)| port_ok(port))
}

fn port_ok(port: &str) -> bool {
    !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())
}

fn loopback_addr(raw: &str) -> Option<SocketAddr> {
    if let Some(rest) = raw.strip_prefix('[') {
        let (host, port) = rest.split_once("]:")?;
        if host == "::1" && port_ok(port) {
            return Some(SocketAddr::new(
                IpAddr::V6(Ipv6Addr::LOCALHOST),
                port.parse().ok()?,
            ));
        }
        return None;
    }
    let (host, port) = raw.rsplit_once(':')?;
    if host == "127.0.0.1" && port_ok(port) {
        return Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            port.parse().ok()?,
        ));
    }
    if host == "::1" && port_ok(port) {
        return Some(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            port.parse().ok()?,
        ));
    }
    None
}

/// Block on `bind`, serving the store from `cfg`. Same db and JSONL as the CLI.
pub fn serve(cfg: &DeadboltConfig, bind: &Path) -> Result<(), DeadboltError> {
    if bind_refused(bind) {
        return Err(DeadboltError::BindRefused);
    }
    let token = resolve_token(cfg)?;
    match classify_bind(bind) {
        BindKind::Loopback(addr) => {
            if token.is_none() {
                return Err(DeadboltError::TokenRequired);
            }
            listen_tcp(Deadbolt::open(cfg), addr, token)
        }
        BindKind::Unix => listen(Deadbolt::open(cfg), bind, token),
        BindKind::Refused => Err(DeadboltError::BindRefused),
    }
}

fn resolve_token(cfg: &DeadboltConfig) -> Result<Option<String>, DeadboltError> {
    if let Some(name) = cfg.token_env.as_deref() {
        if let Ok(raw) = std::env::var(name) {
            let value = raw.trim();
            if !value.is_empty() {
                return Ok(Some(value.to_string()));
            }
        }
    }
    let Some(path) = cfg.token_file.as_deref() else {
        return Ok(None);
    };
    let path = Path::new(path);
    if !path.exists() {
        return Ok(None);
    }
    #[cfg(not(unix))]
    return Err(DeadboltError::BindRefused);
    #[cfg(unix)]
    {
        let mode = fs::metadata(path)
            .map_err(|_| DeadboltError::BindRefused)?
            .permissions()
            .mode()
            & 0o777;
        if mode != 0o600 {
            return Err(DeadboltError::BindRefused);
        }
        let raw = fs::read_to_string(path).map_err(|_| DeadboltError::BindRefused)?;
        let value = raw.trim();
        if value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value.to_string()))
        }
    }
}

#[cfg(not(unix))]
fn listen(_gate: Deadbolt, _bind: &Path, _token: Option<String>) -> Result<(), DeadboltError> {
    Err(DeadboltError::BindRefused)
}

#[cfg(unix)]
fn listen(gate: Deadbolt, bind: &Path, token: Option<String>) -> Result<(), DeadboltError> {
    if let Some(parent) = bind.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|_| DeadboltError::BindRefused)?;
        }
    }
    match fs::symlink_metadata(bind) {
        Ok(meta) => {
            if !meta.file_type().is_socket() {
                return Err(DeadboltError::BindRefused);
            }
            match std::os::unix::net::UnixStream::connect(bind) {
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(bind).map_err(|_| DeadboltError::BindRefused)?;
                }
                _ => return Err(DeadboltError::BindRefused),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(DeadboltError::BindRefused),
    }
    let listener = UnixListener::bind(bind).map_err(|_| DeadboltError::BindRefused)?;
    fs::set_permissions(bind, fs::Permissions::from_mode(0o600))
        .map_err(|_| DeadboltError::BindRefused)?;
    let active = Arc::new(AtomicUsize::new(0));
    for conn in listener.incoming() {
        let Ok(stream) = conn else {
            continue;
        };
        spawn_worker(&gate, stream, &token, &active);
    }
    Ok(())
}

fn listen_tcp(
    gate: Deadbolt,
    addr: SocketAddr,
    token: Option<String>,
) -> Result<(), DeadboltError> {
    let listener = TcpListener::bind(addr).map_err(|_| DeadboltError::BindRefused)?;
    let active = Arc::new(AtomicUsize::new(0));
    for conn in listener.incoming() {
        let Ok(stream) = conn else {
            continue;
        };
        spawn_worker(&gate, stream, &token, &active);
    }
    Ok(())
}

fn handle_io(
    gate: &Deadbolt,
    mut stream: impl Connection,
    token: Option<&str>,
    timeout: Duration,
) -> std::io::Result<()> {
    let deadline = Instant::now() + timeout;
    stream.write_timeout(timeout)?;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 2048];
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|t| !t.is_zero())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::TimedOut, "request deadline"))?;
        stream.read_timeout(remaining)?;
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "incomplete request",
            ));
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > MAX_REQUEST_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request too large",
            ));
        }
        if let Some(header_end) = find_header_end(&buf) {
            let header = std::str::from_utf8(&buf[..header_end]).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid header")
            })?;
            let need = content_length(header)?;
            if need > MAX_REQUEST_BYTES.saturating_sub(header_end + 4) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "request too large",
                ));
            }
            let have = buf.len().saturating_sub(header_end + 4);
            if have >= need {
                buf.truncate(header_end + 4 + need);
                break;
            }
        }
    }
    let raw = String::from_utf8_lossy(&buf).to_string();
    let (status, body) = dispatch_http(gate, &raw, token);
    let resp = format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes())?;
    Ok(())
}

fn unique_header(raw: &str, name: &str) -> Result<Option<String>, ()> {
    let mut found = None;
    let headers = raw.split_once("\r\n\r\n").map(|(h, _)| h).unwrap_or(raw);
    for line in headers.lines().skip(1) {
        let (key, value) = line.split_once(':').ok_or(())?;
        if key.eq_ignore_ascii_case(name) {
            if found.is_some() {
                return Err(());
            }
            found = Some(value.trim().to_string());
        }
    }
    Ok(found)
}

fn token_eq(got: &str, expected: &str) -> bool {
    let got = got.as_bytes();
    let expected = expected.as_bytes();
    if got.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in got.iter().zip(expected) {
        diff |= a ^ b;
    }
    diff == 0
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(header: &str) -> std::io::Result<usize> {
    let invalid = || std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid HTTP framing");
    let mut length = None;
    for line in header.lines().skip(1) {
        let (key, value) = line.split_once(':').ok_or_else(invalid)?;
        if key.eq_ignore_ascii_case("transfer-encoding") {
            return Err(invalid());
        }
        if key.eq_ignore_ascii_case("content-length") {
            if length.is_some()
                || value.trim().is_empty()
                || !value.trim().bytes().all(|b| b.is_ascii_digit())
            {
                return Err(invalid());
            }
            length = Some(value.trim().parse::<usize>().map_err(|_| invalid())?);
        }
    }
    Ok(length.unwrap_or(0))
}

fn dispatch_http(gate: &Deadbolt, raw: &str, token: Option<&str>) -> (u16, String) {
    let mut lines = raw.split("\r\n");
    let Some(req) = lines.next() else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let mut parts = req.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let body = raw.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
    // A supplied workload credential never falls back to operator/Unix access.
    let workload = match unique_header(raw, "x-deadbolt-admission") {
        Ok(v) => v,
        Err(()) => return (401, json!({"code":"unauthorized"}).to_string()),
    };
    let operator = match unique_header(raw, "x-deadbolt-token") {
        Ok(v) => v,
        Err(()) => return (401, json!({"code":"unauthorized"}).to_string()),
    };
    if let Some(secret) = workload {
        if operator.is_some() {
            return (401, json!({"code":"unauthorized"}).to_string());
        }
        if method != "POST" || !matches!(path, "/admit" | "/admit-action") {
            return (403, json!({"code":"forbidden"}).to_string());
        }
        if path == "/admit-action" {
            return admit_action(gate, body, Some(&secret));
        }
        return admit_scoped(gate, body, &secret);
    }
    if let Some(expected) = token {
        if !operator.is_some_and(|got| token_eq(&got, expected)) {
            return (401, json!({"code":"unauthorized"}).to_string());
        }
    } else {
        // Once scoped credentials exist, socket possession alone cannot confer
        // operator authority. CLI controls remain available to the trusted owner.
        match gate.has_credentials() {
            Ok(false) if operator.is_none() => {}
            Ok(_) => return (401, json!({"code":"unauthorized"}).to_string()),
            Err(_) => return (503, json!({"code":"store_unavailable"}).to_string()),
        }
    }
    match (method, path) {
        ("POST", "/admit") => admit(gate, body),
        ("POST", "/admit-action") => admit_action(gate, body, None),
        ("POST", "/ensure") => ensure(gate, body),
        ("POST", "/register_child") => register_child(gate, body),
        ("POST", "/policy") => policy(gate, body),
        ("POST", "/spend") => spend(gate, body),
        ("GET", "/status") => status(gate, query),
        _ => (404, json!({"code":"not_found"}).to_string()),
    }
}

fn admit(gate: &Deadbolt, body: &str) -> (u16, String) {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let Some(agent_id) = v.get("agent_id").and_then(|x| x.as_str()) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let Some(tool) = v.get("tool").and_then(|x| x.as_str()) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let dest = v.get("dest").and_then(|x| x.as_str());
    match gate.admit_dest(agent_id, tool, dest) {
        AdmitDecision::Allow => (200, json!({"decision":"allow"}).to_string()),
        AdmitDecision::Deny { code } => (
            200,
            json!({"decision":"deny","code": code.as_str()}).to_string(),
        ),
    }
}

fn admit_scoped(gate: &Deadbolt, body: &str, secret: &str) -> (u16, String) {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let (Some(agent), Some(tool)) = (
        v.get("agent_id").and_then(Value::as_str),
        v.get("tool").and_then(Value::as_str),
    ) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let dest = match v.get("dest") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.as_str()),
        _ => return (400, json!({"code":"bad_request"}).to_string()),
    };
    match gate.admit_credential(secret, agent, tool, dest) {
        Ok(AdmitDecision::Allow) => (200, json!({"decision":"allow"}).to_string()),
        Ok(AdmitDecision::Deny { code }) => (
            200,
            json!({"decision":"deny","code":code.as_str()}).to_string(),
        ),
        Err(DeadboltError::TokenRequired) => (401, json!({"code":"unauthorized"}).to_string()),
        Err(DeadboltError::BadRequest) => (400, json!({"code":"bad_request"}).to_string()),
        Err(_) => (503, json!({"code":"store_unavailable"}).to_string()),
    }
}

fn admit_action(gate: &Deadbolt, body: &str, secret: Option<&str>) -> (u16, String) {
    let action = match crate::ActionRequest::from_json(body) {
        Ok(a) => a,
        Err(_) => return (400, json!({"code":"bad_request"}).to_string()),
    };
    let decision = match secret {
        Some(secret) => gate.admit_action_credential(secret, &action),
        None => gate.admit_action(&action),
    };
    match decision {
        Ok(AdmitDecision::Allow) => (200, json!({"decision":"allow"}).to_string()),
        Ok(AdmitDecision::Deny { code }) => (
            200,
            json!({"decision":"deny","code":code.as_str()}).to_string(),
        ),
        Err(DeadboltError::TokenRequired) => (401, json!({"code":"unauthorized"}).to_string()),
        Err(DeadboltError::BadRequest) => (400, json!({"code":"bad_request"}).to_string()),
        Err(_) => (503, json!({"code":"store_unavailable"}).to_string()),
    }
}

fn policy(gate: &Deadbolt, body: &str) -> (u16, String) {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let Some(agent_id) = v.get("agent_id").and_then(|x| x.as_str()) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let patch = crate::PolicyPatch {
        tools_allow: string_list(v.get("tools")),
        dest_allow: string_list(v.get("dest")),
        spend_cap_usd: v.get("spend_cap").and_then(|x| x.as_f64()),
        irreversible: string_list(v.get("irreversible")),
    };
    match gate.set_policy(agent_id, patch) {
        Ok(()) => (200, json!({"ok":true}).to_string()),
        Err(e) => (200, json!({"ok":false,"code": err_token(&e)}).to_string()),
    }
}

fn spend(gate: &Deadbolt, body: &str) -> (u16, String) {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let Some(agent_id) = v.get("agent_id").and_then(|x| x.as_str()) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let Some(usd) = v.get("usd").and_then(|x| x.as_f64()) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    match gate.spend_add(agent_id, usd) {
        Ok(added) => (
            200,
            json!({"ok":true,"spend_usd": added.spend_usd, "code": if added.paused { "spend_cap" } else { "ok" }}).to_string(),
        ),
        Err(e) => (200, json!({"ok":false,"code": err_token(&e)}).to_string()),
    }
}

fn string_list(value: Option<&Value>) -> Option<Vec<String>> {
    let arr = value?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
    )
}

fn ensure(gate: &Deadbolt, body: &str) -> (u16, String) {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let Some(agent_id) = v.get("agent_id").and_then(|x| x.as_str()) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    match gate.ensure_agent(agent_id) {
        Ok(()) => (200, json!({"ok":true}).to_string()),
        Err(e) => (200, json!({"ok":false,"code": err_token(&e)}).to_string()),
    }
}

fn register_child(gate: &Deadbolt, body: &str) -> (u16, String) {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let Some(parent) = v.get("parent").and_then(|x| x.as_str()) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let Some(child) = v.get("child").and_then(|x| x.as_str()) else {
        return (400, json!({"code":"bad_request"}).to_string());
    };
    let swarm = v.get("swarm_task_id").and_then(|x| x.as_str());
    match gate.register_child(parent, child, swarm) {
        Ok(live) => (200, json!({"live": live}).to_string()),
        Err(e) => (200, json!({"ok":false,"code": err_token(&e)}).to_string()),
    }
}

fn status(gate: &Deadbolt, query: &str) -> (u16, String) {
    let agent = query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k == "agent" && !v.is_empty() {
            Some(v)
        } else {
            None
        }
    });
    match gate.status(agent) {
        Ok(rows) => {
            let agents: Vec<Value> = rows
                .into_iter()
                .map(|row| {
                    json!({
                        "agent_id": row.agent_id,
                        "state": row.state,
                        "parent_id": row.parent_id,
                        "expires_at": row.expires_at,
                        "clips": row.clips,
                        "swarm_task_id": row.swarm_task_id,
                    })
                })
                .collect();
            (200, json!({"agents": agents}).to_string())
        }
        Err(e) => (200, json!({"ok":false,"code": err_token(&e)}).to_string()),
    }
}

fn err_token(err: &DeadboltError) -> &'static str {
    match err {
        DeadboltError::StoreUnavailable => "store_unavailable",
        DeadboltError::Disabled => "disabled",
        DeadboltError::NotFound => "not_found",
        DeadboltError::Killed => "killed",
        DeadboltError::DrillFailed(_) => "drill_failed",
        DeadboltError::BindRefused => "bind_refused",
        DeadboltError::TokenRequired => "token_required",
        DeadboltError::ExportRefused(_) => "export_refused",
        DeadboltError::McpSpawn => "mcp_spawn",
        DeadboltError::BadRequest => "bad_request",
    }
}

#[cfg(test)]
mod credential_tests {
    use super::*;
    use std::fs;

    fn request(method: &str, path: &str, headers: &str, body: &str) -> String {
        format!("{method} {path} HTTP/1.1\r\nhost: localhost\r\n{headers}content-length: {}\r\n\r\n{body}", body.len())
    }

    #[test]
    fn exact_route_rejects_malformed_and_mixed_auth_without_consumption() {
        let dir = tempfile::tempdir().unwrap();
        let gate = Deadbolt::open_at(dir.path(), true, 60);
        gate.ensure_agent("A").unwrap();
        let action = crate::ActionRequest::new(
            "A",
            "send",
            None,
            serde_json::json!({"body":"reviewed"}),
            60,
        )
        .unwrap();
        gate.approve_action(&action).unwrap();
        let raw = serde_json::to_string(&action).unwrap();
        let duplicate = raw.replace("\"version\":1", "\"version\":1,\"version\":1");
        assert_eq!(
            dispatch_http(
                &gate,
                &request(
                    "POST",
                    "/admit-action",
                    "X-Deadbolt-Token: operator\r\n",
                    &duplicate
                ),
                Some("operator")
            )
            .0,
            400
        );
        gate.issue_credential("A", "key", 60, &dir.path().join("key"))
            .unwrap();
        let secret = fs::read_to_string(dir.path().join("key")).unwrap();
        let mixed = format!("X-Deadbolt-Admission: {secret}\r\nX-Deadbolt-Token: operator\r\n");
        assert_eq!(
            dispatch_http(
                &gate,
                &request("POST", "/admit-action", &mixed, &raw),
                Some("operator")
            )
            .0,
            401
        );
        let header = format!("X-Deadbolt-Admission: {secret}\r\n");
        assert_eq!(
            dispatch_http(
                &gate,
                &request("POST", "/admit-action", &header, &raw),
                Some("operator")
            ),
            (200, r#"{"decision":"allow"}"#.into())
        );
        assert!(dispatch_http(
            &gate,
            &request("POST", "/admit-action", &header, &raw),
            Some("operator")
        )
        .1
        .contains("needs_human"));
    }

    #[test]
    fn workload_route_matrix_never_grants_operator_or_other_agent_access() {
        let dir = tempfile::tempdir().unwrap();
        let gate = Deadbolt::open(&crate::DeadboltConfig {
            db_path: Some(dir.path().join("db").display().to_string()),
            events_path: Some(dir.path().join("events").display().to_string()),
            ..Default::default()
        });
        gate.ensure_agent("A").unwrap();
        gate.ensure_agent("B").unwrap();
        gate.issue_credential("A", "key", 60, &dir.path().join("key"))
            .unwrap();
        let secret = fs::read_to_string(dir.path().join("key")).unwrap();
        let header = format!("X-Deadbolt-Admission: {secret}\r\n");
        for operator in [None, Some("operator")] {
            let good = request(
                "POST",
                "/admit",
                &header,
                r#"{"agent_id":"A","tool":"read"}"#,
            );
            assert_eq!(
                dispatch_http(&gate, &good, operator),
                (200, r#"{"decision":"allow"}"#.into())
            );
            let wrong = request(
                "POST",
                "/admit",
                &header,
                r#"{"agent_id":"B","tool":"read"}"#,
            );
            assert_eq!(dispatch_http(&gate, &wrong, operator).0, 401);
            for (method, path, body) in [
                ("POST", "/ensure", r#"{"agent_id":"new"}"#),
                ("POST", "/policy", r#"{"agent_id":"A","tools":["send"]}"#),
                ("POST", "/spend", r#"{"agent_id":"A","usd":-1}"#),
                ("POST", "/register_child", r#"{"parent":"A","child":"new"}"#),
                ("GET", "/status?agent=A", ""),
                ("POST", "/approve", r#"{"agent_id":"A","tool":"send"}"#),
                ("POST", "/resume", r#"{"agent_id":"A"}"#),
                ("POST", "/unknown", ""),
            ] {
                assert_eq!(
                    dispatch_http(&gate, &request(method, path, &header, body), operator).0,
                    403,
                    "{path}"
                );
                // Stripping the scoped header must not restore socket-only authority.
                assert_eq!(
                    dispatch_http(&gate, &request(method, path, "", body), operator).0,
                    401,
                    "{path}"
                );
            }
            let mixed = format!("{header}X-Deadbolt-Token: operator\r\n");
            assert_eq!(
                dispatch_http(
                    &gate,
                    &request(
                        "POST",
                        "/admit",
                        &mixed,
                        r#"{"agent_id":"A","tool":"read"}"#
                    ),
                    operator
                )
                .0,
                401
            );
            let duplicate = format!("{header}x-deadbolt-admission: invalid\r\n");
            assert_eq!(
                dispatch_http(
                    &gate,
                    &request(
                        "POST",
                        "/admit",
                        &duplicate,
                        r#"{"agent_id":"A","tool":"read"}"#
                    ),
                    operator
                )
                .0,
                401
            );
            assert_eq!(
                dispatch_http(
                    &gate,
                    &request(
                        "POST",
                        "/admit",
                        "X-Deadbolt-Admission: \r\n",
                        r#"{"agent_id":"A","tool":"read"}"#
                    ),
                    operator
                )
                .0,
                401
            );
        }
        assert_eq!(gate.status(Some("new")).unwrap().len(), 0);
        assert_eq!(
            dispatch_http(
                &gate,
                &request("GET", "/status", "X-Deadbolt-Token: operator\r\n", ""),
                Some("operator")
            )
            .0,
            200
        );
        let duplicate_operator = "X-Deadbolt-Token: operator\r\nx-deadbolt-token: operator\r\n";
        assert_eq!(
            dispatch_http(
                &gate,
                &request("GET", "/status", duplicate_operator, ""),
                Some("operator")
            )
            .0,
            401
        );
        gate.revoke_credential("key").unwrap();
        assert_eq!(
            dispatch_http(
                &gate,
                &request(
                    "POST",
                    "/admit",
                    &header,
                    r#"{"agent_id":"A","tool":"read"}"#
                ),
                None
            )
            .0,
            401
        );
        // Revoking the last credential does not re-enable anonymous operator access.
        assert_eq!(
            dispatch_http(&gate, &request("GET", "/status", "", ""), None).0,
            401
        );
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::DeadboltConfig;
    use std::net::TcpStream;
    use std::os::unix::net::UnixStream;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn sock_dir() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(1);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dbs{n}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cfg_at(dir: &Path) -> DeadboltConfig {
        DeadboltConfig {
            db_path: Some(dir.join("deadbolt.db").display().to_string()),
            events_path: Some(dir.join("events.jsonl").display().to_string()),
            ..DeadboltConfig::default()
        }
    }

    fn start_with(gate: Deadbolt, sock: PathBuf, token: Option<String>) {
        let listen_at = sock.clone();
        thread::spawn(move || {
            let _ = listen(gate, &listen_at, token);
        });
        for _ in 0..50 {
            if sock.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("socket did not appear");
    }

    fn start(gate: Deadbolt, sock: PathBuf) {
        start_with(gate, sock, None);
    }

    fn http(sock: &Path, method: &str, path: &str, body: Option<&str>) -> Value {
        let body = body.unwrap_or("");
        let req = format!(
            "{method} {path} HTTP/1.1\r\nhost: local\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let mut last = String::new();
        for _ in 0..20 {
            if let Ok(mut stream) = UnixStream::connect(sock) {
                stream.write_all(req.as_bytes()).unwrap();
                let _ = stream.shutdown(std::net::Shutdown::Write);
                last.clear();
                stream.read_to_string(&mut last).unwrap();
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let json = last.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        serde_json::from_str(json).unwrap_or_else(|_| json!({"raw": last}))
    }

    #[test]
    fn listener_preserves_files_and_live_sockets() {
        let dir = sock_dir();
        let gate = Deadbolt::open(&cfg_at(&dir));
        let path = dir.join("existing");
        fs::write(&path, "keep").unwrap();
        assert!(listen(gate.clone(), &path, None).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep");
        let socket = dir.join("live.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        assert!(listen(gate, &socket, None).is_err());
        assert!(std::os::unix::net::UnixStream::connect(&socket).is_ok());
        drop(listener);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saturated_worker_closes_connection_and_recovers() {
        let dir = sock_dir();
        let gate = Deadbolt::open(&cfg_at(&dir));
        let active = Arc::new(AtomicUsize::new(MAX_WORKERS));
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        spawn_worker(&gate, server, &None, &active);
        assert_eq!(client.read(&mut [0u8; 1]).unwrap(), 0);
        assert_eq!(active.load(Ordering::Acquire), MAX_WORKERS);
        active.store(0, Ordering::Release);
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        spawn_worker(&gate, server, &Some("test-token".into()), &active);
        client.write_all(b"POST /ensure HTTP/1.1\r\nx-deadbolt-token: test-token\r\ncontent-length: 16\r\n\r\n{\"agent_id\":\"A\"}").unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.contains("\"ok\":true"), "{response}");
        for _ in 0..20 {
            if active.load(Ordering::Acquire) == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(active.load(Ordering::Acquire), 0);
        assert_eq!(gate.status(Some("A")).unwrap().len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn worker_capacity_releases_on_completion_and_rejects_excess() {
        let active = Arc::new(AtomicUsize::new(0));
        let first = WorkerPermit::acquire(&active, 2).unwrap();
        let second = WorkerPermit::acquire(&active, 2).unwrap();
        assert!(WorkerPermit::acquire(&active, 2).is_none());
        assert_eq!(active.load(Ordering::Acquire), 2);
        drop(first);
        let third = WorkerPermit::acquire(&active, 2).unwrap();
        drop(second);
        drop(third);
        assert_eq!(active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn partial_bytes_do_not_renew_total_request_deadline() {
        let dir = sock_dir();
        let gate = Deadbolt::open(&cfg_at(&dir));
        let (mut client, server) = UnixStream::pair().unwrap();
        let started = Instant::now();
        let worker =
            thread::spawn(move || handle_io(&gate, server, None, Duration::from_millis(150)));
        // Each byte arrives before the old per-read timeout, but the complete
        // request never arrives. The total deadline must still terminate it.
        for _ in 0..6 {
            if client.write_all(b"G").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(40));
        }
        let err = worker.join().unwrap().unwrap_err();
        assert!(matches!(
            err.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_or_truncated_http_cannot_mutate_store() {
        let dir = sock_dir();
        let gate = Deadbolt::open(&cfg_at(&dir));
        for framing in [
            "content-length: 99",
            "content-length: nope",
            "content-length: 16\r\ncontent-length: 16",
            "transfer-encoding: chunked",
        ] {
            let (mut client, server) = UnixStream::pair().unwrap();
            let request =
                format!("POST /ensure HTTP/1.1\r\n{framing}\r\n\r\n{{\"agent_id\":\"A\"}}");
            client.write_all(request.as_bytes()).unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            assert!(handle_io(&gate, server, None, Duration::from_secs(1)).is_err());
            assert!(gate.status(Some("A")).unwrap().is_empty());
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn bind_refuses_wildcard() {
        assert!(bind_refused(Path::new("0.0.0.0")));
        assert!(bind_refused(Path::new("0.0.0.0:9")));
        assert!(bind_refused(Path::new("192.168.1.10:9")));
        assert!(bind_refused(Path::new("10.0.0.1:8080")));
        assert!(bind_refused(Path::new("[::]:9")));
        assert!(bind_refused(Path::new("::")));
        assert!(!bind_refused(Path::new("127.0.0.1:9")));
        assert!(!bind_refused(Path::new("[::1]:9")));
        assert!(!bind_refused(Path::new("/tmp/deadbolt.sock")));
        assert!(serve(&DeadboltConfig::default(), Path::new("192.168.1.10:9")).is_err());
    }

    fn free_loopback() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        addr
    }

    fn start_tcp(gate: Deadbolt, addr: SocketAddr, token: &str) {
        let token = Some(token.to_string());
        thread::spawn(move || {
            let _ = listen_tcp(gate, addr, token);
        });
        for _ in 0..50 {
            if TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("tcp did not listen");
    }

    fn tcp_status(
        addr: SocketAddr,
        method: &str,
        path: &str,
        body: Option<&str>,
        token: Option<&str>,
    ) -> (u16, Value) {
        let body = body.unwrap_or("");
        let token_line = token
            .map(|t| format!("x-deadbolt-token: {t}\r\n"))
            .unwrap_or_default();
        let req = format!(
            "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\n{token_line}content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(req.as_bytes()).unwrap();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        let mut last = String::new();
        stream.read_to_string(&mut last).unwrap();
        let status = last
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let json = last.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        let value = serde_json::from_str(json).unwrap_or_else(|_| json!({"raw": last}));
        (status, value)
    }

    #[test]
    fn tcp_serve_refuses_start_without_token() {
        let err = serve(&DeadboltConfig::default(), Path::new("127.0.0.1:9")).unwrap_err();
        assert!(matches!(err, DeadboltError::TokenRequired));
    }

    #[test]
    fn tcp_loopback_admit_allow_then_killed() {
        let dir = sock_dir();
        let gate = Deadbolt::open(&cfg_at(&dir));
        let addr = free_loopback();
        start_tcp(gate.clone(), addr, "s3cret");
        let (status, _) = tcp_status(
            addr,
            "POST",
            "/ensure",
            Some(r#"{"agent_id":"A"}"#),
            Some("s3cret"),
        );
        assert_eq!(status, 200);
        let (status, allowed) = tcp_status(
            addr,
            "POST",
            "/admit",
            Some(r#"{"agent_id":"A","tool":"shell"}"#),
            Some("s3cret"),
        );
        assert_eq!(status, 200);
        assert_eq!(allowed["decision"], "allow");
        gate.kill("A").unwrap();
        let (status, denied) = tcp_status(
            addr,
            "POST",
            "/admit",
            Some(r#"{"agent_id":"A","tool":"shell"}"#),
            Some("s3cret"),
        );
        assert_eq!(status, 200);
        assert_eq!(denied["decision"], "deny");
        assert_eq!(denied["code"], "killed");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tcp_wrong_token_401() {
        let dir = sock_dir();
        let gate = Deadbolt::open(&cfg_at(&dir));
        let addr = free_loopback();
        start_tcp(gate.clone(), addr, "s3cret");
        let (missing, body) =
            tcp_status(addr, "POST", "/ensure", Some(r#"{"agent_id":"A"}"#), None);
        assert_eq!(missing, 401);
        assert_eq!(body["code"], "unauthorized");
        let (wrong, _) = tcp_status(
            addr,
            "POST",
            "/admit",
            Some(r#"{"agent_id":"A","tool":"shell"}"#),
            Some("nope"),
        );
        assert_eq!(wrong, 401);
        assert!(gate.status(Some("A")).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    fn http_status(
        sock: &Path,
        method: &str,
        path: &str,
        body: Option<&str>,
        token: Option<&str>,
    ) -> (u16, Value) {
        let body = body.unwrap_or("");
        let token_line = token
            .map(|t| format!("x-deadbolt-token: {t}\r\n"))
            .unwrap_or_default();
        let req = format!(
            "{method} {path} HTTP/1.1\r\nhost: local\r\n{token_line}content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let mut last = String::new();
        for _ in 0..20 {
            if let Ok(mut stream) = UnixStream::connect(sock) {
                stream.write_all(req.as_bytes()).unwrap();
                let _ = stream.shutdown(std::net::Shutdown::Write);
                last.clear();
                stream.read_to_string(&mut last).unwrap();
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let status = last
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let json = last.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        let value = serde_json::from_str(json).unwrap_or_else(|_| json!({"raw": last}));
        (status, value)
    }

    #[test]
    fn socket_mode_is_0600() {
        let dir = sock_dir();
        let sock = dir.join("s");
        let gate = Deadbolt::open(&cfg_at(&dir));
        start(gate, sock.clone());
        let mode = fs::metadata(&sock).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn serve_with_token_rejects_missing_header() {
        let dir = sock_dir();
        let sock = dir.join("s");
        let gate = Deadbolt::open(&cfg_at(&dir));
        start_with(gate.clone(), sock.clone(), Some("s3cret".into()));
        let (status, body) =
            http_status(&sock, "POST", "/ensure", Some(r#"{"agent_id":"A"}"#), None);
        assert_eq!(status, 401);
        assert_eq!(body["code"], "unauthorized");
        let (wrong, _) = http_status(
            &sock,
            "POST",
            "/admit",
            Some(r#"{"agent_id":"A","tool":"shell"}"#),
            Some("nope"),
        );
        assert_eq!(wrong, 401);
        let (status_get, _) = http_status(&sock, "GET", "/status", None, None);
        assert_eq!(status_get, 401);
        let (child, _) = http_status(
            &sock,
            "POST",
            "/register_child",
            Some(r#"{"parent":"A","child":"C"}"#),
            None,
        );
        assert_eq!(child, 401);
        assert!(gate.status(Some("A")).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn serve_with_token_accepts_matching_header_and_admit_still_killed_after_kill() {
        let dir = sock_dir();
        let sock = dir.join("s");
        let gate = Deadbolt::open(&cfg_at(&dir));
        start_with(gate.clone(), sock.clone(), Some("s3cret".into()));
        let (status, _) = http_status(
            &sock,
            "POST",
            "/ensure",
            Some(r#"{"agent_id":"A"}"#),
            Some("s3cret"),
        );
        assert_eq!(status, 200);
        let (status, allowed) = http_status(
            &sock,
            "POST",
            "/admit",
            Some(r#"{"agent_id":"A","tool":"shell"}"#),
            Some("s3cret"),
        );
        assert_eq!(status, 200);
        assert_eq!(allowed["decision"], "allow");
        gate.kill("A").unwrap();
        let (status, denied) = http_status(
            &sock,
            "POST",
            "/admit",
            Some(r#"{"agent_id":"A","tool":"shell"}"#),
            Some("s3cret"),
        );
        assert_eq!(status, 200);
        assert_eq!(denied["decision"], "deny");
        assert_eq!(denied["code"], "killed");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_file_not_0600_is_refused() {
        let dir = sock_dir();
        let file = dir.join("token");
        fs::write(&file, "abc\n").unwrap();
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&file, perms.clone()).unwrap();
        let mut cfg = cfg_at(&dir);
        cfg.token_file = Some(file.display().to_string());
        assert!(resolve_token(&cfg).is_err());
        perms.set_mode(0o600);
        fs::set_permissions(&file, perms).unwrap();
        assert_eq!(resolve_token(&cfg).unwrap().as_deref(), Some("abc"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidecar_post_kill_admit_is_killed() {
        let dir = sock_dir();
        let sock = dir.join("s");
        let cfg = cfg_at(&dir);
        let gate = Deadbolt::open(&cfg);
        start(gate.clone(), sock.clone());
        assert_eq!(
            http(&sock, "POST", "/ensure", Some(r#"{"agent_id":"A"}"#))["ok"],
            true
        );
        assert_eq!(
            http(&sock, "POST", "/ensure", Some(r#"{"agent_id":"B"}"#))["ok"],
            true
        );
        assert_eq!(
            http(
                &sock,
                "POST",
                "/admit",
                Some(r#"{"agent_id":"A","tool":"shell"}"#)
            )["decision"],
            "allow"
        );
        gate.kill("A").unwrap();
        let denied = http(
            &sock,
            "POST",
            "/admit",
            Some(r#"{"agent_id":"A","tool":"shell"}"#),
        );
        assert_eq!(denied["decision"], "deny");
        assert_eq!(denied["code"], "killed");
        assert_eq!(
            http(
                &sock,
                "POST",
                "/admit",
                Some(r#"{"agent_id":"B","tool":"shell"}"#)
            )["decision"],
            "allow"
        );
        let kill = http(&sock, "POST", "/kill", Some(r#"{"agent_id":"B"}"#));
        assert_eq!(kill["code"], "not_found");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidecar_parent_kill_denies_child_admit() {
        let dir = sock_dir();
        let sock = dir.join("s");
        let cfg = cfg_at(&dir);
        let gate = Deadbolt::open(&cfg);
        start(gate.clone(), sock.clone());
        http(&sock, "POST", "/ensure", Some(r#"{"agent_id":"P"}"#));
        http(&sock, "POST", "/ensure", Some(r#"{"agent_id":"B"}"#));
        let reg = http(
            &sock,
            "POST",
            "/register_child",
            Some(r#"{"parent":"P","child":"C","swarm_task_id":"swarm-side"}"#),
        );
        assert_eq!(reg["live"], true);
        assert_eq!(
            http(
                &sock,
                "POST",
                "/admit",
                Some(r#"{"agent_id":"C","tool":"read_file"}"#)
            )["decision"],
            "allow"
        );
        gate.kill("P").unwrap();
        let denied = http(
            &sock,
            "POST",
            "/admit",
            Some(r#"{"agent_id":"C","tool":"shell"}"#),
        );
        assert_eq!(denied["decision"], "deny");
        assert_eq!(denied["code"], "killed");
        assert_eq!(
            http(
                &sock,
                "POST",
                "/admit",
                Some(r#"{"agent_id":"B","tool":"shell"}"#)
            )["decision"],
            "allow"
        );
        let listed = http(&sock, "GET", "/status", None);
        let agents = listed["agents"].as_array().unwrap();
        assert!(agents.iter().any(|row| {
            row["agent_id"] == "C" && row["state"] == "killed" && row["parent_id"] == "P"
        }));
        let _ = fs::remove_dir_all(&dir);
    }

    fn python_json(sock: &Path, args: &[&str]) -> Value {
        let client = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/deadbolt_client.py");
        let out = std::process::Command::new("python3")
            .arg(client)
            .args(args)
            .env("DEADBOLT_SOCK", sock)
            .output()
            .expect("python3");
        assert!(
            out.status.success(),
            "python failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("client json")
    }

    #[test]
    fn sidecar_python_client() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            eprintln!("sidecar_python_client: python3 missing");
            return;
        }
        let dir = sock_dir();
        let sock = dir.join("s");
        let cfg = cfg_at(&dir);
        let gate = Deadbolt::open(&cfg);
        start(gate.clone(), sock.clone());
        assert_eq!(python_json(&sock, &["ensure", "--agent", "A"])["ok"], true);
        assert_eq!(python_json(&sock, &["ensure", "--agent", "B"])["ok"], true);
        assert_eq!(
            python_json(&sock, &["admit", "--agent", "A", "--tool", "shell"])["decision"],
            "allow"
        );
        gate.kill("A").unwrap();
        let denied = python_json(&sock, &["admit", "--agent", "A", "--tool", "shell"]);
        assert_eq!(denied["decision"], "deny");
        assert_eq!(denied["code"], "killed");
        assert_eq!(
            python_json(&sock, &["admit", "--agent", "B", "--tool", "shell"])["decision"],
            "allow"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    fn node_missing() -> bool {
        std::process::Command::new("node")
            .arg("--version")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
    }

    fn node_json(sock: &Path, args: &[&str]) -> Value {
        let client = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/deadbolt_client.js");
        let out = std::process::Command::new("node")
            .arg(client)
            .args(args)
            .env("DEADBOLT_SOCK", sock)
            .output()
            .expect("node");
        assert!(
            out.status.success(),
            "node failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("node json")
    }

    #[test]
    fn client_deny_store_unavailable_when_serve_down() {
        if python_missing() {
            eprintln!("client_deny_store_unavailable_when_serve_down: python3 missing");
            return;
        }
        let dir = sock_dir();
        let sock = dir.join("absent.sock");
        for args in [
            &["admit", "--agent", "A", "--tool", "shell"][..],
            &["ensure", "--agent", "A"][..],
            &["register-child", "--parent", "P", "--child", "C"][..],
        ] {
            let denied = python_json(&sock, args);
            assert_eq!(denied["decision"], "deny");
            assert_eq!(denied["code"], "store_unavailable");
        }
        let status = python_json(&sock, &["status", "--agent", "A"]);
        assert_eq!(status["code"], "store_unavailable");
        if !node_missing() {
            for args in [
                &["admit", "--agent", "A", "--tool", "shell"][..],
                &["ensure", "--agent", "A"][..],
                &["register-child", "--parent", "P", "--child", "C"][..],
            ] {
                let denied = node_json(&sock, args);
                assert_eq!(denied["decision"], "deny");
                assert_eq!(denied["code"], "store_unavailable");
            }
            let status = node_json(&sock, &["status", "--agent", "A"]);
            assert_eq!(status["code"], "store_unavailable");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    fn python_missing() -> bool {
        std::process::Command::new("python3")
            .arg("--version")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
    }

    #[test]
    fn sample_agent_killed() {
        if python_missing() {
            eprintln!("sample_agent_killed: python3 missing");
            return;
        }
        let dir = sock_dir();
        let sock = dir.join("s");
        let cfg = cfg_at(&dir);
        let gate = Deadbolt::open(&cfg);
        start(gate.clone(), sock.clone());
        let agent =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/deadbolt_sample_agent.py");
        let mut child = std::process::Command::new("python3")
            .arg(&agent)
            .args(["--agent", "samp-a", "--tool", "shell", "--interval", "0.05"])
            .env("DEADBOLT_SOCK", &sock)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("sample agent");
        let stdout = child.stdout.take().expect("stdout");
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        std::io::BufRead::read_line(&mut reader, &mut line).expect("first line");
        let first: Value = serde_json::from_str(line.trim()).expect("allow json");
        assert_eq!(first["decision"], "allow");
        gate.kill("samp-a").unwrap();
        line.clear();
        std::io::BufRead::read_line(&mut reader, &mut line).expect("deny line");
        let denied: Value = serde_json::from_str(line.trim()).expect("deny json");
        assert_eq!(denied["decision"], "deny");
        assert_eq!(denied["code"], "killed");
        let status = child.wait().expect("wait a");
        assert_eq!(status.code(), Some(2));

        let mut other = std::process::Command::new("python3")
            .arg(&agent)
            .args(["--agent", "samp-b", "--tool", "shell", "--interval", "0.05"])
            .env("DEADBOLT_SOCK", &sock)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("sample b");
        let bout = other.stdout.take().expect("stdout b");
        let mut breader = std::io::BufReader::new(bout);
        let mut bline = String::new();
        std::io::BufRead::read_line(&mut breader, &mut bline).expect("b line");
        let live: Value = serde_json::from_str(bline.trim()).expect("b json");
        assert_eq!(live["decision"], "allow");
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &other.id().to_string()])
            .status();
        let bstatus = other.wait().expect("wait b");
        assert!(bstatus.success());
        let _ = fs::remove_dir_all(&dir);
    }
}
