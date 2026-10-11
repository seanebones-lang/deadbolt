# DeadBolt

**A local execution gate for software agents.** Put DeadBolt in the code that runs
an action, then let policy and operator controls decide whether that action runs.
Embed it in Rust, call its local sidecar from another language, or wrap a trusted
stdio MCP server. Free under [Apache-2.0](LICENSE).

[Try it](docs/QUICKSTART.md) · [Install](docs/INSTALL.md) · [Integrate](docs/INTEGRATION.md) · [Project site](https://seanebones-lang.github.io/deadbolt/)

[![CI](https://github.com/seanebones-lang/deadbolt/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/seanebones-lang/deadbolt/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/seanebones-lang/deadbolt)](https://github.com/seanebones-lang/deadbolt/releases/latest)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

## Try it before connecting an agent

No model account, API key, Harness, cloud service or database server is needed.
The [published v1.0.3 release](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.3)
has native ZIPs for Linux x86_64, Windows x86_64, Apple Silicon and Intel Macs.
[Verify and extract the matching archive](docs/INSTALL.md#native-archive-no-rust-required).
From its extracted directory on macOS/Linux:

```sh
chmod 0755 deadbolt
./deadbolt drill
python3 examples/evaluate.py --binary ./deadbolt
```

On Windows, use `.\deadbolt.exe drill` and
`python examples/evaluate.py --binary .\deadbolt.exe`.
Expected: `deadbolt drill ok`, then a JSON report with `"passed": true`.
The evaluation starts a private sidecar, writes harmless temporary files, checks
that policy, approval, kill and outage block effects, and cleans up afterward.
Python 3.9+ is the only additional runtime needed. [Understand the report](docs/FIRST-EVALUATION.md).

## Pick your integration

| Your executor | Smallest integration | Start here |
| --- | --- | --- |
| Rust application | One dependency, an embedded store and a check at dispatch | [Rust](docs/INTEGRATION.md#build-in-rust-executor) |
| Python application | Local sidecar and `dispatch` around the actual function | [Python](docs/PYTHON.md) |
| OpenAI Agents SDK function tools | Local sidecar and `@protected_tool` on each protected function | [SDK adapter](docs/PYTHON.md#add-the-decorator-to-the-actual-body) |
| Node or another language | Local sidecar; bundled Node client or the HTTP protocol | [Sidecar](docs/INTEGRATION.md#bolt-on-http-sidecar) |
| Trusted stdio MCP server | Replace its launch command with `deadbolt mcp-proxy … -- SERVER` | [MCP](docs/INTEGRATION.md#bolt-on-stdio-mcp-proxy) |

Choose the version before copying examples:

| Available now | Included |
| --- | --- |
| **Published v1.0.3** | Native executable, embedded Rust, local HTTP, source Python/Node clients, stdio MCP, leases, policy, broad one-shot approval, kill and evidence |
| **1.1.0-rc.1 prerelease** | All of the above, plus callback dispatch helpers, installable Python wheel/source package, optional SDK decorator, single-agent admission credentials and exact-action approvals |

The [v1.1.0-rc.1 prerelease](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.1.0-rc.1) includes native downloads and a Python wheel. It is for developer evaluation before stable promotion.
They are **not in v1.0.3 downloads**. [Install the prerelease](docs/INSTALL.md#reviewed-source-candidate)
or use the stable examples labeled v1.0.3. Neither the Rust crate nor the Python
client is published to a package registry; Node is a bundled source client.

## Put the gate at the effect

Your trusted executor creates the run and configures policy first. With the
candidate Python client installed and its sidecar configured:

```python
from pathlib import Path
from deadbolt_client import dispatch

outcome = dispatch(
    "executor-run-001", "write_file",
    lambda: Path("output.txt").write_text("allowed work"),
)
if not outcome["executed"]:
    print("Action blocked:", outcome["decision"]["code"])
```

A fresh admission happens before the callback. A denial leaves the body uncalled.
[The complete setup](docs/INTEGRATION.md) covers run creation, policy, credentials,
transport and error handling. Wrap every protected route; never cache an allow
or perform a separate admission before a one-shot dispatch helper.

## Controls you can build on

- Tool allow-lists, destination tokens, reported-spend caps, pause and clips.
- Leases with a default 60-second sliding TTL; killed IDs stay killed.
- Operator kill blocks subsequent actions for an ID and its registered descendants.
- Broad one-shot approval for classified irreversible tools; the candidate adds
  [review of exact arguments](docs/ACTION-APPROVALS.md), including recipient/body/amount.
- Candidate [admission credentials](docs/CREDENTIALS.md) let one dispatcher ask
  for permission for its run without receiving operator controls.
- SQLite evidence, JSONL mirrors and per-agent incident exports.
- Default denial when the store is unavailable; bundled clients deny on transport
  failure, malformed responses and unsuccessful HTTP status.

Unset policy lists are open. Configure each run and child before dispatch.
Identity, tool routing, destinations and reported spend come from trusted
application code. They are not network interception or independent billing.

## Know the boundary

DeadBolt protects actions routed through your executor. The operator owns state,
policy and approvals; the model must not receive those controls. It does not
isolate arbitrary code, stop a model provider, cancel a running body, or make an
external effect transactional with admission. The MCP child is trusted and has
OS access; startup and non-tool MCP methods are outside the tool-call gate.

Evidence hashes identify content, but operator-writable records are not a
compliance certification. Read [trust boundaries](docs/TRUST.md) before deploying.
Use [the pilot checklist](docs/PILOT.md) to verify your application's actual routes.

## Developer documentation

| Need | Guide |
| --- | --- |
| First working result | [Quick start](docs/QUICKSTART.md), [first evaluation](docs/FIRST-EVALUATION.md) |
| Binary, crate or Python installation | [Installation](docs/INSTALL.md) |
| Embed, sidecar or MCP wiring | [Integration](docs/INTEGRATION.md), [Python / SDK](docs/PYTHON.md) |
| Least-authority dispatch and sensitive actions | [Credentials](docs/CREDENTIALS.md), [exact approvals](docs/ACTION-APPROVALS.md) |
| API and upgrade behavior | [HTTP protocol](docs/PROTOCOL.md), [compatibility](docs/COMPATIBILITY.md) |
| Denial or setup problem | [Troubleshooting](docs/TROUBLESHOOTING.md), [FAQ](docs/FAQ.md) |
| Deploy, backup and respond | [Operations](docs/OPERATIONS.md), [incidents](docs/INCIDENT.md) |
| Evaluate or contribute | [Technical brief](docs/EVALUATOR.md), [Hermes showcase](docs/HERMES-SHOWCASE.md), [contributing](CONTRIBUTING.md) |
| Versions and packaging | [Changelog](CHANGELOG.md), [release procedure](docs/RELEASING.md), [third-party notices](THIRD-PARTY.md) |

Source checks: `cargo test --locked`, `cargo clippy --all-targets --locked -- -D warnings`,
and `python3 tests/client_contract.py` after `cargo build --locked --bin deadbolt`
(Python and Node required). CI also checks Rust 1.85, installed Python/SDK behavior
and native packages. Passing these checks does not establish acceptance of your application.

[Report ordinary bugs](https://github.com/seanebones-lang/deadbolt/issues/new/choose)
or use [private security reporting](SECURITY.md). Copyright NextEleven LLC 2026.
[License](LICENSE) · [Notice](NOTICE). Harness is one consumer, not a prerequisite.
