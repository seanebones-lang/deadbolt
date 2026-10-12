# Install DeadBolt

This guide describes **engine 2.0.0**, **Python client 2.0.0**, and the matching
version 2 local bundle. Source builds use your reviewed checkout; select registry,
Homebrew or catalog versions only when that exact version is visible and its
checksums match. [Core releases](https://github.com/seanebones-lang/deadbolt/releases)
record publication, source identity and immutable download hashes. Package-manager
and directory availability are separate from a source merge.

Read [major-version migration](MIGRATING-V2.md) before upgrading an older store or
embedded Rust application. Earlier downloads remain immutable; use their tagged
documentation rather than mixing old binaries with current configuration.

| Route | Requirements | Result |
| --- | --- | --- |
| Native archive | Matching OS/CPU; Python 3.9+ for the evaluation | Prebuilt engine, examples, notices and source/hash identity |
| [Homebrew](HOMEBREW.md) | macOS Apple Silicon or Intel, Homebrew | Signed prebuilt CLI; no Rust build |
| Local MCP bundle | Compatible local client, Node.js 20+, trusted downstream server | Engine and configuration-file launcher; blank setup provides no protection |
| Cargo CLI / embedded Rust | Rust 1.85+, native C compiler/linker | Engine or embedded library |
| Python package | Python 3.9+, pip and a virtual environment | Dependency-free base client; native engine installed separately |
| Optional SDK adapter | Python 3.10+ and openai-agents extra | Tested SDK 0.23.1 adapter |
| [FastMCP recipe](FASTMCP.md) | Python 3.10+, FastMCP 4.1.0, matching client/engine | A gate inside each protected tool body |
| Source Node client | Node.js; version 22 tested | CommonJS source module; no npm package required |
| [Docker source build](DOCKER-MCP.md) | Docker Linux engine and reviewed source/configuration | Local stdio container; check that guide's explicit version and scope |

No model key, Harness, cloud account or database server is needed for the
provider-free local evaluation. Installing a client or binary does not start
protection or configure an agent's execution routes.

## Native archive (no Rust required)

Select the [2.0.0 release](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0)
and download your native ZIP and its matching `.zip.sha256` file. If the release
is not yet published, use a reviewed source build or an explicitly selected
candidate; never substitute another version's checksum.

| Computer | Asset suffix |
| --- | --- |
| Apple Silicon Mac | aarch64-apple-darwin.zip |
| Intel Mac | x86_64-apple-darwin.zip |
| Linux x86_64 with glibc | x86_64-unknown-linux-gnu.zip |
| Windows x86_64 | x86_64-pc-windows-msvc.zip |

On macOS, in the directory holding both matching downloads:

```sh
shasum -a 256 -c deadbolt-2.0.0-aarch64-apple-darwin.zip.sha256
unzip deadbolt-2.0.0-aarch64-apple-darwin.zip
cd deadbolt-2.0.0-aarch64-apple-darwin
chmod 0755 deadbolt
./deadbolt --version
./deadbolt drill
python3 examples/evaluate.py --binary ./deadbolt
```

Linux uses `sha256sum -c` and its matching filenames. On Windows use
`Get-FileHash .\ARCHIVE.zip -Algorithm SHA256`, compare with the checksum file,
then extract with `Expand-Archive`. In the extracted root:

```powershell
.\deadbolt.exe --version
.\deadbolt.exe drill
python examples/evaluate.py --binary .\deadbolt.exe
```

Expected: version `deadbolt 2.0.0`, `deadbolt drill ok`, and evaluation JSON with
`"passed": true`. Retain the folder's examples, notices and `BUILD.json`.
Verify signing/notarization status in the release notes; unsigned CI artifacts
are distinct from published signed Mac downloads. Linux/Windows executables are
unsigned. Linux arm64, Windows arm64 and Alpine/musl are outside this native matrix.

## Local MCP bundle (MCPB)

Use the [matching bundle release](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v2.0.0)
and its `SHA256SUMS`. The direct asset is
[deadbolt-2.0.0.mcpb](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v2.0.0/deadbolt-2.0.0.mcpb).
A bundle has its own version/hash; it is not interchangeable with a native ZIP.
Check publication before installation and never reuse a candidate's checksum.

1. Verify the downloaded bundle with its matching SHA-256.
2. Leave Operator configuration blank to get two read-only setup tools. This
   does not start a downstream server, connect accounts or provide protection.
3. To enable a protected route, create a private configuration outside the agent's
   editable workspace. Follow [version 2 configuration](../dist/mcpb/README.md):
   use absolute paths, a fresh operator-assigned run ID, a fixed future integer
   Unix deadline, reviewed tools/destinations and explicit offline exceptions.
4. Select activity renewal deliberately, or run a separate trusted supervisor
   heartbeat before idle expiry. The default idle TTL is 60 seconds. Neither
   mode extends the fixed deadline or revives killed/expired runs.
5. Install in your compatible client and select that configuration. Claude
   Desktop supports Settings → Extensions → Advanced settings → Install extension.
   Cursor/VS Code use the [version 2 editor guide](EDITORS-V2.md); verify the
   helper's advertised bundle version/hash before exporting configuration.
6. Test with disposable tools and state before attaching effects. An allowed
   call must create the expected effect; denied, killed, expired and unavailable
   cases must leave the body untouched.

Node.js 20+ is required. The trusted downstream server is not bundled and retains
its OS access. Startup and non-tool MCP traffic are outside the call gate. The
launcher provisions broad one-shot tool approvals, not exact-action grants.
Client approval does not grant DeadBolt operator approval. Use readable denial
codes and operator evidence to diagnose failures; do not retry terminal runs.

[Smithery](https://smithery.ai/servers/seanebones/deadbolt) and the
[official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.seanebones-lang%2Fdeadbolt&limit=10)
provide discovery. Check each entry's actual package version, URL and hash.
A directory listing is not a hosted operator endpoint or a certification.

## Cargo registry CLI and embedded Rust

Once 2.0.0 is visible on [crates.io](https://crates.io/crates/n11-deadbolt), install
with the reviewed package lockfile:

```sh
cargo install n11-deadbolt --version 2.0.0 --locked --bin deadbolt
deadbolt --version
deadbolt drill
```

For an embedded application:

```toml
[dependencies]
deadbolt = { package = "n11-deadbolt", version = "=2.0.0" }
```

The package is n11-deadbolt; the binary and library are deadbolt. Registry install
requires no publishing token. Commit your application's own Cargo.lock and use
`cargo build --locked`; a dependency's lockfile is not your application's lock.
[Build-in example](INTEGRATION.md#build-in-rust-executor).

## Published source

Use the release tag once published, or a reviewed exact source revision:

```sh
git clone --branch v2.0.0 --depth 1 https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
git rev-parse HEAD
cargo install --path . --locked --bin deadbolt
deadbolt --version
deadbolt drill
```

Compare the SHA with release notes and BUILD.json. macOS needs Command Line Tools;
Linux needs a C compiler/linker; Windows needs the MSVC toolchain, C++ Build Tools
and Windows SDK. Cargo installs into its bin directory; add it to PATH if needed.
Replacing an existing Cargo binary requires intentional `--force` after backup.

## Python and Node

When the matching 2.0.0 package is visible on PyPI, in your application's environment:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install 'n11-deadbolt-client==2.0.0'
python -c "import deadbolt_client; print(deadbolt_client.__file__)"
```

Windows activation is `.\.venv\Scripts\Activate.ps1`. The imported module
must be inside that environment. Alternatively, verify the release wheel's hash
and install `/absolute/path/to/n11_deadbolt_client-2.0.0-py3-none-any.whl`.
A reviewed source installation uses `python -m pip install /path/to/deadbolt`.
The wheel runs on Python 3.9+; the build backend requires Python 3.10+.
The optional SDK adapter needs Python 3.10+:

```sh
python -m pip install 'n11-deadbolt-client[openai-agents]==2.0.0'
```

The wheel does not include/start the engine. Configure the separately installed
sidecar and protect actual bodies as described in [Python setup](PYTHON.md).
Use `examples/deadbolt_client.js` from the matching selected source for Node.

## Homebrew on macOS

```sh
brew install --cask seanebones-lang/tap/n11-deadbolt
deadbolt --version
deadbolt drill
```

Check the [tap guide](HOMEBREW.md) and cask version before installing; promotion
is separate from GitHub publication. The preview cask remains a distinct channel
and cannot coexist with the stable cask. Installation does not start a service.

## Choose the transport and authority

Embedded Rust needs no sidecar. Version 2 sidecars require operator authentication
for both the same-user mode-0600 Unix socket and loopback TCP. Windows uses TCP.
Use [HTTP authentication](HTTP-AUTHENTICATION.md), configure policy before dispatch,
and give the dispatcher only its [admission credential](CREDENTIALS.md).
Keep operator/supervisor authority outside model-facing processes. Public binds
are refused. Same-user state write access grants file authority, not isolation.

`ensure` confirms record presence, not a live permission grant. Use status for
diagnostics and each dispatch's fresh admission decision before any effect.
Destination declarations are policy, not OS network enforcement.

## Upgrade and remove

Stop protected effects/services, back up state consistently and record source/hash
identity before replacing a binary. Follow [migration](MIGRATING-V2.md),
[operations](OPERATIONS.md) and [compatibility](COMPATIBILITY.md). Run the drill and
your application's own effect tests after upgrading. Older binaries cannot enforce
new credential and exact-action requirements.

`cargo uninstall n11-deadbolt`, `python -m pip uninstall n11-deadbolt-client`, or
`brew uninstall --cask n11-deadbolt` removes the relevant installation. Native
archives are removed by deleting the executable you installed. These commands
leave private state, revocations and evidence intact; preserve or remove those
under your retention policy. [Troubleshooting](TROUBLESHOOTING.md).
