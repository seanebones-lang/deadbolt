# Use DeadBolt with VS Code and Cursor

For new installations use [the stable 2.0.0 guide](EDITORS-V2.md) and its
version-matched helper. This page retains the historical bundle 1.1.0 / engine
1.0.3 instructions; these older bytes do not include the later version 2 fixes.


Start with two read-only setup tools, then test a disposable downstream server.
The setup connection verifies the bundled executable and explains integration;
it provides **no active protection**. A configured proxy gates only downstream
`tools/call` requests routed through it.

This guide uses **MCP bundle 1.1.0**, containing **stable engine 1.0.3**. It does
not include the prerelease's exact-action approvals or scoped credentials. No
model key, cloud account or Rust toolchain is needed for the local protocol test.
Using an editor's AI chat may require that editor's own account and subscription.

## Prepare the setup connection

You need Python 3.9+ and an installed Node.js 20+ executable. Supported native
hosts are Apple Silicon/Intel macOS, Linux x64 with glibc, and Windows x64. The
outer bundle and Linux/Windows binaries are unsigned; macOS binaries retain their
published signatures. Read the [release notes](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v1.1.0).

1. Download [bundle 1.1.0](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v1.1.0/deadbolt-1.1.0.mcpb).
2. Download and review [editor_setup.py](https://seanebones-lang.github.io/deadbolt/assets/editor_setup.py),
   the dependency-free configuration helper. It is [source in this repository](assets/editor_setup.py).
   Save both files in your Downloads directory. It makes no network requests,
   installs no dependencies and never edits an editor profile.
3. Choose a **new private directory outside any workspace the agent can edit**.
   From Downloads on macOS/Linux:

   ```sh
   OUT="$HOME/.local/share/deadbolt/editor-1.1.0"
   python3 editor_setup.py prepare --bundle deadbolt-1.1.0.mcpb --out "$OUT"
   python3 editor_setup.py verify --out "$OUT"
   ```

   On Windows PowerShell:

   ```powershell
   $Out = Join-Path $env:LOCALAPPDATA 'NextEleven\DeadBolt\editor-1.1.0'
   python editor_setup.py prepare --bundle deadbolt-1.1.0.mcpb --out $Out
   python editor_setup.py verify --out $Out
   ```

The helper checks the entire bundle against the published SHA-256
`7840d23f116b8a162322a372124718ca374296e3b9ca1d781e99f466d7a6634c`
before extraction. A mismatch fails without writing an installation. It refuses
to overwrite an existing installation. Use `--node /absolute/path/to/node` on
`prepare` if Node is not on PATH. Keep the installation at that path: generated
configurations contain absolute paths. Moving it requires new configuration.

Expected verification: `"passed": true`, with allowed marker effects, off-list
denial, human-approval denial, one approved execution, approval consumption, and
kill denial. Denied calls must leave the marker file unchanged. This is a real
stdio protocol test of a disposable server, **not a test of your actual accounts
or a model's behavior**. Temporary verification state is removed afterward.

## Add the setup connection to Cursor

Open `install.html` from the private installation directory and click **Add
deadbolt-setup to Cursor**. Review the command and arguments before clicking
Install. The link is generated on your computer and contains only local runtime
configuration; it is not sent to a hosted operator service.

In current Cursor, open **Customize → MCPs**. The setup connection should show
two enabled tools: `deadbolt_get_integration_guide` and `deadbolt_check_bundle`.
Ask the agent to call the guide with `{"route":"stdio_client"}`, then call the
bundle check with `{}`. The guide remains available in the repository even if
you are not signed in to chat.

Cursor may store user MCP entries in `~/.cursor/mcp.json` even when launched with
a separate user-data directory. A fresh editor profile alone does not isolate
that MCP file. Back up and preserve existing entries when testing or removing a
connection.

If install links are disabled by your organization, merge the entry from
`cursor-mcp.json` into the appropriate Cursor MCP configuration. Preserve other
servers. Review [Cursor's official configuration and installation guidance](https://cursor.com/docs/mcp/install-links)
and your team's policies; a public registry listing does not mean a client has
automatically installed or approved the server.

## Add the setup connection to VS Code

Run **MCP: Open User Configuration** from the Command Palette. Merge the server
entry from `vscode-mcp.json` into its `servers` object, preserving existing entries
and any `inputs`. Save, start the server, and review the local-server trust prompt.

For a **new** workspace configuration, `portable-mcp.json` can be used as
`.mcp.json`; its top-level key is `mcpServers`. This file contains the runtime
command only. Keep operator state and private configuration outside the workspace.
Do not replace an existing configuration file wholesale.

VS Code also offers the `code --add-mcp` command. Pass the generated entry with
`"name":"deadbolt-setup"` to that command if you prefer CLI installation. GUI
controls and portable configuration availability depend on your client version.
See [VS Code's official MCP guide](https://code.visualstudio.com/docs/agent-customization/mcp-servers).

## Try a disposable protected route

Create a new run when ready to use it:

```sh
python3 editor_setup.py demo --out "$OUT"
```

The output gives a fresh `demo` directory. Set `DEMO` to that exact path (on
PowerShell, set `$Demo`). Open its `install.html` for Cursor, or merge its
`vscode-mcp.json` entry for VS Code. This adds **deadbolt-demo**, whose three tools
only append tool names to the fixed `effects.txt` file in the private demo
directory. No email, billing, filesystem selection, network destination or real
account is connected. One harmless marker tool is classified as irreversible
solely to demonstrate approval.

| Call | Expected outcome | Local effect |
| --- | --- | --- |
| `demo_write_marker({})` | Allowed | One marker line |
| `demo_off_list({})` | `purpose_exceeded` | No new line |
| `demo_publish_marker({})` | `needs_human` | No new line |

Approve from **your trusted operator terminal**, outside the agent:

```sh
python3 editor_setup.py operator --out "$OUT" --demo "$DEMO" approve
```

Call `demo_publish_marker({})` once: one line should be added. Call it again:
`needs_human` and no new line. Then revoke the run:

```sh
python3 editor_setup.py operator --out "$OUT" --demo "$DEMO" kill
```

The next `demo_write_marker({})` must return `killed` without writing a line.
An editor's Allow/Run button permits an attempted call; it does not grant
DeadBolt's operator approval. Inspect `effects.txt` and `events.jsonl` to check
actual outcomes. Some clients present a generic MCP error instead of the full
denial; local evidence carries the reason.

**The idle lease is 60 seconds.** Admission renews a live lease, but initialization
and listing tools do not. If setup or conversation takes longer, the call may
return `lease_expired`. Stop/remove the old demo connection and run `demo` again
for a fresh ID. Restarting a killed or expired ID cannot revive it. Do not reuse
one ID for concurrent sessions. The automated `verify` command avoids this
manual timing dependency and never modifies your editor demo's policy or state.

## Connect a real downstream server deliberately

Use a server you trust, with explicit tool names, destination conventions and
operator-owned state. Follow [the configuration guide](https://github.com/seanebones-lang/deadbolt/blob/mcpb-v1.1.0/dist/mcpb/README.md) and
[integration boundaries](INTEGRATION.md#bolt-on-stdio-mcp-proxy). Replace that
server's direct connection with the proxy connection; leaving an ungated direct
connection available lets the agent bypass this route.

DeadBolt does not sandbox the child, intercept the editor's built-in shell or file
tools, govern other MCP servers, inspect arbitrary traffic, or cancel an effect
already running. Initialization, resources and other non-tool MCP methods remain
outside this gate. Same-user malicious code can access same-user files even with
private directory permissions. Operator authority must remain outside workload
access in a trustworthy deployment.

Stable 1.0.3 approvals are broad one-shot approvals for an agent/tool pair, not
approvals bound to exact recipients or arguments. Use the separately documented
[prerelease exact-action API](ACTION-APPROVALS.md) to evaluate stronger action
binding; do not claim it is included in this bundle.

## Remove the evaluation

Remove **deadbolt-setup** and **deadbolt-demo** from your editor's MCP configuration,
preserving other entries, then stop their processes. Retain or delete your private
installation and demo evidence according to your needs. Removing configuration
does not erase state, evidence or terminal revocations. Keep a fresh operator ID
for any later evaluation.

## Validation coverage

On macOS ARM64, Cursor 3.21.16 accepted the generated install links and discovered
the two setup tools and three disposable demo tools. VS Code 1.141.0 accepted the
generated configuration through its CLI and discovered both sets.
[Client validation record](evidence/editor-client-macos-arm64-2026-10-11.json).
These GUI checks cover configuration, startup and discovery; effect assertions
run separately through real stdio. They do not establish model-mediated calls in
the graphical clients or your actual routes.

Next: [the practical walkthrough](EDITOR-TUTORIAL.md), [first evaluation](FIRST-EVALUATION.md),
and [troubleshooting](TROUBLESHOOTING.md).
