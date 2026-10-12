# Install DeadBolt with Homebrew

The [NextEleven tap](https://github.com/seanebones-lang/homebrew-tap) installs
the published, signed macOS executable on Apple Silicon or Intel. No Rust
toolchain or model account is needed. This is an upstream-maintained tap;
it is not a Homebrew core/cask listing, endorsement or security certification.

You need macOS and an existing [Homebrew installation](https://brew.sh/).
The cask links `deadbolt` on your Homebrew PATH and retains the release's
examples and documentation. It does not start a service, create operator
credentials or configure protection. Set an explicit policy and put the gate
at your application's dispatch boundary. Version 2 raw MCP requires a preissued lease
and explicit tool policy; review the version-specific configuration.

## Stable installation

```sh
brew install --cask seanebones-lang/tap/n11-deadbolt
deadbolt --version
deadbolt drill
```

Expected: `deadbolt 2.0.0`, then `deadbolt drill ok`.
Homebrew selects the matching architecture and checks the pinned archive
SHA-256. The [cask source](https://github.com/seanebones-lang/homebrew-tap/blob/main/Casks/n11-deadbolt.rb)
and [release verification manifest](https://github.com/seanebones-lang/homebrew-tap/blob/main/releases.json)
record the reviewed archives and executable hashes. Both Mac archives have
Developer ID signatures and matching accepted Apple notarization submissions.
No quarantine or Gatekeeper override is part of this installation.

### If Homebrew asks you to trust the cask

Review the cask above. On Homebrew versions with the `trust` command, grant
trust to this specific cask, then repeat the installation:

```sh
brew tap seanebones-lang/tap
brew trust --cask seanebones-lang/tap/n11-deadbolt
brew install --cask seanebones-lang/tap/n11-deadbolt
```

Earlier Homebrew versions may not provide `brew trust`; follow their normal
installation flow. Do not disable trust checks or grant whole-tap trust just
to install this package. [Homebrew's trust documentation](https://docs.brew.sh/Tap-Trust)
explains the scope of these permissions.

## Run the installed release's evaluation

Python 3.9+ is needed only for this disposable evaluation. The command below
selects the **installed stable archive**, rather than a different binary from
a source checkout:

```sh
DEADBOLT_RELEASE="$(brew --caskroom)/n11-deadbolt/2.0.0/deadbolt-2.0.0-$(uname -m | sed 's/arm64/aarch64/')-apple-darwin"
python3 "$DEADBOLT_RELEASE/examples/evaluate.py" --binary "$DEADBOLT_RELEASE/deadbolt"
```

Expected: a JSON report with `"passed": true` and eleven checks. Allowed
temporary file writes run; off-policy, unapproved, consumed approval, revoked
and unavailable-store cases leave their file effects untouched. Revocation
survives restart. The test uses a private temporary sidecar and cleans up.
No provider requests or real accounts are used. This proves the demonstration
dispatcher's behavior, not containment of every tool an agent can access.
[Understand the report](FIRST-EVALUATION.md), then [integrate one real route](INTEGRATION.md).

## Historical preview

The separate `n11-deadbolt-preview` cask remains pinned to 2.0.0-rc.2 for
historical evaluation. It does not contain the later destination and damaged-store
fixes. Use stable 2.0.0 for new installations. The two casks conflict because
they link the same command; stop dispatch, back up state and uninstall the older
selected channel before installing stable. Read [the migration guide](MIGRATING-V2.md).
The matching Python client and editor/MCP bundles are separate installations;
see [INSTALL.md](INSTALL.md) for exact available versions.

## Update, switch channels or remove

Stop protected dispatch and the sidecar deliberately, record the current
version and back up application-owned databases, evidence and configuration
before replacing an executable. Review [operations](OPERATIONS.md) and
[compatibility](COMPATIBILITY.md). An older engine cannot enforce newer scoped
credentials or exact-action requirements. Re-run the drill, evaluation and your
application's own acceptance checks before resuming dispatch.

For the selected stable channel:

```sh
brew update
brew upgrade --cask seanebones-lang/tap/n11-deadbolt
```

Use `n11-deadbolt-preview` instead for the preview channel. The two casks
conflict because they link the same executable. To switch deliberately after
the preparation above:

```sh
brew uninstall --cask n11-deadbolt-preview
brew install --cask seanebones-lang/tap/n11-deadbolt
```

To remove the package:

```sh
brew uninstall --cask n11-deadbolt
```

Use the preview name if that is installed. Uninstall removes the package and
command link, leaving application-owned state and credentials in place. The
casks contain no state deletion or `zap` hooks. Retain or remove your private
state separately according to your own policy.

If an archive, Cargo installation or another package already provides
`deadbolt`, identify it with `command -v deadbolt` and inspect its version
before choosing one owner for the command. Do not use `--force` to overwrite
an installation or symlink you have not identified. See [troubleshooting](TROUBLESHOOTING.md).

## Validation and release maintenance

The [tap acceptance workflow](https://github.com/seanebones-lang/homebrew-tap/actions/workflows/test.yml)
checks stable and preview on separate Apple Silicon and Intel macOS runners.
It verifies exact installed binary hashes, Developer ID signatures, the drill,
all eleven file-effect cases, refusal of the conflicting channel, reinstall
and uninstall while preserving a disposable private database and unrelated
casks. Each successful job publishes its JSON acceptance report. Reinstall is
tested; this does not claim an upgrade between distinct stable versions or
independent customer adoption.

Maintainers must review the engine release, checksums and Apple notarization
logs for both architectures, update the cask and manifest together, pass all
four jobs, then check installation from the public tap. Bundle version changes
alone do not change the engine casks. See the
[tap maintenance procedure](https://github.com/seanebones-lang/homebrew-tap#maintainers-and-validation)
and [engine release guide](RELEASING.md).
