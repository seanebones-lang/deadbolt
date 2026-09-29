# Demonstrate enforcement with observable effects

For a Python-only first run, use [first evaluation](FIRST-EVALUATION.md).
The following demonstration exercises the third-party Hermes MCP transport.

Use the published v1.0.2 executable and a disposable workspace. Prepare dependencies
before a meeting; their download time is not part of the demonstration. No model
key, paid model call or installed Hermes profile is needed.

## Prepare

Follow [the Hermes reproduction guide](HERMES-SHOWCASE.md) to obtain the pinned
Hermes source, isolated Python 3.12 environment and reference filesystem server.
Use new temporary paths if any example path already contains work.

Download and verify the matching [release archive](RELEASING.md#install-an-archive).
Extract it and note the absolute executable path. Use that path instead of
`target/release/deadbolt` in the acceptance command. This avoids building a
different executable for the demonstration. The archive includes the test and
requirements; run from its extracted root or from the pinned source checkout.

Run once before presenting. A failed assertion or nonzero exit means the
demonstration has failed; inspect the failure before claiming acceptance.

## Present

Run `tests/hermes_mcp_acceptance.py` using the prepared paths from the guide.
It creates temporary state and actual filesystem markers, verifies their content
or absence, and prints a JSON acceptance report. The workspace is removed on exit;
save stdout if a persistent result is needed.

| Show | Observation in the report | Why it matters |
| --- | --- | --- |
| Allowed write | `allowed.txt`, body ran | Protected work can execute |
| Off-policy write | `off-list.txt`, body did not run | Denial prevents the observable body effect |
| Approval | `unapproved.txt` denied, `approved-once.txt` ran, `approval-consumed.txt` denied | One approval permits one admission |
| Operator revocation | `killed-parent.txt` and `killed-child.txt` did not run | Registered descendants are revoked too |
| Attempted reuse | `ensure-does-not-resurrect.txt` did not run | Re-ensuring the identity does not clear revocation |
| Outage and wrong token | Both bodies did not run | These failures do not grant permission |
| Restart | `restart-revoked.txt` did not run; `restart-unrelated.txt` ran | Revocation persists while unrelated work remains usable |

The script checks effects during execution. The report records those assertions;
it is not a screen recording or an independently signed attestation.

## Close with the integration question

Ask the evaluating engineer which dispatcher and action routes they own, what
they need to revoke, and how those routes assign identities. Use [the pilot
checklist](PILOT.md) to define the next proof in their application.

State the scope: this is a selected Hermes MCP route. It does not cover Hermes'
other tools or stop an action already running. Whole-application coverage requires
an inventory and acceptance of all intended routes.
