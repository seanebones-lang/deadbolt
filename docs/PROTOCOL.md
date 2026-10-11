# Sidecar protocol

HTTP/1.1 over a Unix socket or loopback TCP. `0.0.0.0`, `[::]`, and any non-loopback host are refused. Default socket: `~/.deadbolt/deadbolt.sock` (mode `0600`). TCP bind: `deadbolt serve --bind 127.0.0.1:PORT` or `[::1]:PORT`. `DEADBOLT_TOKEN` is required to start TCP. On a Unix socket operator authentication is optional only while the store has never issued an admission credential. Clients treat `DEADBOLT_SOCK` as loopback HTTP when it is `127.0.0.1:PORT`, `[::1]:PORT`, or `http://127.0.0.1:PORT`. A Unix path still uses the socket.

Operator requests use `X-Deadbolt-Token: <value>` when configured. Workload requests instead use `X-Deadbolt-Admission: <secret>`; this header authorizes only `POST /admit` and `POST /admit-action` for its bound agent. Mixed or duplicated authentication headers are refused. Unknown, revoked, expired or wrong-agent credentials return 401; workload control routes return 403. Neither status can grant admission. See [credential setup and migration](CREDENTIALS.md). TCP serve refuses to start when that token is missing. A missing or wrong header is HTTP 401 and does not admit, ensure, register, or status.

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

`decision` is `allow` or `deny`. `code` is present only on deny. Tokens: `killed`, `paused`, `purpose_exceeded`, `lease_expired`, `store_unavailable`, `no_lease`, `spend_cap`, `needs_human`. Optional `dest` is a host token. If `dest_allow` is unset, `dest` is ignored. A present dest not on the list is `purpose_exceeded`. A missing dest is `purpose_exceeded` only for a network-class tool. HTTP status on a parsed admit is 200. The deny is in `decision`, not the status line.

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

## POST /register_child

```json
{"parent":"P","child":"C","swarm_task_id":"task-1"}
```

`swarm_task_id` is optional. Success: `{"live":true}` or `{"live":false}` when the child is recorded killed. Failure: `{"ok":false,"code":"<token>"}`.

## GET /status

`GET /status` lists every lease. `GET /status?agent=shop-bot` lists one.

```json
{"agents":[{"agent_id":"shop-bot","state":"active","parent_id":null,"expires_at":0,"clips":[],"swarm_task_id":null}]}
```

`state` is `active`, `paused`, or `killed`. Expiration is checked at admission; a stored active row may already have a past `expires_at`.

## Errors

| HTTP | body `code` | meaning |
| --- | --- | --- |
| 401 | `unauthorized` | token missing or wrong; no admit |
| 400 | `bad_request` | missing JSON fields |
| 404 | `not_found` | unknown path |

`kill`, `pause`, `clip`, and `resume` are CLI only. They are not HTTP routes.
