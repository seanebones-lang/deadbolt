# Destination policy: explicit offline tools

This is **2.0 candidate behavior**, added after stable 1.0.3 and the
original 1.1.0-rc.1 archives, crate and MCP bundle. Those published binaries still
use the older name-based missing-destination rule. Updating documentation or the
Python client alone cannot change an installed engine's admission behavior.
Follow [the 2.0 migration guide](MIGRATING-V2.md) for the versioned candidate.

## Admission rule

When `dest_allow` is set, **all** tool names require a declared allowed host.
The only missing-destination exception is an operator-provided per-lease
`offline_tools` list. A present foreign host is always refused, including for an
offline-classified tool. Leaving `dest_allow` unset preserves open destination
policy; set narrow tool and destination lists deliberately.

```rust
gate.set_policy("run-001", deadbolt::PolicyPatch {
    tools_allow: Some(vec!["fetch_invoice".into(), "write_note".into()]),
    dest_allow: Some(vec!["api.example.com".into()]),
    offline_tools: Some(vec!["write_note".into()]),
    ..Default::default()
})?;
```

The trusted executor supplies the actual route's destination for
`fetch_invoice`. `write_note` may omit a destination because the operator has
reviewed its body as offline. Do not list `shell`, `python`, `curl` or arbitrary
code execution as offline: those bodies can initiate network activity.

Operator CLI (built from current source):

```sh
deadbolt policy --agent run-001 --tools fetch_invoice,write_note \
  --dest api.example.com --offline-tools write_note
deadbolt policy --agent run-001 --offline-tools ''
```

The second command clears all offline exceptions. Omitted fields preserve the
stored policy. The operator-only HTTP `POST /policy` accepts `offline_tools` as
an array of tool-name strings; `[]` clears it. Malformed offline fields are
refused. A scoped admission credential cannot configure these exceptions.
Each explicit list replacement, including a clear, is recorded as Observed
policy evidence. JSONL/text exports retain the replacement list; incident exports
include `offline_policy_changes` in order alongside the current policy. Omitted
fields do not claim an exception change. Records identify the lease and token-
authorized operation, not the human who held the operator credential.

## Migration and recovery

Stop protected dispatch and back up the SQLite store consistently before an
engine upgrade. The migration adds nullable `offline_tools`; existing leases
receive **no exceptions**. An existing destination policy therefore becomes
stricter: tools that previously omitted a host may now return
`purpose_exceeded`. Review real implementations before adding exceptions, and
configure each child’s real routes. Offline classifications persist across
restart. Unreleased source also checks all ancestor policies dynamically: a child
may omit a destination only when each configured destination policy on its
ancestry permits that tool offline. A child exception cannot widen a parent’s
rule. Published 2.0.0-rc.1 and 1.x engines do not enforce those ancestor ceilings.

Verify one allowed declared destination, one missing destination on a custom
tool, one foreign destination, one deliberately offline tool, and clearing the
offline list. Check actual effects, not just decision strings. Existing Rust
consumers using exhaustive `PolicyPatch` literals must add `offline_tools` or
use `..Default::default()`.

Older engines do not read or enforce this field and retain the old permissive
missing-destination behavior. Do not roll back a protected deployment expecting
equivalent enforcement; quiesce execution and review the recovery plan first.

## Declared policy is not egress containment

The gate compares a declared token with an allowlist. It does not inspect DNS,
redirects, IP routes, sockets, subprocesses, package proxies or an arbitrary
tool body's network traffic. A tool can still claim an allowed host and use
another one if your executor is dishonest or compromised. Derive destinations
from trusted execution parameters, constrain redirect behavior at the actual
HTTP client, and enforce network egress separately through reviewed OS/container
controls (for example Linux network namespaces and nftables). A host allowlist
alone is not evidence that a code-execution escape would have been prevented.

`tools_allow` and `offline_tools` are explicit operator decisions. Operator
controls belong outside model-facing capabilities; a naming heuristic is never
an authorization boundary.
