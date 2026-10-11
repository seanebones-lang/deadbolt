# DeadBolt local MCP bundle

This bundle includes the published **v1.0.3** DeadBolt executable and a dependency-free Node.js launcher. Bundle version 1.1.0 describes this packaging, not a new engine release. It does not include the v1.1.0-rc.1 exact-action or admission-credential features.

## Try read-only setup first

Install and enable the bundle with **Operator configuration left blank**. Ask your agent to call `deadbolt_get_integration_guide` with your route (`claude_desktop`, `stdio_client`, `rust` or `http`), or call `deadbolt_check_bundle` to verify the packaged engine checksum on this host. Both return structured results, output schemas and read-only annotations. No accounts, operator state or downstream server are accessed. This mode provides **no active protection**. Checksum verification detects corruption relative to packaged metadata; it is not a security audit or independent signature verification.

Supplying a configuration path switches to the actual protected downstream server: its generated tool list replaces these two setup tools. A missing, unreadable or malformed supplied file stops startup; it never silently falls back to setup. To return deliberately to setup, stop the extension, clear its setting and restart. Terminal/editor clients can run `node /absolute/bundle/server/launcher.cjs` without a configuration argument for setup, or pass the absolute configuration path for the gate.

## Configure the tool-call gate

1. Install the `.mcpb` in a client that supports local MCP bundles and Node.js 20+. The bundle contains macOS ARM64/Intel, Linux x64 and Windows x64 binaries. Linux requires a compatible glibc system; ARM Linux and ARM Windows are not included.
2. Create a private state directory outside the bundle and outside any workspace the agent can edit. On macOS/Linux, use `mkdir -m 700 /your/private/state` and keep your configuration file private. On Windows use an operator-owned directory with appropriate ACLs. Same-user malicious code can access these files; permissions do not turn the bundle into a sandbox.
3. Copy `operator-config.example.json` to your private configuration location. Replace every placeholder with an absolute path. On Windows use JSON escaped backslashes or forward slashes. Specify the executable and arguments for an MCP server you trust. No shell command string is evaluated.
4. Enter that configuration file's absolute path in the client's **Operator configuration** setting. Choose the server's actual tool names, destination tokens and irreversible tools deliberately. `tools` must be nonempty; an empty destination list permits no destination token, while calls without a discovered destination follow the engine's documented classification rules. `spend_cap_usd` is a cap on application-reported amounts, not an independent billing meter.
5. Save the configuration and enable the extension in your client. In Claude Desktop, check **Settings → Developer** for a **Running** status. The launcher creates a missing lease through its own verified `--version` command and sets your explicit policy **before starting your configured server**. It never grants approvals, resumes a pause or clears a kill. Setup errors exit before the configured server starts. Diagnostics go to stderr; stdout remains MCP-only.

The engine's default lease TTL is 60 seconds. Live admission renews it; silence expires it, and an expired lease cannot be renewed by admission or restarting this launcher. For a new run, stop the old session and choose a fresh operator-assigned agent ID. A killed ID stays killed. Do not reuse one ID for concurrent sessions: the launcher applies the configuration's policy to that ID at each startup. The example uses a $10 cap on reported spend; choose your own limit. A zero cap blocks all tool admission, including requests reporting no spend.

Bundle 1.0.1 supports ordinary Node and hosted runtimes that import the entry point and bridge message streams, including Claude Desktop's built-in Node. It fixes bundle 1.0.0's startup stall in that runtime. The trusted downstream server still needs its own executable/runtime; the host's embedded Node executable is not a general-purpose Node command.

If the client says the server failed to start, inspect its developer logs and verify the private configuration, existing state directory and downstream executable. If a connected tool returns a generic connector error, inspect the local evidence or `incident --agent ID` with the configured state paths to distinguish policy denial, expired lease and setup/transport failure. Claude Desktop's own **Allow once** permits an attempt; it does not grant DeadBolt operator approval. Do not retry denied effects or clear a kill to hide a failure.

## Operator controls

Use the binary under `native/<your-target>/deadbolt` (`deadbolt.exe` on Windows). Set `DEADBOLT_DB` and `DEADBOLT_EVENTS` to the **same absolute paths as your configuration file**. Then use `status --agent ID`, `pause --agent ID`, `clip TOOL --agent ID`, `approve --agent ID --tool TOOL`, `kill --agent ID`, or `incident --agent ID`.

Approvals in v1.0.3 are broad one-shot approvals for a classified tool, not approvals bound to exact recipients or arguments. Review the action in your trusted application. Operator CLI commands are not exposed to the agent as MCP tools. Keep the state, configuration and operator access under trusted control.

## Boundary

DeadBolt admits routed `tools/call` requests before they reach the child. Startup and non-tool methods (including resources and initialization) are forwarded outside that gate. The downstream server has its ordinary OS access and remains trusted. Destination discovery and reported spend are conventions described in `engine-docs/INTEGRATION.md`; custom network tools may need an executor adapter. Nothing here intercepts arbitrary network traffic, contains arbitrary code, cancels an already running tool body or makes an external action atomic with permission.

The launcher removes inherited `DEADBOLT_*` variables and sets only the configuration's state paths, so unrelated environment settings cannot select another store or inject proxy credentials. Other environment variables are inherited by the trusted server: supply only the credentials that server requires. Decision records remain local and operator-writable. This is not compliance certification.

`PROVENANCE.json` records the source revisions, published archive checksums and binary checksums. The launcher checks the selected binary against this packaged metadata before execution. This catches packaging corruption; it is not a signature against an attacker who can edit the whole bundle. macOS binaries retain their original Developer ID signatures; Linux and Windows releases are unsigned. The outer bundle is not signed.

In configured mode the bundle's tools come from your trusted downstream server. In blank-configuration mode only the two built-in read-only setup tools are exposed. No downstream server is bundled. Local acceptance tests use disposable fixture tools, not your accounts, files or credentials. See the distribution acceptance report for actual tested platforms; including a platform's binary does not prove this wrapper was tested on that platform.

Source: https://github.com/seanebones-lang/deadbolt

Captioned demonstration: https://huggingface.co/spaces/Nextleven/deadbolt
