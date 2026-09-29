# Security

Report a vulnerability through GitHub Security Advisories on `seanebones-lang/deadbolt`. Do not open a public issue for an unfixed socket-auth or store bug.

Deadbolt does not shut down frontier models. It does not discover shadow agents. It does not halt a fleet. It denies the next tool, MCP, or spawn call for one `agent_id` and that agent's children after `kill`. A missing or wrong `X-Deadbolt-Token` is HTTP 401 and does not admit. The socket is mode `0600` and is not a TCP listener.
