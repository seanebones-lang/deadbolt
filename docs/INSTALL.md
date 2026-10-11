# Install DeadBolt

Choose a published native executable for a quick evaluation, or a reviewed source
candidate for the new APIs. The Python client connects to a **separately installed
executable**; installing the client does not start a sidecar.

| Path | You need | What you get |
| --- | --- | --- |
| Native archive | Matching OS/CPU; Python 3.9+ only for the evaluation | Stable v1.0.3 or prerelease 1.1.0-rc.1 executable, source clients, examples and docs |
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

The Rust crate and Python client are not published to package registries by this
release. Use the Git/local paths or downloadable wheel; do not use
`cargo install n11-deadbolt` or `pip install n11-deadbolt-client`.

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
