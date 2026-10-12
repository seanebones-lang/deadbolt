# DeadBolt

**A local execution gate for software agents.** Put DeadBolt in the code that runs
an action, then let policy and operator controls decide whether that action runs.
Embed it in Rust, call its local sidecar from another language, or wrap a trusted
stdio MCP server. Free under [Apache-2.0](LICENSE).

Keep DeadBolt state and operator controls outside untrusted agent processes.

RC2 creates private Unix state and requires reviewed permission
migration for older stores; see [private state](docs/PRIVATE-STATE.md).
A process that can write the database or invoke the operator CLI has operator
authority; socket credentials cannot contain it. For untrusted tool execution,
use separate OS/service ownership and a protected dispatcher. RC2
requires operator authentication for both Unix and TCP sidecars by default.
[Trust boundary](docs/TRUST.md) · [HTTP setup](docs/HTTP-AUTHENTICATION.md)

[Download MCP bundle 1.1.0](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v1.1.0/deadbolt-1.1.0.mcpb) · [Try it](docs/QUICKSTART.md) · [Install](docs/INSTALL.md) · [Integrate](docs/INTEGRATION.md) · [Project site](https://nexteleven.dev/deadbolt)

[![CI](https://github.com/seanebones-lang/deadbolt/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/seanebones-lang/deadbolt/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/seanebones-lang/deadbolt)](https://github.com/seanebones-lang/deadbolt/releases/latest)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

## Evaluate the 2.0 developer candidate

**Published corrected preview: engine 2.0.0-rc.2 / Python 2.0.0rc2.**
It adds supervisor renewal by default for fresh roots, complete ancestor policy
checks, authenticated operator setup, private persisted state and model-readable
denials. Existing stored renewal modes stay unchanged. Actual graphical-client
acceptance and the independent rerun remain open; stable promotion is gated on them.

[Download the matching engine and Python wheel](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0-rc.2).
The release includes four native targets, checksums, source identity and acceptance
receipts; both Mac ZIPs are signed/notarized. The Python wheel installs the client,
not an engine or sidecar. [Read the migration guide](docs/MIGRATING-V2.md) before upgrading.

The [Homebrew preview](docs/HOMEBREW.md#explicitly-select-the-prerelease) and
[local MCPB RC2](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v2.0.0-rc.2)
are available. [Use the RC2 editor helper](docs/EDITORS-V2.md) for Cursor/VS Code.
PyPI now supplies the matching `2.0.0rc2` client. Cargo remains on RC1 until its
separate corrected publication is verified. Use the RC2
native download, verified GitHub wheel or immutable source tag now:

```sh
git clone --branch v2.0.0-rc.2 --depth 1 https://github.com/seanebones-lang/deadbolt.git
cd deadbolt
cargo install --path . --locked --bin deadbolt
# In your application's virtual environment:
python -m pip install 'n11-deadbolt-client==2.0.0rc2'
```

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

## Install in a desktop MCP client

[Download the local MCPB bundle 1.1.0 directly](https://github.com/seanebones-lang/deadbolt/releases/download/mcpb-v1.1.0/deadbolt-1.1.0.mcpb)
from GitHub. [Verify its checksum](docs/INSTALL.md#local-mcp-bundle-mcpb)
before installing it. No GitHub sign-in or Rust build is required.
The bundle contains the published v1.0.3 executable and a Node.js launcher.
Install it in a compatible client and select one private operator
configuration file defining your trusted downstream server, tool allow-list and
state paths. Node.js 20+ is required; a compatible client can supply that runtime.

Bundle **1.1.0** packages stable engine **1.0.3**. Leave configuration blank for
read-only integration guidance and bundle checksum verification; this setup mode
provides no active protection. Supply an explicit operator configuration to enable
the gate, whose tools come from
the server you configure. Operator approval, pause and kill remain local CLI
actions. The trusted child retains OS access; this is not a sandbox.
[Installation and verification](docs/INSTALL.md#local-mcp-bundle-mcpb) ·
[Configuration and operator controls](https://github.com/seanebones-lang/deadbolt/blob/mcpb-v1.1.0/dist/mcpb/README.md).

For the new controls, [download MCPB 2.0.0-rc.2 preview](https://github.com/seanebones-lang/deadbolt/releases/tag/mcpb-v2.0.0-rc.2). It requires version 2 configuration with a fixed absolute deadline, deliberate renewal mode and offline exceptions that default to empty. The launcher uses broad one-shot tool approvals; exact-action grant provisioning is not included. [Preview configuration and migration](dist/mcpb/README.md). Both versions offer blank read-only setup with no active protection. Choose the artifact and matching configuration before installing.


Discover the local bundles through [Smithery](https://smithery.ai/servers/seanebones/deadbolt)
and listed in the [official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.seanebones-lang%2Fdeadbolt&limit=10)
as `io.github.seanebones-lang/deadbolt`. The latest catalog entry is the 2.0 preview; the stable direct download above remains pinned. The registry is a discovery record, not a
hosted operator endpoint or an endorsement. [server.json](server.json) records
the published package URL and SHA-256. This bundle release is separate from the
engine's 2.0.0-rc.2 developer candidate.

## Pick your integration

**Using VS Code or Cursor?** Follow the [editor installation guide](docs/EDITORS.md)
for a checksum-verified configuration helper, a local Cursor install link and a
disposable effect test. Start with read-only setup, then demonstrate denial,
one-shot approval and revocation on one routed MCP connection.
[Practical tutorial](docs/EDITOR-TUTORIAL.md).

| Your executor | Smallest integration | Start here |
| --- | --- | --- |
| Rust application | One dependency, an embedded store and a check at dispatch | [Rust](docs/INTEGRATION.md#build-in-rust-executor) |
| Python application | Local sidecar and `dispatch` around the actual function | [Python](docs/PYTHON.md) |
| OpenAI Agents SDK function tools | Local sidecar and `@protected_tool` on each protected function | [SDK adapter](docs/PYTHON.md#add-the-decorator-to-the-actual-body) |
| FastMCP server you own | Local sidecar and `dispatch_async` inside protected tool bodies | [Offline stdio recipe](docs/FASTMCP.md) |
| Node or another language | Local sidecar; bundled Node client or the HTTP protocol | [Sidecar](docs/INTEGRATION.md#bolt-on-python-node-or-any-http-client) |
| Trusted stdio MCP server | Replace its launch command with `deadbolt mcp-proxy … -- SERVER` | [MCP](docs/INTEGRATION.md#bolt-on-stdio-mcp-proxy) |
| Desktop client supporting MCPB | Install the local bundle and select a private operator configuration | [Local bundle](docs/INSTALL.md#local-mcp-bundle-mcpb) |
| Docker-launched MCP client | Build the pinned stdio container; configure one trusted downstream inside it | [Docker source build](docs/DOCKER-MCP.md) |

Choose the version before copying examples:

| Available now | Included |
| --- | --- |
| **Published v1.0.3** | Native executable, embedded Rust, local HTTP, source Python/Node clients, stdio MCP, leases, policy, broad one-shot approval, kill and evidence |
| **Older 1.1.0-rc.1 prerelease** | All of the above, plus callback dispatch helpers, installable Python wheel/source package, optional SDK decorator, single-agent admission credentials and exact-action approvals |
| **2.0.0-rc.2 corrected developer candidate** | Includes the 1.1 features, complete ancestor enforcement, fixed deadlines, default supervisor renewal for fresh roots, explicit offline exceptions, private state, authenticated operator setup, model-readable denials and POSIX owned-process cancellation; major Rust API migration required |

The [v2.0.0-rc.2 prerelease](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0-rc.2)
is for developer evaluation before stable promotion. These additions are **not in
v1.0.3 downloads or older editor/MCP bundles**. [Install the candidate](docs/INSTALL.md)
or use the stable examples labeled v1.0.3. Registry publication is separately
versioned; select the exact package and matching engine. Node remains a source client.

**On macOS, install the signed stable CLI with Homebrew:**

```sh
brew install --cask seanebones-lang/tap/n11-deadbolt
deadbolt --version
deadbolt drill
```

Apple Silicon and Intel are supported. [Trust, evaluation, preview and removal](docs/HOMEBREW.md).
This installs the engine without starting a service or configuring protection.

**Or build the stable CLI with Cargo:**

```sh
cargo install n11-deadbolt --version 1.0.3 --locked --bin deadbolt
deadbolt --version
deadbolt drill
```

**Embed the stable library** in your application's `Cargo.toml`:

```toml
[dependencies]
deadbolt = { package = "n11-deadbolt", version = "=1.0.3" }
```

For RC2, use its immutable source tag or reviewed release archive; the older
RC1 registry dependency does not include every correction. Source builds need
Rust 1.85+ and a native C compiler/linker; prebuilt downloads avoid that build.
[All installation routes](docs/INSTALL.md#cargo-registry-cli-and-embedded-rust).
[Stable Rust API](https://docs.rs/n11-deadbolt/1.0.3/deadbolt/) ·
[Earlier RC1 Rust API](https://docs.rs/n11-deadbolt/2.0.0-rc.1/deadbolt/).

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

## 2.0 release candidate in this source

This source matches published **engine 2.0.0-rc.2** and **Python client 2.0.0rc2**.
It consolidates strict destination policy, immutable run deadlines, optional
supervisor-only liveness and the owned-process cancellation reference. Existing
1.x packages and MCPB/plugin bundles stay separately versioned. Publication and
installation checks are separate from a source merge.

[Migration and source compatibility](docs/MIGRATING-V2.md) explains the major Rust
version, changed destination policy and preservation of existing runs. Use a
reviewed candidate checkout or its verified versioned artifacts; the 1.x registry
commands above do not install these new controls. See [local data/privacy](docs/PRIVACY.md).

## Controls you can build on

- Tool allow-lists, destination tokens, reported-spend caps, pause and clips.
- Leases with a default 60-second idle TTL; killed IDs stay killed. Earlier
  rc.1 default roots slide it on activity. Published RC2 defaults
  fresh roots to supervisor renewal, preserving existing modes and offering
  explicit activity setup.
- The 2.0 candidate adds [supervisor-only liveness](docs/SUPERVISION.md):
  a separate root-bound credential maintains live runs and children; agent
  activity cannot renew them. Optional fixed deadlines still win.
- The 2.0 candidate adds [fixed operator-issued deadlines](docs/LEASE-DEADLINES.md)
  for new runs and their children. This blocks new admissions, with no automatic
  cancellation of already running work; published 1.x engine/plugin downloads do not include it.
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

For trusted executors that need to stop already running local commands, the
[owned-process cancellation reference](docs/EXECUTOR-CANCELLATION.md) adds a
source-only POSIX supervisor pattern with disposable acceptance. It does not
change the gate into a sandbox or upgrade published packages.
