# Contributing to Deadbolt

Use a branch from the current main checkout. Rust 1.85 is the minimum supported
version; Python 3.12 and Node 22 are the CI client runtimes. Install a native
C/C++ toolchain for bundled SQLite. Run:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --locked --bin deadbolt
cargo run --locked --bin deadbolt -- drill
cargo run --locked --example build_in
python3 tests/client_contract.py
cargo +1.85.0 check --all-targets --locked
```

On Windows use `python` and set `DEADBOLT_BIN=target/debug/deadbolt.exe` in your
shell's environment. Integration tests must use temporary stores and dummy tool
bodies. Never run resource-exhaustion experiments on a shared service.

A pull request should explain the observable problem, resulting behavior,
validation and any platform or integration gates still open. Keep documentation
and protocol examples consistent with the code. Add regressions for meaningful
state, authorization or concurrency changes. Preserve denial codes and make sure
an unavailable service or store cannot accidentally permit dispatch.

Protect these properties: kill is terminal for an ID, parent kill covers registered
descendants, approvals are one shot, default storage failures deny, sidecar binds
stay local, credentials stay out of version control, and every forwarded MCP tool
call is admitted. Do not introduce universal containment or compliance claims.

Use ordinary GitHub issues for bugs or feature requests; include a redacted minimal
reproduction and source SHA. Report suspected security vulnerabilities privately
under [SECURITY.md](SECURITY.md). Follow the Apache-2.0 license and NOTICE.
