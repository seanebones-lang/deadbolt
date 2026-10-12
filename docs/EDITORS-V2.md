# Install stable DeadBolt 2.0.0 in VS Code and Cursor

This route uses **MCP bundle 2.0.0** and stable **engine 2.0.0**. The helper
checks the exact bundle hash before extracting, then exports client-specific
commands without editing an editor profile. Its protocol tests use disposable
marker effects. Earlier model/GUI receipts keep their original versions and
scope; they are not stable-byte GUI acceptance.

For the historical engine 1.0.3 / bundle 1.1.0 route, see [the archived-version
instructions](EDITORS.md). Use version 2 for a new installation.

You need Python 3.9+, Node.js 20+, and one of macOS ARM/Intel, Linux x64/glibc
or Windows x64. The Mac engines are signed/notarized; the outer bundle and
Linux/Windows engines are unsigned. Your editor's AI chat may require an account.
No model key, cloud account or Rust toolchain is needed for the protocol test.

## Prepare and verify outside the workspace

1. Download [deadbolt-2.0.0.mcpb](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v2.0.0/deadbolt-2.0.0.mcpb).
2. Download and review [editor_setup_v2.py](assets/editor_setup_v2.py).
   It has no third-party dependencies, makes no network requests and does not
   edit an editor profile. Keep it separate from the historical 1.x helper.
3. In the directory holding both downloads, select a **new private directory
   outside any agent-editable workspace**:

   ```sh
   OUT="$HOME/.local/share/deadbolt/editor-2.0.0"
   python3 editor_setup_v2.py prepare --bundle deadbolt-2.0.0.mcpb --out "$OUT"
   python3 editor_setup_v2.py verify --out "$OUT"
   ```

   On Windows PowerShell:

   ```powershell
   $Out = Join-Path $env:LOCALAPPDATA 'NextEleven\DeadBolt\editor-2.0.0'
   python editor_setup_v2.py prepare --bundle deadbolt-2.0.0.mcpb --out $Out
   python editor_setup_v2.py verify --out $Out
   ```

The helper verifies the entire bundle before extraction against SHA-256
`50f9b7ac4c56f9a6cfa1c20e310b1a4970b373903edc0dd3faf4f5fb4173c977`.
A mismatch fails without installing; an existing directory is preserved. Supply
`--node /absolute/path/to/node` to `prepare` if Node is not on PATH. Exported
configurations contain absolute paths; prepare new configuration if you move it.

Expected verification: `"passed": true` with **nine checks**, including read-only
setup, allowed writes, off-list denial, required/consumed approval, pause/resume
and kill. Every denial must leave the marker unchanged. Policy denials are MCP
tool results with `isError: true` and the actual reason code, rather than generic
protocol errors. The test removes its temporary run afterward.

## Add the read-only setup connection

For Cursor, open the generated `install.html`, review the local command and click
**Add deadbolt-v2-setup to Cursor**. The helper follows
[Cursor's documented install-link format](https://cursor.com/docs/mcp/install-links).
If the browser cannot open the editor link, use Cursor's native MCP configuration
UI to open the intended profile/workspace `mcp.json`, then merge the generated
`cursor-mcp.json` entry into `mcpServers`. Review and enable only that connection.
The historical RC2 GUI run used this native configuration route.
For VS Code, use **MCP: Open User Configuration**, then merge the generated
`vscode-mcp.json` entry into `servers`, preserving existing servers and `inputs`.
Start the connection and review its local-server trust prompt. See
[VS Code's MCP guide](https://code.visualstudio.com/docs/agent-customization/mcp-servers).
For another client supporting `mcpServers`, use the exported `portable-mcp.json`.
Do not overwrite an existing profile or workspace configuration.

Setup exposes only `deadbolt_get_integration_guide` and `deadbolt_check_bundle`.
Ask the guide for `{"route":"stdio_client"}`, then check the bundle with `{}`.
**Setup has no active protection and starts no downstream server.** A separate
profile does not necessarily isolate a client's global MCP file; preserve other
connections when testing and removing this one. Keep MCP approvals manual during
a disposable test; approve individual dummy calls once, rather than granting
persistent execution permission.

## Historical RC2 graphical acceptance

On October 12, 2026, the checksum-pinned RC2 engine was called by the actual
agent inside each editor: **18 gated calls / 7 effects in Cursor** and
**17 gated calls / 5 effects in VS Code**. All 23 denied calls produced no new
dummy effect. Both agents reported `purpose_exceeded`, `needs_human`, `paused`,
`killed`, `lease_expired`, `store_unavailable` and `deadline_exceeded`.
Checks included one-use grants, pause/resume, clipping, rejected late renewal,
a disposable store outage and deadline precedence. A native MCP connection
reload/restart refused the same killed identity without starting the child again.
These checks did not establish damaged-store reload refusal. A later Cursor
RC2 plugin retest found that defect; it was corrected in RC3 and retained in
stable 2.0.0. A subsequent actual Cursor RC3 retest passed conflicting
destinations, damaged-store Reload and a post-fixed-deadline model call.
That separate run used Cursor 3.24.12 on macOS arm64, with the underlying Auto
model unknown; it did not rerun real idle expiry or certify the product.
Full editor quit/reopen was not tested.

These runs used separate four-tool marker fixtures and finite trusted supervisor
helpers, rather than the three-tool activity-mode demo below. The store outage
stopped each heartbeat; the subsequent deadline probe tested reason precedence
after both bounds elapsed. Claude Desktop Chat separately tested live supervision
clamped to its fixed deadline. Two early Cursor expiry probes correctly ran
before expiry; the later confirmed-expiry probe denied. The receipt preserves
these procedural details instead of counting them as product failures.

No real accounts or documents were connected. Only the dummy MCP tools were
covered: editor built-ins and unrelated connections were not routed through
DeadBolt. These are NextEleven checks, not independent certification, Linux/Windows
GUI acceptance, or a guarantee for your integration. Test your actual effect route.

## Try the protected disposable route

Create the run when you are ready:

```sh
python3 editor_setup_v2.py demo --out "$OUT"
```

The output identifies a fresh `demo` directory. Set `DEMO` to that exact path
(`$Demo` in PowerShell). Its `install.html` or `vscode-mcp.json` configures the
distinct **deadbolt-v2-demo** server. It exposes three dummy tools that append
only their name to this demo's fixed `effects.txt`. No accounts are connected.

This short dummy demo explicitly selects **activity renewal**, a **60-second idle
TTL** and a **fixed deadline ten minutes after preparation**. Connect promptly
and keep gaps short. Pausing/permission prompts count toward idle time. A dead
run cannot be resumed by retrying; create a new demo and reconnect. Ordinary
fresh version 2 roots default to supervisor renewal instead. Provision the separate
supervisor heartbeat before initial expiry for a real supervised run; see
[supervision](SUPERVISION.md). Neither mode can move the fixed deadline.

Use a fresh editor chat and call `demo_write_marker` with `{}`: one marker should
appear. Call `demo_off_list`: expect `purpose_exceeded`, with no marker. Call
`demo_publish_marker`: expect `needs_human`, with no marker. From your operator
terminal, grant its single broad tool approval:

```sh
python3 editor_setup_v2.py operator --out "$OUT" --demo "$DEMO" approve
```

Call `demo_publish_marker` once: one marker. The next call requires approval
again. This fixture uses a broad one-shot tool grant, not an exact-action grant.
Operator controls remain outside the model's tool list:

```sh
python3 editor_setup_v2.py operator --out "$OUT" --demo "$DEMO" pause
# Call demo_write_marker: paused; no new marker.
python3 editor_setup_v2.py operator --out "$OUT" --demo "$DEMO" resume
# Call it before expiry: allowed; exactly one new marker.
python3 editor_setup_v2.py operator --out "$OUT" --demo "$DEMO" kill
# Call it: killed; no new marker. Reconnecting must not revive it.
```

Use `status` through the same helper to inspect this run. Record model tool
results and the corresponding marker contents when performing a GUI test;
running `verify` in a terminal alone does not prove that test passed.

## Remove the test and integrate deliberately

Disable/remove only `deadbolt-v2-demo` and `deadbolt-v2-setup` from the editor
before deleting the private test directory. Preserve unrelated connections.
For real tools, read [the matching bundle configuration](../dist/mcpb/README.md),
[migration](MIGRATING-V2.md), [trust boundary](TRUST.md) and [privacy](PRIVACY.md).
Keep credentials, policy and operator state outside the model's writable context.

Only calls routed through the gate are protected. Destination declarations and
offline exceptions are policy inputs, not OS egress enforcement. A local private
directory does not defend against another process with the same user privileges.
Kill stops future admission; cancellation of already running work belongs to
the executor. Start with one real route and its own effect-based acceptance test.
