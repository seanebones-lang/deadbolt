# Integrate Deadbolt in another project

Deadbolt has no Harness, Witness, model-provider, or API-key dependency. It uses
bundled SQLite and local files. Choose the integration that owns your executor's
actual dispatch boundary. Calling `admit` alone does not intercept execution:
your dispatcher must stop on every result other than `allow`.

## Install from source

Requires Rust 1.85 or newer and a C/C++ build toolchain for bundled SQLite.
The package is named `n11-deadbolt`, the Rust library is `deadbolt`, and the
executable is `deadbolt`. As checked on 2026-09-29, this package is not on
crates.io. Use Git or a local checkout until a registry release is published.

```sh
git clone --branch v1.0.2 --depth 1 https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo install --path . --locked --bin deadbolt
deadbolt drill
```

Pin a reviewed commit when installing for production:

```sh
cargo install --git https://github.com/seanebones-lang/deadbolt.git --rev YOUR_REVIEWED_COMMIT --locked --bin deadbolt
```

## Build in: Rust executor

In your project's `Cargo.toml`:

```toml
[dependencies]
deadbolt = { package = "n11-deadbolt", git = "https://github.com/seanebones-lang/deadbolt.git", tag = "v1.0.2" }
```

For an adjacent source checkout, replace `git` and `tag` with
`path = "../deadbolt"`. No socket or service is needed.

```rust
use deadbolt::{AdmitDecision, Deadbolt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = std::path::PathBuf::from("./agent-state/deadbolt");
    let gate = Deadbolt::open_at(&state, true, 60);
    gate.ensure_agent("my-executor-run-001")?;
    match gate.admit("my-executor-run-001", "write_file") {
        AdmitDecision::Allow => std::fs::write("output.txt", "allowed work")?,
        AdmitDecision::Deny { code } => return Err(code.as_str().into()),
    }
    Ok(())
}
```

Use an executor-assigned ID for each run. IDs and tool names are tokens of at
most 128 bytes. Killed and expired IDs cannot be refreshed with `ensure`; start
a new run with a new ID. `admit_dest(agent, tool, Some(host))` adds an explicit
host for destination policy. `set_policy`, `spend_add`, and `register_child`
are available on the library. Register children before starting them and check
that registration returned `true`. Registration is idempotent for the same
parent, preserves existing state, and rejects reparenting and self-parenting.
Missing, killed, paused, or expired parents cannot create new live children.
Children do not inherit tool, destination, or spend policies: configure each
child before dispatch. Calling `admit` for a spawn is the executor's job.

For an operator CLI to control this store, use these same paths:

```sh
export DEADBOLT_DB="$PWD/agent-state/deadbolt/deadbolt.db"
export DEADBOLT_EVENTS="$PWD/agent-state/deadbolt/deadbolt-events.jsonl"
deadbolt kill --agent my-executor-run-001
```

The gate denies subsequent calls; it does not cancel a tool already running.
A one-shot approval is consumed by admission, even if the subsequent tool or
evidence write fails. Never cache an allow result for later execution.

## Bolt on: Python, Node, or any HTTP client

Run `deadbolt serve` on macOS/Linux for a mode-0600 Unix socket. On Windows,
use loopback TCP and `DEADBOLT_TOKEN`. TCP works on macOS/Linux too. Generate
a fresh 256-bit token before exporting it to the server:

```sh
DEADBOLT_TOKEN=$(python3 -c 'import secrets; print(secrets.token_hex(32))')
export DEADBOLT_TOKEN
export DEADBOLT_SOCK=127.0.0.1:9782
deadbolt serve --bind 127.0.0.1:9782
```

Keep that terminal running. Supply the same endpoint and token to your trusted
executor. For PowerShell, use `$env:DEADBOLT_TOKEN` and `$env:DEADBOLT_SOCK`.
Keep operator access and credentials away from the model-controlled process.

Copy `examples/deadbolt_client.py` or `examples/deadbolt_client.js` into your
project. These are source clients, not published pip/npm packages. Python uses
only its standard library; Node uses built-in modules and CommonJS exports.
Both expose ensure, admit (with optional destination), child registration,
status, policy, and spend. In Node the registration function is `registerChild`.

```python
from deadbolt_client import ensure, admit

agent = "my-executor-run-002"
if ensure(agent).get("ok") is not True:
    raise RuntimeError("deadbolt lease unavailable")

def dispatch(tool_name, body, dest=None):
    result = admit(agent, tool_name, dest)
    if result.get("decision") != "allow":
        raise RuntimeError(result.get("code", "store_unavailable"))
    return body()

dispatch("write_file", lambda: print("tool body runs here"))
```

```js
const { ensure, admit } = require("./deadbolt_client.js");

async function run() {
  const agent = "my-executor-run-003";
  if ((await ensure(agent)).ok !== true) throw new Error("deadbolt lease unavailable");
  async function dispatch(tool, body, dest) {
    const result = await admit(agent, tool, dest);
    if (result.decision !== "allow") throw new Error(result.code || "store_unavailable");
    return body();
  }
  await dispatch("write_file", () => console.log("tool body runs here"));
}
run().catch(err => { console.error(err.message); process.exitCode = 1; });
```

Every language can implement [the HTTP protocol](PROTOCOL.md). Require a
successful HTTP response, valid JSON, and explicit `decision: "allow"` before
running a body. The source clients return deny on transport failures,
unusable decision responses, and unsuccessful HTTP status codes. They remain
cooperative: your executor must prevent any direct dispatch that skips them.

## Bolt on: stdio MCP proxy

```sh
deadbolt mcp-proxy --agent my-mcp-run-001 -- python3 your_mcp_server.py
```

Use that command as the MCP server command in your host's configuration.
It supports newline-delimited JSON-RPC 2.0 objects over stdio. Batches and
invalid envelopes are rejected; forwarded messages are serialized from the
same parsed value used for admission. It does not proxy remote
HTTP/SSE MCP servers. `tools/call` is gated; other methods (including resource
reads and initialization) are forwarded. Those methods and startup side effects
are outside the tool gate. Only place servers you trust behind the proxy.

The executor assigns the fixed agent ID; model arguments cannot change it.
By default the proxy opens the local store. To use a running sidecar, add
`--serve-sock 127.0.0.1:9782` and supply `DEADBOLT_TOKEN`.

Destination discovery examines `url`, `uri`, `href`, `endpoint`, and `host`
arguments. Spend discovery examines numeric `amount`, `usd`, and `cost`.
These are conventions, not authoritative network or billing measurements.
Configure policies for the server's actual tool names; explicitly classify
irreversible tools. With a destination allow-list, only recognized network
classes (`http`, `fetch`, `browser`, `web_search`) require a missing destination.
Custom network tool names need an executor adapter with a known destination.
See [trust boundaries](TRUST.md).

## Linux service

Install the binary at `/usr/local/bin/deadbolt`, create a dedicated `deadbolt`
system account, then install `dist/deadbolt.service`. Set a nonempty token in
`/etc/deadbolt/deadbolt.env` with permissions `0600` before starting the service.
The unit uses `/var/lib/deadbolt` for both database and evidence. Operator CLI
commands must use those same paths and account, for example:

```sh
sudo -u deadbolt env DEADBOLT_DB=/var/lib/deadbolt/deadbolt.db DEADBOLT_EVENTS=/var/lib/deadbolt/deadbolt-events.jsonl /usr/local/bin/deadbolt status
```

## Containers

```sh
cp dist/deadbolt.env.example dist/deadbolt.env
chmod 0600 dist/deadbolt.env
docker compose -f dist/docker-compose.yml up --build
```

Compose uses a Unix socket and a persistent named volume. Attach a trusted
executor container to `deadbolt-state`, run it as UID 10001, and set its
`DEADBOLT_SOCK=/home/deadbolt/.deadbolt/deadbolt.sock`. If you set a token in
the env file, provide the same token to the executor. Use
`docker compose -f dist/docker-compose.yml exec deadbolt deadbolt status`
for operator access. The socket and database are local to the container volume;
Docker Desktop host apps should use a native sidecar instead.

No port is published. Binding `127.0.0.1` inside a container cannot service
Docker's usual forwarded port. Public binds remain refused. Do not change that
boundary to make container networking work.

## Verify your integration

Run `cargo test --locked`, `cargo run --example build_in`, and `deadbolt drill`.
After building the binary, run `python3 tests/client_contract.py` (Python 3 and
Node required). It exercises imported clients against real TCP serve, policy,
spend, operator kill, sidecar outage, and malformed/error responses.

In your own dispatcher, check that kill, expiry, unavailable storage, missing
sidecar, and denied policy prevent observable side effects. Check that killing
one ID leaves unrelated IDs operational. macOS and Linux-container behavior is locally validated in
[the standalone review](STANDALONE-REVIEW.md). Remote Linux/macOS/Windows and Rust 1.85 CI passed on 2026-09-29.
The systemd template passed bounded service acceptance in a disposable Ubuntu
container. Native host installation and actual application acceptance remain open.
