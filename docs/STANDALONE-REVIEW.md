# Standalone integration review — 2026-09-29

Reviewed starting commit: `b8a13036567ca1b71ed14c6b36cbf84df80fe647`, matching
GitHub main at review start. Work is on `codex/standalone-integration`.

## Assessment

Deadbolt is a standalone Rust library and operator executable. It has no
Harness, Witness, model-provider, or API-key dependency. Apache-2.0 licensing
permits use in other projects subject to the license and notices. Bundled
SQLite removes the need for a separately provisioned database server.

It can be embedded in a Rust executor, used as a local HTTP sidecar by other
languages, or placed in front of a newline-delimited stdio MCP server. Universal
plug-and-play support would be an overclaim: each executor must own every tool
and spawn dispatch path, assign stable identities, and refuse execution on deny.
The MCP proxy covers routed tools/call; startup, resources, other methods, and
already-running actions are outside that gate. It does not contain arbitrary
untrusted code that can bypass the executor or write its own store.

## Findings and changes

| Finding | Change |
| --- | --- |
| README instructed installation of an unpublished crates.io package | Document source/Git installation and reviewed-commit pinning; registry lookup found no n11-deadbolt package |
| Node client ran CLI on import and exposed no module API | Export ensure, admit, registerChild, status, policy, spend; run CLI only as main module |
| Python/Node could not supply destination context | Add optional destination to admit and CLI; expose policy/spend APIs |
| Client responses could be unusable JSON or unsuccessful HTTP responses | Admit normalizes unusable responses to deny; non-2xx denies; Node handles aborted/error responses |
| Existing child registration overwrote revocation, parent, and clips | Preserve existing child state; refuse reparenting and self-parenting |
| Paused/expired parents could create live children | New children of inactive parents are recorded killed |
| Approval check and consumption were separate read/write operations | One SQLite immediate transaction checks state and consumes approval |
| Concurrent store connections could overwrite state or lose spend updates | Serialize lease creation, lineage, kill, resume, clip, policy, approval, spend, and pause mutations |
| Per-connection evidence sequence counters collided | Reserve unique sequence numbers through SQLite; serialize evidence writes and write newline with record |
| Unix-only imports prevented compilation on other platforms | Conditional Unix transports; default to token-required loopback TCP on non-Unix platforms; token files stay Unix-only |
| Container loopback listener was unreachable through ordinary port publishing; state was ephemeral | Use a shared mode-0600 Unix socket and persistent named volume; retain public-bind refusal |
| Systemd unit's dedicated account was commented out | Enable dedicated user/group and managed state directory |
| Custom CLI database could still write evidence to shared default location | Add DEADBOLT_EVENTS and document matching executor/operator paths |
| Integration docs omitted complete dependency/import/deployment details | Replace integration guide with complete consumer examples and boundaries |
| CI checked only Linux stable | Prepare macOS/Linux/Windows stable checks, consumer contract tests, and Rust 1.85 compatibility job |

## Local evidence

- Original baseline: 50 Rust tests passed.
- Child revocation and inactive-parent regressions failed against the original
  implementation before fixes.
- First integration revision: 54 Rust tests passed, including separate-connection approval,
  spend, evidence, and lineage checks.
- Three external Python/Node contract tests passed against a real loopback TCP
  sidecar: module imports, destination policy, spend, operator kill, outage, and
  malformed/error HTTP responses.
- All-target Clippy with warnings denied, rustfmt, and diff whitespace checks
  passed.
- Rust 1.85 all-target compatibility check passed on macOS.
- Self-drill and build-in example passed (allow, then killed).
- Cargo package verification passed; tests are included in the package.
- A separate temporary Rust consumer project compiled and exercised allow then
  killed using the standalone package as a dependency.

Checks used a fresh target directory to avoid relying on the checkout's stale
binary. No model credentials were needed. Tests used temporary stores.

## Additional hardening and runtime validation

A completed standard security scan of integration commit `102bc2b` identified
an MCP batch admission bypass conditional on a batch-capable downstream, and
unbounded pre-authentication sidecar worker allocation. These were fixed in the
subsequent hardening work: reject batches/invalid envelopes, forward canonical
parsed messages, bound workers at 64, impose a total five-second request read
deadline and write timeout, and reject ambiguous/truncated/oversized HTTP framing.

The harmless MCP reproduction ran a downstream marker after kill before the fix;
a fresh post-fix binary rejected the same batch without running the body.
Regressions cover invalid envelopes, duplicate-key interpretation, non-2xx MCP
sidecar responses, total deadline, saturation/recovery, framing and preservation
of existing files/live sockets. Updated executable help includes version output.

- The macOS suite now contains 62 tests (57 library, one CLI, four shared-store
  integration tests). The Linux container suite ran the 61-test suite before the
  final non-2xx regression and that added regression separately, all passing.
  Python-dependent Rust checks may skip when Python is absent in the builder.
- Fresh-binary external Python/Node contract tests passed on macOS.
- Docker release build with Rust 1.85 succeeded; nonroot self-drill passed.
- A separate Python container running as UID 10001 connected over the shared
  mode-0600 socket, observed allow, then operator kill, then persistent killed
  denial after restart. An unrelated lease remained operational.
- Rust 1.85 all-target check, all-target Clippy and formatting checks passed.

Developer onboarding now includes installation, runnable quick start, FAQ,
troubleshooting, operations, contributor checks and an unreleased changelog.
A source install into a temporary install root passed version output, self-drill
and all three Python/Node contracts. The documented quick start passed allow,
operator kill, expected sample exit 2, incident export, persistence after restart
and an unrelated new run, using isolated state and that installed release binary.

## Remaining gates and integration requirements

Native Windows runtime, remote CI and systemd host installation remain acceptance
gates. The service setup requires installing the binary at the unit's path,
creating its account, and configuring a nonempty token before startup. Real
application acceptance still requires exercising every actual dispatch path,
identity assignment and policy adapter on the selected target framework.

The crate is not published to crates.io, and source clients are not pip/npm
packages. This review does not publish a registry package, deploy a service, or
establish every third-party framework integration.

Default configuration is enabled and fail-closed, but consumers can explicitly
opt out. Enforce those settings in the trusted executor. The sidecar's shared
token authorizes all exposed HTTP routes, including policy changes; it is not a
per-agent identity or least-privilege client role. Keep the token, operator CLI,
store, and executor under trusted control.

Destination and spend metadata are caller-supplied. MCP argument discovery is a
convention, not actual network interception or authoritative cost accounting.
Custom tool names need explicit integration. Children do not inherit policies;
configure each child before running it. Kill is terminal for an ID, denies later
admissions, and does not cancel an action already running. Admission and side
effect execution are not one atomic operation.

SQLite evidence and JSONL are separate storage outputs. A crash or file-write
failure can leave one output ahead of the other; content IDs do not prove that
an operator with filesystem access could not alter the records. The bounded standard security scan and
regressions do not constitute security certification, a dependency-advisory audit
or proof of every third-party integration.
