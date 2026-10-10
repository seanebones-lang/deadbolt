# Changelog

Versioned GitHub releases identify publication and the exact source revision.
The older `v0.1.1-product` tag is not the standalone hardening release.

## Unreleased

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
  on client EOF, drain final output for at most one second, then kill/wait the
  immediate child without joining blocked readers. Descendants remain executor-owned.
- Create the Docker image's state directory with mode 0700. Existing named
  volumes retain their existing modes and require operator review.
- Include r-efi 6.0.0's packaged AUTHORS copyright/license notices in the native
  dependency inventory; its upstream crate does not use a LICENSE filename.
- Exercise dispatcher effects, concurrent one-shot callbacks, invalid configuration,
  oversized/partial responses, independent drill state and MCP shutdown in regression tests.
- Verify a separate embedded application against the unpacked Cargo artifact in
  CI, using real allowed file effects and denied callbacks outside the checkout.

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
