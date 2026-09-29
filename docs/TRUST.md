# Trust

Deadbolt denies the next tool, MCP, or spawn call. It does not shut down a model, discover a shadow agent, or halt a fleet. It is not a model tool.

## Boundary

The trust boundary is the process that can run a tool without calling admit. Inside that boundary, the body cannot start unless admit returned allow. Outside it, Deadbolt only denies callers that ask.

Enforced: build-in (`Deadbolt::admit` in the executor), `deadbolt mcp-proxy`, and any in-executor hook that refuses to dispatch the tool on deny. The tool body cannot run without admit.

Cooperative: `examples/deadbolt_client.py` and `examples/deadbolt_client.js`. They ask serve, then the caller is supposed to stop. A compromised agent that skips admit is outside the trust boundary. The client cannot prove the tool did not run.

## Fail closed

Default `fail_closed=true`. A missing store, a down sidecar, a timeout, an empty body, or HTTP 5xx is deny `store_unavailable`. The clients return that object. They do not raise into a caller that might catch the error and run the tool.

## Lease

`ensure` issues the lease. Default TTL is 60 seconds. Every admit rechecks it. Expiry is deny `lease_expired`.

A live admit, allow or deny, renews the TTL inside `evaluate` before the deny return. `killed` and `lease_expired` return before that update and are not slid. Silence does not renew.

## Agent id

The agent id is a claim. `DEADBOLT_TOKEN` authenticates the caller to serve. It does not authenticate the agent. Whoever holds the token can assert any id. The binding that matters is the executor or the proxy assigning the id. The model does not.

## TOCTOU

Admit, then the tool, is a race. Admit immediately before the body. Do not admit once per session and reuse the allow.

## Not claimed

No frontier shutdown. No shadow-agent discovery. No fleet halt. No proof that a cooperative caller obeyed. Unix socket mode `0600`, or loopback TCP with a required token. `0.0.0.0` is refused. Deadbolt does not contain another lab's agents on the public internet.

## Blast radius

Unset policy is open. A set `tools_allow` denies `purpose_exceeded`. `dest_allow` denies `purpose_exceeded` when dest is present and not on the list, or when the tool is network-class (`http`, `fetch`, `browser`, `web_search`) and dest is missing. A local tool with no host is not denied for `dest_allow` alone. The deny is inferred, with the attempt cid in premises. Crossing `spend_cap_usd` pauses the lease. The next admit is `spend_cap`, not a generic pause. An irreversible tool is `needs_human` until `deadbolt approve --agent ID --tool TOOL` grants one shot. `deadbolt resume --agent ID --approve TOOL` is the same shot after a pause. There is no model-facing approve tool. A live admit, allow or deny, renews the TTL in `evaluate`. Silence does not. An expired lease is not slid.

Harness is one consumer of this crate. This repository does not include it.
