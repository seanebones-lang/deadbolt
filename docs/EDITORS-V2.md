# Evaluate DeadBolt RC2 in VS Code and Cursor

This route uses the published **MCP bundle 2.0.0-rc.2**, containing the matching
corrected engine. It is a developer preview. Actual graphical-client/model
acceptance and independent installation remain open gates; the tests below
exercise exported commands and real disposable effects through stdio.
For the unchanged stable route, see [bundle 1.1.0 installation](EDITORS.md).

You need Python 3.9+, Node.js 20+, and one of macOS ARM/Intel, Linux x64/glibc
or Windows x64. The Mac engines are signed/notarized; the outer bundle and
Linux/Windows engines are unsigned. Your editor's AI chat may require an account.
No model key, cloud account or Rust toolchain is needed for the protocol test.

## Prepare and verify outside the workspace

1. Download [deadbolt-2.0.0-rc.2.mcpb](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v2.0.0-rc.2/deadbolt-2.0.0-rc.2.mcpb).
2. Download and review [editor_setup_v2.py](assets/editor_setup_v2.py).
   It has no third-party dependencies, makes no network requests and does not
   edit an editor profile. Keep it separate from the stable helper.
3. In the directory holding both downloads, select a **new private directory
   outside any agent-editable workspace**:

   ```sh
   OUT="$HOME/.local/share/deadbolt/editor-2.0.0-rc.2"
   python3 editor_setup_v2.py prepare --bundle deadbolt-2.0.0-rc.2.mcpb --out "$OUT"
   python3 editor_setup_v2.py verify --out "$OUT"
   ```

   On Windows PowerShell:

   ```powershell
   $Out = Join-Path $env:LOCALAPPDATA 'NextEleven\DeadBolt\editor-2.0.0-rc.2'
   python editor_setup_v2.py prepare --bundle deadbolt-2.0.0-rc.2.mcpb --out $Out
   python editor_setup_v2.py verify --out $Out
   ```

The helper verifies the entire bundle before extraction against SHA-256
`a0634e69da89c6e2c38bf72b8c1a1b8ed75e30e77df218841692808998c7b95e`.
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
**Add deadbolt-rc2-setup to Cursor**. The helper follows
[Cursor's documented install-link format](https://cursor.com/docs/mcp/install-links).
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
connections when testing and removing this one.

## Try the protected disposable route

Create the run when you are ready:

```sh
python3 editor_setup_v2.py demo --out "$OUT"
```

The output identifies a fresh `demo` directory. Set `DEMO` to that exact path
(`$Demo` in PowerShell). Its `install.html` or `vscode-mcp.json` configures the
distinct **deadbolt-rc2-demo** server. It exposes three dummy tools that append
only their name to this demo's fixed `effects.txt`. No accounts are connected.

This short dummy demo explicitly selects **activity renewal**, a **60-second idle
TTL** and a **fixed deadline ten minutes after preparation**. Connect promptly
and keep gaps short. Pausing/permission prompts count toward idle time. A dead
run cannot be resumed by retrying; create a new demo and reconnect. Ordinary
fresh RC2 roots default to supervisor renewal instead. Provision the separate
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

Disable/remove only `deadbolt-rc2-demo` and `deadbolt-rc2-setup` from the editor
before deleting the private test directory. Preserve unrelated connections.
For real tools, read [the matching bundle configuration](../dist/mcpb/README.md),
[migration](MIGRATING-V2.md), [trust boundary](TRUST.md) and [privacy](PRIVACY.md).
Keep credentials, policy and operator state outside the model's writable context.

Only calls routed through the gate are protected. Destination declarations and
offline exceptions are policy inputs, not OS egress enforcement. A local private
directory does not defend against another process with the same user privileges.
Kill stops future admission; cancellation of already running work belongs to
the executor. Start with one real route and its own effect-based acceptance test.
