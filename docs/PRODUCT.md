# Deadbolt

Capability is compounding. Containment is lagging.

Deadbolt is a local execution gate: leased tool access, explicit policy, operator stop and structured incident evidence.

It does not shut down GPT. Sidecar down = deny. A bolt-on client is cooperative. `mcp-proxy` gates routed tool calls; build-in requires a dispatcher that stops on deny.

This is your agent. It is not a lab research swarm on the public internet.

## Install

```bash
cargo install --git https://github.com/seanebones-lang/deadbolt.git --locked --bin deadbolt
```

The binary name stays `deadbolt`. The crate name is `n11-deadbolt`.

## Two modes

Build-in. Depend on crate `n11-deadbolt` and call the gate in process:

```rust
use deadbolt::Deadbolt;
```

The tool body does not run unless `admit` allows it.

Bolt-on. `deadbolt serve` on a Unix socket, or loopback TCP with `DEADBOLT_TOKEN`. `deadbolt mcp-proxy` admits `tools/call` before the child runs it. A client that skips admit is outside the trust boundary. If the sidecar is down, the answer is deny.

## Blast radius

Operator commands. Not model tools.

```bash
deadbolt policy --agent ID --tools shell,read_file,write_file --dest github.com --irreversible shell --spend-cap 5
deadbolt approve --agent ID --tool shell
deadbolt kill --agent ID
deadbolt incident --agent ID --json --children
```

`policy` updates an existing lease's policy. An off-list tool is deny `purpose_exceeded`. A present foreign host is deny `purpose_exceeded`. A missing host is deny `purpose_exceeded` only for a network-class tool (`http`, `fetch`, `browser`, `web_search`). A local `write_file` or `shell` with no host is not denied for the dest list alone.

`approve` is one shot for one irreversible tool. The next call of that tool is allow. The one after that is deny `needs_human`.

`kill` revokes that agent and its children. The next tool is deny `killed`.

`incident` writes a token file: decisions, policy snapshot, children. No generated prose.

## Measured live fire

2026-09-28. Harness `5f87be3`. One route that emitted tools. Ids as recorded. Not a benchmark.

Agent `h-59378-18d9acf67c5ecdf8`:

- `write_file` → `ok`
- `shell` → `needs_human` (body did not run)
- `approve` then `shell` → `ok`, body `LIVE-POLICY-SHELL`
- second `shell` → `needs_human`
- `kill` then `read_file` → `killed`

Agent `h-60207-18d9ad7dce9803c8` wrote `LIVE-POLICY-B2`. That write was allow. Killing the first agent did not block it.

## Trust boundary

Clients are cooperative. `mcp-proxy` gates routed tool calls; build-in requires a dispatcher that stops on deny. Sidecar down = deny.

Deadbolt does not shut down a vendor model. It does not contain another lab's agents on the public internet.
