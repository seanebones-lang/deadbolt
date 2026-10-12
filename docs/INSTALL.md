# Install DeadBolt

## Published 2.0 developer candidate

Engine **2.0.0-rc.1** and matching Python client **2.0.0rc1** are available in
[the versioned GitHub release](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0-rc.1). Download the native ZIP and its
checksum for your target, or select the matching Cargo, PyPI and Homebrew preview
packages below. The immutable Git tag remains an alternative Rust source build.
Read [the migration guide](MIGRATING-V2.md) before opening existing state or
changing an embedded Rust consumer. The final Mac ZIPs are signed/notarized;
Linux and Windows binaries are unsigned. See the release’s acceptance receipt
for the tested targets and remaining installation boundaries.

For the Rust CLI:

```sh
cargo install n11-deadbolt --version 2.0.0-rc.1 --locked --bin deadbolt
```

For an embedded Rust library:

```toml
deadbolt = { package = "n11-deadbolt", version = "=2.0.0-rc.1" }
```

For Python, install the pinned [PyPI client](https://pypi.org/project/n11-deadbolt-client/2.0.0rc1/)
in your application's virtual environment. Install the matching native engine separately:

```sh
python -m pip install 'n11-deadbolt-client==2.0.0rc1'
```

On macOS, the signed preview engine is also available without a Rust build:

```sh
brew install --cask seanebones-lang/tap/n11-deadbolt-preview
```

The stable commands below deliberately retain 1.0.3. The separately published
[MCPB 2.0 preview](#mcpb-20-preview) packages engine 2.0 with new operator
configuration; stable MCPB/editor artifacts retain their older pinned engines.

Choose a published native executable for a quick evaluation, or a reviewed source
candidate for the new APIs. The Python client connects to a **separately installed
executable**; installing the client does not start a sidecar.

| Path | You need | What you get |
| --- | --- | --- |
| Native archive | Matching OS/CPU; Python 3.9+ only for the evaluation | Stable v1.0.3, older 1.1.0-rc.1 or new 2.0.0-rc.1 executable, source clients, examples and docs |
| [Homebrew tap](HOMEBREW.md) | macOS Apple Silicon or Intel and Homebrew | Signed prebuilt stable CLI or explicitly selected preview; no Rust build |
| Local MCP bundle (MCPB) | Compatible local MCP client, Node.js 20+ and a trusted downstream stdio server | Bundle 1.1.0 with stable v1.0.3 binaries and a configuration-file launcher |
| [Docker stdio MCP](DOCKER-MCP.md) | Git and a running Docker Linux engine, ARM64 or AMD64 | Source-built package 1.0.0 with stable engine 1.0.3; catalog review pending |
| Cargo registry CLI / embedded Rust | Rust 1.85+, native C compiler/linker for bundled SQLite | Stable 1.0.3 or explicitly selected 2.0.0-rc.1 |
| Source binary / embedded Rust | Git, Rust 1.85+, native C compiler/linker for bundled SQLite | APIs at your selected tag or exact commit |
| Python source installation | Python 3.10+, pip and a reviewed checkout | Base client with no third-party runtime dependencies |
| Python package | Python 3.9+ and pip; pinned PyPI wheel or reviewed `.whl` | Prerelease client; engine installed separately; no Rust needed for the wheel |
| Optional SDK adapter | Python 3.10+ and the `openai-agents` extra | Pinned tested SDK 0.23.1 integration |
| [FastMCP source recipe](FASTMCP.md) | Python 3.10+, FastMCP 4.1.0, a matching 1.1 or 2.0 client/engine pair | A check inside each protected MCP tool; offline stdio demo |
| Bundled Node client | Node (22 tested) | CommonJS source module; no npm installation |

No model key, Harness, cloud account or database server is needed for setup or
local demonstrations. Docker is optional. Build tools are unnecessary for a native archive.

## Homebrew on macOS

```sh
brew install --cask seanebones-lang/tap/n11-deadbolt
deadbolt --version
deadbolt drill
```

Expected version: `deadbolt 1.0.3`. The separate `n11-deadbolt-preview` cask
selects `2.0.0-rc.1`; both casks cannot be installed together. Installation
does not start a service or configure policy. See [the Homebrew guide](HOMEBREW.md)
for scoped trust, the installed release's disposable evaluation, updates,
channel switching and state-preserving removal.

## Native archive (no Rust required)

Choose [stable v1.0.3](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.3)
or [new developer candidate v2.0.0-rc.1](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0-rc.1) or [older prerelease v1.1.0-rc.1](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.1.0-rc.1).
The prerelease includes callback dispatch, scoped credentials and exact-action review;
use it to evaluate those new APIs before stable promotion. Download the ZIP **and
its matching `.zip.sha256` file**. Use the same version throughout these commands:

| Computer | Asset suffix |
| --- | --- |
| Apple Silicon Mac | `aarch64-apple-darwin.zip` |
| Intel Mac | `x86_64-apple-darwin.zip` |
| Linux x86_64 with glibc | `x86_64-unknown-linux-gnu.zip` |
| Windows x86_64 | `x86_64-pc-windows-msvc.zip` |

For example, on macOS in the directory holding both downloads:

```sh
shasum -a 256 -c deadbolt-1.0.3-aarch64-apple-darwin.zip.sha256
unzip deadbolt-1.0.3-aarch64-apple-darwin.zip
cd deadbolt-1.0.3-aarch64-apple-darwin
chmod 0755 deadbolt
./deadbolt --version
./deadbolt drill
python3 examples/evaluate.py --binary ./deadbolt
```

Linux uses `sha256sum -c` and its matching filenames. Windows uses
`Get-FileHash .\ARCHIVE.zip -Algorithm SHA256`; compare the result with the
checksum file before extracting with `Expand-Archive`. From the extracted root:

```powershell
.\deadbolt.exe --version
.\deadbolt.exe drill
python examples/evaluate.py --binary .\deadbolt.exe
```

Expected: the checksum matches, version matches your selected release (`deadbolt 1.0.3`, `deadbolt 1.1.0-rc.1` or `deadbolt 2.0.0-rc.1`), the drill reports
`deadbolt drill ok`, and evaluation JSON has `"passed": true`. Keep this extracted
folder to retain its examples, notices and source identity in `BUILD.json`.
Add it to PATH or copy the executable to your chosen binary directory if desired;
the examples remain in the extracted folder.

Published v1.0.3 Mac archives are Developer ID signed and Apple-notarized;
Linux/Windows binaries are unsigned. Read the selected release's notes and your
OS download policy. CI artifacts are unsigned candidates, not those published
Mac downloads. See [release verification](RELEASING.md#install-an-archive).
Linux arm64, Windows arm64 and Alpine/musl are outside the native asset matrix.

## MCPB 2.0 preview

[Download the 2.0.0-rc.1 preview bundle](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v2.0.0-rc.1/deadbolt-2.0.0-rc.1.mcpb), or inspect its [release, checksums and four-host acceptance reports](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v2.0.0-rc.1). Bundle SHA-256:

```text
ee984aaead77f4cc5df4f586c0c4709f967fcf8de667bdcecb253cad743de1e7
```

This is a separate preview, not a silent update of stable bundle 1.1.0. Read the [version 2 configuration and migration guide](../dist/mcpb/README.md), use a fresh run ID, set a fixed future integer Unix deadline, choose activity or separately operated supervisor renewal, and review offline exceptions (default empty). Old configuration is refused. Restart cannot extend a deadline or revive expired/killed state. Destination declarations are policy, not OS egress enforcement; this gate does not cancel already running effects. Its irreversible tool approvals remain broad one-shot approvals; the launcher does not provision exact-action grants.

Node.js 20+ and the same four native targets are required. Blank configuration provides only read-only setup and no protection. Four-host tests exercised actual disposable allow/deny effects and startup/restart refusal; graphical client installation of this preview and independent outside-adopter reproduction remain open. The stable instructions below use their own versioned configuration.

## Local MCP bundle (MCPB)

For VS Code and Cursor, use [the editor installation kit](EDITORS.md): verify and
unpack the published bundle, export configuration without changing profiles,
and test disposable effects before connecting a real server.

[Download bundle 1.1.0 directly from GitHub](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v1.1.0/deadbolt-1.1.0.mcpb)
without signing in. [Release notes and SHA256SUMS](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v1.1.0)
are published alongside the bundle. For catalog discovery, see
[Smithery](https://smithery.ai/servers/seanebones/deadbolt) and listed in the
[official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.seanebones-lang%2Fdeadbolt&limit=10)
as `io.github.seanebones-lang/deadbolt`. Catalogs now advertise the separate 2.0 preview; use the direct stable download above when selecting 1.1.0.
Clients download it and run it on their own computer; these listings are not
public DeadBolt operator APIs. Your selected client may require its own account.
DeadBolt itself requires no model key or hosted service for local operation.

Bundle 1.1.0 adds optional read-only setup: leave Operator configuration blank for integration guidance and bundled-engine checksum checks. This does not start a downstream server or provide protection. Enter an explicit private configuration path to enable the gate; invalid supplied configuration stops startup. Configured tools replace setup tools.

Bundle **1.1.0** contains **engine v1.0.3**. It does not include the prerelease's
exact-action approvals or admission credentials. Supported binaries are macOS
ARM64/Intel, Linux x64 with glibc and Windows x64. Node.js 20+ is required; clients
such as Claude Desktop can supply a built-in Node runtime. Other clients may need
an installed runtime. Linux/Windows executables and the outer bundle are unsigned;
the macOS executables retain the published release's signatures.

1. [Download `deadbolt-1.1.0.mcpb`](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v1.1.0/deadbolt-1.1.0.mcpb).
   Verify the matching bundle checksum before installation. For bundle
   **1.1.0**, the SHA-256 is:

   ```text
   7840d23f116b8a162322a372124718ca374296e3b9ca1d781e99f466d7a6634c
   ```

   On macOS use `shasum -a 256 deadbolt-1.1.0.mcpb`; Linux uses `sha256sum`;
   PowerShell uses `Get-FileHash .\deadbolt-1.1.0.mcpb -Algorithm SHA256`.
   A future bundle version needs its own verified checksum.
2. Create a private state directory outside the agent's editable workspace.
   Copy [the example configuration](https://github.com/seanebones-lang/deadbolt/blob/mcpb-v1.1.0/dist/mcpb/operator-config.example.json)
   into a private JSON file and replace every placeholder with an absolute path.
   Select an MCP server you trust, its executable and arguments, actual tool names,
   destination tokens, irreversible-tool classifications and reported-spend cap.
   Read [configuration and operator controls](https://github.com/seanebones-lang/deadbolt/blob/mcpb-v1.1.0/dist/mcpb/README.md) before connecting
   a real account. No downstream server is bundled. With Operator configuration left blank, the two read-only setup tools can explain integration and verify packaged-engine integrity; they provide no active protection.
3. Install the `.mcpb` in your client. For Claude Desktop, use **Settings →
   Extensions → Advanced settings → Install extension**, then select the file
   and review the access notice. See
   [Claude's official installation guide](https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop)
   for client-specific controls and organization policies.
4. Select the private JSON file for **Operator configuration** when configuring
   the extension, save, and enable it. In Claude Desktop, check **Settings →
   Developer** for **Running** and the extension settings for your downstream tools.
   Setup failures exit before your configured server starts.
   First test with disposable tools and state: confirm an allowed call runs and
   a denied call leaves the tool body untouched.

The default lease expires after 60 seconds without admission. Admission renews a
live lease; initialization and listing tools do not. An idle or killed run ID
cannot be revived by restarting the extension. Stop the old session and choose a
fresh operator-assigned ID for a new run. Do not reuse one ID for concurrent sessions.
Keep operator controls outside the agent; v1.0.3 approvals are broad one-shot
approvals for a classified tool, not exact-argument approvals.

The bundle gates routed `tools/call` requests. The child remains trusted and retains
its OS permissions; startup and non-tool MCP methods are outside the gate. An
extension access notice describes the host's OS access, not a DeadBolt sandbox.
State and configuration ownership are part of your trusted integration.

Four-platform installed-bundle acceptance is recorded in the bundle's
`VALIDATION.md` and `validation/` reports. These exercise real stdio against
disposable tool effects; they do not establish a particular graphical client's
installation or your application's routes. Bundle 1.1.0 graphical testing on
macOS ARM64 checked both read-only setup tools, checksum verification and
switching to a disposable operator configuration. The same runtime files were
tested before adding the final evidence reports. Bundle 1.0.1 was also installed and
exercised in Claude Desktop 2.19675.1 on macOS ARM64 with built-in Node 24.21.0:
an allowed dummy call ran, while off-list and approval-required calls left their
bodies untouched. Other graphical clients have not been tested. Claude may show
a generic connector error for a denial; use local evidence or the operator's
`incident` command to inspect the reason. Host approval does not grant DeadBolt
operator approval. `PROVENANCE.json` records release
binary checksums and the packaging revision. Hashes identify the artifact; the
outer bundle has no publisher signature.

## Cargo registry CLI and embedded Rust

The [registry package](https://crates.io/crates/n11-deadbolt) is `n11-deadbolt`;
the executable and Rust library are both named `deadbolt`. You need no registry
account or publishing token to install it.

Install the stable CLI with its reviewed dependency lockfile:

```sh
cargo install n11-deadbolt --version 1.0.3 --locked --bin deadbolt
deadbolt --version
deadbolt drill
```

Expected version: `deadbolt 1.0.3`; expected drill: `deadbolt drill ok`.
To embed the library, add this to your application's manifest:

```toml
[dependencies]
deadbolt = { package = "n11-deadbolt", version = "=1.0.3" }
```

This dependency alias lets examples continue to use `use deadbolt::…`.
Commit your application's own `Cargo.lock` and build it with `cargo build --locked`.
Cargo does not use a dependency's lockfile when resolving your application.
See the [dispatch-boundary example](INTEGRATION.md#build-in-rust-executor).
[Stable Rust API reference](https://docs.rs/n11-deadbolt/1.0.3/deadbolt/).

For candidate evaluation, select it explicitly:

```sh
cargo install n11-deadbolt --version 2.0.0-rc.1 --locked --bin deadbolt
```

Use `version = "=2.0.0-rc.1"` in the embedded dependency for callback helpers,
scoped credentials, exact-action approvals and the 2.0 deadline/supervision controls.
These remain prerelease APIs. Review [major-version migration](MIGRATING-V2.md) first.
[Candidate Rust API reference](https://docs.rs/n11-deadbolt/2.0.0-rc.1/deadbolt/).
Replacing an installed CLI requires an intentional `--force`; review
[upgrade preparation](#upgrade-and-remove) first.

The registry versions come from their existing reviewed Git tags. Their
packaged READMEs are frozen release snapshots and may describe registry
publication as pending on the original release date. This guide tracks current
installation routes. The prerelease Python client is on PyPI; Node is not on npm.

## Published source

To build the published version instead of downloading an executable:

```sh
git clone --branch v1.0.3 --depth 1 https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo install --path . --locked --bin deadbolt
deadbolt --version
deadbolt drill
```

On macOS install Apple's Command Line Tools; Linux needs its distribution's C
compiler/linker; Windows needs Rust's MSVC toolchain, C++ Build Tools and Windows
SDK. Cargo normally installs into `~/.cargo/bin` or `%USERPROFILE%\.cargo\bin`.
If `deadbolt` is not found, call that executable by its full path or fix PATH.

## Reviewed source candidate

The `2.0.0-rc.1` APIs are not in v1.0.3. Select the versioned prerelease tag:

```sh
git clone --branch v2.0.0-rc.1 --depth 1 https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo install --path . --locked --bin deadbolt
git rev-parse HEAD
deadbolt --version
deadbolt drill
```

Record the SHA and compare it with the release notes and `BUILD.json` when using
an archive. Replacing an existing Cargo-installed binary requires an intentional
`--force`; follow [upgrade preparation](#upgrade-and-remove) first. Direct Git install:

```sh
cargo install --git https://github.com/seanebones-lang/deadbolt.git --tag v2.0.0-rc.1 --locked --bin deadbolt
```

The Rust library and CLI are also available through the [Cargo registry route](#cargo-registry-cli-and-embedded-rust).
The prerelease Python client is also on [PyPI](https://pypi.org/project/n11-deadbolt-client/2.0.0rc1/).
Pin `n11-deadbolt-client==2.0.0rc1`; install the matching engine separately.

## Python and Node

In your application, create and activate a virtual environment. On macOS/Linux:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install 'n11-deadbolt-client==2.0.0rc1'
python -c "import deadbolt_client; print(deadbolt_client.__file__)"
```

Windows: `python -m venv .venv`, then `.\.venv\Scripts\Activate.ps1` and the same
pip/import commands. To install reviewed source instead, use
`python -m pip install /absolute/path/to/reviewed/deadbolt` with Python 3.10+
for its build backend. The PyPI wheel runs on Python 3.9+. For a manual wheel, download
`n11_deadbolt_client-2.0.0rc1-py3-none-any.whl` and `SHA256SUMS` from the prerelease,
verify its hash, then run `python -m pip install /absolute/path/to/n11_deadbolt_client-2.0.0rc1-py3-none-any.whl`. The imported path should be inside your environment.

For SDK function tools, install the extra instead:

```sh
python -m pip install 'n11-deadbolt-client[openai-agents]==2.0.0rc1'
```

See [Python setup](PYTHON.md) for the operator/dispatcher split and optional
provider-free SDK demo. The wheel does not contain the native executable.
For stable v1.0.3 Python or Node, copy `examples/deadbolt_client.py` or
`examples/deadbolt_client.js` beside your application; Node remains a source client
in the candidate too. Use files from the same selected revision as your executable.

## Choose the transport

Embedded Rust needs no sidecar or token. A sidecar defaults to the same-user,
mode-0600 Unix socket `~/.deadbolt/deadbolt.sock` on macOS/Linux. Windows uses
loopback TCP and requires a nonempty operator token. Unreleased source requires
that token for Unix sidecars as well; [HTTP authentication](HTTP-AUTHENTICATION.md)
explains the version boundary. TCP is also available on Unix.

For a disposable evaluation, the script configures all of this itself. For your
application, follow [sidecar setup](INTEGRATION.md#bolt-on-python-node-or-any-http-client), configure
policy before dispatch, and provision [admission-only credentials](CREDENTIALS.md)
when using the candidate. Keep operator authority outside workload access.
Public network binds are refused.

## Upgrade and remove

Stop admissions and the service, preserve the state, and record the source SHA
before replacing a binary. Run the drill and your application's acceptance checks
afterward. See [operations](OPERATIONS.md) and [compatibility](COMPATIBILITY.md):
older binaries cannot enforce new credential/exact-action requirements.

`cargo uninstall n11-deadbolt` removes a Cargo-installed executable;
`python -m pip uninstall n11-deadbolt-client` removes the client in that environment.
Archive installation is removed by deleting the executable you installed.
For Homebrew, use `brew uninstall --cask n11-deadbolt` (or the preview cask
name); follow [the package lifecycle guide](HOMEBREW.md#update-switch-channels-or-remove).
These steps leave databases, revocations and evidence in place. Preserve them
according to your retention policy.

Next: [quick start](QUICKSTART.md), [integration](INTEGRATION.md), or
[troubleshooting](TROUBLESHOOTING.md).
