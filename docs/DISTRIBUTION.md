# Which DeadBolt version am I installing?

Audit started November 3, 2026; distribution follow-up completed November 4.
The current released engine is **2.0.1**. Python client **2.0.1.post1** is a documentation-only postrelease
with unchanged runtime modules. The matching desktop MCP bundle is **2.0.1**. These are separate
installations; replacing one does not update a running sidecar or a vendored
editor plugin. Earlier downloads remain immutable.

## Verified public routes

| Route | Observed published version | What it installs / next action |
| --- | --- | --- |
| [Native GitHub release](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.1) | 2.0.1 | Engine plus examples. Select your OS/CPU, verify its checksum and run the drill/evaluation. |
| [Cargo](https://crates.io/crates/n11-deadbolt/2.0.1) | 2.0.1, not yanked | `cargo install n11-deadbolt --version 2.0.1 --locked --bin deadbolt`. |
| [PyPI](https://pypi.org/project/n11-deadbolt-client/2.0.1.post1/) | 2.0.1.post1 | Client only: `python -m pip install 'n11-deadbolt-client==2.0.1.post1'`. Corrected documentation, same runtime modules as 2.0.1; engine remains 2.0.1. Original package bytes remain immutable. |
| [Homebrew tap](https://github.com/seanebones-lang/homebrew-tap) | Stable cask 2.0.1 | Signed Mac engine; preview cask is a separate historical channel. [Instructions](HOMEBREW.md). |
| [Direct MCPB](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v2.0.1) | 2.0.1 | Four-platform bundled engine/launcher; verify SHA-256 `709bd27afe9c1ce8a833d124eceb2e6ca6458a40924534a34882b253fe4ae644`. Blank configuration gives read-only setup, not active protection. |
| [Claude plugin](https://github.com/seanebones-lang/deadbolt-claude-plugin) | Main packages 2.0.1 | Whole-bundle replacement passed exported-command and isolated CLI-install checks plus the four-platform PR matrix before merge. Verify the actual `BUNDLE-SOURCE.json` and refresh your installed copy. |
| [Cursor plugin](https://github.com/seanebones-lang/deadbolt-cursor-plugin) | Main packages 2.0.1 | Whole-bundle replacement passed exported-command checks and the four-platform PR matrix before merge. Refreshing the standalone CLI does not update the plugin. |
| [Docker wrapper](https://github.com/seanebones-lang/deadbolt-docker-mcp) | Main builds engine 2.0.1 | Exact source and supervisor-wheel pins retain the patched Node base. ARM64/AMD64 container, supervisor and gateway acceptance passed before merge. Rebuild old images; the historical `v2.0.0` tag predates these updates. No public prebuilt image is established here. |
| [Hermes plugin](https://github.com/seanebones-lang/deadbolt-hermes-plugin) | Plugin 0.1.1 recommends engine 2.0.1 | Bundles no engine; upgrading the sidecar is an operator action. Sixteen real framework scenarios, four unit tests and fifteen catalog checks passed locally; macOS/Linux PR acceptance also passed before merge. |
| [Official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.seanebones-lang%2Fdeadbolt&limit=30) | Latest entry 2.0.1 | Published entry and exact bundle SHA-256 verified through the public API after publication. |

[Harness's source integration](https://github.com/seanebones-lang/harness/blob/main/docs/DEADBOLT.md)
also pins the released 2.0.1 engine. Its migration passed 203 focused tests,
CLI compilation, strict focused Clippy, independent code review and the full
PR check set before merge. Previously downloaded Harness binaries remain their
own versions; a source merge does not replace them. The adapter's documented
activity-renewal and first-declared-destination limits remain explicit.

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
