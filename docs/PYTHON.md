# Python client and OpenAI function tools (unreleased)

This source candidate provides an installable Python client and an optional
OpenAI Agents SDK adapter. It is not published on PyPI and is not in v1.0.3.
The client talks to the separately installed DeadBolt executable; the wheel
does not bundle or start it. The base client uses only Python's standard library
and requires Python 3.9+. The optional adapter uses the pinned, tested
`openai-agents==0.23.1`; use Python 3.10+ for that SDK.

## Install a reviewed checkout

Create your application's virtual environment, activate it, then install from
an absolute path to the reviewed source or extracted candidate archive:

```sh
python -m pip install /absolute/path/to/deadbolt
# Optional SDK integration:
python -m pip install '/absolute/path/to/deadbolt[openai-agents]'
```

For a wheel, build with `python -m build --outdir release-artifacts/python` in
the source checkout. Install the resulting `.whl` by its full path. Install with
`[openai-agents]` to include the SDK, or omit it to keep the client dependency-free.
Do not use `pip install n11-deadbolt-client` until a registry release exists.

The wheel installs `deadbolt_client`, `deadbolt_openai_agents` and the
`deadbolt-client` command. Existing source imports of `deadbolt_client` keep
working; there is one implementation, rather than a copied package client.

## Operator setup, outside the agent

See [operations](OPERATIONS.md) for durable state, private paths and credentials.
Use a new executor-assigned run ID and configure its policy before running it.
On Unix the default local socket can be used. Windows requires a loopback TCP
address and `DEADBOLT_TOKEN` in both the sidecar and client environments.

```sh
deadbolt serve
# In another terminal, with the same sidecar/state configuration:
deadbolt-client ensure --agent my-run-001
deadbolt policy --agent my-run-001 --tools write_file
```

Check that `ensure` returns `ok: true`. Do not expose `ensure`, policy, approval,
resume, spend or child-registration methods as model tools. Lease creation and
explicit `ensure` calls belong to the trusted executor; this decorator never
calls `ensure`. Ordinary admission retains DeadBolt's sliding TTL for already
live leases. It cannot revive an expired or killed run.

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
arguments.** Prefer the SDK's application-controlled per-call review for exact
payload review while designing a payload-bound DeadBolt approval protocol.
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
  policy or operator approval. The sidecar token currently grants all route
  authority; it is not a scoped agent credential.
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
