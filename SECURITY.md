# Security

Stable 2.0.0 and 2.0.1 create new Unix core/Witness state privately and refuse
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

Unix sockets are mode `0600`. TCP listens only on loopback. Stable 2.0.0 and 2.0.1 require a nonempty operator token for Unix and TCP sidecars by default; published rc.1/1.x Unix listeners retain their older socket-possession behavior. The Unix-only `--legacy-unauthenticated-unix` compatibility flag explicitly opts into that older authority and cannot weaken a configured token or tokenless-TCP refusal. A missing or wrong `X-Deadbolt-Token` is HTTP 401 and does not admit. The operator token authorizes all exposed HTTP routes. Admission credentials use `X-Deadbolt-Admission`, are bound to one agent and fixed expiry, and authorize only `POST /admit` and `POST /admit-action`. Once any admission credential has been issued, anonymous Unix HTTP authority is disabled permanently in that store; removing the header cannot restore operator access. See [credentials](docs/CREDENTIALS.md) for issuance, revocation and migration. Protect the token, database, evidence files, CLI and executor from model-controlled code.

Each listener caps active workers at 64 and closes excess connections. Complete HTTP requests are limited to 65,536 bytes and a five-second read deadline; writes have a five-second timeout. Saturation denies availability rather than granting execution. Use host process limits where local hostile accounts are in scope.

The stdio MCP proxy accepts individual JSON-RPC 2.0 objects and rejects batches. It gates routed `tools/call` and forwards canonical parsed JSON. Other methods and downstream startup are outside the tool gate. Downstream servers are trusted processes: they inherit the proxy's OS permissions and environment except the two DeadBolt bearer-token variables and `DEADBOLT_ACTION_GRANTS`. Stable 2.0.0 and 2.0.1 additionally remove `DEADBOLT_DB`, `DEADBOLT_EVENTS` and `DEADBOLT_SOCK`; published RC2 still passes those location variables. Downstream servers are not sandboxed by DeadBolt.

## Bundled SQLite dependency update

Engine 2.0.2 native releases and source replace affected bundled SQLite for
[RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459.html).
The advisory concerns opening a database together with an attacker-influenced
WAL file. Both core and Witness use SQLite. Unix private-state checks reduce
who can provide those files; they do not repair the vulnerable SQLite parser.
Same-user/operator state and Windows ACLs retain their documented limits.
Do not discard WAL files or reset stores as a workaround. Previously published
artifacts keep their original dependencies until a new release is installed.
