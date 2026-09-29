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
- Updated build: 54 Rust tests passed, including separate-connection approval,
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

## Remaining gates and integration requirements

Linux and Windows runtime results are not established by this local macOS run.
The expanded CI must run and pass. Docker runtime was unavailable locally;
container build, shared-socket access as UID 10001, volume persistence, and
service deployment require runtime verification. The service setup also requires
installing the binary at the unit's path, creating its account, and configuring
a nonempty token before startup.

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
an operator with filesystem access could not alter the records. This review is
an integration/correctness pass, not a claim of adversarial security certification.
