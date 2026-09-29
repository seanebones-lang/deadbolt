<img width="1642" height="848" alt="deadbolt" src="https://github.com/user-attachments/assets/4b0ddbbd-bbf4-400a-894d-984d9a353673" />

# Deadbolt

Capability is compounding. Containment is lagging.

Deadbolt is fail-closed execution: leased tools, blast radius, human stop, evidence for a lawyer.

It does not shut down GPT. Sidecar down = deny. Bolt-on client is cooperative; mcp-proxy and build-in are enforced.

Apache-2.0. Not a model tool.

Default fail-closed. Sidecar down or timeout = deny `store_unavailable`. Clients default deny. A gate with `fail_closed=true` does the same.

Build-in, `mcp-proxy`, and an in-executor hook are enforced: the tool body cannot run without admit. A bolt-on client is cooperative. A compromised agent that skips admit is outside the trust boundary.

A lease is issued on `ensure`. TTL default is 60 seconds. Every admit rechecks it. Expiry is deny `lease_expired`. A successful admit renews the TTL. An expired lease is not slid, and a deny does not renew. Silence longer than the TTL expires the hands.

The agent id is a claim. `DEADBOLT_TOKEN` authenticates the caller to serve, not the agent. Whoever holds the token can assert any id. The executor or the proxy assigns the id. The model does not.

Admit-then-tool is a race. Admit immediately before the body. Do not admit once per session.

Blast radius is the lease. `tools_allow`, `dest_allow`, `spend_cap_usd`, and `irreversible` are unset by default, so an old lease stays open. An off-list tool or dest is deny `purpose_exceeded`. Crossing the spend cap pauses the agent and the next admit is deny `spend_cap`. An irreversible tool is deny `needs_human` until `deadbolt approve --agent ID --tool TOOL`, one shot. Deadbolt does not contain another lab's agents on the public internet.

Detail: `docs/TRUST.md`.

## Two modes

Build-in: link the crate and call `admit` in-process. See `docs/INTEGRATION.md` and `cargo run --example build_in`.

Bolt-on: `deadbolt serve` plus a client that admits before the tool. Python: `examples/deadbolt_client.py`. Node, no npm: `examples/deadbolt_client.js`. Protocol: `docs/PROTOCOL.md`.

## Install

```bash
cargo add n11-deadbolt
cargo install n11-deadbolt --bin deadbolt
deadbolt drill
```

From a checkout:

```bash
git clone https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo build --release
./target/release/deadbolt drill
```

Debug binary, same drill: `cargo build && ./target/debug/deadbolt drill`.

## CLI

```bash
deadbolt status [--agent ID]
deadbolt pause --agent ID
deadbolt clip TOOL --agent ID
deadbolt resume --agent ID
deadbolt kill --agent ID
deadbolt drill
deadbolt serve [--bind PATH]
deadbolt export --agent ID [--out PATH] [--json] [--children]
deadbolt policy --agent ID [--tools a,b] [--dest host,host] [--spend-cap N] [--irreversible a,b]
deadbolt approve --agent ID --tool TOOL
deadbolt incident --agent ID [--out PATH]
deadbolt mcp-proxy --agent ID [--serve-sock PATH] -- COMMAND...
```

The incident file is tokens. Taxonomy: `docs/INCIDENT.md`.

Default socket is `~/.deadbolt/deadbolt.sock` (mode `0600`; token optional). TCP is loopback only: `deadbolt serve --bind 127.0.0.1:PORT`. `0.0.0.0`, `[::]`, and any other host are refused. TCP requires `DEADBOLT_TOKEN` at start. A missing or wrong `X-Deadbolt-Token` is HTTP 401 and does not admit. Set `DEADBOLT_SOCK=127.0.0.1:PORT` or `http://127.0.0.1:PORT` for the clients.

`kill` requires `--agent`. It revokes that agent and its children. It does not halt a fleet and it does not shut down a model vendor.

## Run as a service

TCP requires `DEADBOLT_TOKEN`. The listener is `127.0.0.1` only.

```bash
sudo install -d -m 0755 /etc/deadbolt
sudo install -m 0600 dist/deadbolt.env.example /etc/deadbolt/deadbolt.env
sudo install -m 0644 dist/deadbolt.service /etc/systemd/system/deadbolt.service
sudo systemctl enable --now deadbolt
```

```bash
cp dist/deadbolt.env.example dist/deadbolt.env && chmod 0600 dist/deadbolt.env
docker compose -f dist/docker-compose.yml up --build
```

Compose publishes `127.0.0.1:9782:9782`. It does not open `0.0.0.0`. Host network is the other way to reach a process bound to `127.0.0.1`.

Security: `SECURITY.md`.

## Admit before the tool

```python
from deadbolt_client import admit
if admit(agent, "shell")["decision"] != "allow":
    raise SystemExit("deadbolt deny")
# then run the tool
```

Stdlib clients: `examples/deadbolt_client.py` and `examples/deadbolt_client.js`. Sample loop: `examples/deadbolt_sample_agent.py`. `DEADBOLT_SOCK` overrides the socket.

Operator detail: `docs/DEADBOLT.md`.

Harness is one consumer of this crate. This repository does not include it.
