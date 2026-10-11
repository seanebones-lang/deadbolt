# Security

Report a suspected vulnerability through
[GitHub's private vulnerability reporting](https://github.com/seanebones-lang/deadbolt/security/advisories/new).
Private reporting is enabled for this repository. Include the source revision,
affected integration, expected security property and a minimal redacted
reproduction. Keep credentials, customer data and unfixed vulnerability details
out of public issues. For ordinary bugs, use [the issue forms](https://github.com/seanebones-lang/deadbolt/issues/new/choose).

Deadbolt denies later admissions for an agent and its registered descendants after kill. The trusted executor must check admission before every tool or spawn dispatch. Already-running work, direct dispatch outside the executor, and arbitrary untrusted code are outside this boundary.

Unix sockets are mode `0600`. TCP listens only on loopback and requires a nonempty token. A missing or wrong `X-Deadbolt-Token` is HTTP 401 and does not admit. The operator token authorizes all exposed HTTP routes. Admission credentials use `X-Deadbolt-Admission`, are bound to one agent and fixed expiry, and authorize only `POST /admit` and `POST /admit-action`. Once any admission credential has been issued, anonymous Unix HTTP authority is disabled permanently in that store; removing the header cannot restore operator access. See [credentials](docs/CREDENTIALS.md) for issuance, revocation and migration. Protect the token, database, evidence files, CLI and executor from model-controlled code.

Each listener caps active workers at 64 and closes excess connections. Complete HTTP requests are limited to 65,536 bytes and a five-second read deadline; writes have a five-second timeout. Saturation denies availability rather than granting execution. Use host process limits where local hostile accounts are in scope.

The stdio MCP proxy accepts individual JSON-RPC 2.0 objects and rejects batches. It gates routed `tools/call` and forwards canonical parsed JSON. Other methods and downstream startup are outside the tool gate. Downstream servers are trusted processes: they inherit the proxy's OS permissions and environment except the two DeadBolt bearer-token variables and `DEADBOLT_ACTION_GRANTS` and are not sandboxed by Deadbolt.
