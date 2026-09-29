# Deadbolt

Out-of-band lease gate. Apache-2.0. Not a model tool.

Deadbolt does not shut down frontier models. It cuts tool, MCP, and spawn calls for one agent id and that agent's children. The sidecar is a local Unix socket only. Harness is a consumer of this crate. This repository does not include the Harness executor, TUI, providers, confirm-gate, or jail, and it does not relicense Harness.

Inference may be probabilistic. Execution is admit or deny.

## Two modes

Build-in: link the crate and call `admit` in-process. See `docs/INTEGRATION.md` and `cargo run --example build_in`.

Bolt-on: `deadbolt serve` plus a client that admits before the tool. Python: `examples/deadbolt_client.py`. Node, no npm: `examples/deadbolt_client.js`. Protocol: `docs/PROTOCOL.md`.

## Install

```bash
git clone https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo build --release
./target/release/deadbolt drill
```

Debug binary, same drill: `cargo build && ./target/debug/deadbolt drill`.

## CLI

```bash
deadbolt status [--agent ID]
deadbolt pause --agent ID
deadbolt clip TOOL --agent ID
deadbolt resume --agent ID
deadbolt kill --agent ID
deadbolt drill
deadbolt serve [--bind PATH]
deadbolt export --agent ID [--out PATH] [--json] [--children]
```

Default socket is `~/.deadbolt/deadbolt.sock` (mode `0600`; token optional). TCP is loopback only: `deadbolt serve --bind 127.0.0.1:PORT`. `0.0.0.0`, `[::]`, and any other host are refused. TCP requires `DEADBOLT_TOKEN` at start. A missing or wrong `X-Deadbolt-Token` is HTTP 401 and does not admit. Set `DEADBOLT_SOCK=127.0.0.1:PORT` or `http://127.0.0.1:PORT` for the clients.

`kill` requires `--agent`. It revokes that agent and its children. It does not halt a fleet and it does not shut down a model vendor.

Security: `SECURITY.md`.

## Admit before the tool

```python
from deadbolt_client import admit
if admit(agent, "shell")["decision"] != "allow":
    raise SystemExit("deadbolt deny")
# then run the tool
```

Stdlib clients: `examples/deadbolt_client.py` and `examples/deadbolt_client.js`. Sample loop: `examples/deadbolt_sample_agent.py`. `DEADBOLT_SOCK` overrides the socket.

Operator detail: `docs/DEADBOLT.md`.
