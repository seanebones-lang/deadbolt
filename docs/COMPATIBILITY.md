# Compatibility contract for v1

This policy defines the 1.0 contract. Version 1.0.0 is published; its release
notes identify the exact source revision and validated platforms. Later source
changes are not a new released version until separately tagged and published.

## Supported surfaces

- Rust library: documented public configuration, admission, lease, policy,
  operator and evidence interfaces exported by `deadbolt` in `n11-deadbolt`.
- Local HTTP routes and JSON shapes in [the protocol](PROTOCOL.md), with a
  mode-0600 Unix socket or authenticated loopback TCP.
- CLI commands and documented JSON output used by operator integrations.
  Human-readable status text, help layout and diagnostic prose are not parsers.
- Included Python/Node source clients, and individual stdio MCP JSON-RPC 2.0
  objects with routed `tools/call`. The optional Python client is an installable source-candidate wheel/sdist;
  native and registry publication remain separate release actions.

Consumers must execute only on explicit allow. Unknown decisions, unsuccessful
responses and invalid payloads cannot become permission. Existing denial tokens
retain their meaning in compatible releases; integrations should still treat
unknown denial codes as denial. Consumers of JSON should tolerate added fields.
Rust additions that break exhaustive matches or struct construction are breaking
changes and need a major version after 1.0.

Kill stays terminal for an identity in the same store. Ensure cannot restore a
killed identity. Live admission refreshes the lease, silence does not, and an
expired lease is not refreshed by admission. Children need registration and their
own policy. One-shot approval is consumed by a successful admission, not successful
completion of the subsequent tool body.

`probe` is an admission recheck with reduced evidence logging, not a dry run.
It can refresh leases and consume approvals. Do not call both `admit` and `probe`
for the same one-shot dispatch. Never reuse a previous allow as current permission.

## Versions and upgrades

After 1.0, compatible bug fixes use patch versions, compatible additions use minor
versions, and incompatible public API/protocol/configuration behavior needs a
major version. Security fixes may tighten acceptance of malformed inputs; document
these changes. Do not depend on invalid or undocumented input behavior.

State is internal storage, not a supported direct SQL API. Supported upgrades
must preserve identities, policies, approvals, evidence and terminal revocations.
Every release that changes storage must specify supported source versions and
test migration before publication. A migration failure must not erase state or
silently create a new store. Downgrade is not generally supported; use the
[backup and rollback procedure](OPERATIONS.md) and resolve revocations recorded
after a backup before reopening dispatch.

The candidate's recorded upgrade acceptance starts with original source
`b8a13036567ca1b71ed14c6b36cbf84df80fe647` and preserves revocation, lineage,
policy, approvals, spend and evidence. See [the upgrade record](evidence/upgrade-rc.1-2026-09-29.json)
and `tests/upgrade_acceptance.py`. Other historical revisions require fresh
acceptance; version 0.1.0 alone does not identify an exact development commit.

The same upgrade passes against the 1.0.0 executable in
[the v1 upgrade record](evidence/upgrade-v1-2026-09-29.json).

## Platforms and boundaries

Source CI validates Linux, macOS, Windows and Rust 1.85. Native Windows uses
tokened loopback TCP; Unix sockets and token-file permissions are Unix-only.
Release binaries are supported only on the architectures and OS requirements
listed in their release notes. A source CI pass is not a binary-installation test.

Containers have a separately recorded nonroot shared-socket acceptance. The
systemd unit remains an experimental deployment template until native host
installation is recorded. Arbitrary third-party frameworks and future upstream
versions require their own dispatch acceptance.

V1 does not commit to hard cancellation, arbitrary-code sandboxing, authoritative
cost accounting, remote MCP, per-agent sidecar credentials, atomic execution with
admission, tamper-proof evidence, an SLA or a throughput target. See
[trust boundaries](TRUST.md). Optional Witness-shaped output is an additional
record format, not a requirement to run another service.

## Admission-credential protocol

The unreleased `1.1.0-rc.1` candidate adds a credential table without changing
existing lease, policy, approval or evidence fields. It supports legacy operator
HTTP authentication and a separate single-agent admission header. See
[credential migration](CREDENTIALS.md): issuing the first credential deliberately
disables anonymous Unix HTTP authority. This persists after revocation/expiry.
Older binaries do not enforce this boundary; quiesce workload access before
rollback and do not expose an anonymous old Unix listener to scoped clients.

## Exact-action source candidate

The opt-in [exact-action protocol](ACTION-APPROVALS.md) binds reviewed arguments,
run, tool, destination, nonce and deadline. Its new endpoint cannot fall back to
legacy admission. Approval enables exact-only policy for that run/tool;
nonces persist after use/revocation. Existing unconfigured integrations retain
their behavior. This is unreleased. Older binaries cannot enforce these new
requirements; quiesce protected dispatch before any rollback.
