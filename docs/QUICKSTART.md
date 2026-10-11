# Quick start: see an allowed effect and a blocked effect

[Install and verify](INSTALL.md) the native executable first. Run the following
from the extracted archive or reviewed source checkout. This uses harmless files
in temporary state and needs no model, API key or account connection.

## One command, real effects

macOS/Linux, for an extracted native archive:

```sh
python3 examples/evaluate.py --binary ./deadbolt
```

Windows:

```powershell
python examples/evaluate.py --binary .\deadbolt.exe
```

Source checkout: first run `cargo build --locked --bin deadbolt`, then use
`--binary target/debug/deadbolt` (Windows: `target/debug/deadbolt.exe`).
A binary on PATH can be used by omitting `--binary`.

Expected: a JSON report with `"passed": true` and eleven checks. The script
starts/stops a token-protected loopback sidecar and cleans up its files. It checks
allowed writes, off-policy denial, one-shot approval, parent/child kill, unrelated
work, outage and persistent revocation. [What the report establishes](FIRST-EVALUATION.md).

## Build in without a sidecar

From a reviewed candidate source checkout with Rust 1.85+ and a C compiler:

```sh
cargo run --locked --example build_in
```

Expected: `allow`, then `killed`. This small Rust example uses a private temporary
store. [Add the library to your application](INTEGRATION.md#build-in-rust-executor).

## Connect your own dispatcher

Pick [Rust, sidecar or MCP](INTEGRATION.md), create an executor-assigned run and
configure policy, then put admission immediately before the actual effect.
Candidate integrations can use callback helpers; sensitive effects can use
[exact-action review](ACTION-APPROVALS.md). Keep
[operator and dispatcher credentials separate](CREDENTIALS.md).

Verify an allowed effect, an off-policy denial, kill, expiry and unavailable gate
in your application. Observe file/network/body effects rather than only printing
an admission result. [The pilot worksheet](PILOT.md) records route coverage.

## Optional: operate the gate yourself

This manual stable-compatible demonstration prints admission decisions; it does
not run a shell or model. It uses three terminals so you can kill a live lease.
Run from the same extracted archive or checkout with `deadbolt` on PATH. For this
isolated operator demonstration only, remove `DEADBOLT_ADMISSION_TOKEN` from the
client environment if previously set; workload credentials cannot create leases.

### Start a Unix sidecar (macOS/Linux)

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

### Run the sample

In terminal B, from the same checkout:

```sh
export DEADBOLT_SOCK="$PWD/.demo-state/deadbolt.sock"
python3 examples/deadbolt_sample_agent.py --agent demo-run-001 --interval 1
```

It creates a lease and prints `{"decision": "allow", "tool": "shell"}` roughly
once a second. Each line is a fresh admission check. The sample does not run an
OS shell or contact a model.

### Kill the lease

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

### Windows or a TCP integration

Use the [token-required TCP setup](INTEGRATION.md#bolt-on-http-sidecar). In each
client terminal, set the same `DEADBOLT_SOCK` and `DEADBOLT_TOKEN`; set matching
`DEADBOLT_DB` and `DEADBOLT_EVENTS` in the server and operator terminals. Run
`python examples/deadbolt_sample_agent.py --agent demo-run-001 --interval 1`,
then `deadbolt kill --agent demo-run-001` from the operator terminal. The
expected allow/deny sequence is the same. Windows builds and TCP client contracts passed on GitHub Actions Windows
Server 2025 on 2026-09-29; validate your own application dispatch paths too.
