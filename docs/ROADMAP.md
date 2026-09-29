# Adoption roadmap after v1.0.1

The immediate goal is reproducible use by developers outside NextEleven.
This roadmap describes intended work, not completed adoption or release promises.

| Order | Milestone | Completion evidence |
| --- | --- | --- |
| 1 | Reduce first-evaluation friction | A Python-only disposable workflow checks real body effects against the published executable; CI exercises it on Linux, macOS and Windows |
| 2 | Obtain outside developer reproduction | An unfamiliar developer records installation, command, report, environment and confusing steps; failures are resolved or documented |
| 3 | Integrate one adopter-owned dispatcher | Route inventory, trusted identity/policy ownership, harmless acceptance cases and recovery procedure agreed with the adopter |
| 4 | Define a paid integration offer | Deliverables, exclusions, acceptance, support and pricing established from the work required; free software remains independently available |
| 5 | Improve distribution where adoption is blocked | Signing, notarization or registry packages prioritized from observed installation barriers and validated before publication |

Start with [first evaluation](FIRST-EVALUATION.md), then the
[Hermes showcase](HERMES-SHOWCASE.md) and [application pilot](PILOT.md).

Outside developer feedback is an open milestone. Automated tests using upstream
software do not substitute for independent human acceptance. Production claims
require acceptance of the selected application's actual routes and deployment.
Systemd remains experimental until tested on a disposable Linux host.
