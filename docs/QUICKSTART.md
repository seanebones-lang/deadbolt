# Quick start: allow, stop, deny

This demonstration uses no model and performs no real shell tool action. It
shows the admission boundary with the bundled Python sample. Complete
[installation](INSTALL.md) first and run these commands from the extracted
release archive or a source checkout.

## 1. Check the embedded mode (source checkout only)

```sh
cargo run --locked --example build_in
```

Expected output includes `allow` followed by `killed`. The example uses a
temporary store and requires no server. This works wherever the Rust build does.
If you installed a native archive without Rust, skip this step and continue
with the sidecar demonstration.

## 2. Start a Unix sidecar (macOS/Linux)

In terminal A, choose isolated demo state:

```sh
mkdir -p .demo-state
export DEADBOLT_DB="$PWD/.demo-state/deadbolt.db"
export DEADBOLT_EVENTS="$PWD/.demo-state/events.jsonl"
export DEADBOLT_SOCK="$PWD/.demo-state/deadbolt.sock"
deadbolt serve --bind "$DEADBOLT_SOCK"
```

The server waits for requests. Leave this terminal running. If you have a
`DEADBOLT_TOKEN` set already, either provide the same token to terminal B or
unset it for this same-user Unix demonstration before starting the server.

## 3. Run the sample

In terminal B, from the same checkout:

```sh
export DEADBOLT_SOCK="$PWD/.demo-state/deadbolt.sock"
python3 examples/deadbolt_sample_agent.py --agent demo-run-001 --interval 1
```

It creates a lease and prints `{"decision": "allow", "tool": "shell"}` roughly
once a second. Each line is a fresh admission check. The sample does not run an
OS shell or contact a model.

## 4. Kill the lease

In terminal C, from the same checkout:

```sh
export DEADBOLT_DB="$PWD/.demo-state/deadbolt.db"
export DEADBOLT_EVENTS="$PWD/.demo-state/events.jsonl"
deadbolt status --agent demo-run-001
deadbolt kill --agent demo-run-001
```

Terminal B should print `{"decision": "deny", "code": "killed"}` and exit
with code 2. That exit is the expected denial, not a test failure.

```sh
deadbolt status --agent demo-run-001
deadbolt incident --agent demo-run-001 --json --out .demo-state/incident.json
```

The lease remains killed after a service restart. Re-running the demo requires
a new ID, for example `demo-run-002`; `resume` and `ensure` do not undo kill.
Stop terminal A with Ctrl-C when finished. Demo state holds the revocations and
evidence; deleting it would erase those records.

## Windows or a TCP integration

Use the [token-required TCP setup](INSTALL.md#choose-the-transport). In each
client terminal, set the same `DEADBOLT_SOCK` and `DEADBOLT_TOKEN`; set matching
`DEADBOLT_DB` and `DEADBOLT_EVENTS` in the server and operator terminals. Run
`python examples/deadbolt_sample_agent.py --agent demo-run-001 --interval 1`,
then `deadbolt kill --agent demo-run-001` from the operator terminal. The
expected allow/deny sequence is the same. Windows builds and TCP client contracts passed on GitHub Actions Windows
Server 2025 on 2026-09-29; validate your own application dispatch paths too.

## Put the check in your dispatcher

An allow has an effect only when your trusted code makes execution conditional:

```python
from deadbolt_client import admit

def dispatch(agent, tool_name, body, dest=None):
    decision = admit(agent, tool_name, dest)
    if decision.get("decision") != "allow":
        raise RuntimeError(decision.get("code", "store_unavailable"))
    return body()
```

Copy `examples/deadbolt_client.py` next to your application module, create the
lease once, configure policy, and call this dispatcher for every action.
[The integration guide](INTEGRATION.md) shows full Rust, Python, Node and MCP
examples. [The FAQ](FAQ.md) explains kill, TTL, approval and policy defaults.
