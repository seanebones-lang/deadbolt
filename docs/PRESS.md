# Deadbolt

28 September 2026

Capability is compounding. Containment is lagging.

Deadbolt is fail-closed execution: leased tools, blast radius, human stop, evidence for a lawyer.

It does not shut down GPT. Sidecar down = deny. A bolt-on client is cooperative. `mcp-proxy` and build-in are enforced.

This is your agent. It is not a lab research swarm on the public internet.

## Install

```bash
cargo install n11-deadbolt --bin deadbolt
```

The binary name stays `deadbolt`. The crate name is `n11-deadbolt`.

## Two modes

Build-in. Depend on crate `n11-deadbolt` and call the gate in process:

```rust
use deadbolt::Deadbolt;
```

The tool body does not run unless `admit` allows it.

Bolt-on. `deadbolt serve` on a Unix socket, or loopback TCP with a token. `deadbolt mcp-proxy` admits `tools/call` before the child runs it. A client that skips admit is outside the trust boundary. If the sidecar is down, the answer is deny.

## Measured live fire

2026-09-28. Not a benchmark. No customer count.

Agent `h-59378-18d9acf67c5ecdf8`:

- `write_file` → `ok`
- `shell` → `needs_human` (body did not run)
- `approve` then `shell` → `ok`, body `LIVE-POLICY-SHELL`
- second `shell` → `needs_human`
- `kill` then `read_file` → `killed`

Agent `h-60207-18d9ad7dce9803c8` wrote `LIVE-POLICY-B2`. That write was allow. Killing the first agent did not block it.

## Links

- Repository: https://github.com/seanebones-lang/deadbolt
- Release: https://github.com/seanebones-lang/deadbolt/releases/tag/v0.1.1-product
- Page: https://www.mothership-ai.com/deadbolt/

Apache-2.0. NextEleven LLC. nextelevenstudios@gmail.com
