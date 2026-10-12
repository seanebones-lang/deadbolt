# Python client and optional OpenAI Agents SDK adapter

## Published stable 2.0.0

Use matching engine **2.0.0** and client **2.0.0** from the
[immutable GitHub release](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0).
Install the engine separately from its verified native archive or the macOS
Homebrew stable cask. The client wheel does not include the executable or start a
sidecar. [Migration](MIGRATING-V2.md) covers old state and Rust source compatibility.

The 2.0.0 wheel includes `deadbolt_client`, `deadbolt_openai_agents`,
`deadbolt_supervisor`, `deadbolt-client`, the job-scoped `SupervisorHeartbeat`
helper and finite `deadbolt-supervisor-heartbeat` operator command. See
[heartbeat lifecycle](SUPERVISION.md#job-scoped-helper-rc2-source) before setup.
The base client has no third-party runtime dependencies. The optional extra pins
`openai-agents==0.23.1` and requires Python 3.10+. Older releases retain their bytes.

## Install from PyPI

In your application's virtual environment:

```sh
python -m pip install 'n11-deadbolt-client==2.0.0'
# Optional SDK adapter, Python 3.10+:
python -m pip install 'n11-deadbolt-client[openai-agents]==2.0.0'
```

The [PyPI release](https://pypi.org/project/n11-deadbolt-client/2.0.0/)
preserves the exact verified GitHub wheel/source bytes. Install the engine separately.
The packaged description is a frozen source snapshot; this current guide tracks
available routes. A registry account or publishing token is not needed to install.

## Install the verified release wheel

Verify `SHA256SUMS` from the release, activate your application's virtual
environment, then install the matching wheel:

```sh
python -m pip install /absolute/path/to/n11_deadbolt_client-2.0.0-py3-none-any.whl
# Optional SDK adapter:
python -m pip install '/absolute/path/to/n11_deadbolt_client-2.0.0-py3-none-any.whl[openai-agents]'
```

The wheel runs on Python 3.9+; the optional process-cancellation reference needs
its documented POSIX/Python runtime. Select the exact 2.0.0 package; an RC1
registry command does not install the new heartbeat helper. No account or API key is needed
for wheel installation or the provider-free acceptance tests.

Alternatively install the reviewed 2.0.0 source with Python 3.10+:

```sh
python -m pip install /absolute/path/to/reviewed/deadbolt
```

Use the immutable `v2.0.0` source tag and matching executable. Installing a
new wheel alone does not change stored renewal modes or upgrade a running sidecar.

## Operator setup, outside the agent

See [operations](OPERATIONS.md) for durable state, private paths and credentials.
Use a new executor-assigned run ID and configure its policy before running it.
On Unix the default local socket can be used. Windows requires a loopback TCP
address. Keep `DEADBOLT_TOKEN` in the sidecar/operator environment.
Leave `DEADBOLT_ADMISSION_TOKEN` absent during operator setup below. Set it
only in the dispatcher environment after lease/policy setup and issuance; see
[credential issuance](CREDENTIALS.md).

```sh
deadbolt serve
# In another terminal, with the same sidecar/state configuration:
deadbolt-client ensure --agent my-run-001
deadbolt policy --agent my-run-001 --tools write_file
# Then issue the admission credential and launch the dispatcher with it.
```

`ensure` returning `ok: true` confirms the record exists, not that the run is
currently authorized. Existing expiry, deadlines, pause and kill state are
preserved. The CLI also prints the record's effective state and expiry; its
success exit code means record reconciliation succeeded. Check `status` for
diagnostics and use each dispatch's admission decision before running effects.
Do not expose `ensure`, policy, approval,
resume, spend or child-registration methods as model tools. Lease creation and
explicit `ensure` calls belong to the trusted executor; this decorator never
calls `ensure`. Published RC1 and existing activity-mode leases retain a sliding
idle TTL. Fresh 2.0.0 runs instead default to supervisor renewal: provision
the separate heartbeat before their initial idle expiry, or deliberately choose
activity mode through trusted setup. Neither mode's admission revives an expired
or killed run.

## Add the decorator to the actual body

```python
from pathlib import Path
from agents import Agent
from deadbolt_openai_agents import protected_tool

@protected_tool(agent_id="my-run-001")
async def write_file(content: str) -> str:
    """Write the application-owned output file."""
    Path("output.txt").write_text(content)
    return "written"

agent = Agent(name="Writer", tools=[write_file])
```

Use `protected_tool` in place of `function_tool` on Python functions. The SDK
still generates the schema, validates arguments, handles context and runs its
own guardrails and approval flow. DeadBolt checks immediately before the
validated function body runs, including after a paused SDK run is approved and
resumed. Synchronous bodies run in a worker thread; asynchronous bodies are
awaited. Return values are preserved. A denial raises `DeadboltDenied`; the SDK
Runner wraps it in `agents.exceptions.UserError`, preserving it as the cause.
It stops the run and does not invoke the body. Ordinary body errors also stop
the run through that SDK wrapper. Handle denials without parsing error text:

```python
from agents import Runner
from agents.exceptions import UserError
from deadbolt_openai_agents import denial_code

try:
    result = await Runner.run(agent, user_input)
except UserError as error:
    code = denial_code(error)
    if code is None:
        raise  # Ordinary tool/SDK failure, not an admission denial.
    print("Action blocked:", code)
```

Display a blocked action or request operator intervention. Do not blindly retry.

The decorator forwards SDK options such as `name_override`, `needs_approval`,
`tool_input_guardrails` and `timeout`. For example:

```python
@protected_tool(agent_id="my-run-001", name_override="send_email", needs_approval=True)
async def send_email(recipient: str, body: str) -> str:
    # Your trusted, validated mail implementation goes here.
    ...
```

The SDK's human approval and DeadBolt admission are separate decisions. This
adapter does not automatically grant either one. If the operator configures
`send_email` as irreversible in DeadBolt, it also needs a DeadBolt one-shot
approval. **That existing approval covers the agent/tool pair, not these exact
arguments.** Use the SDK's application-controlled per-call review or configure the stable release's [exact-action grant](ACTION-APPROVALS.md#optional-openai-agents-sdk-adapter)
with `action_grant` to bind the actual validated arguments.
SDK error-as-output handlers are rejected; `failure_error_function` must be
`None`, so a blocked action or failing body stops instead of encouraging retries.

For destination policy, supply a fixed host or a trusted synchronous resolver:

```python
@protected_tool(agent_id="my-run-001", destination=lambda args: args["host"])
async def fetch(host: str) -> str:
    # Validate/canonicalize host and enforce it in the actual HTTP implementation.
    ...
```

Resolvers see the SDK-validated Python arguments, including defaults and context
when present. They must return a valid DeadBolt host token; a resolver failure
or missing destination stops before admission and does not consume a one-shot.
They must identify the host the body really uses. DeadBolt compares tokens; it
does not constrain redirects, DNS, socket traffic or other destinations visited
inside the body. A custom tool name is not automatically recognized as a
network tool; always provide its actual destination when using destination policy.

## Coverage and trust boundary

For Python MCP servers you own, the separate [FastMCP recipe](FASTMCP.md) uses
the same installed client's `dispatch_async` inside the effect function. It
needs no OpenAI SDK or model key and is tested with FastMCP 4.1.0. Install that
recipe in its own environment; it does not change the pinned optional SDK contract.

- Decorate every protected function on every agent, including specialists reached
  through handoffs. This adapter does not scan an Agent or silently wrap all tools.
- Existing `FunctionTool` objects, `Agent.as_tool()` wrappers, hosted tools,
  computer/shell/apply-patch tools and handoffs are not accepted by this decorator.
  Gate their actual local implementations separately. For local stdio MCP, use
  the [MCP proxy](INTEGRATION.md#bolt-on-stdio-mcp-proxy).
- SDK schema validators, approval callbacks, guardrails, destination resolvers,
  error handlers and hooks are trusted application code outside the protected
  function body. Do not put protected effects there.
- The model must not control the run ID, tool identity, gate credentials, stored
  policy or operator approval. Use a fixed-agent admission credential for the
  dispatcher. `DEADBOLT_TOKEN` retains all operator route authority and must stay
  outside workload access. Setting `DEADBOLT_ADMISSION_TOKEN` puts all client
  calls in workload mode, including control calls, with no operator fallback.
- Admission is not atomic with execution, does not revoke work already running,
  and does not create a filesystem/network/process sandbox. A cancelled sync
  body may continue in its worker thread; other already admitted tools in a
  parallel batch may finish when one tool is denied. Protect resources with separate OS and
  service controls when the process can execute untrusted code.
- This adapter supports the pinned Python SDK contract. TypeScript, hosted MCP
  and other SDK versions need their own acceptance; they are not claimed here.

Try the disposable, one-command demonstration first:

```sh
python /absolute/path/to/deadbolt/examples/openai_agents_demo.py --binary /absolute/path/to/deadbolt-executable
```

It starts its own temporary sidecar, writes one allowed file, revokes the run,
verifies the second write was blocked, and cleans up. It uses the real SDK with
scripted tool calls, disables tracing and makes no provider requests.

Run the installed-wheel acceptance suite with the real sidecar:

```sh
python /absolute/path/to/deadbolt/tests/openai_agents_acceptance.py
```

Set `DEADBOLT_BIN` to your reviewed executable's absolute path when it is not
at the source checkout's `target/debug/deadbolt`. Tests use the SDK's
`ScriptedModel` and disable tracing. They perform local file effects and
approval/resume flows without provider requests, API credentials or paid turns.
Automated framework acceptance is not outside customer adoption.

## Exact-action approval

The opt-in [exact-action protocol](ACTION-APPROVALS.md) binds reviewed arguments,
run, tool, destination, nonce and deadline. Its new endpoint cannot fall back to
legacy admission. Approval enables exact-only policy for that run/tool;
nonces persist after use/revocation. Existing unconfigured integrations retain
their behavior. This was introduced in 1.1.0-rc.1 and is included in stable 2.0.0. Older binaries cannot enforce these new
requirements; quiesce protected dispatch before any rollback.

## Optional owned-process executor (2.0.0)

For commands already running, see the [trusted supervisor reference](EXECUTOR-CANCELLATION.md).
It adds POSIX process-group cleanup to 2.0.0 wheels; the older published 1.1.0rc1 wheel
does not contain it. It uses read-only status, never repeated admissions, and
refuses unsupported hosts before starting work. This is separate from the normal
callback wrappers and does not turn them into cancellable processes.

## Optional supervised idle expiry (2.0.0)

[Supervisor-only renewal](SUPERVISION.md) is the default for fresh 2.0.0
root runs. RC1 requires explicit supervisor setup; existing activity-mode runs
retain their original renewal mode. Admissions do not refresh those leases; a
separate root-bound credential maintains only still-live members. A fixed deadline remains immutable
and wins even during heartbeat activity. The explicit Python renewal call never
uses operator/workload environment authentication. Published 1.x artifacts
do not include this feature; source, migration and release gates apply. Running
work still needs executor-owned cancellation and OS isolation where appropriate.
