# Integration

Inference may be probabilistic. Execution is admit or deny. Call admit before the tool body. A deny does not run the tool.

## Build-in

Link the crate. No socket. No model tool.

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

TCP requires `DEADBOLT_TOKEN`. Bind stays `127.0.0.1:9782`. Unit: `dist/deadbolt.service`. Env file mode `0600`: `dist/deadbolt.env.example`. Compose publishes only `127.0.0.1:9782:9782`.
