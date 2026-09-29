# Install Deadbolt

## Prerequisites

| Component | Required for |
| --- | --- |
| Rust 1.85 or newer and Cargo | Building/installing the binary or Rust library |
| Native C/C++ compiler and linker | Building bundled SQLite |
| Git | Cloning or installation from Git |
| Python 3 | Python client and quick start; Python 3.12 is used by CI |
| Node.js | Node client and consumer tests; Node 22 is used by CI |
| Docker with Compose | Optional container deployment |

You do not need a model key, Harness, a cloud account, or a database server.
Use your OS's supported Rust/compiler installation method. On macOS, Apple's
Command Line Tools provide the native compiler; on Linux, install the C compiler
and linker from your distribution; on Windows, use Rust's MSVC toolchain with
Visual Studio C++ Build Tools and the Windows SDK. Native Windows builds and the TCP client contracts passed on GitHub
Actions Windows Server 2025 on 2026-09-29; validate your target application too.

Check the environment:

```sh
rustc --version
cargo --version
git --version
```

## Native release archives

V1 ZIP archives target Linux x86_64, Windows x86_64 and macOS arm64/x86_64.
Download the matching ZIP and checksum from
[v1.0.0](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.0).
Follow [checksum verification and archive installation](RELEASING.md#install-an-archive),
then run the drill. This path does not require Rust or a C compiler. Python and
Node are needed only for their source clients and demonstrations.

Release binaries are unsigned; macOS downloads are not notarized. Consult the
release's tested OS requirements. Source builds remain an alternative. A ZIP
download is available only for a published release; the release page identifies
the exact source revision.

## Install the source checkout

Run these commands in Terminal, a Linux shell, or PowerShell:

```sh
git clone https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo install --path . --locked --bin deadbolt
deadbolt --version
deadbolt drill
```

Expected drill output: `deadbolt drill ok`. Cargo puts the binary in its bin
directory, normally `~/.cargo/bin` or `%USERPROFILE%\.cargo\bin`. If it is not
on PATH, use that full path or fix PATH and reopen the terminal.

`deadbolt --version` reports the Cargo version; unreleased commits may share a
version. Record `git rev-parse HEAD` as well for an exact source identity.

## Install directly from Git

```sh
cargo install --git https://github.com/seanebones-lang/deadbolt.git --locked --bin deadbolt
```

For a reviewed, reproducible deployment, replace `REVIEWED_COMMIT` below with
the exact full Git commit SHA approved for your integration:

```sh
cargo install --git https://github.com/seanebones-lang/deadbolt.git --rev REVIEWED_COMMIT --locked --bin deadbolt
```

The package name is `n11-deadbolt`, but it is not on crates.io as checked on
2026-09-29. `cargo install n11-deadbolt` is not an available installation path.
The Python and Node clients are source files, not pip/npm packages.

## Choose the transport

macOS/Linux default to `~/.deadbolt/deadbolt.sock`, mode `0600`. The server and
client need the same OS user or deliberate container UID mapping.

Windows defaults to `127.0.0.1:9782` and requires a nonempty token. TCP works on
macOS/Linux as well. Configure the endpoint and token in both the server's and
client's environment. Do not put a real token in source control.

Unix shell:

```sh
export DEADBOLT_SOCK=127.0.0.1:9782
export DEADBOLT_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
deadbolt serve --bind "$DEADBOLT_SOCK"
```

PowerShell:

```powershell
$env:DEADBOLT_SOCK = "127.0.0.1:9782"
$env:DEADBOLT_TOKEN = python -c "import secrets; print(secrets.token_hex(32))"
deadbolt serve --bind $env:DEADBOLT_SOCK
```

Keep the server terminal open. A different terminal does not inherit changes
made in this terminal; securely supply the same token to the trusted client.
For an easy local Unix demonstration, follow [the quick start](QUICKSTART.md).
For Rust embedding, no transport or token is necessary.

## Upgrade and remove

Stop admissions and the service, preserve the state, and record the current
commit before upgrading. Install the reviewed source with `--force` if Cargo
refuses to replace an installed binary; then run the drill and your acceptance
checks. See [operations](OPERATIONS.md) for backup and rollback.

`cargo uninstall n11-deadbolt` removes the installed executable. It does not
remove databases, evidence, or copied clients. Preserve those records according
to your retention policy; removing state would erase the existing revocations.

Next: [quick start](QUICKSTART.md), [integration](INTEGRATION.md), or
[troubleshooting](TROUBLESHOOTING.md).
