# Developer adoption roadmap

The immediate goal is reproducible use by developers outside NextEleven.
This roadmap describes intended work, not completed adoption or release promises.

The 2.0 series provides an installable Python client, a tested OpenAI function-tool
adapter, scoped admission credentials, exact-action grants and bounded leases.
Corrected source also requires explicit proxy policy and operator authentication,
checks complete ancestor liveness/policy, defaults fresh roots to supervision and
creates private Unix state. Follow [next architecture decisions](ARCHITECTURE-NEXT.md)
for rationale and [release gates](RELEASING.md) for artifact requirements.
Published older versions keep their original behavior. A source test, published
package, listing or submission does not establish adoption or client acceptance.

| Order | Milestone | Completion evidence |
| --- | --- | --- |
| 1 | Reduce first-evaluation friction | A Python-only disposable workflow checks real body effects against the published executable; CI exercises it on Linux, macOS and Windows |
| 2 | Obtain outside developer reproduction | An unfamiliar developer records installation, command, report, environment and confusing steps; failures are resolved or documented |
| 3 | Integrate one adopter-owned dispatcher | Route inventory, trusted identity/policy ownership, harmless acceptance cases and recovery procedure agreed with the adopter |
| 4 | Verify each developer client and adapter | The exact artifact is installed in the actual client; model-invoked allowed/denied effects, pause, kill, restart, outage and liveness are recorded without guessing from source tests |
| 5 | Maintain dependable free distribution | Matching native, Cargo, Python and MCP artifacts have immutable identities, installation/upgrade receipts and accurate instructions; platform signing and provenance have explicit coverage |

Start with [first evaluation](FIRST-EVALUATION.md), then the
[Hermes showcase](HERMES-SHOWCASE.md) and [application pilot](PILOT.md).

Outside developer feedback is an open milestone. Automated tests using upstream
software do not substitute for independent human acceptance. Production claims
require acceptance of the selected application's actual routes and deployment.
Systemd passed a bounded disposable-container test; native Linux host and adopter
application acceptance remain open.

Free software and documentation remain independently usable. Funding and optional
implementation services are separate company decisions, not an installation gate.
