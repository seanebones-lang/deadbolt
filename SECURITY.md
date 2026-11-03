# Security

Unreleased source creates new Unix core/Witness state privately and refuses
unsafe existing files/sidecars before SQLite writes. Older stores need an
explicit quiesced permission migration; Windows ACLs remain operator-managed.
See [private state](docs/PRIVATE-STATE.md). These checks do not isolate code
with the same OS identity or access to operator state/administration.

Current `main` independently gates every MCP `tools/call` request. Admission for a completed call does not authorize a later or secondary call; each invocation is evaluated again before forwarding. The regression test covers a denied follow-up call and verifies it is not sent to the downstream server. This boundary applies to routed tool calls through the proxy; other MCP methods, downstream process behavior, and direct dispatch outside the proxy remain outside it.

Report a suspected vulnerability through
[GitHub's private vulnerability reporting](https://github.com/seanebones-lang/deadbolt/security/advisories/new).
Private reporting is enabled for this repository. Include the source revision,
affected integration, expected security property and a minimal redacted
reproduction. Keep credentials, customer data and unfixed vulnerability details
out of public issues. For ordinary bugs, use [the issue forms](https://github.com/seanebones-lang/deadbolt/issues/new/choose).

Deadbolt denies later admissions for an agent and its registered descendants after kill. The trusted executor must check admission before every tool or spawn dispatch. Already-running work, direct dispatch outside the executor, and arbitrary untrusted code are outside this boundary.

Unix sockets are mode `0600`. TCP listens only on loopback. Unreleased source requires a nonempty operator token for Unix and TCP sidecars by default; published rc.1/1.x Unix listeners retain their older socket-possession behavior. The Unix-only `--legacy-unauthenticated-unix` compatibility flag explicitly opts into that older authority and cannot weaken a configured token or tokenless-TCP refusal. A missing or wrong `X-Deadbolt-Token` is HTTP 401 and does not admit. The operator token authorizes all exposed HTTP routes. Admission credentials use `X-Deadbolt-Admission`, are bound to one agent and fixed expiry, and authorize only `POST /admit` and `POST /admit-action`. Once any admission credential has been issued, anonymous Unix HTTP authority is disabled permanently in that store; removing the header cannot restore operator access. See [credentials](docs/CREDENTIALS.md) for issuance, revocation and migration. Protect the token, database, evidence files, CLI and executor from model-controlled code.

Each listener caps active workers at 64 and closes excess connections. Complete HTTP requests are limited to 65,536 bytes and a five-second read deadline; writes have a five-second timeout. Saturation denies availability rather than granting execution. Use host process limits where local hostile accounts are in scope.

The stdio MCP proxy accepts individual JSON-RPC 2.0 objects and rejects batches. It gates routed `tools/call` and forwards canonical parsed JSON. Other methods and downstream startup are outside the tool gate. Downstream servers are trusted processes: they inherit the proxy's OS permissions and environment except the two DeadBolt bearer-token variables and `DEADBOLT_ACTION_GRANTS`. Unreleased source additionally removes `DEADBOLT_DB`, `DEADBOLT_EVENTS` and `DEADBOLT_SOCK`; published RC2 still passes those location variables. Downstream servers are not sandboxed by DeadBolt.
