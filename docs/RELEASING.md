# Candidate packaging and release procedure

The `release candidate` workflow builds unsigned native archives and retains
review artifacts for seven days. It does not publish a release or registry
package. Standard GitHub-hosted runners are used; no larger runner is required.
An additional Linux job repeats the pinned Hermes MCP/filesystem acceptance.

| Asset target | Candidate validation host | Compatibility boundary |
| --- | --- | --- |
| x86_64-unknown-linux-gnu | Ubuntu 22.04 x86_64 | glibc Linux; validate on your distribution. Not Alpine/musl |
| aarch64-apple-darwin | macOS 15 Apple Silicon | macOS 15 tested; older systems need fresh acceptance |
| x86_64-apple-darwin | macOS 15 Intel | macOS 15 tested; older systems need fresh acceptance |
| x86_64-pc-windows-msvc | GitHub Windows runner | Native Windows TCP; test your client OS and runtime environment |

Until each job passes, these are planned asset targets rather than confirmed
published downloads. Linux arm64, Windows arm64, notarization and signing are not
part of this candidate. Checksums detect mismatched files; they are not a
signature establishing publisher identity.

## Build and verify

Build natively with Rust 1.85 and a C compiler for bundled SQLite:

```sh
cargo build --release --locked --bin deadbolt
python3 dist/package_release.py --binary target/release/deadbolt --target aarch64-apple-darwin
```

Replace the target with the native Rust host triple from `rustc -vV`. Windows
uses `python` and `target/release/deadbolt.exe`. The packager refuses mismatched
binary versions and host labels, builds a ZIP plus SHA-256 file, extracts the
archive and runs its executable's version, drill and help commands. The archive
contains source, docs, examples, tests, license/notice, deployment templates and
`BUILD.json` with its exact source revision and binary hash.

Run the workflow on the candidate PR. Review every matrix result, retrieve the
archives and check their contents. The ZIP itself is nested inside the Actions
artifact download; distribute the versioned ZIP and its checksum, not the outer
Actions wrapper. After merge, run against the exact candidate source revision so
the release's source identity is unambiguous.

## Install an archive

Download only from the selected repository release, alongside its SHA-256 file.
On Linux run `sha256sum -c ARCHIVE.zip.sha256`; macOS can use
`shasum -a 256 -c ARCHIVE.zip.sha256`. On Windows use
`Get-FileHash .\ARCHIVE.zip -Algorithm SHA256` and compare with the checksum file.
Extract the ZIP. On Unix ensure the executable is executable with `chmod 0755
deadbolt`. Run `./deadbolt --version` and `./deadbolt drill` (Windows:
`.\deadbolt.exe --version` and `.\deadbolt.exe drill`) before adding that directory
to PATH or installing it in your chosen binary directory.

Do not bypass an OS security warning without reviewing the source and selected
artifact. macOS candidates are unsigned and unnotarized. A verified source build
is an available alternative if your installation policy requires it.

## Final publication gates

1. All candidate source checks and native archive checks pass at the selected SHA.
2. Recorded independent-host acceptance passes against the candidate executable.
3. The documented quick start and operator recovery drill pass against disposable
   candidate state. Existing killed identities remain denied after restart/restore.
4. Inspect packaged documentation and verify links, artifact names, version,
   license, source identity and checksums. No tokens or local production state enter
   an archive.
5. Review public API/compatibility policy and release notes. Keep experimental
   deployments and integration limits explicit.
6. Create a reviewed prerelease with the RC tag, or set 1.0.0 and repeat candidate
   checks before a stable tag/release. Never move an already published version tag.

Registry publication is separate. Confirm package ownership, contents and
authentication before using `cargo publish`; never claim an unpublished install
path works. Python and Node integrations remain included source clients.
