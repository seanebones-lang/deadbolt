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
published downloads. Linux arm64 and Windows arm64 are outside this matrix.
The workflow's macOS ZIPs are unsigned candidates; the local signing procedure
below produces the distributable macOS ZIPs. Checksums detect mismatched files;
they are not a signature establishing publisher identity.

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
`BUILD.json` with its exact source revision and binary hash. It also includes a
generated locked-dependency license inventory and the third-party source/license
distribution described in [THIRD-PARTY.md](../THIRD-PARTY.md).

Run the workflow on the candidate PR. Review every matrix result, retrieve the
archives and check their contents. The ZIP itself is nested inside the Actions
artifact download; distribute the versioned ZIP and its checksum, not the outer
Actions wrapper. After merge, run against the exact candidate source revision so
the release's source identity is unambiguous.

Windows packaging requests static C-runtime linkage to reduce runtime installation
requirements; the native job still needs to pass. This does not remove the need
to validate your Windows edition and any OS-managed DLL dependencies.

## Sign and notarize the Mac archives

Use a locally installed **Developer ID Application** identity and a separate
`notarytool` Keychain profile. Never pass a private key or app-specific password
through a build log, command argument, repository file or chat. Run this on a
Mac after retrieving the exact-source candidate ZIPs from CI:

```sh
security find-identity -v -p codesigning
python3 dist/sign_macos_archive.py \
  --input release-artifacts/deadbolt-1.1.0-rc.1-aarch64-apple-darwin.zip \
  --output signed/deadbolt-1.1.0-rc.1-aarch64-apple-darwin.zip \
  --identity DEVELOPER_ID_APPLICATION_SHA1
```

Repeat for `x86_64-apple-darwin`. The script checks the candidate archive's
version, target and original binary hash, signs its executable with hardened
runtime and a secure timestamp, verifies the Developer ID signature, updates
`BUILD.json` with the signed executable's hash, team ID and CDHash, and writes a
new ZIP and checksum. It leaves the source revision intact. Reject any failed
signature or mismatched archive. Keep the unsigned CI archives only as build
evidence; do not publish them as the signed downloads.

Submit **each final signed ZIP**, not a smaller test ZIP, to Apple:

```sh
xcrun notarytool submit signed/deadbolt-1.1.0-rc.1-aarch64-apple-darwin.zip \
  --keychain-profile DeadboltNotarization --wait --output-format json
xcrun notarytool log SUBMISSION_ID \
  --keychain-profile DeadboltNotarization --output-format json
```

Repeat for Intel. Require `Accepted`, `issues: null`, the final ZIP's SHA-256 in
the notary log, and the executable's CDHash in `ticketContents`. Extract each
final ZIP afresh and verify `codesign --verify --strict`, the checksum,
`--version`, `drill`, and the first evaluation where the host supports that
architecture. Record both submission IDs in the release notes. Apple's online
ticket is associated with the signed code; a bare command-line executable is
not an app bundle and may not pass `spctl -a -t execute`'s app assessment. A
clean, downloaded-Mac installation check remains a separate release gate.

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
artifact. Check the selected release notes for macOS signing and notarization
status; v1.0.2's published Mac archives are unnotarized. A verified source build
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
6. Create a reviewed prerelease with the RC tag, or repeat candidate checks at
   the selected stable source before a versioned tag/release. Never move an
   already published version tag.

Registry publication is separate. The first useful registry target is crates.io
for the Rust crate `n11-deadbolt`; it supports both the library and CLI. Inspect
`cargo package --list` and run `cargo publish --dry-run` at the exact release
source, then verify name availability, publisher account/team ownership and the
package contents before an actual upload. A registry version cannot be replaced
after publication. Do not claim `cargo install n11-deadbolt` until the uploaded
crate is visible and installable. The candidate Python client already builds an installable wheel/sdist; registry
publication still needs API, ownership and exact-package review. Node remains a
bundled source client. Do not imply either is published before verifying it.
