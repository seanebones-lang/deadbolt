# A bounded application pilot

Use this worksheet with the evaluating engineering team before connecting
Deadbolt to a real workload. Start with disposable state and harmless bodies.
Agree on scope and acceptance before estimating delivery or making production claims.

## Scope worksheet

| Decision | Record before implementation |
| --- | --- |
| Application and owner | Repository/revision, engineering contact and operator |
| Selected integration | Rust, HTTP sidecar or stdio MCP; exact deployment OS |
| Control problem | The action or failure that must be prevented |
| Protected routes | Every tool, spawn, retry and reconnect path within scope |
| Other routes | Explicitly identify what remains outside the pilot |
| Identity | Trusted assignment, stable reconnect identity, child registration |
| Policy | Tool names, destination context, irreversible classification, spend source and child policy |
| Operator boundary | Who controls credentials, policy, approvals and state; what the model can access |
| Recovery | Stop-dispatch procedure, backup/restore and restart behavior |
| Evidence | Observable body effects, expected decision codes and retained results |

## Delivery sequence

1. Reproduce the release demonstration in the evaluator's environment.
2. Inventory the selected application's actual dispatch paths, including retries
   and delegated children. Resolve missing routes before claiming coverage.
3. Add admission immediately before each protected dispatch; execute only on a
   fresh explicit allow. Do not cache permission or use `probe` as a dry run.
4. Configure operator-owned policy and registered-child policy before dispatch.
5. Run the agreed acceptance cases using harmless bodies and disposable state.
6. Review results together and record remaining gaps before a production decision.

## Minimum acceptance

- Allowed body runs once and produces the expected effect.
- Off-policy tool/destination produces no body effect.
- An irreversible action is denied without approval; one approval permits one
  admission, then the next attempt is denied.
- Kill blocks the identity and registered descendants. Reconnect, retry and
  re-ensure do not restore that identity. An unrelated identity remains usable.
- Restart and the selected recovery procedure preserve revocation.
- Outage, wrong token, malformed/unusable decision and unavailable state do not
  start the body on the chosen integration.
- No inventoried protected route bypasses admission. The model cannot change
  operator credentials, policy, identity assignment or state within the stated boundary.

Record release and application revisions, OS, integration, exact commands,
body observations and failures. Mark each case passed, failed or not exercised.
Do not convert a skipped case into a pass.

## Commercial scope

The Apache-licensed code is available independently of any services. A separate
paid engagement could cover integration, route review, deployment acceptance,
training or support. Agree in writing on deliverables, exclusions, acceptance,
customer responsibilities, support hours, fee and payment terms before starting
such an engagement. This document sets no price or contractual commitment.

Do not promise whole-agent containment, cancellation of running bodies, audited
spend, certification or production readiness based only on the demonstration.
Systemd needs deployment acceptance before it becomes a supported pilot route.

## Handoff

Provide the route inventory, configuration, acceptance results, operating and
rollback procedures, known limits and named operational owner. Use
[operations](OPERATIONS.md) and [compatibility](COMPATIBILITY.md) as the baseline.

