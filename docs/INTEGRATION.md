# Integrate DeadBolt at your dispatch boundary

Choose the route your application actually uses to execute effects. DeadBolt needs
no Harness, Witness, provider or API key. Your trusted executor must deny every
result other than a fresh explicit `allow`.

| Route | Installation | Where the check belongs |
| --- | --- | --- |
| Embedded Rust | Cargo registry or Git/local dependency | Immediately before your tool callback |
| Local HTTP | Native executable plus Python/Node client or HTTP implementation | Trusted dispatcher, before the body |
| Stdio MCP | Native executable used as a server launch wrapper | Every routed `tools/call` |
| Python SDK function tools | Sidecar plus optional Python extra | Decorator on each actual effect function |
| FastMCP server you own | Sidecar and Python client | `dispatch_async` inside each protected tool; [tested recipe](FASTMCP.md) |
| Docker-launched MCP client | Pinned source-built stdio container | One configured trusted downstream inside the container; [Docker guide](DOCKER-MCP.md) |

**Version rule:** v1.0.3 supports the manual admission examples below. Callback
helpers, installable Python/SDK integration, scoped credentials and exact-action
review are available in the 2.0.0-rc.1 prerelease (and the older 1.1 candidate). [Select and install the right source](INSTALL.md).

Use this order in your application:

1. Give the run an executor-assigned ID; create its lease in trusted setup.
2. Configure the actual tool names, destination policy and sensitive-action review.
3. Register each child and configure its routes before dispatch. Unreleased source
   also applies every ancestor’s restrictions; a child can narrow that ceiling.
4. Give the dispatcher only its required authority. The candidate supports a
   [single-agent admission credential](CREDENTIALS.md).
5. Wrap every protected body, then verify effects after allow, policy denial,
   kill, expiry, approval replay and unavailable gate. [Record route coverage](PILOT.md).

## Optional fixed run deadline (2.0 prerelease)

In trusted setup, use `deadbolt issue --agent FRESH_RUN_ID --hard-deadline-at
FUTURE_UNIX_SECONDS`, `gate.issue_agent` in embedded Rust, or operator
`POST /issue`. This creates a new run with a deadline that activity cannot
slide. Registered children inherit it. Existing `ensure` integrations stay
idle-only unless setup opts in with a fresh ID. [Full setup and trust
limits](LEASE-DEADLINES.md) include restart, rollback and wall-clock behavior.
This addition ships in engine 2.0.0-rc.1; the older editor/MCP bundles do not
include it. Select the matching release and record its identity. The normal Python/Node admission response already
carries a denial code; keep callbacks uncalled on every denial.

Unreleased corrected source defaults fresh `ensure` and `issue` identities to
supervisor renewal. Existing identities keep their stored mode; trusted setup
must provision a separate heartbeat before initial idle expiry. Explicit legacy
activity setup and version-aware migration are documented in
[supervision](SUPERVISION.md). A fixed deadline never extends on heartbeat.

<a id="current-source-dispatch-helpers-110-rc1-prerelease"></a>

## Dispatch helpers (2.0 prerelease)

The current source adds helpers that take your actual tool callback and check
immediately before invoking it. They are not in the published v1.0.3 archives.
Select the `2.0.0-rc.1` registry crate or reviewed source tag for these APIs;
install the matching Python release wheel/source package, or copy its Node
source client as described below.

Configure the local gate/sidecar, stable run ID and policy first, then wrap the
actual body. Operator setup and credentials belong outside model control.

```rust
// The callback's I/O result is preserved inside the admission result.
match gate.dispatch("my-run-001", "write_file", None, || {
    std::fs::write("output.txt", "allowed work")
}) {
    Ok(tool_result) => tool_result?,
    Err(code) => eprintln!("blocked: {}", code.as_str()),
}
```

```python
from pathlib import Path
from deadbolt_client import dispatch

outcome = dispatch("my-run-001", "write_file", lambda: Path("output.txt").write_text("allowed work"))
if not outcome["executed"]:
    print("blocked:", outcome["decision"]["code"])
else:
    print("tool result:", outcome["result"])
```

```js
const { dispatch } = require("./deadbolt_client.js");
const fs = require("fs/promises");

async function run() {
  const outcome = await dispatch("my-run-001", "write_file", () => fs.writeFile("output.txt", "allowed work"));
  if (!outcome.executed) console.log("blocked:", outcome.decision.code);
}
run().catch(error => { console.error(error.message); process.exitCode = 1; });
```

Python async applications can `await dispatch_async(agent, tool, body, dest=None)`;
admission I/O runs in a worker thread and the returned awaitable is awaited once.
The synchronous Python helper rejects async bodies rather than returning a
deferred coroutine under an old admission. Node's `dispatch` supports sync and
async callbacks. Rust async executors can
`gate.dispatch_async(agent, tool, dest, || async { /* actual body */ }).await`;
admission occurs when the wrapper is polled, not when its future is created.
Rust admission performs synchronous SQLite I/O; plan runtime capacity for storage
contention. Use synchronous Rust `dispatch` for synchronous bodies.

Python/Node helpers return `executed: false` plus the denial on blocked work,
or `executed: true`, the admission decision and callback result on success.
Rust returns `Result<T, DenyCode>` and preserves the callback's value in `T`.
Callback errors propagate normally and are never retried. Validate callback
arguments in your trusted executor; destination is an optional host token and
spend must be recorded separately. Configure each child's policy.

For Python package installation and the OpenAI Agents SDK's validated tool-body
decorator, see [Python integration](PYTHON.md). This optional integration supports
the pinned Python SDK contract; it does not automatically gate an entire agent
or hosted tools. Its installed-wheel tests exercise the actual SDK runner and
sidecar without a model-provider account.

Each protected route needs this wrapper. It does not intercept arbitrary code,
make the body atomic with admission, or cancel running work. Never call `admit`
separately before a one-shot helper invocation or cache an allow. Try
`cargo run --locked --example build_in` and
`python3 examples/evaluate.py --binary /absolute/path/to/deadbolt` from this source
checkout to inspect real allowed/blocked effects without an AI account.

## Installation and version selection

[Installation](INSTALL.md) is the canonical guide for native archives, reviewed
source, the Cargo registry and Python release packages. The Rust package is
`n11-deadbolt`, its library is `deadbolt`, and its executable is `deadbolt`.
Pin the reviewed registry version or a full Git revision for a deployment.
The prerelease Python client is on [PyPI](https://pypi.org/project/n11-deadbolt-client/2.0.0rc1/):
`python -m pip install 'n11-deadbolt-client==2.0.0rc1'`. Install the matching
candidate engine separately; see [Python setup](PYTHON.md). Node remains a source client.

## Build in: Rust executor

In your project's `Cargo.toml`:

```toml
[dependencies]
deadbolt = { package = "n11-deadbolt", version = "=1.0.3" }
```

That dependency uses the **published v1.0.3 API**. For the candidate helpers,
replace the version value with `"=2.0.0-rc.1"`, or use a reviewed adjacent checkout:

```toml
[dependencies]
deadbolt = { package = "n11-deadbolt", path = "../deadbolt" }
```

No socket, bearer token, Python package or service is needed. State is persistent;
keep it outside disposable build output. This complete example works with v1.0.3
or the candidate and configures policy before dispatch:

```rust
use deadbolt::{AdmitDecision, Deadbolt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = std::path::PathBuf::from("./agent-state/deadbolt");
    let gate = Deadbolt::open_at(&state, true, 60);
    gate.ensure_agent("my-executor-run-001")?;
    gate.set_policy("my-executor-run-001", deadbolt::PolicyPatch {
        tools_allow: Some(vec!["write_file".into()]),
        ..Default::default()
    })?;
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
In unreleased source, admissions apply every ancestor’s current tool, destination,
clip and approval restrictions. A child’s configuration can narrow those limits,
not widen them. A spending cap covers its registered subtree. Policies and grants
are not copied: approvals remain bound to the actual child being admitted. Each
MCP child still needs its own explicit tool allowlist. Published 2.0.0-rc.1 and
1.x engines require independently configured child policies and do not provide
these ceilings. Calling `admit` for a spawn is the executor's job.

For an operator CLI to control this store, use these same paths:

```sh
export DEADBOLT_DB="$PWD/agent-state/deadbolt/deadbolt.db"
export DEADBOLT_EVENTS="$PWD/agent-state/deadbolt/deadbolt-events.jsonl"
deadbolt kill --agent my-executor-run-001
```

The gate denies subsequent calls; it does not cancel a tool already running.
An admitted one-shot approval is consumed even if the subsequent body fails.
Post-commit evidence mirror errors can also require reconciliation before retry.
Never cache an allow result for later execution.

<a id="bolt-on-http-sidecar"></a>

## Bolt on: Python, Node, or any HTTP client

Run `deadbolt serve` on macOS/Linux for a mode-0600 Unix socket; unreleased
source also requires an operator token there by default. On Windows, use
loopback TCP and `DEADBOLT_TOKEN`. TCP works on macOS/Linux too.
[HTTP authentication](HTTP-AUTHENTICATION.md) describes the version boundary
and explicit legacy migration option. Generate
a fresh 256-bit token before exporting it to the server:

```sh
DEADBOLT_TOKEN=$(python3 -c 'import secrets; print(secrets.token_hex(32))')
export DEADBOLT_TOKEN
export DEADBOLT_SOCK=127.0.0.1:9782
deadbolt serve --bind 127.0.0.1:9782
```

Keep that terminal running. Supply the same endpoint and token to your trusted
executor. PowerShell server setup:

```powershell
$env:DEADBOLT_TOKEN = python -c "import secrets; print(secrets.token_hex(32))"
$env:DEADBOLT_SOCK = "127.0.0.1:9782"
deadbolt serve --bind $env:DEADBOLT_SOCK
```

Another terminal does not inherit these variables. Supply the same endpoint and
private operator token through trusted setup; never print or commit the token.
Keep operator access and credentials away from the model-controlled process.
For the current candidate, finish trusted lease/policy setup and issue an
[admission credential](CREDENTIALS.md); pass only `DEADBOLT_ADMISSION_TOKEN`
to the dispatcher. The shared operator token remains a compatibility path.

Install the candidate [Python package](PYTHON.md), or copy the selected
revision's `examples/deadbolt_client.py` / `examples/deadbolt_client.js` beside
your application. The Node client is source-only; neither is on a registry.
Python uses
only its standard library; Node uses built-in modules and CommonJS exports.
Both expose ensure, admit (with optional destination), child registration,
status, policy, and spend. Those control methods belong to trusted operator setup.
The following stable-compatible examples keep setup and dispatch together for a
local demonstration; a candidate workload credential cannot call `ensure`.
Create the lease through the operator before switching to workload credentials.
In Node the registration function is `registerChild`.

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

**Unreleased 2.0 candidate:** the raw proxy no longer creates leases at runtime.
Before starting it, trusted operator setup must create a fresh run and set its
tool allowlist. For a local idle-only demonstration:

```sh
deadbolt ensure --agent my-mcp-run-001
deadbolt policy --agent my-mcp-run-001 --tools read_file,write_file
deadbolt mcp-proxy --agent my-mcp-run-001 -- python3 your_mcp_server.py
```

Use your server's actual tool names. The short demonstration runs within the
initial idle window. For ongoing work, provision the supervisor and heartbeat
described above; corrected source defaults fresh `ensure` roots to that mode.
Explicit `--activity-renewal` is available for a deliberately selected legacy
activity run. A missing lease denies `no_lease`; an unset tool policy denies
`policy_required`; an empty allowlist denies every tool. The check also applies
to exact-action grants and workload credentials. Setup denials do not slide the
idle TTL or consume approvals. Published rc.1 and 1.x proxies retain their older
setup behavior until a corrected candidate is published.

Use that command as the MCP server command in your host's configuration.
It supports newline-delimited JSON-RPC 2.0 objects over stdio. Batches and
invalid envelopes are rejected; forwarded messages are serialized from the
same parsed value used for admission. It does not proxy remote
HTTP/SSE MCP servers. `tools/call` is gated; other methods (including resource
reads and initialization) are forwarded. Those methods and startup side effects
are outside the tool gate. Only place servers you trust behind the proxy.

The executor assigns the fixed agent ID; model arguments cannot change it.
By default the proxy opens the local store. To use a running sidecar, add
`--serve-sock 127.0.0.1:9782`. Stable integrations use the operator token in the
trusted proxy; candidate integrations can use an admission credential after
operator lease/policy setup. See [MCP credential restrictions](CREDENTIALS.md)
and [exact-action metadata](ACTION-APPROVALS.md#mcp-proxy).

The candidate socket proxy uses `/admit-mcp` and `/admit-mcp-action`; pair it with
the matching sidecar. Older sidecars lacking those routes deny execution, with
no fallback to legacy admission or implicit `/ensure`. Server startup,
initialization and other non-tool methods remain outside the tool-call gate.

Destination discovery examines `url`, `uri`, `href`, `endpoint`, and `host`
arguments. Spend discovery examines numeric `amount`, `usd`, and `cost`.
These are conventions, not authoritative network or billing measurements.
Configure policies for the server's actual tool names; explicitly classify
irreversible tools. In current source, a destination allow-list requires every
tool to declare an allowed destination unless the operator explicitly classifies
it in per-lease `offline_tools`. Missing-destination exceptions are never inferred
from tool names. The original released engines use the older name-based rule;
see the [migration guide](DESTINATION-POLICY.md). A declared host is not proof of
the network route: general code execution still needs trusted routing and OS
egress controls.
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

## Exact-action source candidate

The opt-in [exact-action protocol](ACTION-APPROVALS.md) binds reviewed arguments,
run, tool, destination, nonce and deadline. Its new endpoint cannot fall back to
legacy admission. Approval enables exact-only policy for that run/tool;
nonces persist after use/revocation. Existing unconfigured integrations retain
their behavior. This is available in 1.1.0-rc.1. Older binaries cannot enforce these new
requirements; quiesce protected dispatch before any rollback.

## Optional supervised idle expiry (2.0 prerelease)

[Supervisor-only renewal](SUPERVISION.md) is a separate opt-in control for fresh
root runs. Admissions do not refresh those leases; a separate root-bound
credential maintains only still-live members. A fixed deadline remains immutable
and wins even during heartbeat activity. The explicit Python renewal call never
uses operator/workload environment authentication. Engine 2.0.0-rc.1 includes
this feature; older 1.x artifacts do not. Review the migration guide. Running
work still needs executor-owned cancellation and OS isolation where appropriate.
