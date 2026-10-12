# Sidecar protocol

HTTP/1.1 over a Unix socket or loopback TCP. `0.0.0.0`, `[::]`, and any non-loopback host are refused. Default socket: `~/.deadbolt/deadbolt.sock` (mode `0600`). TCP bind: `deadbolt serve --bind 127.0.0.1:PORT` or `[::1]:PORT`. `DEADBOLT_TOKEN` is required to start TCP. On a Unix socket operator authentication is optional only while the store has never issued a scoped credential (admission or supervisor). Clients treat `DEADBOLT_SOCK` as loopback HTTP when it is `127.0.0.1:PORT`, `[::1]:PORT`, or `http://127.0.0.1:PORT`. A Unix path still uses the socket.

Operator requests use `X-Deadbolt-Token: <value>` when configured. Workload requests instead use `X-Deadbolt-Admission: <secret>`; this header authorizes admission routes for its bound agent, including the unreleased MCP routes below. Mixed or duplicated authentication headers are refused. Unknown, revoked, expired or wrong-agent credentials return 401; workload control routes return 403. Neither status can grant admission. See [credential setup and migration](CREDENTIALS.md). TCP serve refuses to start when that token is missing. A missing or wrong header is HTTP 401 and does not admit, ensure, register, or status.

```http
HTTP/1.1 401
content-type: application/json

{"code":"unauthorized"}
```

Request body is JSON. Response body is JSON. `Content-Type: application/json`. `Connection: close`.

Send one request per connection with a valid `Content-Length` when there is a body. Chunked transfer, duplicate/invalid lengths, incomplete bodies and requests over 65,536 bytes are rejected by closing the connection. Complete requests have a five-second read deadline; writes have a five-second timeout. Each listener allows at most 64 active workers and closes excess connections. A close, timeout, malformed decision or non-2xx status must never grant admission. Token-valid parsed denials still use HTTP 200 and `decision: deny`.

## POST /admit

```json
{"agent_id":"shop-bot","tool":"shell"}
```

```json
{"decision":"allow"}
```

```json
{"decision":"deny","code":"killed"}
```

`decision` is `allow` or `deny`. `code` is present only on deny. Tokens: `killed`, `paused`, `purpose_exceeded`, `lease_expired`, `store_unavailable`, `no_lease`, `spend_cap`, `needs_human`; current unreleased source adds `deadline_exceeded`. Optional `dest` is a host token. If `dest_allow` is unset, `dest` is ignored. A present dest not on the list is `purpose_exceeded`. Current source also denies a missing destination for every tool unless the operator explicitly lists it in `offline_tools`; published engines still use the earlier name-based classification. See [destination policy and version boundary](DESTINATION-POLICY.md). HTTP status on a parsed admit is 200. The deny is in `decision`, not the status line.

## POST /admit-action (1.1.0-rc.1 prerelease)

A distinct endpoint for the exact version-1 envelope described in
[action approvals](ACTION-APPROVALS.md). It accepts an operator token or an
admission credential bound to the envelope's agent. It never grants or changes
operator approval. It checks current policy and consumes one matching unexpired,
unrevoked grant in one writer transaction. Responses use the existing admit
allow/deny shape. Missing, changed, expired or replayed grants deny `needs_human`.
Malformed/oversized/duplicate-key envelopes return 400; authentication returns
401/403; gate/storage failure returns 503. None authorize execution. An old
sidecar lacking this route must fail closed; never retry through `/admit`.

## MCP admission routes (unreleased 2.0 candidate)

`POST /admit-mcp` takes the same payload as `/admit`; `/admit-mcp-action`
takes the same exact envelope as `/admit-action`. Both require an existing live
lease with an explicitly set tool allowlist. Unset `tools_allow` denies
`policy_required`; an empty list denies `purpose_exceeded`. Liveness, ancestor
checks, policy and grant consumption share the same writer transaction. Missing
setup cannot refresh a TTL or consume a grant. Neither route creates leases.
Both accept the operator or bound admission credential; supervisor credentials
cannot admit. Disabled enforcement and legacy fail-open configuration cannot
permit MCP execution. An older sidecar lacking these routes fails closed; clients
must never retry through `/admit`, `/admit-action` or `/ensure`.

Ordinary `/admit` and `/admit-action` retain their trusted executor contract:
an unset tool list remains open. Use MCP-specific routes for MCP dispatch.

## POST /policy

Operator-only. An admission credential cannot access this route. Omitted fields stay as stored. Unset lists stay open.

```json
{"agent_id":"shop-bot","tools":["shell","read_file"],"dest":["api.stripe.com"],"spend_cap":5,"irreversible":["shell"]}
```

## POST /spend

```json
{"agent_id":"shop-bot","usd":1.5}
```

Crossing the cap pauses the lease. The body `code` is `spend_cap` or `ok`.

## POST /ensure

```json
{"agent_id":"shop-bot"}
```

```json
{"ok":true}
```

Failure: `{"ok":false,"code":"<token>"}`.

## POST /issue (current source, unreleased)

Operator-only. Requires a fresh ID and a fixed future integer Unix timestamp:

```json
{"agent_id":"my-run-002","hard_deadline_at":1791766800}
```

Supply your actual future timestamp; the value above is illustrative. Success
is `{"ok":true}`. Existing IDs, past/unsafe timestamps and malformed requests
return 400 `bad_request`; duplicate/unknown fields are refused. Workload
credentials cannot issue or extend a run. The deadline is immutable and inherited
by registered children; `/ensure` cannot change it. See [deadline setup and
limits](LEASE-DEADLINES.md). New admissions deny `deadline_exceeded` at the
fixed cutoff, before idle expiry. No in-flight cancellation is performed.

## POST /register_child

```json
{"parent":"P","child":"C","swarm_task_id":"task-1"}
```

`swarm_task_id` is optional. Success: `{"live":true}` or `{"live":false}` when the child is recorded killed. Failure: `{"ok":false,"code":"<token>"}`.

The live result describes the child's stored state at registration, not permission
to execute. It does not recheck all ancestors or apply their tool policies.
Current unreleased source rechecks the complete registered ancestor chain for
regular and exact-action admission, with either operator or workload credentials.
Inactive ancestors deny before TTL refresh or grant consumption; malformed,
missing or cyclic lineage fails closed with `store_unavailable`.

## GET /status

`GET /status` lists every lease. `GET /status?agent=shop-bot` lists one.

```json
{"agents":[{"agent_id":"shop-bot","state":"killed","parent_id":null,"expires_at":0,"hard_deadline_at":null,"supervision_root":null,"clips":[],"swarm_task_id":null}]}
```

Current source reports read-only effective `state`: `active`, `paused`, `killed`,
`expired`, or `deadline_exceeded`. Kill wins; otherwise the fixed deadline wins
over idle expiry, including for paused leases. Status never rewrites the lease
or records an admission. It describes that lease's own state and clocks, not a
complete tool-policy or ancestor-liveness admission decision. Check fresh admission before
every effect. Published 2.0.0-rc.1 and 1.x binaries still report stored state.

The HTTP enumeration contract remains an empty `agents` list for a missing ID.
In current source the operator CLI `deadbolt status --agent <missing>` exits 1
with `deadbolt:not_found`; an empty unfiltered inventory exits 0. A successful
status query, including one reporting expiry, is not permission to execute.

## MCP admission denials (unreleased source)

Well-formed denied `tools/call` requests receive a normal JSON-RPC result with
`isError: true`, rather than a protocol error. For example:

```json
{"jsonrpc":"2.0","id":7,"result":{"isError":true,"content":[{"type":"text","text":"DeadBolt denied: lease_expired. Do not retry this run."}],"_meta":{"io.github.seanebones-lang/deadbolt":{"code":"lease_expired","retry":"never"}}}}
```

The namespaced metadata contains stable `code` and `retry` tokens. Text also
contains the code and guidance so clients can show it to the model without
exposing metadata. Terminal kill/idle expiry/deadline denials say `never`;
pause says `after_operator_resume`; storage failure says `after_service_recovery`;
other denials require `after_operator_intervention`. No automatic retry occurs.
Unknown sidecar reason strings become `store_unavailable`. No arguments,
allowlists, credentials or renewal instructions are reflected. This response
does not forward the denied call or weaken its admission decision.

Malformed envelopes and tool-call parameters remain JSON-RPC protocol errors.
This follows [MCP's tool error distinction](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#error-handling).
The published 2.0.0-rc.1 and 1.x engines still use JSON-RPC errors for denials;
these source changes require a separately validated new release.

## Errors

| HTTP | body `code` | meaning |
| --- | --- | --- |
| 401 | `unauthorized` | token missing or wrong; no admit |
| 400 | `bad_request` | missing JSON fields |
| 404 | `not_found` | unknown path |

`kill`, `pause`, `clip`, and `resume` are CLI only. They are not HTTP routes.

## Supervisor-only renewal (unreleased source)

`POST /issue-supervised` is operator-only fresh-run issuance with `agent_id` and
an optional integral future `hard_deadline_at`. Duplicate/unknown fields and
null/string/fraction deadlines are refused. Existing IDs cannot be converted or
extended. Status adds nullable `supervision_root`; children inherit it.

`POST /renew` accepts only `X-Deadbolt-Supervisor` and a strict `{"agent_id":"root"}`
body. This root-bound credential maintains only still-live matching root/children;
it cannot admit, change policy, unpause, revive expiry/kill or slide a fixed
deadline. Success is `{"ok":true}`; every other response must stop continuation.
No operator/workload fallback exists. Supervisor headers on other routes get 403;
mixed/duplicate headers get 401. See [full setup, status codes and migration](SUPERVISION.md).
