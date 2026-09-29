# Deadbolt

Out-of-band lease gate. Apache-2.0. Not a model tool.

Deadbolt does not shut down frontier models. It cuts tool, MCP, and spawn calls for one agent id and that agent's children. The sidecar is a local Unix socket only. Harness is a consumer of this crate. This repository does not include the Harness executor, TUI, providers, confirm-gate, or jail, and it does not relicense Harness.

## Install

```bash
git clone https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo build --release
./target/debug/deadbolt drill
```

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

Default socket is `~/.deadbolt/deadbolt.sock`. Store files live under `~/.deadbolt/`. Harness keeps `~/.harness/deadbolt.sock` and `~/.harness/deadbolt.db` as its consumer paths. To attach this binary to a Harness store, pass `--bind ~/.harness/deadbolt.sock` and construct `DeadboltConfig` with the Harness db paths. The `deadbolt` binary itself defaults to `~/.deadbolt`.

`0.0.0.0` and non-loopback TCP binds are refused. The socket is mode `0600`. If `DEADBOLT_TOKEN` is set, every sidecar request must send `X-Deadbolt-Token`. A missing or wrong token is HTTP 401 and does not admit.

`kill` requires `--agent`. It revokes that agent and its children. It does not halt a fleet and it does not shut down a model vendor.

## Admit before the tool

```python
from deadbolt_client import admit
if admit(agent, "shell")["decision"] != "allow":
    raise SystemExit("deadbolt deny")
# then run the tool
```

Stdlib client: `examples/deadbolt_client.py`. Sample loop: `examples/deadbolt_sample_agent.py`. `DEADBOLT_SOCK` overrides the socket.

Operator detail: `docs/DEADBOLT.md`.
