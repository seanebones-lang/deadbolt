# DeadBolt

**A local execution gate for software agents.** Put DeadBolt in the code that runs
an action, then let policy and operator controls decide whether that action runs.
Embed it in Rust, call its local sidecar from another language, or wrap a trusted
stdio MCP server. Free under [Apache-2.0](LICENSE).

[Install](docs/INSTALL.md) · [Try it](docs/QUICKSTART.md) · [Integrate](docs/INTEGRATION.md) · [Migration](docs/MIGRATING-V2.md) · [Project site](https://nexteleven.dev/deadbolt)

[![CI](https://github.com/seanebones-lang/deadbolt/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/seanebones-lang/deadbolt/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/seanebones-lang/deadbolt)](https://github.com/seanebones-lang/deadbolt/releases/latest)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

## Version 2.0.0

This source targets **engine 2.0.0 and Python client 2.0.0**, with scope frozen at
the tested RC3 behavior. [Versioned releases](https://github.com/seanebones-lang/deadbolt/releases)
identify available downloads, exact source and checksums; [installation](docs/INSTALL.md)
tracks package-manager, bundle and editor availability separately. A source merge
does not publish a package or update a directory listing.

Version 2 requires authenticated operator setup for Unix and TCP sidecars,
creates private state, defaults fresh roots to supervisor renewal, and enforces
ancestor policy and fixed run deadlines. Existing stored renewal modes remain
unchanged. Read [major-version migration](docs/MIGRATING-V2.md) and
[private state](docs/PRIVATE-STATE.md) before replacing an older installation.

The artifact-pinned RC3 retest passed conflicting-destination denial, damaged-store
Cursor Reload refusal, and actual model calls after a fixed deadline on macOS
arm64. The underlying Auto model was unknown. Separate Linux headless checks
cover their recorded scope. These are execution receipts, not certification;
stable artifact and installed-consumer checks remain distinct from the RC3 results.

## Try it before connecting an agent

No model account, API key, Harness, cloud service or database server is needed.
Choose a checksum-verified native archive in the [installation guide](docs/INSTALL.md#native-archive-no-rust-required).
From its extracted directory on macOS/Linux:

```sh
chmod 0755 deadbolt
./deadbolt --version
./deadbolt drill
python3 examples/evaluate.py --binary ./deadbolt
```

On Windows, use `.\deadbolt.exe --version`, `.\deadbolt.exe drill` and
`python examples/evaluate.py --binary .\deadbolt.exe`.
Expected: your selected version, `deadbolt drill ok`, and JSON with `"passed": true`.
The evaluation starts a private sidecar, writes harmless temporary files, checks
that policy, approval, kill and outage block effects, and cleans up afterward.
Python 3.9+ is the additional runtime needed. [Understand the report](docs/FIRST-EVALUATION.md).

For a reviewed source checkout, Rust 1.85+ and a native C compiler/linker:

```sh
cargo install --path . --locked --bin deadbolt
deadbolt --version
deadbolt drill
```

The matching [Python client 2.0.0](https://pypi.org/project/n11-deadbolt-client/2.0.0/) is published.
The matching [Rust package 2.0.0](https://crates.io/crates/n11-deadbolt/2.0.0) is also published.
Install the CLI from Cargo, or the Python client in your application environment:

```sh
cargo install n11-deadbolt --version 2.0.0 --locked --bin deadbolt
# In your application's virtual environment:
python -m pip install 'n11-deadbolt-client==2.0.0'
```

For embedded Rust, use `deadbolt = { package = "n11-deadbolt", version = "=2.0.0" }`
and commit your application's own lockfile. The Python wheel contains the client,
not the engine. Node remains a bundled source module. [Install and verify](docs/INSTALL.md).

## Pick your integration

| Your executor | Smallest integration | Start here |
| --- | --- | --- |
| Rust application | Embedded store and a check at actual dispatch | [Rust](docs/INTEGRATION.md#build-in-rust-executor) |
| Python application | Local sidecar and `dispatch` around the actual function | [Python](docs/PYTHON.md) |
| OpenAI Agents SDK function tools | Sidecar and `@protected_tool` on each protected function | [SDK adapter](docs/PYTHON.md#add-the-decorator-to-the-actual-body) |
| FastMCP server you own | Sidecar and `dispatch_async` inside protected tool bodies | [Offline recipe](docs/FASTMCP.md) |
| Node or another language | Local sidecar; source Node client or HTTP protocol | [Sidecar](docs/INTEGRATION.md#bolt-on-python-node-or-any-http-client) |
| Trusted stdio MCP server | `deadbolt mcp-proxy … -- SERVER` | [MCP](docs/INTEGRATION.md#bolt-on-stdio-mcp-proxy) |
| Desktop MCP client | Verified local bundle and private operator configuration | [Bundle and editor routes](docs/INSTALL.md#local-mcp-bundle-mcpb) |
| Docker-launched MCP client | Source-built stdio container and trusted downstream | [Docker](docs/DOCKER-MCP.md) |

Choose the exact bundle and matching configuration before installation. Blank
configuration exposes two read-only setup tools and provides no active protection.
Configured tools come from your trusted downstream server. Version 2 bundle runs
require a fixed deadline, deliberate renewal mode and reviewed offline exceptions.
The bundle uses broad one-shot tool approvals; it does not provision exact-action
grants. [Configuration](dist/mcpb/README.md) · [Editor guide](docs/EDITORS-V2.md).

Discover versioned bundles through [Smithery](https://smithery.ai/servers/seanebones/deadbolt)
and the [official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.seanebones-lang%2Fdeadbolt&limit=10).
Check each entry's version and checksum. Discovery is not hosted operator access,
marketplace acceptance of another submission, or an endorsement.

## Put the gate at the effect

Your trusted executor creates the run and configures policy first. With the
matching Python client installed and its sidecar configured:

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

A fresh admission happens before the callback; a denial leaves the body uncalled.
[Complete setup](docs/INTEGRATION.md) covers provisioning, policy, credentials,
transport and errors. Wrap every protected route. Never cache an allow or split
one-shot admission from execution. `ensure` confirms record presence, not liveness.

## Controls and limits

- Tool and destination policies, reported-spend caps, pause, clips and operator kill.
- Default 60-second idle TTL. A separate supervisor maintains fresh runs; explicit
  activity mode is available. Neither mode can move a fixed deadline or revive
  expired/killed state. [Liveness](docs/SUPERVISION.md) · [Deadlines](docs/LEASE-DEADLINES.md).
- Descendants obey ancestor restrictions. A child policy cannot widen its parent.
- Broad one-shot tool approvals and [exact-action grants](docs/ACTION-APPROVALS.md)
  for trusted integrations using the content-bound API.
- [Scoped admission credentials](docs/CREDENTIALS.md) separate dispatch from
  operator controls. `/spend` remains operator-only.
- SQLite evidence, JSONL mirrors and per-agent incident exports.
- Store or transport failure denies protected dispatch; malformed or unsuccessful
  responses never authorize the body.

Ordinary trusted-executor admission treats unset policy lists as open; the MCP
proxy requires explicit tool policy. Configure runs and children before dispatch.
MCP discovery can advertise tools that the policy will deny. Destination parsing
covers recognized fields, not arbitrary arguments or network traffic.
[Destination policy](docs/DESTINATION-POLICY.md).

## Know the boundary

DeadBolt protects connected execution routes. It does not sandbox arbitrary code,
stop a provider, or make external effects transactional with admission. Kill blocks
subsequent admissions; stopping in-flight work requires an executor cancellation
pattern. The MCP child remains trusted and has OS access; startup and non-tool
methods are outside the tool-call gate.

Keep state and operator authority outside untrusted agent processes. A process
able to write the database or invoke the operator CLI has operator authority;
socket credentials cannot contain it. Use separate OS/service ownership and OS
egress controls where your application requires those boundaries. Evidence hashes
identify content; operator-writable records are not tamper-proof certification.
[Trust](docs/TRUST.md) · [HTTP authentication](docs/HTTP-AUTHENTICATION.md) · [Pilot checklist](docs/PILOT.md).

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

For trusted executors that need to stop already running local commands, the
[owned-process cancellation reference](docs/EXECUTOR-CANCELLATION.md) adds a
POSIX supervisor pattern with disposable acceptance. It does not
change the gate into a sandbox or add enforcement to older packages.
