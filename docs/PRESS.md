# Deadbolt v1.0.0

29 September 2026

NextEleven has released Deadbolt v1.0.0, a local admission gate for software
agents. A trusted executor checks permission before starting each protected
action and executes only on explicit allow. Operators can apply policy, require
one-shot approval and revoke an identity and its registered descendants.

The release includes a Rust library, local HTTP sidecar with Python/Node source
clients, stdio MCP proxy, and native archives for Linux x86_64, Windows x86_64,
and Apple Silicon/Intel Macs. Source, documentation, checksums and dependency
notices accompany the native distribution.

Tests using pinned Hermes MCP transport and the reference filesystem server
verified 13 cases through actual file effects: four allowed writes ran and nine
denied attempts created no file. NextEleven ran this acceptance against
third-party software. It covers a selected MCP route, not a full Hermes model
conversation, all Hermes tools, independent human validation or endorsement.

The exact released source passed platform/MSRV CI, native packaging drills,
client contracts, backup/restore and the tested legacy-state upgrade. Release
notes link the runs and attached acceptance reports.

Kill blocks later admissions; it does not cancel a body already running.
Application coverage requires a trusted dispatcher and mandatory admission on
all protected routes. This release does not claim whole-agent containment or
security certification. Native binaries are unsigned and Mac binaries are
unnotarized. Systemd remains an experimental template.

- [Release and downloads](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.0)
- [Repository](https://github.com/seanebones-lang/deadbolt)
- [Evaluator brief](EVALUATOR.md)
- [Reproducible demonstration](DEMO.md)
- [Pilot checklist](PILOT.md)

Deadbolt's own code is Apache-2.0; bundled dependencies retain their licenses.
NextEleven LLC. Contact: nextelevenstudios@gmail.com
