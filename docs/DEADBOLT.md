# Deadbolt system overview

This page describes standalone Deadbolt v1. Historical Harness integration
behavior is not part of the library's contract. Harness is one consumer and is
not included in this repository.

## Architecture

```text
Trusted executor assigns identity and prepares policy
    -> admission through Rust library, HTTP client or stdio MCP proxy
    -> explicit allow: executor starts the protected body
    -> every other result: executor refuses the body

Operator controls policy, approval and revocation
    -> local SQLite state and structured JSONL evidence
```

The Rust library requires no sidecar. The sidecar serves a mode-0600 Unix socket
or token-required loopback TCP. Python/Node clients ask the sidecar; their caller
must honor the result. The stdio MCP proxy admits individual routed `tools/call`
messages before forwarding them to a trusted server. Other protocol methods and
server startup are outside that tool gate.

## Identity and operator control

The trusted executor assigns identities, registers children before dispatch and
sets each child's policy. Kill is terminal for that identity in the same store,
including its registered descendants. Re-ensuring an identity does not restore
permission. Kill blocks later admissions; it does not terminate a process or
cancel already-running tasks. Cancellation requires a separate executor mechanism.

Live admission refreshes the lease; silence does not. Expired identities are not
revived by admission. Policy lists are open when unset. One-shot approval is
consumed by admission, not by successful completion of the body. Spend and
host context are supplied by the caller.

## State and evidence

For enforcement, keep Rust configuration `enabled = true` and
`fail_closed = true`. Disabling the gate admits without storage; selecting
fail-open permits admission when storage is unavailable. These are explicit
operator configurations, not production enforcement defaults.

SQLite holds leases, policy, approvals, lineage and decision evidence. JSONL
records use observed/inferred classes, premises and content hashes. Operator
exports provide structured records, not generated summaries or proof that a
cooperative caller obeyed. The files are operator-writable; hashes do not make
them tamper-proof. Back up quiescent state using the documented operations procedure.

## Authoritative guides

- [Installation](INSTALL.md) and [quick start](QUICKSTART.md)
- [Integration](INTEGRATION.md) and [HTTP protocol](PROTOCOL.md)
- [Compatibility](COMPATIBILITY.md) and [trust boundary](TRUST.md)
- [Operations and recovery](OPERATIONS.md) and [incident evidence](INCIDENT.md)
- [Evaluator brief](EVALUATOR.md), [demonstration](DEMO.md) and [pilot](PILOT.md)

The [published release](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.2)
identifies the exact released source, downloads, tested platforms and acceptance.
