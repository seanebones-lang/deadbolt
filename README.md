# Deadbolt

Fail-closed execution gate for software agents.

| | |
| --- | --- |
| Fail-closed | Sidecar down or an unwritable store is deny `store_unavailable`. |
| Two modes | Build-in crate `n11-deadbolt`. Bolt-on `serve` and `mcp-proxy`. |
| Install | `cargo install n11-deadbolt --bin deadbolt` |
| Not a model tool | `approve`, `kill`, `pause`, and `resume` are operator CLI. |

**Abstract.** Deadbolt is a fail-closed execution gate for software agents. It issues a lease, checks it before the tool body, and denies the next call when the lease is dead. It does not shut down GPT or any other vendor model.

Two modes. Build-in links the crate `n11-deadbolt` and calls `admit` in process. Bolt-on runs `deadbolt serve`, or `deadbolt mcp-proxy` in front of an MCP child. The crate and the proxy are enforced. Python and Node clients are cooperative. Sidecar down is deny.

A lease starts at `ensure`. The default TTL is 60 seconds. Every admit rechecks it. A live deny renews the TTL. Silence expires the hands. A killed lease does not slide.

On 2026-09-28 one live policy dance denied an irreversible shell, allowed it once after approve, denied the next shell, and returned `killed` after kill. That run is a measurement, not a benchmark.

## 0. Status

Apache-2.0. Crate `n11-deadbolt`. Binary `deadbolt`. Lib name stays `deadbolt`, so `use deadbolt::` still compiles.

Tag `v0.1.1-product` is `04a36c1`. Later commits are on `main`.

```bash
cargo install n11-deadbolt --bin deadbolt
```

License: [Apache-2.0](LICENSE). Repository: <https://github.com/seanebones-lang/deadbolt>.

Harness is a consumer of this crate. This repository does not include Harness, and it does not relicense Harness.

## 1. Problem

Agents act. Containment lags.

A kill switch is what draft city law and buyer reviews ask for. "The model did it" is the vendor's failure. Deadbolt does not shut that model down. It denies the next tool, MCP, or spawn call on your agent.

Scope is your agents, on your executor or your MCP path. This is not a lab research swarm on the public internet.

## 2. Design

### Lease

`ensure` issues the lease. It does not resurrect a killed lease. Default TTL is 60 seconds (`lease_ttl_secs`). Every `admit` rechecks it inside `evaluate`.

| event | result |
| --- | --- |
| live admit, allow or deny | TTL renewed in `evaluate` before the deny return |
| silence past TTL | deny `lease_expired`; expired row is not slid |
| `killed` | deny `killed`; not slid |

### Admit before the body

The tool body does not run unless `admit` returned allow. Deny codes are tokens, not sentences.

| token | when |
| --- | --- |
| `allow` | decision, not a deny code |
| `killed` | lease killed |
| `paused` | lease paused |
| `purpose_exceeded` | off-list tool, foreign dest, network-class tool with no dest, or a clip |
| `lease_expired` | TTL passed |
| `no_lease` | no row |
| `store_unavailable` | store missing, sidecar down, timeout, empty body, or HTTP 5xx |
| `needs_human` | irreversible tool, no one-shot approve |
| `spend_cap` | spend crossed `spend_cap_usd`; lease is paused |

### Blast radius

Unset lists stay open. A default deny would break a caller that never set a policy.

| field | deny |
| --- | --- |
| `tools_allow` | tool not on the list → `purpose_exceeded` |
| `dest_allow` | present foreign host → `purpose_exceeded`. Missing dest → `purpose_exceeded` only for a network-class tool (`http`, `fetch`, `browser`, `web_search`). A local `write_file` or `shell` with no host is not denied for the dest list alone |
| `spend_cap_usd` | crossing the cap pauses the lease; next admit is `spend_cap` |
| `irreversible` | `needs_human` until `deadbolt approve --agent ID --tool TOOL`, one shot. The next call is allow. The one after that is `needs_human` |

### Enforcement

| path | boundary |
| --- | --- |
| crate `admit`, `mcp-proxy` | enforced; the body cannot start on deny |
| `examples/deadbolt_client.py`, `examples/deadbolt_client.js` | cooperative; a caller that skips admit is outside the boundary |

### Sidecar

| bind | rule |
| --- | --- |
| Unix socket | default `~/.deadbolt/deadbolt.sock`, mode `0600`; token optional |
| TCP | `127.0.0.1` or `::1` only. `DEADBOLT_TOKEN` required at start. Missing or wrong `X-Deadbolt-Token` is HTTP 401 and does not admit |
| `0.0.0.0`, `::`, any other host | refused |

### Evidence

| class | rule |
| --- | --- |
| observed | a fact the gate recorded. `premises` empty |
| inferred | a conclusion. `purpose_exceeded`, `lease_expired`, and `no_lease` carry premises. An inferred row with empty premises is refused on export |
| generated | refused. Prose is not a row |

Each record carries a sha256 content id. The incident file is tokens. A sentence is not proof.

## 3. Interface

### CLI

Operator commands. Not model tools.

| command | effect |
| --- | --- |
| `deadbolt status [--agent ID]` | list leases |
| `deadbolt pause --agent ID` | pause one agent |
| `deadbolt clip TOOL --agent ID` | clip one tool |
| `deadbolt resume --agent ID [--approve TOOL]` | clear pause and clips. Does not resurrect a kill. `--approve` is one shot |
| `deadbolt kill --agent ID` | revoke that agent and its children. No `--all` |
| `deadbolt drill` | in-process self-check. No API keys |
| `deadbolt serve [--bind PATH]` | Unix socket or loopback TCP |
| `deadbolt export --agent ID [--out PATH] [--json] [--children]` | JSONL evidence |
| `deadbolt policy --agent ID [--tools a,b] [--dest host,host] [--spend-cap N] [--irreversible a,b]` | set blast radius. Omitted fields stay open |
| `deadbolt approve --agent ID --tool TOOL` | one shot for one irreversible tool |
| `deadbolt incident --agent ID [--out PATH] [--json] [--children]` | token JSON. Bare flags need no value |
| `deadbolt mcp-proxy --agent ID [--serve-sock PATH] -- COMMAND...` | admit `tools/call` before the child runs it |

### Serve

HTTP/1.1. JSON in, JSON out. `kill`, `pause`, `clip`, and `resume` are not HTTP routes.

| method | path |
| --- | --- |
| POST | `/admit` |
| POST | `/ensure` |
| POST | `/register_child` |
| POST | `/policy` |
| POST | `/spend` |
| GET | `/status` |

Protocol: [docs/PROTOCOL.md](docs/PROTOCOL.md).

### mcp-proxy

```bash
deadbolt mcp-proxy --agent shop-bot -- npx whatever-mcp
```

`tools/call` is admitted first. A deny is a JSON-RPC error whose message is the code token. The child is not invoked.

### Build-in

```rust
use deadbolt::{AdmitDecision, Deadbolt};

let db = Deadbolt::open_at(dir, true, 60);
db.ensure_agent("shop-bot").expect("ensure");
// admit immediately before the body
match db.admit("shop-bot", "shell") {
    AdmitDecision::Allow => { /* tool body */ }
    AdmitDecision::Deny { code } => {
        eprintln!("{}", code.as_str());
        return;
    }
}
```

Full loop: `cargo run --example build_in`. Integration: [docs/INTEGRATION.md](docs/INTEGRATION.md).

### Bolt-on

```python
from deadbolt_client import admit, ensure

agent = "shop-bot"
ensure(agent)
gate = admit(agent, "shell")
if gate.get("decision") != "allow":
    raise SystemExit(gate.get("code", "store_unavailable"))
# tool body runs only here
```

Clients: `examples/deadbolt_client.py`, `examples/deadbolt_client.js`.

### Docs

| page | what |
| --- | --- |
| [docs/PROTOCOL.md](docs/PROTOCOL.md) | sidecar HTTP |
| [docs/INTEGRATION.md](docs/INTEGRATION.md) | build-in, bolt-on, service |
| [docs/TRUST.md](docs/TRUST.md) | trust boundary |
| [docs/INCIDENT.md](docs/INCIDENT.md) | taxonomy. Not copied into the JSON |
| [docs/PRODUCT.md](docs/PRODUCT.md) | public page copy |
| [docs/PRESS.md](docs/PRESS.md) | press copy |

## 4. Measured

2026-09-28. Ids as recorded. Not a benchmark.

### Siblings

| agent | result |
| --- | --- |
| `h-39522-18d9a42581e14078` | killed after `LIVE-A-RAN`. Later `write_file` and `shell` denied `killed` |
| `h-39581-18d9a4352ece0758` | wrote `live-b.txt` `LIVE-B-OK` |

### Lineage

| role | id | result |
| --- | --- | --- |
| P | `h-40211-18d9a4cbb7bb3ec8` | parent |
| C | `h-40211-18d9a4cd92843e50-1` | `parent=P`, then child killed |

### Policy dance

| agent | sequence |
| --- | --- |
| P `h-59378-18d9acf67c5ecdf8` | `write_file` ok; `shell` `needs_human`; approve then `shell` ok (`LIVE-POLICY-SHELL`); second `shell` `needs_human`; `kill` then `read_file` `killed` |
| B `h-60207-18d9ad7dce9803c8` | wrote `LIVE-POLICY-B2`. Allow. Killing P did not block B |

### Honesty

The first post-kill code is `killed`. It is not rewritten to `store_unavailable` when a sink append fails.

### Tests

Counted in this tree: 50 `#[test]` functions (`src/lib.rs`, `src/serve.rs`, `src/mcp.rs`, `src/witness.rs`, `tests/incident_flag.rs`).

| check | command |
| --- | --- |
| tests | `cargo test` |
| clippy | `cargo clippy -- -D warnings` |
| drill | `deadbolt drill` |

## 5. Trust boundary

The agent id is a claim. `DEADBOLT_TOKEN` authenticates the caller to serve, not the agent. Whoever holds the token can assert any id. The executor or the proxy assigns the id. The model does not.

Admit, then the tool, is a race. Admit immediately before the body. Do not admit once per session.

This does not shut down GPT. It does not discover a shadow agent, halt a fleet, or contain another lab's agents on the public internet. It cannot prove a cooperative caller obeyed.

Detail: [docs/TRUST.md](docs/TRUST.md).

## 6. Operations

| piece | where |
| --- | --- |
| systemd | `dist/deadbolt.service` runs `deadbolt serve --bind 127.0.0.1:9782` |
| compose | `dist/docker-compose.yml` publishes `127.0.0.1:9782:9782` only |
| 24-hour notice | operator steps in [docs/INCIDENT.md](docs/INCIDENT.md). Deadbolt writes the JSON. The operator sends the notice |

Install commands: [docs/INTEGRATION.md](docs/INTEGRATION.md). Security: [SECURITY.md](SECURITY.md).

## 7. License and trademark

Apache-2.0. Copyright NextEleven LLC 2026.

The crate name on crates.io is `n11-deadbolt` because the name `deadbolt` is taken. This project is unrelated to other "deadbolt" file-encryption projects. It does not relicense Harness.
