# Fixed lease deadlines (2.0 candidate)

The operator can issue a fresh run with a fixed Unix-seconds deadline. At or
after that timestamp, new protected admissions deny `deadline_exceeded`, even
if the agent keeps making allowed or denied requests. This is separate from
the default 60-second idle timer. Published rc.1 default setup still slides
that timer on activity before the deadline. Published RC2
defaults new roots to supervisor renewal instead; only explicit legacy activity
setup slides on workload traffic. Idle expiry can deny earlier with `lease_expired`.

This feature belongs to the reviewed 2.0 candidate, **not** the published
v1.0.3 engine, 1.1.0-rc.1 registry engine, or existing 1.x MCPB/plugin downloads.
Follow [2.0 migration and artifact verification](MIGRATING-V2.md). Record the
source commit, binary version and checksums; installing a newer client alone
does not upgrade the engine.

## Issue in trusted setup

Choose a fresh executor-assigned run ID. Calculate a deadline once, before
starting the workload. For example, this trusted Python operator script issues
a ten-minute run to the selected current-source executable:

```python
import subprocess
import time

deadline = int(time.time()) + 600
subprocess.run([
    "/absolute/path/to/reviewed/deadbolt", "issue",
    "--agent", "my-run-002", "--hard-deadline-at", str(deadline),
], check=True)
```

The CLI shares `DEADBOLT_DB` and `DEADBOLT_EVENTS` with your sidecar; configure
those private paths consistently before setup. Embedded Rust uses
`gate.issue_agent("my-run-002", deadline)?`. Operator HTTP uses:

```http
POST /issue HTTP/1.1
X-Deadbolt-Token: <operator secret>
Content-Type: application/json

{"agent_id":"my-run-002","hard_deadline_at":1791766800}
```

The HTTP timestamp is illustrative. Supply an integer strictly in the future,
no greater than 9007199254740991. Fractional, string, null, duplicate or unknown
fields are refused. The time is validated after the SQLite writer lock is
acquired; a request that waits past its proposed deadline cannot issue a run.

`/issue` refuses **every existing ID**, including repeated identical requests,
idle-only leases, expired leases and killed leases. If a response is lost, use
operator `status` to reconcile the stored timestamp before starting the run;
do not treat an ambiguous setup failure as permission. A new authorization
requires a fresh ID. There is no deadline update or extension endpoint.

`ensure_agent` and `POST /ensure` retain their existing-identity behavior: they
create idle-only leases when absent and leave existing rows unchanged. In
published RC2, absent identities instead become supervised roots;
explicit activity setup is available through the migration guide. All variants
preserve existing rows. They
cannot attach, remove or slide a fixed deadline. Current-source `/ensure`
refuses a `hard_deadline_at` field rather than silently acknowledging an idle-only
lease when the caller requested a deadline. Configure policy and approval
after issuance, and give the dispatcher only a scoped admission credential.
Keep `/issue`, CLI access, credentials for operator routes and writable state
outside the workload's reach.

## Children, restart and evidence

New registered children inherit the parent's fixed deadline, including through
multiple generations. Their idle expiry is bounded by that timestamp. Repeated
registration preserves the original row; reparenting and reissuing a child as
a root are refused. A missing, inactive or expired immediate parent cannot create
a live child. Registration's live result is not
an admission decision for the full ancestor chain. Published RC2
checks every ancestor's state, idle expiry and fixed deadline before a tool call
can proceed. Policy remains separate: configure each child's tools, destinations
and approvals explicitly.

SQLite persists the deadline across restart. Existing stores migrate with NULL
deadlines and keep their prior idle-only behavior; opt in using new run IDs.
Status reports `hard_deadline_at` (null for idle-only leases) separately from
`expires_at`. Current source computes `expired` and `deadline_exceeded` on status
reads without rewriting stored state or evidence. Killed state takes precedence;
the fixed deadline takes precedence over idle expiry. Published 2.0.0-rc.1 and
1.x engines report stored state, so their active rows can already be expired.
Status describes only the selected lease's own state and clocks; a healthy child
can still be denied by an inactive ancestor. Status is never a substitute for
fresh tool admission.

Root issuance and child registration record the timestamp as Observed lease
evidence. With the default SQLite/JSONL sink, failed primary issuance evidence
rolls back the new lease. JSONL/text export includes the issuance timestamp;
incident output includes the current run's deadline and observed denial codes.
Optional Witness mirroring occurs after the primary commit: a mirror failure
can report failure after issuance succeeded, so reconcile status before retry.
Custom evidence sinks retain their documented post-commit boundary.

After the deadline, resume is refused. Policy changes, one-shot approvals and
credential activity cannot extend it. Expired runs cannot receive new exact
action approvals or new admission credentials. Deadline denials leave existing
one-shot approvals unconsumed. A killed lease continues to deny `killed`.

## Limits and verification

Keep `enabled=true` and `fail_closed=true`. The clock is the trusted host's
wall clock, not a monotonic or remote time authority; clock adjustments affect
the effective cutoff. Protect host time and the operator state from workloads.
Older executables ignore the new column and do not enforce it. Stop protected
dispatch before rollback; retaining the database is not retaining enforcement.

A deadline blocks **new admissions**. Admission and the external effect are
not atomic: a callback admitted just before the deadline may start or continue
after it. No running process is automatically terminated. The executor must
implement cancellation, process confinement and OS egress controls where
required. `wait_until_revoked` currently watches operator kill only, not this
deadline. Optional [supervisor-only idle renewal](SUPERVISION.md) is a separate 2.0 candidate
source control; its heartbeat also cannot extend the fixed deadline.

Run `cargo test --test deadline` for real-time allow/deny loops, withheld file
effects, writer-lock delay, exact/scoped approval preservation, child inheritance,
restart and old-store migration. `cargo test --all-targets` also checks CLI,
operator HTTP validation, scoped-route refusal, MCP forwarding refusal and
primary-evidence failure. After installing the Python client, run
`python tests/deadline_client_acceptance.py --binary /absolute/path/to/deadbolt`
with Node 22+ to check real loopback Python/Node file effects and scoped access.
CI runs this acceptance on Linux, macOS and Windows. No provider key or real
account is needed.

## Optional supervised idle expiry (2.0 candidate)

[Supervisor-only renewal](SUPERVISION.md) is the default for fresh RC2
root runs. RC1 requires explicit supervisor setup; existing activity-mode runs
retain their original renewal mode. Admissions do not refresh those leases; a
separate root-bound credential maintains only still-live members. A fixed deadline remains immutable
and wins even during heartbeat activity. The explicit Python renewal call never
uses operator/workload environment authentication. Published 1.x artifacts
do not include this feature; source, migration and release gates apply. Running
work still needs executor-owned cancellation and OS isolation where appropriate.
