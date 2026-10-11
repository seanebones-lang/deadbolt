# Put a permission gate between your agent and its tools

An agent can understand a task and still attempt an action you did not authorize.
The useful place to decide whether that action may run is the dispatcher that
actually performs it. A prompt can explain the rules; a dispatcher can enforce
them before a connected tool body starts.

I built DeadBolt at NextEleven to provide that local admission point. It is free,
open source under Apache-2.0, and usable as an embedded Rust library, local HTTP
sidecar, or proxy in front of a trusted stdio MCP server. This walkthrough uses
the MCP route, with disposable marker tools instead of real accounts.

**Authorship disclosure:** this article was drafted by an AI coding assistant.
The implementation and instructions were checked against the real published
bundle and disposable tool effects. Those checks do not establish that every
agent, client or deployment is protected.

## Choose one route to protect

For this example, the path is:

```text
Editor / MCP client
        ↓ tools/call
DeadBolt admission check ← trusted operator approval / kill
        ↓ only after admission
Trusted disposable MCP server
        ↓
Local marker file
```

The editor can request an action. The operator owns policy and approval. DeadBolt
checks the routed request before forwarding it. The marker file makes it possible
to distinguish a denied response from a tool that ran anyway.

This is a specific integration boundary. The proxy does not intercept built-in
terminal commands, file editing, other MCP connections or arbitrary network
traffic. The child server retains its operating-system permissions and must be
trusted. Initialization and non-tool MCP methods are outside the admission gate.

## Start without a model account

Download [DeadBolt MCP bundle 1.1.0](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v1.1.0/deadbolt-1.1.0.mcpb)
and [the configuration helper](https://seanebones-lang.github.io/deadbolt/assets/editor_setup.py).
Review the helper, save both in Downloads, and use Python 3.9+ with Node.js 20+.
The bundle supports Apple Silicon/Intel Macs, Linux x64 with glibc and Windows x64.

From Downloads on macOS/Linux:

```sh
OUT="$HOME/.local/share/deadbolt/editor-1.1.0"
python3 editor_setup.py prepare --bundle deadbolt-1.1.0.mcpb --out "$OUT"
python3 editor_setup.py verify --out "$OUT"
```

Windows users can follow the PowerShell equivalents in
[the editor installation guide](https://seanebones-lang.github.io/deadbolt/editors.html).
Use a new private directory outside any agent-editable workspace.

The helper verifies the full bundle's published checksum, refuses to overwrite
an installation, and exports configuration without modifying existing profiles.
The verification starts a disposable server, tests real calls and removes its
temporary state. It needs no model key, hosted operator endpoint or connected
account. The bundle is version 1.1.0; its engine is stable 1.0.3.

Expected results include:

```json
{"case":"allowed_marker","passed":true,"body_ran":true}
{"case":"purpose_exceeded","passed":true,"body_ran":false}
{"case":"needs_human","passed":true,"body_ran":false}
{"case":"killed","passed":true,"body_ran":false}
```

The complete report also verifies one approved execution and that its approval
is consumed. This is a protocol test with local effects, not a benchmark of a
model's judgment or proof of enforcement in your application.

## Add it to an editor

For Cursor, open the generated `install.html` and review the local command in its
installation prompt. For VS Code, merge the exported `vscode-mcp.json` server
entry into the configuration opened by **MCP: Open User Configuration**. Preserve
existing servers. A new portable workspace configuration is also exported.

Initially, only two read-only setup tools are available: an integration guide and
a checksum check. **Connected setup is not active protection.** It starts no
downstream server. Use this mode to check the runtime before wiring an action.
An editor's AI chat may need its own account even though the local protocol test
does not.

## See denial, approval and revocation

Create a fresh disposable demo with:

```sh
python3 editor_setup.py demo --out "$OUT"
```

Use the returned directory's install link or configuration to add **deadbolt-demo**.
Its three tools append their names to a fixed local file. Set `DEMO` to the
returned directory path. There are no real emails, payments or account actions.

1. Call `demo_write_marker({})`. The allow-listed action adds one line.
2. Call `demo_off_list({})`. The policy blocks it with `purpose_exceeded`; no line
    is added.
3. Call `demo_publish_marker({})`. It is deliberately classified as irreversible
    for the demonstration and returns `needs_human`; no line is added.
4. In your trusted operator terminal, run:

    ```sh
    python3 editor_setup.py operator --out "$OUT" --demo "$DEMO" approve
    ```

    Call `demo_publish_marker({})` once. It writes a line. A second attempt needs
    approval again.
5. Revoke the run from the operator terminal:

    ```sh
    python3 editor_setup.py operator --out "$OUT" --demo "$DEMO" kill
    ```

    The next marker call returns `killed` and leaves the file unchanged.

Inspect the demo's `effects.txt` and `events.jsonl`. A tool's error response is
useful; evidence that its body did not run is the stronger check. An editor's
Allow/Run button permits an attempted call and does not grant operator approval.

The default lease expires after 60 seconds without admission. If your setup or
conversation takes longer, remove/stop the old demo connection and create a fresh
one. Admission renews a live lease; reconnecting cannot revive an expired or
killed ID. The automated verification handles this demonstration without a manual
conversation delay.

## Carry the boundary into your own application

Replace a trusted downstream server's direct launch with the DeadBolt proxy.
Keep policy, approval and revocation in a separate trusted operator path. Remove
the ungated direct connection, configure actual tool/destination conventions,
and repeat the effect checks against one bounded action in your executor.

Stable 1.0.3 approvals apply once to an agent/tool pair; they are not bound to
exact arguments or recipients. A separately published 1.1.0 prerelease adds
exact-action APIs for evaluation. Do not assume those features are in this bundle.

DeadBolt does not cancel an action already running, make an external action atomic
with admission, or protect routes you have not connected. Same-user malicious code
can also reach same-user state: private folders are useful hygiene, not an OS
sandbox. A deployment's executor and operator separation must be trustworthy.

The next useful test is small: one real route, an allowed action, a denied action
whose body stays untouched, and a revoked run that cannot perform the next effect.
Measure installation time and what confused the developer before widening scope.

[Source and installation guide](https://github.com/seanebones-lang/deadbolt)
· [Captioned overview and full walkthrough](https://seanebones-lang.github.io/deadbolt/#watch)
· [Integration boundaries](https://github.com/seanebones-lang/deadbolt/blob/main/docs/INTEGRATION.md)

If you try it, which step took the most work: installation, choosing the routed
tools, or keeping operator authority separate from the agent?
