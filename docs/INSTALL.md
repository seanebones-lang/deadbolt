# Install DeadBolt

Choose a published native executable for a quick evaluation, or a reviewed source
candidate for the new APIs. The Python client connects to a **separately installed
executable**; installing the client does not start a sidecar.

| Path | You need | What you get |
| --- | --- | --- |
| Native archive | Matching OS/CPU; Python 3.9+ only for the evaluation | Stable v1.0.3 or prerelease 1.1.0-rc.1 executable, source clients, examples and docs |
| Local MCP bundle (MCPB) | Compatible local MCP client, Node.js 20+ and a trusted downstream stdio server | Bundle 1.1.0 with stable v1.0.3 binaries and a configuration-file launcher |
| Cargo registry CLI / embedded Rust | Rust 1.85+, native C compiler/linker for bundled SQLite | Stable 1.0.3 or explicitly selected 1.1.0-rc.1 |
| Source binary / embedded Rust | Git, Rust 1.85+, native C compiler/linker for bundled SQLite | APIs at your selected tag or exact commit |
| Python source installation | Python 3.10+, pip and a reviewed checkout | Base client with no third-party runtime dependencies |
| Python wheel | Python 3.9+, pip and a reviewed `.whl` | Same base client; no Rust needed for the wheel |
| Optional SDK adapter | Python 3.10+ and the `openai-agents` extra | Pinned tested SDK 0.23.1 integration |
| Bundled Node client | Node (22 tested) | CommonJS source module; no npm installation |

No model key, Harness, cloud account or database server is needed for setup or
local demonstrations. Docker is optional. Build tools are unnecessary for a native archive.

## Native archive (no Rust required)

Choose [stable v1.0.3](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.3)
or [developer prerelease v1.1.0-rc.1](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.1.0-rc.1).
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

Expected: the checksum matches, version matches your selected release (`deadbolt 1.0.3` or `deadbolt 1.1.0-rc.1`), the drill reports
`deadbolt drill ok`, and evaluation JSON has `"passed": true`. Keep this extracted
folder to retain its examples, notices and source identity in `BUILD.json`.
Add it to PATH or copy the executable to your chosen binary directory if desired;
the examples remain in the extracted folder.

Published v1.0.3 Mac archives are Developer ID signed and Apple-notarized;
Linux/Windows binaries are unsigned. Read the selected release's notes and your
OS download policy. CI artifacts are unsigned candidates, not those published
Mac downloads. See [release verification](RELEASING.md#install-an-archive).
Linux arm64, Windows arm64 and Alpine/musl are outside the native asset matrix.

## Local MCP bundle (MCPB)

For VS Code and Cursor, use [the editor installation kit](EDITORS.md): verify and
unpack the published bundle, export configuration without changing profiles,
and test disposable effects before connecting a real server.

[Download bundle 1.1.0 directly from GitHub](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v1.1.0/deadbolt-1.1.0.mcpb)
without signing in. [Release notes and SHA256SUMS](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v1.1.0)
are published alongside the bundle. It is also available through
[Smithery](https://smithery.ai/servers/seanebones/deadbolt) and listed in the
[official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.seanebones-lang%2Fdeadbolt&limit=10)
as `io.github.seanebones-lang/deadbolt`.
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
   Copy [the example configuration](../dist/mcpb/operator-config.example.json)
   into a private JSON file and replace every placeholder with an absolute path.
   Select an MCP server you trust, its executable and arguments, actual tool names,
   destination tokens, irreversible-tool classifications and reported-spend cap.
   Read [configuration and operator controls](../dist/mcpb/README.md) before connecting
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
cargo install n11-deadbolt --version 1.1.0-rc.1 --locked --bin deadbolt
```

Use `version = "=1.1.0-rc.1"` in the embedded dependency for callback helpers,
scoped credentials and exact-action approvals. These remain prerelease APIs.
[Candidate Rust API reference](https://docs.rs/n11-deadbolt/1.1.0-rc.1/deadbolt/).
Replacing an installed CLI requires an intentional `--force`; review
[upgrade preparation](#upgrade-and-remove) first.

Both registry versions come from their existing reviewed Git tags. Their
packaged READMEs are frozen release snapshots and may describe registry
publication as pending on the original release date. This guide tracks current
installation routes. The Python client is not on PyPI; Node is not on npm.

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

The `1.1.0-rc.1` APIs are not in v1.0.3. Select the versioned prerelease tag:

```sh
git clone --branch v1.1.0-rc.1 --depth 1 https://github.com/seanebones-lang/deadbolt.git
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
cargo install --git https://github.com/seanebones-lang/deadbolt.git --tag v1.1.0-rc.1 --locked --bin deadbolt
```

The Rust library and CLI are also available through the [Cargo registry route](#cargo-registry-cli-and-embedded-rust).
The Python client is not on PyPI. Use its reviewed source or downloadable wheel
instead of `pip install n11-deadbolt-client`.

## Python and Node

In your application, create and activate a virtual environment. On macOS/Linux:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install /absolute/path/to/reviewed/deadbolt
python -c "import deadbolt_client; print(deadbolt_client.__file__)"
```

Windows: `python -m venv .venv`, then `.\.venv\Scripts\Activate.ps1` and the same
pip/import commands with a real Windows path. Source installation needs Python
3.10+ for its build backend. For Python 3.9, install the release wheel by its absolute filename. Download
`n11_deadbolt_client-1.1.0rc1-py3-none-any.whl` and `SHA256SUMS` from the prerelease,
verify its hash, then run `python -m pip install /absolute/path/to/n11_deadbolt_client-1.1.0rc1-py3-none-any.whl`. The imported path should be inside your environment.

For SDK function tools, install the extra instead:

```sh
python -m pip install '/absolute/path/to/reviewed/deadbolt[openai-agents]'
```

See [Python setup](PYTHON.md) for the operator/dispatcher split and optional
provider-free SDK demo. The wheel does not contain the native executable.
For stable v1.0.3 Python or Node, copy `examples/deadbolt_client.py` or
`examples/deadbolt_client.js` beside your application; Node remains a source client
in the candidate too. Use files from the same selected revision as your executable.

## Choose the transport

Embedded Rust needs no sidecar or token. A sidecar defaults to the same-user,
mode-0600 Unix socket `~/.deadbolt/deadbolt.sock` on macOS/Linux. Windows uses
loopback TCP and requires a nonempty operator token. TCP is also available on Unix.

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
These steps leave databases, revocations and evidence in place. Preserve them
according to your retention policy.

Next: [quick start](QUICKSTART.md), [integration](INTEGRATION.md), or
[troubleshooting](TROUBLESHOOTING.md).
