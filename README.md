# Deadbolt

<img width="360" alt="Deadbolt" src="https://github.com/user-attachments/assets/12175eb7-67b1-4758-92c9-e2562df80cb4" />

[![CI](https://github.com/seanebones-lang/deadbolt/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/seanebones-lang/deadbolt/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/seanebones-lang/deadbolt)](https://github.com/seanebones-lang/deadbolt/releases/latest)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

A local execution gate for software agents: check before each tool call, enforce
policy, and let an operator revoke subsequent protected actions by an identity
and its registered descendants.

Deadbolt runs without a model provider, API key, Harness, or separate database
server. Embed the Rust library in your dispatcher, use the local HTTP sidecar
from another language, or wrap a trusted stdio MCP server. Your executor must
refuse execution unless the decision is explicitly `allow`.

## Start here

V1 native binaries target Linux x86_64, Windows x86_64, Apple Silicon and Intel
Macs. Use the
[latest release](https://github.com/seanebones-lang/deadbolt/releases/latest)
and verify its checksum using [the archive installation instructions](docs/RELEASING.md#install-an-archive).
Source installation is available below; the release page is the authority for
published downloads and their exact source identity.

For a first evaluation, download the v1.0.3 archive for your OS, verify and
extract it, run `deadbolt drill`, then run the [Python-only first evaluation](docs/FIRST-EVALUATION.md)
from the extracted directory. The native archive path needs Python 3.9+ for
that evaluation, but no Rust toolchain, model account or API key. On macOS,
v1.0.3 archives are Developer ID signed and Apple-notarized; read the release
notes and your organization's download policy before running them.

1. [Install](docs/INSTALL.md): prerequisites, macOS/Linux/Windows, verification.
2. [Quick start](docs/QUICKSTART.md): allow a call, kill it, observe denial.
3. [Integrate](docs/INTEGRATION.md): Rust, Python, Node, MCP, containers, services.
4. [FAQ](docs/FAQ.md) and [troubleshooting](docs/TROUBLESHOOTING.md).

Evaluating for a team? Start with the [technical brief](docs/EVALUATOR.md),
[observable demonstration](docs/DEMO.md) and [application pilot](docs/PILOT.md).
For a Python-only workflow with real file effects, use
[first evaluation](docs/FIRST-EVALUATION.md). See the [adoption roadmap](docs/ROADMAP.md)
for the next adoption milestones.

To build from source instead:

```sh
git clone --branch v1.0.3 --depth 1 https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo install --path . --locked --bin deadbolt
deadbolt --version
deadbolt drill
```

Requires Rust 1.85+ and a C/C++ toolchain for bundled SQLite. Python and Node are
only needed for their examples and consumer tests. The Cargo package is
`n11-deadbolt`, the Rust library is `deadbolt`, and the executable is `deadbolt`.
The package is not published to crates.io as checked on 2026-09-29. Install from
source; pin a reviewed Git commit for reproducible use.

## Choose an integration

| Your application | Use | What you control |
| --- | --- | --- |
| Rust executor | Embedded library | Every tool/spawn dispatch checks admission in process |
| Python, Node, or another language | Local HTTP sidecar | Trusted dispatcher calls admit and refuses every non-allow result |
| Trusted stdio MCP server | `deadbolt mcp-proxy` | Host routes `tools/call` through the proxy |

The sidecar uses a mode-0600 Unix socket or token-required loopback TCP. Public
binds are refused. Python and Node source clients are included; they are not
published pip/npm packages. The MCP proxy accepts individual JSON-RPC 2.0
objects over stdio and rejects batches. Remote HTTP/SSE MCP is not supported.

## What it does

- Maintains agent leases with a default 60-second TTL.
- Checks tool allow-lists, destination context, spend caps, clips and pause state.
- Requires one-shot operator approval for explicitly classified irreversible tools.
- Makes kill terminal for an ID and its registered descendants; unrelated agents
  remain usable.
- Denies on unavailable storage by default; the source clients deny on sidecar
  failure, malformed decisions, and unsuccessful HTTP responses.
- Records structured evidence in SQLite and JSONL and exports per-agent incidents.

Unset policy lists are open. Configure policy before running an agent. Destination
and spend are supplied by the trusted caller; they are not network interception
or independent billing measurement. Children do not inherit policy automatically.

## Try the gate

This self-contained Rust example requires no sidecar:

```sh
cargo run --locked --example build_in
```

Expected sequence: `allow`, then `killed`. For the operator-controlled,
three-terminal sidecar demonstration, follow [the quick start](docs/QUICKSTART.md).

Operator commands include:

```sh
deadbolt status
deadbolt policy --agent RUN_ID --tools read_file,write_file,shell --irreversible shell
deadbolt approve --agent RUN_ID --tool shell
deadbolt pause --agent RUN_ID
deadbolt resume --agent RUN_ID
deadbolt kill --agent RUN_ID
deadbolt incident --agent RUN_ID --json --children
deadbolt --help
```

The agent must first have a lease. Operator commands and the executor must use
the same database and evidence paths. `kill`, `pause`, `clip`, `resume`, and
`approve` are operator actions, not model tools.

## Trust and limits

Deadbolt gates later admissions; it does not stop a vendor model or cancel a
body already running. Admission and execution are separate operations: check
immediately before the side effect and never cache an allow. A caller that can
skip the dispatcher, change the store, or assert a new identity is outside the
intended boundary. A sidecar token authorizes all exposed routes, not one agent.

The MCP child is a trusted process with the proxy's OS access and environment.
Only routed tool calls are gated; other methods and server startup can have side
effects. Evidence hashes identify content; they do not make operator-writable
files tamper-proof or establish legal compliance.

See [trust boundaries](docs/TRUST.md) and [security reporting](SECURITY.md).

## Verify and operate

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --locked --bin deadbolt
python3 tests/client_contract.py
```

The last command requires Python 3 and Node. On Windows use `python` and set
`DEADBOLT_BIN` to `target/debug/deadbolt.exe`.

macOS and Linux container validation is recorded in [the standalone review](docs/STANDALONE-REVIEW.md).
Remote Linux/macOS/Windows and Rust 1.85 CI passed on 2026-09-29. The
[Hermes MCP showcase](docs/HERMES-SHOWCASE.md) records independent-host execution
acceptance on macOS and Linux. The systemd template passed a bounded disposable
Ubuntu-container test; native host installation remains experimental;
each application must validate its own actual dispatch paths.
These checks establish specific behavior, not universal integration or security
certification. See [operations](docs/OPERATIONS.md) for deployment and recovery.

## Documentation

| Guide | Purpose |
| --- | --- |
| [Installation](docs/INSTALL.md) | Build requirements, source/Git install, platform setup |
| [Quick start](docs/QUICKSTART.md) | Runnable allow/kill/deny demo |
| [Integration](docs/INTEGRATION.md) | Embedded and sidecar client APIs, MCP, deployment |
| [Protocol](docs/PROTOCOL.md) | HTTP routes, response shapes, framing and limits |
| [FAQ](docs/FAQ.md) | Adoption, licensing, enforcement, policy and lifecycle |
| [Troubleshooting](docs/TROUBLESHOOTING.md) | Symptoms, checks and recovery |
| [Operations](docs/OPERATIONS.md) | Configuration, service, container, backup and acceptance |
| [Trust](docs/TRUST.md) | Trust boundaries and limits |
| [Incident](docs/INCIDENT.md) | Evidence taxonomy and operator workflow |
| [Contributing](CONTRIBUTING.md) | Development checks and reporting |
| [Changelog](CHANGELOG.md) | Versioned changes and candidate history |
| [Hermes showcase](docs/HERMES-SHOWCASE.md) | Independent MCP host and real filesystem execution acceptance |
| [Compatibility](docs/COMPATIBILITY.md) | V1 API, lifecycle and upgrade contract |
| [V1 roadmap](docs/V1-READINESS.md) | Completed release and next adoption milestones |
| [Third-party components](THIRD-PARTY.md) | Dependency notices and included MPL source distribution |

Apache-2.0. Copyright NextEleven LLC 2026. [License](LICENSE) and [notice](NOTICE).
Harness is one consumer; this repository does not include or relicense it.

For integration evaluation, see the [pilot checklist](docs/PILOT.md). Report
ordinary bugs through [GitHub issues](https://github.com/seanebones-lang/deadbolt/issues/new/choose)
and suspected vulnerabilities through [private security reporting](SECURITY.md).
