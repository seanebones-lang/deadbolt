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
killed identity. Legacy live admission refreshes the lease, silence does not, and an
expired lease is not refreshed by admission. Optional supervised source mode
requires a separate trusted heartbeat. Children need registration and their
own policy. One-shot approval is consumed by a successful admission, not successful
completion of the subsequent tool body.

`probe` is an admission recheck with reduced evidence logging, not a dry run.
It can refresh leases and consume approvals. Do not call both `admit` and `probe`
for the same one-shot dispatch. Never reuse a previous allow as current permission.

## Versions and upgrades

Published RC2 requires a nonempty operator token
for Unix sidecar startup as well as TCP. `DeadboltConfig` adds default-false
`allow_legacy_unauthenticated_unix`; complete Rust struct literals must add it or
use `..Default::default()`. Explicit `serve --legacy-unauthenticated-unix` preserves
older Unix access only before any scoped credential has ever been issued. A
configured token always wins; TCP remains token-required. The shared HTTP handler
also refuses missing authentication by default, regardless of enforcement mode.
This major-version default change does not change local CLI/library authority
or existing credential scope. Published rc.1/1.x retain their prior behavior.
[Setup and migration](HTTP-AUTHENTICATION.md) cover the changed startup contract.

Published RC2 removes runtime provisioning from the raw
proxy and requires an explicit tool allowlist in all MCP admission paths. Trusted
setup can use `deadbolt ensure --agent ID` for a fresh supervised idle lease or
choose `issue` to add a fixed deadline. Activity renewal is an explicit
`--activity-renewal` opt-in on `ensure`/`issue`; existing leases retain their
persisted mode. Provision the separate supervisor heartbeat before handing a
new supervised run to the executor. The new MCP sidecar
routes fail closed against older servers; ordinary Rust/HTTP admission retains
its existing open-policy contract. `policy_required` is a new denial token, so
exhaustive Rust enum matches need updating. Existing leases, policies, grants and
terminal revocations are preserved. Earlier RC1 and 1.x artifacts retain their original behavior.

Published RC2 tightens admission for registered
trees in both activity and supervised modes. Every ancestor must be live; a
paused parent blocks new descendant calls without rewriting descendant state.
Missing, cyclic or inconsistent lineage fails closed. The check precedes target
TTL renewal and broad or exact approval consumption, so blocked requests preserve
their grants. Public API, wire format and database schema are unchanged. Own-lease
status and registration's live result remain separate from fresh admission.
The RC2 child-policy change below supplies capability and
reported-spend ceilings. Cancellation of already admitted work remains an
executor responsibility. Published rc.1 and 1.x artifacts retain their older behavior.

Published RC2 tightens registered-tree authorization.
Every ancestor's current tool, destination/offline, clip and irreversible/exact
review restrictions apply to each descendant. Grants never transfer between
identities. MCP still requires an explicit tool allowlist on the target itself.
Policy denials on live, configured activity-mode requests renew only the target's
idle TTL, as local policy denials do; supervised mode does not. Inactive ancestry
and missing MCP setup still deny before renewal. A cap covers the budget owner's
registered subtree, counting each identity's own ledger once. `SpendAdded.paused`
now means this report reaches an identity or ancestor cap; incident policy adds
`subtree_spend_usd`. Existing ledger values and grants are preserved, but already
recorded descendant costs can exhaust a previously available parent cap. This
is a major-version behavior change with no new schema or provider-billing claim.
SQLite traverses registered lineage and capped subtrees under the writer lock;
large trees need their own workload validation. Published rc.1/1.x are unchanged.

Published RC2 changes MCP admission denials from JSON-RPC
errors to `isError: true` tool results, reports effective status expiry without
mutating state, and makes a named missing CLI status query exit 1. Consumers must
check `result.isError` as well as protocol errors; it never means the body ran.
Evidence decisions use the actual `purpose_exceeded` token rather than the old
`clipped` alias. Stored prior evidence is preserved, including its old labels.
The 2.0 preview launcher accepts the exact not-found response only during fresh
operator setup; it does not treat arbitrary errors as a missing lease. Published
rc.1 and stable artifacts retain their bytes and prior behavior. RC2 publishes these changes as a new candidate; repeat actual GUI/model and
independent validation before stable promotion.

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

The `1.1.0-rc.1` prerelease adds a credential table without changing
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
their behavior. This is available in 1.1.0-rc.1. Older binaries cannot enforce these new
requirements; quiesce protected dispatch before any rollback.

## Fixed lease deadline source addition

The 2.0 candidate adds a nullable SQLite `hard_deadline_at` column,
operator-only `issue`/`POST /issue`, a status field and `deadline_exceeded`.
Existing rows migrate to idle-only leases with no fixed deadline. New deadline
runs require fresh IDs; setup cannot attach or change one in place. Child
registration inherits it. JSONL/text issuance exports and incident output include
the timestamp. Rust `AgentStatus` and `ExportRow` add public fields; downstream
struct literals must supply them. Clients must withhold effects on every denial,
including codes they have not seen before.

Published 1.x engine/MCPB/plugin versions do not include this addition. Older
executables ignore the column, so rollback does not preserve the guarantee.
Quiesce protected dispatch before changing binaries. See
[deadline setup, evidence and limits](LEASE-DEADLINES.md).

## Supervised liveness source addition

The 2.0 candidate adds nullable `leases.supervision_root` and a credential `scope`
column defaulting to admission. Existing identities and credentials preserve
legacy authority and renewal behavior. Supervised runs require fresh operator
issuance; children inherit mode. New renewal credentials cannot admit or control
policy. Their sole route has no operator/workload fallback. Migration tests
reconstruct the prior schema and preserve old admission credentials and kills.

Rust `AgentStatus` adds `supervision_root`; `ExportRow` adds `supervision_root`,
`expires_at` and `renewed`. These public struct additions require changes in
consumers constructing literals and are included in the selected 2.0 candidate,
along with the new deadline enum variant. JSON consumers tolerate added
fields while preserving strict allow/deny decisions. Do not publish this source
as a compatible stable 1.1 update without resolving that version boundary.

Older executables ignore supervision, so rollback does not preserve its liveness
guarantee. Quiesce protected dispatch first and follow the backup/rollback plan.
No existing release or plugin is upgraded by merging this source. See
[supervision](SUPERVISION.md) and the consolidated release gates.

## Selected 2.0 candidate

The corrected candidate is now `2.0.0-rc.2`, with Python `2.0.0rc2`, resolving the
major-version boundary described above. [The migration guide](MIGRATING-V2.md)
and compile acceptance cover original exhaustive matches and public struct
literals, as well as a migrated downstream consumer. Versioned publication,
registry installation, native signing and editor/plugin adoption remain separately
verified actions. Old release/tag identities are immutable.
