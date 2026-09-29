# Evaluate Deadbolt v1.0.0

Deadbolt gives an operator a local admission gate in front of protected agent
actions. A trusted executor checks permission before starting each body and runs
it only on an explicit `allow`. The operator can revoke an identity and its
registered descendants without asking the model to cooperate.

## Who it fits

Teams that own their agent dispatcher or can route a selected trusted stdio MCP
server through a proxy. A good first evaluation has one concrete control need:
tool restrictions, explicit approval, revocation or failure handling.

Installation alone does not cover an application's other action paths. The team
must be able to keep operator credentials, policy and state outside model-controlled
access and make admission mandatory on every route it intends to protect.

## What is available

- Embedded Rust library for a dispatcher owned by the application.
- Local HTTP sidecar with Python and Node source clients.
- Stdio MCP proxy for individual routed `tools/call` messages.
- Native executables for Linux x86_64, Windows x86_64, and Apple Silicon/Intel Macs.

The [v1.0.0 release](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.0)
contains checksums, source identity, source code, examples, documentation and
third-party notices. Deadbolt's own code is Apache-2.0; bundled dependencies retain
their licenses. Native binaries are unsigned and Mac binaries are unnotarized.
Python/Node clients are included source files; registry packages are not published.

## Evidence an evaluator can reproduce

Pinned Hermes MCP transport and the reference filesystem server passed 13 checks:
four allowed writes produced expected contents and nine denied attempts produced
no file. These include policy denial, one-shot approval, parent/child kill,
re-ensure of a killed ID, service outage, restart and wrong token. An unrelated
identity remained usable. This exercises third-party software; our team ran the
test. It is not independent human validation or Hermes endorsement.

The exact release source passed Linux/macOS/Windows source CI, Rust 1.85 checks,
four native packaging drills, Python/Node contracts, backup/restore and a tested
legacy-state upgrade. See the release's linked CI and attached acceptance reports.

## Boundaries to review before a pilot

Kill blocks subsequent admissions; it does not terminate the agent process or
cancel a body already running. Admission and execution are separate operations.
Caller-supplied destination and spend are not interception or independent billing
measurement. Policy lists are open when unset. Children need explicit policy.
The sidecar token authorizes all exposed routes, not just one agent identity.
The MCP server is trusted; its startup and other protocol methods are outside the
tool-call gate. Evidence is operator-writable. Systemd remains experimental.

The Hermes demonstration covers its selected MCP route, not built-in terminal,
browser, delegation or a full model conversation.

## Start with one dispatcher

1. [Install](INSTALL.md) the pinned release and run `deadbolt --version` and
   `deadbolt drill` in disposable state.
2. [Reproduce the demonstration](DEMO.md), checking actual body effects.
3. Select an [integration](INTEGRATION.md) and enumerate the application's action routes.
4. Agree on the [pilot scope and acceptance criteria](PILOT.md).

For questions use [FAQ](FAQ.md), [troubleshooting](TROUBLESHOOTING.md),
[operations](OPERATIONS.md) and [compatibility](COMPATIBILITY.md).

An evaluation succeeds when the selected application's protected bodies cannot
start without a fresh explicit allow, including during the agreed failure cases.

