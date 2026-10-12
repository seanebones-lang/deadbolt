# Changelog

Versioned GitHub releases identify publication and the exact source revision.
The older `v0.1.1-product` tag is not the standalone hardening release.

## Unreleased after 2.0.0-rc.1

- Default fresh Rust, CLI and HTTP roots to supervisor-only idle renewal;
  preserve existing persisted modes and offer explicit legacy activity setup.
  Strict HTTP mode parsing rejects duplicate, unknown and nonboolean options.
  Keep the bundle's deliberate mode choice across old and corrected engines.

- Add exact-byte native CI build attestations and repository/workflow/source
  verification after the full main-branch packaging matrix succeeds. Existing
  releases and later locally signed macOS bytes remain separate proof gates.

- Add operator CLI child registration and a bounded global idle-TTL option.
  Inactive registrations exit unsuccessfully; setup never refreshes existing
  identities. Separate supervisor revocation from the provisioning example.

- Require distribution hashes for the complete OpenAI Agents, Hermes and FastMCP
  Python acceptance dependency sets, retaining their tested versions and adding
  missing conditional platform dependencies.

- Report changed operator action reviews with the nonsecret
  `fingerprint_mismatch` error, without granting permission or echoing arguments.

- Create Unix core/Witness SQLite and JSONL state privately before SQLite writes;
  refuse unsafe existing files, sidecars and writable parents. Document explicit
  quiesced permission migration and continuing OS ownership/Windows ACL limits.

- Require operator authentication for both Unix and loopback TCP sidecar startup.
  Preserve older Unix socket-possession authority only through the explicit
  `--legacy-unauthenticated-unix` flag / default-false Rust config. Configured
  tokens and the persisted scoped-credential boundary always take precedence.
  Shared HTTP dispatch never infers default operator authority from no token.
- Validate header field names/values consistently in framing and role selection,
  so malformed spaced role names cannot fall back to operator/legacy access.
- Add real Unix listener and extracted-native acceptance for default refusal,
  scoped role separation and explicit legacy migration. Update setup/operations
  recipes. Published artifacts keep their original identity and behavior.

## 2.0.0-rc.1 candidate (publication is a separate gate)

- Select a major Rust engine version and matching Python candidate spelling.
  Document downstream enum/struct migration and validate installed native ZIPs
  and published-version state upgrades separately from source tests.
- Anchor Cargo package paths to the source root so ignored local build files
  with matching basenames cannot enter a registry archive.
- Add optional supervisor-only idle renewal for fresh root runs and registered
  descendants. Agent allow/deny activity cannot keep this mode alive; immutable
  fixed deadlines remain independent and always win. Existing leases keep their
  renewal behavior.
- Add separate root-bound renewal credentials, scope-preserving migration,
  operator issuance/list/revoke, renewal-only HTTP access and explicit Python
  authentication without operator/workload fallback. No expired/killed revival
  or policy/approval changes. Observed renewal exports retain exact maintained IDs
  and expiry; failed default primary evidence rolls the mutation back.
- Validate schema migration, restart, writer-lock expiry, HTTP role boundaries and
  installed-client effects against a real sidecar and actual 60-second idle TTL.
  See [supervision setup and limitations](docs/SUPERVISION.md).

- Add a source-built Python owned-process supervisor reference: fresh admission,
  read-only lease monitoring and POSIX group cleanup, with installed-consumer
  kill/deadline/outage acceptance. Refuse unsupported hosts before execution.
- Reject ambiguous duplicate-key and nonfinite JSON in Python client responses.


- Add immutable operator-issued `hard_deadline_at` for fresh root leases through
  Rust `issue_agent`, CLI `issue` and HTTP `/issue`. Deadline denials are distinct
  from idle expiry and happen before TTL renewal or approval consumption.
  Registered children inherit the fixed cutoff; restart preserves it. Existing
  leases stay idle-only, and no setup/resume/policy endpoint extends a deadline.
- Record deadline issuance in Observed evidence and include it in status,
  exports and incidents. Roll back root/child issuance on failed primary evidence
  with the default sink. Validate time after writer-lock acquisition and exercise
  installed Python/Node callbacks on three CI platforms.
- These source additions are not in existing registry engines or plugin bundles.
  They stop new admissions; the executor still owns in-flight cancellation,
  host clock trust and process/egress confinement. Optional supervisor-only renewal is also an unreleased source addition. See [fixed lease deadlines](docs/LEASE-DEADLINES.md).

## 1.1.0-rc.1

Developer-evaluation prerelease. Native archives and the Python wheel/sdist are
distributed through the versioned GitHub release; package registries remain separate.
See its release notes for the exact source SHA, checks and signing status.

- Keep hosted walkthrough media out of native and Cargo packages. Validate successful
  accounting and evidence under bounded SQLite contention without retrying spends.

- Reorganize developer onboarding around one-command real-effect evaluation,
  explicit published/candidate API selection, native/source/wheel installation,
  embedded Rust and operator/dispatcher setup. Correct stale future-work and
  package-installation statements; keep registry publication separate.

- Add opt-in operator-reviewed exact-action grants, RFC 8785 fingerprints, fixed
  deadlines, atomic single-use admission and a separate `/admit-action` route.
  Exact-only tool policy closes normal admit/probe/broad approval paths under
  enabled fail-closed enforcement. Add private-review CLI with expected digest,
  snapshotting Rust/Python/Node dispatch, optional SDK integration and MCP metadata.
  See `docs/ACTION-APPROVALS.md` for trust boundaries, migration and retry limits.

- Add operator-issued single-agent admission credentials with OS-generated 256-bit
  bearer secrets, stored hashes, fixed expiry and independent revocation. Credential
  authentication and policy/approval evaluation share a SQLite writer transaction.
  Add CLI issue/revoke/list controls; issuance writes an exclusive output file
  (0600 on Unix) and primary-evidence failure rolls back activation.
- Keep operator HTTP authority separate: workload credentials authorize only
  `POST /admit` and `POST /admit-action`, mixed/duplicate auth headers fail closed, and the first credential
  permanently disables anonymous Unix operator access in that store. Existing
  stores migrate without changing lease/policy/approval state.
- Add workload-mode Python/Node/MCP requests without operator fallback. Scoped
  MCP skips lease creation, requires a sidecar and refuses workload spend updates;
  child servers no longer inherit the two DeadBolt bearer-token environment values.
  Preserve useful `unauthorized`/`forbidden` denials in Python and Node.
- Package the existing stdlib Python client as an installable wheel/sdist with
  an optional OpenAI Agents SDK 0.23.1 decorator. Keep native sidecar installation
  separate and preserve source imports. No PyPI publication is performed.
- Gate validated Python tool bodies through the SDK's normal schema/approval
  machinery, recheck after approval/resume, preserve sync/async results, and stop
  the run on denial or tool failure. Reject SDK error-as-output handlers and
  deferred sync awaitables. Exact-action SDK review remains separate from
  DeadBolt's existing agent/tool one-shot grant.
- Verify outside-checkout Python packaging and installed SDK runner/sidecar
  effects on Linux, macOS and Windows CI. Pin the SDK acceptance dependencies;
  tests use the upstream ScriptedModel with tracing disabled and no API requests.
- Pin the Python build backend to setuptools 83.0.0 after auditing its source
  distribution exclusions, and audit the pinned optional SDK/build dependency
  set in CI with a retained machine-readable report.

- Add `dispatch` helpers for embedded Rust, Python and Node. A trusted callback
  runs only after a fresh explicit allow; denial leaves it untouched. Preserve
  callback results and errors without retrying effects. Node supports async bodies.
- Isolate each production drill in an exclusively created random workspace,
  explicitly mode 0700 on Unix, and create its fixture exclusively. Concurrent
  drills no longer share state or cleanup. Use temporary ownership in the Rust example.
- Normalize synchronous Node HTTP/configuration/serialization failures to denial,
  enforce a five-second total Node request deadline, bound source-client and MCP
  sidecar HTTP responses to 1 MiB, and close Python connections on all paths.
- Bound stdio MCP frames to 16 MiB and coordinate both readers with a bounded
  queue. Child EOF/read failures no longer wait for client EOF. Close child input
  on client EOF, drain final output for at most one second, allow one further
  second for natural child exit, then kill/wait without joining blocked readers.
  Descendants remain executor-owned.
  Report an already completed child failure as a proxy error.
- Create the Docker image's state directory with mode 0700. Existing named
  volumes retain their existing modes and require operator review.
- Include r-efi 6.0.0's packaged AUTHORS copyright/license notices in the native
  dependency inventory; its upstream crate does not use a LICENSE filename.
- Accept Cargo prerelease/build version identities in macOS signing preflight,
  still binding the full version/target to BUILD.json and checking archive bytes.
- Exercise dispatcher effects, concurrent one-shot callbacks, invalid configuration,
  oversized/partial responses, independent drill state and MCP shutdown in regression tests.
- Verify a separate embedded application against the unpacked Cargo artifact in
  CI, using real allowed file effects and denied callbacks outside the checkout.
- Audit the locked dependencies against the current RustSec database in CI and
  retain the machine-readable report, including on failure.

These changes are source-only until a new reviewed release is published; v1.0.3
downloads do not contain the new helpers or hardening.

## 1.0.3 — 2026-09-29

- Prepare Developer ID signing and Apple notarization for the macOS native
  archives. The signed-archive tool verifies hardened runtime, secure timestamp,
  publisher identity, embedded binary hash and checksum before submission.
- Keep Linux and Windows archive signing status explicit. No admission, storage,
  policy, protocol, API or dependency behavior changes from v1.0.2.
- Publish the exact signed archives only after Apple's notarization service and
  final download checks pass. The v1.0.2 release remains unchanged.

## 1.0.2 — 2026-09-29

- Keep operator `approve`, `resume`, and policy grants in the same transaction as
  primary evidence on the default sink. A failed primary evidence write now
  rolls back the grant instead of reporting an error after making it live.
- Validate the systemd unit in a disposable Ubuntu 24.04 arm64 container with
  real systemd: empty-token startup denial, tokened admission and policy,
  operator kill, restart persistence, and unrelated-agent isolation.
- Reduce default-store writer contention: reserve an evidence sequence and insert
  its SQLite row in one transaction; include default spend updates in that same
  transaction. Roll back balance, pause state and sequence if primary evidence
  writing fails. Preserve the five-second busy timeout and SQLite durability mode.
- Add write-failure/held-lock regressions, stronger cross-store evidence checks,
  repeated Windows independent-connection tests and concurrent-writer guidance.
- Add opt-in, code-only diagnostics for spend and primary-evidence failures;
  enable them in source CI without exposing SQL, paths, tokens or payloads.
- The original intermittent Windows failure did not recur in ten diagnostic runs;
  its precise failing stage remains unproven. This change addresses demonstrated
  partial spend/evidence failure and reduces transaction count, not all possible
  storage outages. Published v1.0.1 archives do not contain these changes.

## 1.0.1 — 2026-09-29

- Include the evaluator brief, observable demo runbook and scoped pilot worksheet
  in native/source distribution, alongside the updated product and press material.
- Replace the legacy Harness-specific overview and correct API comments about
  task cancellation, token requirements and explicitly disabled enforcement.
- Pin default installation to this maintenance release and document the current
  standalone boundaries. Add structured issue forms and private reporting links.
- Check Rust API documentation with warnings denied on the minimum Rust version.
- Preserve the 1.0.0 admission, state, protocol and public API behavior. No runtime
  policy changes or dependency updates are included.

## 1.0.0 — 2026-09-29

- Promote the tested local Rust/HTTP/stdio MCP integration contract to v1.
- Provide native Linux x86_64, Windows x86_64, macOS arm64 and macOS x86_64
  archives with checksums, source identity, source clients and complete guides.
- Verify actual file effects through independent Hermes MCP transport and the
  reference filesystem server, plus backup/restore and original-state upgrade.
- Keep systemd experimental and whole-agent Hermes integration outside the
  supported routed-MCP claim. No crates.io/npm/pip publication is implied.

Final source and artifact checks gate publication. Registry packages are not
published by the binary release process.

### Integration and hardening included in 1.0.0

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

## Candidate preparation history

The 1.0.0-rc.1 source was developed and tested before the stable cut: standalone
integration, protocol hardening, compatibility policy, independent Hermes
acceptance and native archives. A candidate source version is not itself a
published prerelease; published versions are identified by GitHub releases.
