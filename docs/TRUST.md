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

A live admit, allow or deny, renews the TTL inside `evaluate` before the deny return. `killed` and `lease_expired` return before that update and are not slid. Silence does not renew.

## Agent id

The operator token `DEADBOLT_TOKEN` authenticates full HTTP authority and can
assert any agent ID. The current candidate also accepts single-agent admission
credentials: their stored hash, agent binding, fixed expiry and revocation are
checked in the same writer transaction as lease/policy evaluation. These
credentials authorize only `POST /admit` and `POST /admit-action`, not setup or control routes. See
[credential setup](CREDENTIALS.md), including migration from anonymous Unix
access. In either mode the trusted executor assigns the ID and tool identity;
the model does not. Scoped admission always fails closed, even when the trusted
Rust host has disabled enforcement or configured legacy fail-open behavior.

## TOCTOU

Admit, then the tool, is a race. Admit immediately before the body. Do not admit once per session and reuse the allow.

## Not claimed

No frontier shutdown. No shadow-agent discovery. No fleet halt. No proof that a cooperative caller obeyed. Unix socket mode `0600`, or loopback TCP with a required token. `0.0.0.0` is refused. Deadbolt does not contain another lab's agents on the public internet.

## Blast radius

Unset policy is open. A set `tools_allow` denies `purpose_exceeded`. `dest_allow` denies `purpose_exceeded` when dest is present and not on the list, or when the tool is network-class (`http`, `fetch`, `browser`, `web_search`) and dest is missing. A local tool with no host is not denied for `dest_allow` alone. The deny is inferred, with the attempt cid in premises. Crossing `spend_cap_usd` pauses the lease. The next admit is `spend_cap`, not a generic pause. An irreversible tool is `needs_human` until `deadbolt approve --agent ID --tool TOOL` grants one shot. `deadbolt resume --agent ID --approve TOOL` is the same shot after a pause. There is no model-facing approve tool. A live admit, allow or deny, renews the TTL in `evaluate`. Silence does not. An expired lease is not slid.

Harness is one consumer of this crate. This repository does not include it.

## Exact-action source candidate

The opt-in [exact-action protocol](ACTION-APPROVALS.md) binds reviewed arguments,
run, tool, destination, nonce and deadline. Its new endpoint cannot fall back to
legacy admission. Approval enables exact-only policy for that run/tool;
nonces persist after use/revocation. Existing unconfigured integrations retain
their behavior. This is unreleased. Older binaries cannot enforce these new
requirements; quiesce protected dispatch before any rollback.
