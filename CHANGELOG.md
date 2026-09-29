# Changelog

Changes below are local, unreleased work until merged and published. Cargo version
remains 0.1.0; the older `v0.1.1-product` tag is not this hardening revision.

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
  TCP. Native Windows runtime remains an acceptance gate.
- Add DEADBOLT_EVENTS, executable version output, persistent nonroot containers,
  dedicated systemd state, and multi-platform/MSRV CI checks.
- Add standalone installation, quick start, integration, FAQ, troubleshooting,
  operations and contributor guides with explicit trust/acceptance boundaries.
- Add multi-connection and protocol regressions and live Python/Node contracts.
