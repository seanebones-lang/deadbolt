# Build DeadBolt into a FastMCP tool

[Step-by-step local walkthrough](https://seanebones-lang.github.io/deadbolt/fastmcp-tutorial.html)
with pinned lab commands and effect checks.

If you own the MCP server's Python code, check inside each protected function
instead of adding a proxy process. FastMCP still owns its schema and transport;
DeadBolt checks immediately before the actual effect. If you cannot edit the
server, use the [stdio proxy](INTEGRATION.md#bolt-on-stdio-mcp-proxy) instead.

This source recipe uses **FastMCP 4.1.0**, Python 3.10+, published Python client
**1.1.0rc1** and engine **1.1.0-rc.1**. Both DeadBolt versions are prereleases.
The example is added after that release and is in current source, not the
original release archives or wheel. It does not modify the installed client or
add FastMCP as a mandatory DeadBolt dependency. Other framework versions,
remote transports and graphical/model-mediated use need their own checks.

## Try it without a model or real account

Install the [matching engine](INSTALL.md#native-archive-no-rust-required), then
get a reviewed current checkout. Activate a Python 3.10+ virtual environment:

```sh
python -m pip install 'fastmcp==4.1.0' 'n11-deadbolt-client==1.1.0rc1'
python /absolute/path/to/deadbolt/examples/fastmcp_demo.py --binary /absolute/path/to/deadbolt-executable
```

Use the binary's actual full path; the checkout folder is not an executable.
The demo starts a private temporary sidecar and a separate stdio MCP server,
issues a scoped workload credential, verifies file effects, then removes its
temporary state and processes. No model key, cloud account or provider request
is used. Expected JSON: `"passed": true`, `"count": 17`.

It checks discovery without operator controls, off-policy denial, allow, invalid
arguments, pause/resume, broad one-shot approval/replay, clip, invalid/wrong-run/
revoked/expired credentials, kill, reconnect and unavailable sidecar. Every
denied effect is checked against the note's previous bytes. The broader
[acceptance script](../tests/fastmcp_acceptance.py) copies the recipe outside the
checkout, uses the installed wheel and adds three startup refusals: 20 checks.

CI installs the full pinned dependency set in
[`tests/fastmcp-requirements.txt`](../tests/fastmcp-requirements.txt), audits it,
builds/installs the current client wheel, and repeats consumer acceptance on
Linux, macOS and Windows. These are automated local protocol checks, not
independent certification or customer adoption.

## The integration inside a tool

The complete [server](../examples/fastmcp_server.py) has one fixed output path,
an operator-selected run ID and a bounded string argument. The core pattern is:

```python
from fastmcp import FastMCP
from fastmcp.exceptions import ToolError
from deadbolt_client import dispatch_async

mcp = FastMCP("my_service_mcp")
RUN_ID = "operator-assigned-fresh-run"

@mcp.tool
async def update_record(content: str) -> str:
    async def effect():
        # Your trusted validation and actual database/file operation go here.
        # Keep every protected effect inside this callback.
        return await application_write(content)

    outcome = await dispatch_async(RUN_ID, "update_record", effect)
    if not outcome["executed"]:
        code = outcome["decision"].get("code", "unknown")
        raise ToolError(f"deadbolt:{code}; action withheld. Contact the operator.")
    return outcome["result"]
```

`application_write` represents your own implementation; use the complete
server for a runnable local file example. Keep run ID, tool identity, paths and
credentials out of the MCP input schema. `dispatch_async` does admission I/O
in a worker thread and invokes/awaits the callback only after explicit allow.
Do not call admission separately first or cache a decision. Decorate/register
every protected route deliberately; this recipe does not scan or wrap a server.

## Keep operator setup separate

Use [private durable state and operator credentials](OPERATIONS.md), create a
fresh run lease, set policy for the tool names actually exposed, and issue a
[single-agent admission credential](CREDENTIALS.md). Give the workload process
only its sidecar address and scoped admission key. Keep the operator key and
state paths outside it. The complete server refuses an operator token or absent
sidecar/admission configuration at startup.

For the demonstration server, operator policy uses `demo_write_note`; supply
`--agent YOUR_FRESH_RUN_ID --output /absolute/operator-selected/note.txt` and the
two workload environment variables `DEADBOLT_SOCK` and
`DEADBOLT_ADMISSION_TOKEN`. Keep secrets in private client configuration, never
in a shared repository. The MCP client launches the virtual environment's
Python with the server's absolute path. Operator ensure, policy, approve,
pause/resume, clip, kill and credential issuance stay in a separate trusted
process. The disposable demo shows that separation in code; it is not an
operator service for real accounts.

The run has a sliding lease. Ordinary admission renews a live lease; this server
never calls `ensure` and cannot revive a dead identity. Idle expiry needs an
operator-created new run, not automatic self-approval or a retry loop.

## What the recipe protects

Only the enclosed tool callback is gated. Resources, prompts, startup,
middleware, other tools, background tasks and arbitrary Python/OS/network
access remain outside that callback. MCP annotations are hints, not permission
enforcement. A returned MCP tool error does not stop the entire client/agent:
your application must display the denial or request operator intervention,
and must not retry blindly.

This recipe exercises broad one-shot approval of an agent/tool pair, not exact
arguments. Use the separate [exact-action protocol](ACTION-APPROVALS.md) when
reviewed content must be bound to permission. Admission is not atomic with
execution and does not cancel a running operation. In particular, cancelling
an async call does not stop a file write already running in a worker thread.
Untrusted workload code needs additional process and resource isolation;
scoped headers alone are not an OS sandbox or isolation between same-user
processes. The output path and the body remain trusted application choices.

For changing or multi-user remote servers, assign and authenticate each run in
trusted application code; do not share this local demo's fixed run across users.

Framework references: [FastMCP tools](https://gofastmcp.com/servers/tools) and
[explicit stdio client transport](https://gofastmcp.com/clients/transports).
[Other integrations](INTEGRATION.md) · [Python client](PYTHON.md)
