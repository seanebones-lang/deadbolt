# Security

Report a vulnerability through GitHub Security Advisories on `seanebones-lang/deadbolt`. Do not open a public issue for an unfixed socket-auth or store bug.

Deadbolt denies later admissions for an agent and its registered descendants after kill. The trusted executor must check admission before every tool or spawn dispatch. Already-running work, direct dispatch outside the executor, and arbitrary untrusted code are outside this boundary.

Unix sockets are mode `0600`. TCP listens only on loopback and requires a nonempty token. A missing or wrong `X-Deadbolt-Token` is HTTP 401 and does not admit. A token authorizes all exposed HTTP routes; it is not a per-agent role. Protect the token, database, evidence files, CLI and executor from model-controlled code.

Each listener caps active workers at 64 and closes excess connections. Complete HTTP requests are limited to 65,536 bytes and a five-second read deadline; writes have a five-second timeout. Saturation denies availability rather than granting execution. Use host process limits where local hostile accounts are in scope.

The stdio MCP proxy accepts individual JSON-RPC 2.0 objects and rejects batches. It gates routed `tools/call` and forwards canonical parsed JSON. Other methods and downstream startup are outside the tool gate. Downstream servers are trusted processes: they inherit the proxy's OS permissions and environment and are not sandboxed by Deadbolt.
