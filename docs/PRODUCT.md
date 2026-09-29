# Deadbolt

Deadbolt is a local admission gate for software agents. A trusted executor checks
before each protected action and executes only on explicit allow. Operators can
apply policy, require one-shot approval and revoke an identity and its registered
descendants without asking the model to cooperate.

## Install and integrate

[Deadbolt v1.0.0](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.0)
is available as source and native archives for Linux x86_64, Windows x86_64,
and Apple Silicon/Intel Macs. Verify the download and run the drill using
[installation](INSTALL.md) and [quick start](QUICKSTART.md).

Embed the Rust library, use the local HTTP sidecar from Python/Node or another
language, or route a trusted stdio MCP server through the proxy. The Cargo package
is `n11-deadbolt`; the library and executable are `deadbolt`. Registry packages
are not published. Native binaries are unsigned and Mac binaries are unnotarized.

## Reproduce the evidence

The selected Hermes MCP route and reference filesystem server passed 13 checks:
four permitted writes had the expected contents and nine denied attempts had no
file effect. The checks cover policy, approval, parent/child revocation, outage,
restart and wrong token. Our team ran the tests against third-party software;
this is not independent human validation or Hermes endorsement.

Start with [the evaluator brief](EVALUATOR.md), [demo](DEMO.md), and
[application pilot](PILOT.md). The [Hermes guide](HERMES-SHOWCASE.md) gives the
pinned reproduction steps. Release notes link final source, packaging and upgrade CI.

## Operating boundary

Kill prevents subsequent admissions. It does not terminate an agent process or
cancel a running body. The executor must own identity assignment and require
admission on every protected route. Unset policy lists are open; children need
explicit policy. Spend and destination context are caller-supplied.

The MCP server is trusted. Other action routes and server startup are outside
that selected tool-call gate. Operator credentials and state must remain outside
model-controlled access. See [trust](TRUST.md), [operations](OPERATIONS.md) and
[compatibility](COMPATIBILITY.md).

Deadbolt's own code is Apache-2.0. Bundled components retain their licenses;
see [third-party distribution](../THIRD-PARTY.md).
