# Integration

Inference may be probabilistic. Execution is admit or deny. Call admit before the tool body. A deny does not run the tool. Trust boundary: `docs/TRUST.md`.

Build-in and `mcp-proxy` are enforced: the tool body cannot run without admit. Python and Node `admit` are cooperative. A caller that skips them is outside the trust boundary.

## Build-in

Link the crate. Package name is `n11-deadbolt`. The Rust crate name stays `deadbolt`. No socket. No model tool.

```rust
use deadbolt::{AdmitDecision, Deadbolt, DenyCode};
fn main() {
    let dir = std::env::temp_dir().join("deadbolt-demo");
    std::fs::create_dir_all(&dir).unwrap();
    let db = Deadbolt::open_at(&dir, true, 60);
    db.ensure_agent("shop-bot").unwrap();
    assert!(matches!(db.admit("shop-bot", "shell"), AdmitDecision::Allow));
    db.kill("shop-bot").unwrap();
    assert!(matches!(
        db.admit("shop-bot", "shell"),
        AdmitDecision::Deny { code: DenyCode::Killed }
    ));
}
```

Runnable copy: `cargo run --example build_in`.

## Bolt-on

One process serves the store. Every other process admits before it runs a tool.

```bash
deadbolt serve
python examples/deadbolt_client.py ensure --agent shop-bot
python examples/deadbolt_client.py admit --agent shop-bot --tool shell
```

```python
from deadbolt_client import admit
if admit(agent, "shell")["decision"] != "allow":
    raise SystemExit("deadbolt deny")
# then run the tool
```

`DEADBOLT_SOCK` overrides `~/.deadbolt/deadbolt.sock`. A `host:port` or `http://127.0.0.1:port` value uses loopback HTTP. A Unix path still uses the socket. TCP serve requires `DEADBOLT_TOKEN`. On a Unix socket the token stays optional. If it is set, send `X-Deadbolt-Token`. Protocol: `docs/PROTOCOL.md`.

## Run as a service

TCP requires `DEADBOLT_TOKEN`. Bind stays `127.0.0.1:9782`. `0.0.0.0` and `[::]` are refused. Unit: `dist/deadbolt.service`. Env file mode `0600`: `dist/deadbolt.env.example`. Compose publishes only `127.0.0.1:9782:9782`.

```bash
sudo install -d -m 0755 /etc/deadbolt
sudo install -m 0600 dist/deadbolt.env.example /etc/deadbolt/deadbolt.env
sudo install -m 0644 dist/deadbolt.service /etc/systemd/system/deadbolt.service
sudo systemctl enable --now deadbolt
```

```bash
cp dist/deadbolt.env.example dist/deadbolt.env && chmod 0600 dist/deadbolt.env
docker compose -f dist/docker-compose.yml up --build
```

The unit runs `deadbolt serve --bind 127.0.0.1:9782`. Compose does not publish `0.0.0.0`. Incident steps for a 24-hour notice are in `docs/INCIDENT.md`. The operator sends the notice. Deadbolt does not.

## MCP proxy

Not a model tool. The proxy speaks newline-delimited JSON-RPC on stdio and spawns the real MCP server. `initialize`, `tools/list`, resources, and `ping` are forwarded. `tools/call` is admitted first. A `url`, `uri`, `href`, `endpoint`, or `host` argument is parsed and passed as `dest`. If dest is present and not on `dest_allow`, the deny is `purpose_exceeded`. If `dest_allow` is set and no host parses, the deny is `purpose_exceeded` only for a network-class tool (`http`, `fetch`, `browser`, `web_search`). A local tool with no host is not denied for that reason. An irreversible tool is `needs_human`. `amount`, `usd`, or `cost`, when it parses as a number, is `spend_add` before admit. A deny is a JSON-RPC error whose message is the code token (`killed`, `paused`, `purpose_exceeded`, `lease_expired`, `store_unavailable`, `no_lease`, `spend_cap`, `needs_human`). The child is not invoked.

```bash
deadbolt mcp-proxy --agent shop-bot -- npx whatever-mcp
```

In-process by default (`Deadbolt::open`). `--serve-sock PATH` admits over an already-running Unix socket or `127.0.0.1:PORT`. TCP still requires `DEADBOLT_TOKEN`. `0.0.0.0` is refused.
