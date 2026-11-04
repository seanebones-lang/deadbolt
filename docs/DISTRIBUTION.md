# Which DeadBolt version am I installing?

Audit date: November 3, 2026. The current released engine and Python client are
**2.0.1**. The matching desktop MCP bundle is **2.0.1**. These are separate
installations; replacing one does not update a running sidecar or a vendored
editor plugin. Earlier downloads remain immutable.

## Verified public routes

| Route | Observed published version | What it installs / next action |
| --- | --- | --- |
| [Native GitHub release](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.1) | 2.0.1 | Engine plus examples. Select your OS/CPU, verify its checksum and run the drill/evaluation. |
| [Cargo](https://crates.io/crates/n11-deadbolt/2.0.1) | 2.0.1, not yanked | `cargo install n11-deadbolt --version 2.0.1 --locked --bin deadbolt`. |
| [PyPI](https://pypi.org/project/n11-deadbolt-client/2.0.1/) | 2.0.1 | Client only: `python -m pip install 'n11-deadbolt-client==2.0.1'`. Its frozen long description still shows 2.0.0 commands; use the explicit command here. |
| [Homebrew tap](https://github.com/seanebones-lang/homebrew-tap) | Stable cask 2.0.1 | Signed Mac engine; preview cask is a separate historical channel. [Instructions](HOMEBREW.md). |
| [Direct MCPB](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v2.0.1) | 2.0.1 | Four-platform bundled engine/launcher; verify SHA-256 `709bd27afe9c1ce8a833d124eceb2e6ca6458a40924534a34882b253fe4ae644`. Blank configuration gives read-only setup, not active protection. |
| [Claude plugin](https://github.com/seanebones-lang/deadbolt-claude-plugin) | Main was pinned to 2.0.0 at audit | A whole-bundle 2.0.1 replacement passed local exported-command and isolated CLI-install checks. It remains a proposed change until merged; verify the repository's actual `BUNDLE-SOURCE.json`. |
| [Cursor plugin](https://github.com/seanebones-lang/deadbolt-cursor-plugin) | Main was pinned to 2.0.0 at audit | A whole-bundle 2.0.1 replacement passed local exported-command checks. It remains a proposed change until merged; refreshing the standalone CLI does not update the plugin. |
| [Docker wrapper](https://github.com/seanebones-lang/deadbolt-docker-mcp) | Main builds engine 2.0.0 | Proposed source update pins engine 2.0.1 and retains the newer Node base. Container acceptance must pass before promotion. Its old `v2.0.0` tag also predates the Node base update on main. No public prebuilt image is established here. |
| [Hermes plugin](https://github.com/seanebones-lang/deadbolt-hermes-plugin) | README and integration evidence use engine 2.0.0 | Bundles no engine; upgrading the sidecar is an operator action. Re-run its real framework acceptance with 2.0.1 before claiming updated compatibility. |
| [Official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.seanebones-lang%2Fdeadbolt&limit=30) | Latest entry still 2.0.0 at audit | Prefer the verified direct 2.0.1 bundle for a new install. Prepared source metadata is not a published registry update. |

The Smithery discovery URL linked in older documentation returned HTTP 404 during
this audit; it is not a verified current install route. Glama's indexed overview
still displayed 2.0.0. Directory caches, search results and repository source do
not establish the version a downloader receives. No directory acceptance or
automatic update of existing installations is claimed.

## Upgrade one route deliberately

1. Identify the actual engine with `command -v deadbolt` (Windows: `where.exe
   deadbolt`) and `deadbolt --version`. For Python, run
   `python -m pip show n11-deadbolt-client` inside the executor's environment.
   For a bundled plugin, use a separate read-only setup connection's
   `deadbolt_check_bundle`; inspect both engine and bundle version/checksum.
2. Stop protected dispatch and the sidecar deliberately. Back up operator-owned
   state, evidence and configuration consistently. Follow [migration](MIGRATING-V2.md).
3. Upgrade the selected engine, client and/or plugin from its verified route.
   Do not overwrite unrelated installations or assume a source pull replaces a
   binary. Preserve private configuration, credentials and saved state.
4. Restart/reload the intended connection. Verify its actual executable/version,
   run `deadbolt drill` and the matching evaluation, then test one real protected
   route with disposable effects before resuming work.

The local Mac native release passed signature verification, its drill and all
11 demonstration effect checks. The source's main CI was green at
`04c1caaa7d8cf34fc41abe654ba93437229e46f3`. These are scoped checks, not a
whole-host vulnerability audit or proof that every installed consumer is current.
