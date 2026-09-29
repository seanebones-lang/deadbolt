# Changelog

Changes below are unreleased until a versioned release is published. Cargo version
is 1.0.0-rc.1; the older `v0.1.1-product` tag is not this hardening revision.

## 1.0.0-rc.1 — candidate preparation

- Merge the standalone integration and protocol hardening work after successful
  Linux/macOS/Windows and Rust 1.85 checks.
- Add optional independent-host acceptance using pinned Hermes MCP transport
  and the reference filesystem server; record actual allowed/denied file effects.
- Define the intended v1 compatibility and upgrade contract and ordered roadmap.
- Prepare native candidate archives, source identity, checksums and extraction
  smoke tests for Linux x86_64, macOS arm64/x86_64 and Windows x86_64.

This is a release candidate, not a stable 1.0 declaration. Native archive CI and
the remaining release gates must pass before publication.

## Unreleased — standalone integration and hardening

- Make Python/Node clients importable, add destination/policy/spend APIs and
  normalize transport/malformed/non-2xx admission responses to denial.
- Serialize SQLite lease mutations, approval consumption, spend updates and
  evidence sequences across independent connections.
- Preserve existing child revocation; refuse reparenting/self-parenting and new
  live children under inactive parents.
- Reject MCP batches/invalid envelopes and forward canonical parsed messages;
  a sidecar HTTP error cannot grant MCP admission.
- Bound sidecar workers and request lifetime, reject incomplete/ambiguous/oversized
  HTTP framing, and preserve existing files or live sockets at bind paths.
- Make transports platform-conditional; non-Unix defaults require tokened loopback
  TCP. Native Windows build, drill and TCP client contracts passed in CI.
- Add DEADBOLT_EVENTS, executable version output, persistent nonroot containers,
  dedicated systemd state, and multi-platform/MSRV CI checks.
- Add standalone installation, quick start, integration, FAQ, troubleshooting,
  operations and contributor guides with explicit trust/acceptance boundaries.
- Add multi-connection and protocol regressions and live Python/Node contracts.
