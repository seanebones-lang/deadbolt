# Sidecar protocol

HTTP/1.1 over a Unix domain socket. No TCP. `0.0.0.0` is refused. Default socket: `~/.deadbolt/deadbolt.sock`. Override with `DEADBOLT_SOCK` in clients. The socket is mode `0600`.

If `DEADBOLT_TOKEN` is set when `deadbolt serve` starts, every request must send header `X-Deadbolt-Token: <value>`. Missing or wrong token is HTTP 401 and does not admit, ensure, register, or status.

```http
HTTP/1.1 401
content-type: application/json

{"code":"unauthorized"}
```

Request body is JSON. Response body is JSON. `Content-Type: application/json`. `Connection: close`.

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

`decision` is `allow` or `deny`. `code` is present only on deny. Tokens: `killed`, `paused`, `purpose_exceeded`, `lease_expired`, `store_unavailable`, `no_lease`. HTTP status on a parsed admit is 200. The deny is in `decision`, not the status line.

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

`state` is `active`, `paused`, or `killed`.

## Errors

| HTTP | body `code` | meaning |
| --- | --- | --- |
| 401 | `unauthorized` | token missing or wrong; no admit |
| 400 | `bad_request` | missing JSON fields |
| 404 | `not_found` | unknown path |

`kill`, `pause`, `clip`, and `resume` are CLI only. They are not HTTP routes.
