# External review dispositions — October 2026

The supplied review is input for verification, not a certification of current
source. Its reported 96-test result and repository metrics are not treated as
current measurements. Admission transactions serialize gate state; they do not
make a later external effect atomic or eliminate the admit/execute race.

| Point | Current assessment and action |
| --- | --- |
| Destination checks depend on four recognized tool names | Confirmed. Current source now requires an allowed destination for every tool when `dest_allow` is set, except explicitly operator-classified `offline_tools`. Regression checks cover arbitrary names, actual withheld file effects, proxy forwarding, migration, restart and scoped denial of policy changes. [Migration and limits](DESTINATION-POLICY.md). Published engines remain unchanged until a new release. |
| Denied admits slide TTL | Confirmed and already part of the documented contract. Clarified that it is an idle timer. Scoped credentials have fixed expiry; that bounds authenticated workload admission, not legacy same-user/operator access. Absolute lease lifetime and supervisor-only renewal remain separate proposed features requiring API, persistence, lineage and recovery design. |
| Kill does not cancel in-flight work | Confirmed, explicitly documented. Keep application-owned cancellation separate from admission. A reference process-group supervisor needs distinct POSIX/Windows handling, process identity safety, polling/outage semantics and descendant-cleanup tests. A simple helper must not promise a universal hard stop or cancel the wrong process. No cancellation implementation is claimed here. |
| Operator-name heuristic is case-sensitive | Confirmed. Common names now compare without ASCII case and include `stop` and `shutdown_agent`; the API documentation explicitly calls it a diagnostic heuristic. Do not replace capability isolation with broader prefix matching that gives a false assurance about arbitrary aliases. |
| Spend comes from reported numbers | Confirmed. Clarified the trusted-reporter boundary and provider reconciliation requirement. Scoped admission credentials cannot call spend routes. Do not claim provider metering or atomic budget reservations. Provider integrations need an identified source, authoritative usage evidence and latency/recovery tests. |
| “Kill switch for AI systems” | Reject that broad wording. Use “local execution gate for routed agent effects.” No model shutdown, arbitrary egress containment, universal process termination or legal-compliance claim follows from this project. |

Next engineering proposals: absolute run deadlines with explicit supervisor
renewal; then a tested subprocess supervisor reference. Next adoption evidence:
one independent developer reproducing a bounded route, recording installation
friction and actual allowed/denied/revoked effects. Distribution metrics are
signals to inspect, not proof that engineering quality or market fit is settled.
