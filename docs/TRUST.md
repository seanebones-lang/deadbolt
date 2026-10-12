# Trust

Deadbolt denies the next tool, MCP, or spawn call. It does not shut down a model, discover a shadow agent, or halt a fleet. It is not a model tool.

## Boundary

The trust boundary is the process that can run a tool without calling admit. Inside that boundary, the body cannot start unless admit returned allow. Outside it, Deadbolt only denies callers that ask.

Enforcement belongs to the dispatcher: build-in works when every tool path checks `Deadbolt::admit` and stops on deny. The library cannot intercept an arbitrary tool body. `deadbolt mcp-proxy` gates routed `tools/call`; other MCP methods and server startup are outside that gate. Neither mode cancels a body already running.

Cooperative: `examples/deadbolt_client.py` and `examples/deadbolt_client.js`. They ask serve, then the caller is supposed to stop. A compromised agent that skips admit is outside the trust boundary. The client cannot prove the tool did not run.

## Fail closed

Default `fail_closed=true`. A missing store, a down sidecar, a timeout, an empty body, or HTTP 5xx is deny `store_unavailable`. The clients return that object. They do not raise into a caller that might catch the error and run the tool.

Keep Rust configuration `enabled=true` and `fail_closed=true` for enforcement.
`enabled=false` deliberately returns allow without opening storage;
`fail_closed=false` deliberately permits admission when storage is unavailable.
These operator settings weaken the gate and must remain outside model-controlled access.

Current source caps client/MCP sidecar HTTP responses at 1 MiB; oversized
responses deny. Node applies a five-second total request deadline, including
partial responses; Python uses five-second I/O timeouts and always closes its
connection. These changes are not present in v1.0.3 downloads.

Current-source stdio MCP frames are capped at 16 MiB. On client EOF the proxy
closes child input and drains final output for at most one second. It allows
one further second for the OS child exit status before killing/waiting the
immediate child. Child EOF or a framing/read failure ends the
proxy even when client input remains open; a completed child failure returns a
proxy error. Two read-only threads use a bounded
two-frame queue; only the coordinating thread admits calls and writes output.
Descendants are not terminated; the executor owns process-tree cleanup. Readers
blocked in OS input may remain until their pipes close, but cannot admit or
forward after the proxy returns. Pipe writes still depend on the host/child
consuming input; this is not general process confinement or a write deadline.
Use the CLI as a dedicated stdio process. Embedded `mcp_proxy` owns one session
of process-global stdin/stdout; do not restart it in the same process while an
old stdin reader is still blocked. Use a fresh proxy process for reconnection.
Closing the proxy is distinct from an operator kill,
which still denies subsequent admissions rather than cancelling running bodies.

## Lease

`ensure` issues the lease. Default TTL is 60 seconds. Every admit rechecks it. Expiry is deny `lease_expired`.

A live activity-mode admit, allow or policy deny, renews the requested lease's idle TTL before the policy deny return. Silence does not renew. A stream of policy-denied requests can keep an idle-only lease active. Scoped admission credentials have their own fixed expiry, which activity cannot renew.

**Published RC2:** every admission also checks the complete registered ancestor chain in the same SQLite writer transaction. A killed, paused, idle-expired or deadline-exceeded ancestor denies before the requested lease is refreshed or any approval is consumed. Missing, cyclic or inconsistent lineage fails closed. Descendant activity never renews its ancestors. Published RC2 also applies every ancestor’s current tool allowlist, destination/offline rules, clips and review requirements. A child cannot loosen an ancestor’s policy; broad and exact grants remain bound to the admitted child. Every MCP child still requires its own explicit tool allowlist. Pause does not rewrite descendant rows or cancel work already admitted. Status describes each lease's own state and clocks, so an active child can still be denied by an ancestor. Published 2.0.0-rc.1 and 1.x engines do not provide this complete-chain check.

**2.0 candidate:** operator `issue` creates a fresh run with an immutable `hard_deadline_at`. New admissions at or after that timestamp deny `deadline_exceeded` before idle TTL evaluation or renewal. Child leases inherit the deadline; restart preserves it; `ensure`, resume and policy cannot extend it. Killed leases still deny `killed`. [Deadline setup, migration, clock and cancellation limits](LEASE-DEADLINES.md) distinguish this guarantee from an absolute runtime limit on already admitted work. Published 1.x engines and plugin bundles do not include it. Optional [supervisor-only idle renewal](SUPERVISION.md) is another 2.0 candidate addition with a separate root-bound credential; its heartbeat cannot extend that deadline.

## Agent id

The operator token `DEADBOLT_TOKEN` authenticates full HTTP authority and can
assert any agent ID. Published RC2 requires this token for Unix and TCP
sidecar startup by default. Explicit `--legacy-unauthenticated-unix` opts into
older socket-possession authority only until any scoped credential has been
issued; a configured token always wins, and TCP never becomes tokenless.
[HTTP authentication](HTTP-AUTHENTICATION.md) covers migration and tests. The current candidate also accepts single-agent admission
credentials: their stored hash, agent binding, fixed expiry and revocation are
checked in the same writer transaction as lease/policy evaluation. These
credentials authorize only admission routes, including the RC2 MCP routes, not setup or control routes. See
[credential setup](CREDENTIALS.md), including migration from anonymous Unix
access. In either mode the trusted executor assigns the ID and tool identity;
the model does not. Scoped admission always fails closed, even when the trusted
Rust host has disabled enforcement or configured legacy fail-open behavior.

## TOCTOU

Admit, then the tool, is a race. Admit immediately before the body. Do not admit once per session and reuse the allow.

## Not claimed

No frontier shutdown. No shadow-agent discovery. No fleet halt. No proof that a cooperative caller obeyed. Unix socket mode `0600`, or loopback TCP with a required token. `0.0.0.0` is refused. Deadbolt does not contain another lab's agents on the public internet.

## Blast radius

Unset policy is open. A set `tools_allow` denies `purpose_exceeded` for tools outside the list.

**Published RC2 MCP setup:** raw proxy runtime does not create a lease.
Every MCP tool admission requires an existing live lease and an explicitly set
tool allowlist, including exact grants and scoped calls. Missing policy denies
`policy_required` before target TTL renewal or approval consumption. An empty
list denies all tools. Ordinary trusted Rust/HTTP admission retains the open
unset-policy behavior above. MCP enforcement always fails closed and its new
sidecar routes cannot downgrade to older admission routes. Downstream startup
and non-tool methods are still outside the tool-call gate.

**Published RC2 destination policy:** when `dest_allow` is set, every tool must declare an allowed destination unless the operator explicitly lists it in the per-lease `offline_tools`. A present foreign destination is denied even for an offline-classified tool. Empty `offline_tools` clears the exceptions. This replaces name-based classification; arbitrary names such as `shell`, `python`, `curl` and custom MCP tools no longer receive an implicit missing-destination exception. Existing destination policies migrate to no offline exceptions. Published RC2 additionally evaluates each ancestor’s destination rule: an offline exception on a child cannot override a missing exception on its parent. Configure each child’s actual routes; older published engines apply only the child’s own policy.

The new behavior is not in stable 1.0.3 or the original 1.1.0-rc.1 release. Review the [destination migration guide](DESTINATION-POLICY.md) before upgrading. This is admission policy over executor-declared tokens, **not network egress enforcement**: it cannot verify DNS, redirects, sockets, subprocess networking or a dishonest caller's declared host. Derive destinations from trusted execution parameters and pair sensitive workloads with OS/container egress controls. Never classify general code execution as offline merely because its tool name sounds local.

In RC2, a cap covers the sum of the budget owner’s own reported spending and all registered descendants, including terminal descendants. Each identity retains its own ledger; aggregation counts it once. Reaching a cap pauses that budget owner and the next descendant admission returns `spend_cap`. Resume alone cannot reopen an exhausted budget. `SpendAdded.paused` means this report reached at least one cap on its ancestry; evidence lists the paused budget owners. Published 2.0.0-rc.1 and 1.x caps count only the individual lease. Spend is reported by trusted infrastructure through `spend_add` (the MCP proxy's argument-number discovery is only a convention). It is not provider billing measurement, prepaid reservation or an upper bound on in-flight charges. Scoped workload credentials cannot report spend. Integrate authoritative provider usage in the trusted supervisor and reconcile lag; do not accept an agent's proposed cost as actual usage.

An irreversible tool is `needs_human` until the operator grants one broad agent/tool shot. There is no model-facing approve tool. Operator controls must be excluded by capability/credential separation, not by matching tool names; `is_shutdown_tool` is only a diagnostic heuristic and cannot establish safety for arbitrary aliases.


Harness is one consumer of this crate. This repository does not include it.

## Exact-action source candidate

The opt-in [exact-action protocol](ACTION-APPROVALS.md) binds reviewed arguments,
run, tool, destination, nonce and deadline. Its new endpoint cannot fall back to
legacy admission. Approval enables exact-only policy for that run/tool;
nonces persist after use/revocation. Existing unconfigured integrations retain
their behavior. This is available in 1.1.0-rc.1. Older binaries cannot enforce these new
requirements; quiesce protected dispatch before any rollback.

## Optional supervised idle expiry (2.0 candidate)

[Supervisor-only renewal](SUPERVISION.md) is the default for fresh RC2
root runs. RC1 requires explicit supervisor setup; existing activity-mode runs
retain their original renewal mode. Admissions do not refresh those leases; a
separate root-bound credential maintains only still-live members. A fixed deadline remains immutable
and wins even during heartbeat activity. The explicit Python renewal call never
uses operator/workload environment authentication. Published 1.x artifacts
do not include this feature; source, migration and release gates apply. Running
work still needs executor-owned cancellation and OS isolation where appropriate.

## Downstream environment hygiene

Published RC2 removes `DEADBOLT_TOKEN`, `DEADBOLT_ADMISSION_TOKEN` and
`DEADBOLT_ACTION_GRANTS` before starting the downstream MCP server. Unreleased
source additionally removes `DEADBOLT_DB`, `DEADBOLT_EVENTS` and `DEADBOLT_SOCK`.
Other application variables survive. Removing these locations does not prevent
a same-user process from finding or changing operator files; use separate OS
identities for that boundary. `tools/list` remains discovery passthrough, not an
execution authorization or a statement that every listed tool is allowed.
