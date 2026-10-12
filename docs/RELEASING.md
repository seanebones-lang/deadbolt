# Candidate packaging and release procedure

The `release candidate` workflow builds unsigned native archives and retains
review artifacts for seven days. It does not publish a release or registry
package. Standard GitHub-hosted runners are used; no larger runner is required.
An additional Linux job repeats the pinned Hermes MCP/filesystem acceptance.

The release workflow includes a separate provenance job after the complete
native matrix succeeds on `main`. PR jobs retain read-only credentials and
cannot generate these release-build attestations. The provenance job attests
the exact inner native ZIP, then verifies its repository, signer workflow,
`main` ref and source digest; its bundle and verification receipt are retained
alongside the candidate artifacts. This remains a pending release gate until
the workflow succeeds on the chosen source revision.

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

For an archive attested by that workflow, verify the exact downloaded ZIP:

```sh
gh attestation verify /absolute/path/to/deadbolt-VERSION-TARGET.zip \
  --repo seanebones-lang/deadbolt \
  --signer-workflow seanebones-lang/deadbolt/.github/workflows/release-candidate.yml \
  --source-ref refs/heads/main --source-digest REVIEWED_FULL_SOURCE_SHA
```

Read the release's artifact-specific coverage before requiring this command.
Existing releases have no such native-build attestation. Local Developer ID
signing changes a macOS archive's bytes, so an unsigned CI archive's attestation
does not authenticate the later signed ZIP. Publish the corresponding CI
receipt separately and describe that limit; do not present a receipt for
different bytes as verification of the final download. A build attestation
establishes origin, not a security audit or independent product certification.
See [GitHub's artifact-attestation documentation](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations).

For the 2.0 candidate, the native matrix also builds and installs the matching
Python wheel from an unpacked sdist. `tests/native_candidate_acceptance.py`
extracts the selected ZIP and verifies version, source revision, binary hash,
client payload and migration documentation, then exercises that executable's
client, deadline, supervisor and owned-process behavior. Its published-upgrade
mode downloads checksum-pinned v1.0.3 and v1.1.0-rc.1 archives and preserves
legacy lease/kill/policy/evidence state and, where supported, old credentials
and pending/consumed exact-action approvals. Disposable runtime fixtures do not
inherit the GitHub download token. Review `native-candidate.json` for each target.
Windows validates refusal of unsupported executor cancellation, not POSIX cleanup.
The separate Rust consumer fixture checks v1 compilation, expected unmigrated
v2 compiler errors, and successful migrated compilation. These are release gates,
not a claim that any registry or plugin already contains this candidate.

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
  --input release-artifacts/deadbolt-2.0.0-rc.1-aarch64-apple-darwin.zip \
  --output signed/deadbolt-2.0.0-rc.1-aarch64-apple-darwin.zip \
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
xcrun notarytool submit signed/deadbolt-2.0.0-rc.1-aarch64-apple-darwin.zip \
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

## Cargo registry publication

The Rust crate `n11-deadbolt` provides both library `deadbolt` and CLI `deadbolt`.
Stable `1.0.3` and candidate `1.1.0-rc.1` are published on
[crates.io](https://crates.io/crates/n11-deadbolt/versions). Registry publication
is separate from native archives, MCP bundles and Python release wheels.

For each future version:

1. Select a clean, reviewed release commit and immutable tag. Confirm the
   manifest version, license, publisher ownership and repository identity.
   Never move an already published tag or edit its release source to change
   a package under the same version.
2. Inspect `cargo package --list`, then run `cargo publish --dry-run --locked`
   and `cargo package --locked`. Verify the actual archive includes the lockfile,
   licenses, source, runnable examples and documentation, with no credentials,
   private state or oversized media. Inspect `.cargo_vcs_info.json` and record
   the package SHA-256.
3. Extract the archive outside the checkout and install its CLI with
   `cargo install --path /absolute/extracted/package --locked --root /temporary/install`.
   Run its version, drill and packaged `examples/evaluate.py`. Exercise a
   separate Rust application depending on the extracted library. Verify actual
   permitted effects and that denied, replayed or revoked requests leave the
   protected bodies untouched. Candidate-specific APIs need candidate-specific
   acceptance in addition to the stable gate checks.
4. Use a verified publisher account and a short-lived token restricted to this
   crate's required publish scopes. Keep it in private Cargo credential storage,
   never command-line arguments, Git, screenshots, reports or logs. Use the
   official `cargo login` prompt for manual credential setup. An installer or
   library consumer needs no publishing credential.
5. Run `cargo publish --locked` from the exact clean release source. Publication
   is permanent: a version cannot be overwritten. Follow the
   [Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html).
6. Verify the public registry version and publisher. Download the registry
   archive and compare its SHA-256 with both the tested package and registry
   checksum; confirm the embedded Git SHA. Install the pinned version with
   `cargo install n11-deadbolt --version VERSION --locked --bin deadbolt --root /temporary/install`.
   Repeat the drill and real sidecar effects. Build a separate consumer using
   `deadbolt = { package = "n11-deadbolt", version = "=VERSION" }`, then check
   allowed/blocked effects using that registry dependency. Remove temporary
   installs and revoke the short-lived publishing token when finished.
7. Check the rendered registry README and its links, plus the docs.rs build
   separately. A visible crate is not proof that generated documentation built.
   Update current installation documentation only after registry acceptance.

The first registry publication used existing release tags without source edits:
`v1.0.3` (`3a857f8438dcda4bdaadb414d37f128508dc44f1`) and `v1.1.0-rc.1`
(`52a2085155b658f3d1dda5a73a8d95a7a1d88fad`). Their packaged READMEs retain the
original release-date statements about registry availability; current
[installation instructions](INSTALL.md#cargo-registry-cli-and-embedded-rust)
are maintained separately.

The candidate Python wheel/sdist are also published on
[PyPI](https://pypi.org/project/n11-deadbolt-client/1.1.0rc1/) after separate
ownership, exact-package and registry-consumer checks. Node remains a bundled
source client, not an npm package.

## Python registry publication

`publish-python.yml` is a manually dispatched, single-release workflow for the
existing `v2.0.0-rc.2` Python wheel and source archive. It does not rebuild them
from current `main` or run on pushes, tags or pull requests. With `publish=false`
(the default), it verifies hashes, package identity, installed base-client
behavior and the real optional SDK Runner against the released Linux engine.
It has no PyPI publishing permission in that mode.

Before a first publication:

1. Confirm PyPI ownership, verified email and two-factor authentication. Configure
   a [pending Trusted Publisher](https://docs.pypi.org/trusted-publishers/creating-a-project-through-oidc/)
   for project `n11-deadbolt-client`, GitHub owner `seanebones-lang`, repository
   `deadbolt`, workflow `publish-python.yml` and environment `pypi`.
2. Restrict the GitHub `pypi` environment to `main` and require a maintainer
   reviewer. Review that environment and the exact workflow before authorizing
   it. Trusted Publishing grants this workflow project publishing authority;
   protect future changes to the workflow and repository access accordingly.
3. Run `publish=false` on `main` and inspect the completed verification job.
   Independently inspect the wheel and sdist, rebuild the sdist outside the
   checkout and compare package payloads. Never repackage files under an
   existing version to update documentation.
4. After release approval, dispatch with `publish=true` on `main` and approve the
   protected environment. The isolated publication job only retrieves the
   verified artifacts and invokes the pinned official PyPA publication action
   with short-lived OIDC credentials and attestations. No persistent PyPI token
   is stored locally or in GitHub secrets.
5. Verify both files on the public PyPI JSON API, including SHA-256 and version.
   Install the exact version from PyPI in a fresh environment outside the
   checkout. Repeat fail-closed and real SDK/sidecar acceptance before updating
   installation documentation or declaring registry distribution complete.

This workflow deliberately permits only `2.0.0rc2` with the reviewed release
asset hashes. A future version needs a reviewed workflow update and its own
acceptance evidence. `skip-existing=false` exposes retries/collisions; do not
silently skip a file or treat a partial upload as complete. Once public, a PyPI
version's files cannot be replaced. The engine remains separately installed;
the Python distribution is a client and optional adapter, not a sandbox.

## Homebrew tap updates

The [NextEleven tap](https://github.com/seanebones-lang/homebrew-tap) packages
the existing signed macOS archives; it does not rebuild or republish an engine.
Stable and preview have separate, conflicting casks. For each engine update,
verify both published archive hashes, extracted executable hashes, strict
Developer ID signatures and matching accepted Apple notarization logs. Update
the cask and `releases.json` together, then pass the four-channel/architecture
acceptance jobs before merging. Check a fresh public-tap installation and its
actual file-effect behavior before updating current documentation.

See [Homebrew installation and maintenance](HOMEBREW.md) and the
[tap procedure](https://github.com/seanebones-lang/homebrew-tap#maintainers-and-validation).
Do not infer engine updates from MCP bundle version numbers. A lifecycle
reinstall test is not proof of compatibility across different engine versions.

## HOL listing update

After publication, follow [HOL submission and existing-listing updates](HOL-SUBMISSION.md).
