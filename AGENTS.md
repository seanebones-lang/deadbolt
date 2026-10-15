# Working on DeadBolt

## Start here

Read [README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md) and
[SECURITY.md](SECURITY.md) before editing. Use [installation](docs/INSTALL.md)
for released artifacts, [integration](docs/INTEGRATION.md) for dispatch wiring,
and [private state](docs/PRIVATE-STATE.md) before touching existing stores.
Check the working tree and the exact revision; preserve unrelated edits.

## Local checks without a model account

The Rust manifest requires Rust 1.85 or later. Python client support starts at
Python 3.9; Node is also needed for the cross-language client contract.
From the repository root:

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --locked --bin deadbolt
./target/debug/deadbolt drill
python3 examples/evaluate.py --binary target/debug/deadbolt
python3 tests/client_contract.py
```

On Windows use `target/debug/deadbolt.exe`. These local examples use inert work;
they need no provider key or paid API. Builds may download dependencies. Consult
[CI](.github/workflows/ci.yml) for the additional integration, installed-package,
platform and acceptance checks appropriate to the code being changed. A local
unit pass does not establish Cursor or another editor's model-mediated behavior.

## Boundaries to preserve

- Admission belongs immediately before each protected tool/spawn dispatch.
  Direct execution and already-running work are outside this gate. DeadBolt is
  not an operating-system sandbox.
- Keep operator authority, credentials and private state outside model-controlled
  tools. Use fresh disposable identities/stores for tests, never an operator's
  existing database. Do not print bearer values or weaken permissions for a test.
- Kill, ancestor policy, supervisor liveness, fixed deadlines and one-use grants
  must fail closed. Preserve structured denial reasons and independent effect
  counts when testing changes to those boundaries.
- Blank MCP setup is not active protection. Do not turn an unavailable or damaged
  store into a fresh authorized run without the documented recovery process.
- Keep engine, Python client, editor bundle and registry availability distinct.
  Source changes do not publish packages. Preserve the stable 2.0.0 scope unless
  the owner authorizes a behavior change.

## Handoff

Report the revision, changed behavior, exact checks and their results, platform,
remaining gaps and any effect on migration or compatibility. Report suspected
vulnerabilities privately through SECURITY.md; public examples must be redacted.
