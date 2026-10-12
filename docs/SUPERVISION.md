# Supervisor-only liveness (2.0 candidate)

Use supervisor liveness when the trusted supervisor's continued participation,
not the agent's activity, should keep a run live. Published RC2
defaults fresh `ensure`/`issue` runs to this mode through Rust, CLI and HTTP.
Published 2.0.0-rc.1 still requires explicit supervisor setup and its matching
`2.0.0rc1` wheel. The corrected release is engine `2.0.0-rc.2` with client `2.0.0rc2`;
install its matching files or build both from one reviewed revision as described in [the package guide](PYPI.md#evaluate-the-corrected-source-candidate).
The published 1.x engines, 1.x Python packages, 1.x MCPB bundles and
older stable editor plugins do **not** contain supervisor renewal.
The separately published [MCPB 2.0 preview](INSTALL.md#mcpb-20-preview) supports
explicit supervisor mode, but renewal must be provisioned and operated separately;
its launcher never reads a renewal credential.
Follow [2.0 migration and artifact verification](MIGRATING-V2.md), including
the source revision and checksums; actual GUI/model acceptance and independent reproduction remain separate gates.

Two independent controls answer different questions:

| Control | What it bounds | What can extend it |
| --- | --- | --- |
| Idle TTL, legacy mode | Time since the last live admission, including a denial | Live admissions |
| Idle TTL, supervised mode | Time since issuance or a valid supervisor heartbeat | A separate renewal credential, while still live |
| Optional fixed `hard_deadline_at` | The latest time a run may receive a new admission | Nothing; a new run requires a fresh operator-issued ID |

The default CLI idle TTL is 60 seconds. RC2 accepts the global
`--lease-ttl-secs 1..86400` option; use the same value for trusted issuance,
registration and the running sidecar. It sets the window for future issuance
and renewal; it does not rewrite a persisted expiry or revive a run. A longer
setup window trades slower detection of a lost supervisor for more setup time.
Embedded Rust hosts configure
`DeadboltConfig.lease_ttl_secs`. Both modes deny at idle expiry. `deadline_exceeded`
is checked before `lease_expired`; neither condition consumes an exact-action
approval. Neither admission, setup, resume, policy nor renewal changes a fixed
cutoff. Host wall-clock time is trusted; the executor's independent monotonic
runtime limit is useful alongside these controls.

## Provision through the operator

Protect state, evidence, credential files and the operator token from the
workload. Use the same state paths for the CLI and sidecar. This is trusted setup,
not a model-callable command. The optional deadline must be a future integral
Unix timestamp; omit the option for a supervised run with only idle expiry.

```sh
# Existing protected state/token configuration applies to both CLI and sidecar.
# RUN_DEADLINE_AT is a future Unix timestamp chosen by the operator.
# Use a fresh ID for every run; do not reuse expired or killed identities.
deadbolt supervise --agent review-run-002 --hard-deadline-at "$RUN_DEADLINE_AT"
mkdir -p "$HOME/.deadbolt/credentials"
chmod 700 "$HOME/.deadbolt/credentials"
deadbolt supervisor issue --agent review-run-002 --id review-supervisor-002 \
  --ttl-secs 600 --out "$HOME/.deadbolt/credentials/review-supervisor-002"
deadbolt credential issue --agent review-run-002 --id review-worker-002 \
  --ttl-secs 600 --out "$HOME/.deadbolt/credentials/review-worker-002"
deadbolt supervisor list
```

In RC2 source, register children from the same operator shell:

```sh
deadbolt register-child --parent review-run-002 --agent review-child-002 \
  --swarm-task-id task-002
```

Configure the child before starting it. Registration reports `live=true` or
`live=false`; the latter exits unsuccessfully with `child_inactive`. The result
does not replace fresh admission checks for ancestor liveness and policy. Stop
renewal explicitly when finished: `deadbolt supervisor revoke --id
review-supervisor-002`. Revocation prevents future renewal; use `kill` when new
admissions must stop immediately.

Configure policy and register children before handing admission access to the
trusted dispatcher. Supervisor credentials can be issued only for a live
supervised **root**. Their independent expiration is 1..86400 seconds and never
slides. The CLI writes a fresh credential file without printing or overwriting
it. Unix mode is 0600; Windows operators must provision an owner-protected
containing-directory ACL. SQLite stores only the SHA-256 hash. Renewal IDs and
admission IDs share an immutable namespace; revoked IDs cannot be reused.

Issuance cannot convert an existing lease's mode. Rust exposes
`issue_supervised_agent(agent_id, optional_deadline)`; trusted HTTP setup uses
`POST /issue-supervised`. In published RC2, `ensure_agent` and
`issue_agent` also create supervised roots. CLI `ensure`/`issue` default likewise;
explicit activity renewal uses `--activity-renewal`. Rust compatibility callers
use `ensure_activity_agent`/`issue_activity_agent`, and HTTP setup uses the
strict boolean `activity_renewal: true`. Neither generic `ensure` variant
converts an existing row: inspect its persisted `supervision_root` before
assuming an old identity uses supervision. Use a fresh identity for a new mode.
A fresh registered child inherits its parent's
`supervision_root` and deadline, starts no later than its parent's idle expiry,
and keeps a separate policy and approval ledger. RC2 applies
ancestor capability and reported-spend ceilings without copying policy or grants.
Published RC2 checks every registered
ancestor's liveness during admission, including intermediate parents, before
refreshing the requested lease or consuming an approval. The chain must end at
the declared supervision root, with consistent mode bindings and no missing or
cyclic links. Published 2.0.0-rc.1 checks the supervision root but can skip an
inactive intermediate parent. Keep all workload paths behind fresh admission.

## Run the heartbeat only in trusted supervision

The installed RC2 Python client provides an explicit call:

```python
from pathlib import Path
import deadbolt_client as client

# Read privately in the trusted supervisor; never log, put in argv or pass to a child.
renewal_secret = Path(supervisor_credential_path).read_text()

# Invoke only when the trusted host decides this run should continue.
# Schedule comfortably within its idle window, accounting for transport delays.
if host_wants_run_to_continue:
    result = client.renew_supervised("review-run-002", renewal_secret)
    if result.get("ok") is not True:
        stop_protected_dispatch_and_owned_work()
```

`host_wants_run_to_continue` and `stop_protected_dispatch_and_owned_work` belong
to the application's trusted lifecycle. Do not base liveness on agent text or
start an unconditional heartbeat thread that survives the supervising job.
Schedule renewal with ample margin: a default 60-second lease could use a
10-second interval with bounded transport timeouts. Credential expiry, host
scheduling delays, contention and primary evidence failure can stop renewal.
Treat all unsuccessful responses as lost liveness and stop protected work.

`renew_supervised` sends **only** `X-Deadbolt-Supervisor`, even if operator or
admission environment variables exist in that process. It does not fall back to
other credentials. There is deliberately no model-facing supervisor environment
variable. Operator status remains separately authenticated and read-only;
renewal-only credentials cannot read it.

The heartbeat maintains the root and still-live registered descendants carrying
the same supervision root, in one writer transaction. Renewal considers each
lease individually: a still-live grandchild may receive a new idle expiry while
its expired intermediate parent remains expired. In current RC2 source,
fresh admission still denies that grandchild because the full ancestor chain is
checked. Own-lease status is not permission to execute. Renewal cannot revive an
expired or killed root or child, unpause a lease, clear clips, change policy,
consume approvals, admit tools or extend a fixed deadline. Paused leases can
retain their idle window while paused; renewal does not permit their execution.
Revoking a supervisor credential stops its future heartbeats, not the existing
idle window or another authorized supervisor credential. Use operator kill for
immediate admission revocation.

## Job-scoped helper (RC2 source)

The RC2 release adds `SupervisorHeartbeat` to `deadbolt_supervisor` and the finite
`deadbolt-supervisor-heartbeat` operator command. The published `2.0.0rc1` wheel
does not contain them. Install the matching `2.0.0rc2` release wheel or reviewed
source; updating documentation alone does not change an installed package.

Provision the run and its private renewal credential as above, then start the
helper **within the 60-second idle window**. For the preview MCPB, the launcher
issues the run on enable; provision its supervisor credential immediately
after that and start the helper before requesting model tools. The launcher
does not hold this credential. If setup takes too long, use a fresh run ID;
late renewal, restart and resume cannot revive expiry.

```sh
# Run in the trusted supervisor's terminal, with the same DEADBOLT_SOCK.
# Keep the private key file outside the model and downstream environment.
deadbolt-supervisor-heartbeat --agent review-run-002 \
  --credential-file "$HOME/.deadbolt/credentials/review-supervisor-002" \
  --interval-secs 10 --max-secs 600
```

This command maintains liveness only for the operator-chosen finite duration,
stops on renewal failure, and exits without printing the credential. Ctrl-C
stops its heartbeat. It is not a detached service or an agent-controlled renewal
loop. Stop protected dispatch if it exits unexpectedly. Ending the helper does
not instantly kill the run: its remaining idle window still applies. Use operator
kill for immediate admission revocation.

For an application, tie renewal directly to its trusted job lifecycle:

```python
from deadbolt_supervisor import SupervisorHeartbeat, HeartbeatLost

try:
    with SupervisorHeartbeat(run_id, renewal_secret,
                             should_continue=trusted_job.is_running,
                             interval_secs=10) as heartbeat:
        while trusted_job.has_work():
            heartbeat.check()   # Refuse continuation after heartbeat failure.
            dispatch_next_with_fresh_admission()
except HeartbeatLost:
    stop_protected_dispatch_and_cancel_owned_work()
```

The first renewal must succeed before the context enters. Later failures stop
renewal without retries; `check()` and normal context exit report lost liveness.
The context is single-use and closes its thread with a bounded join. The trusted
lifecycle callback and any injected renewal callback must themselves be bounded;
a blocked callback cannot be forcibly terminated by Python, and cleanup failure
is reported rather than treated as success. One already-started renewal may finish
while the helper is closing. No reissue, resume, policy mutation or automatic
execution is performed. Keep long-running work behind the separate owned-process
monitor or your executor's cancellation controls; the heartbeat alone cannot
cancel it.

Tests use installed modules, synthetic renewal credentials and disposable effects:
`python tests/heartbeat_contract.py` and
`python tests/supervision_client_acceptance.py --binary <candidate-binary>`.
The latter also replays s1–s3 over real MCP stdio: allowed effect, expired-run
tool error with no effect, failed late renewal, then the same terminal tool error.
That protocol test does not replace a graphical model rerun of a new bundle.

## HTTP contract and evidence

- `POST /issue-supervised`: operator-authenticated `{ "agent_id": "run" }`,
  optionally with `"hard_deadline_at": <integer>`. Existing IDs, duplicate or
  unknown fields, null/non-integral/string deadlines and nonfuture deadlines
  are refused. Success is `{"ok":true}`.
- `POST /renew`: only `X-Deadbolt-Supervisor: <secret>` and
  `{ "agent_id": "root-run" }`. Success is `{"ok":true}`. No extra or duplicate
  fields. Invalid, expired, revoked or wrong-root credentials return 401;
  malformed bodies and expired-run renewal return 400. Other failures cannot
  authorize continuation even when represented as HTTP 200 with `ok:false`.
- A supervisor header on any other route returns 403. An admission header cannot
  renew; a full operator header alone has no renewal route (404). Mixed or
  duplicate role headers return 401. RC2 requires an operator token
  for Unix startup by default. In explicit legacy Unix mode, anonymous authority
  is disabled after **any** scoped credential issuance, including supervisor-only issuance; it
  stays disabled after all credentials expire or are revoked.

Status and incident output include nullable `supervision_root`. Root issuance
and child registration record their binding as Observed evidence. Credential
issue/revoke records contain metadata, never bearer values. `supervisor_renew`
records the maintained root expiry and the exact renewed identity list; JSONL
and text exports retain those fields. Authorization, live-state checks,
mutations and default primary evidence share a writer transaction, with time
checked after acquiring the lock. Failed primary evidence rolls back renewal.
An optional Witness/custom sink can fail after commit; such an error is not
permission to continue. Inspect authenticated status and evidence rather than
assuming the mutation rolled back.

## Migration, cancellation and limits

Upgrade adds nullable `leases.supervision_root` and an admission-credential
`scope` column defaulting to `admission`. Existing leases keep admission-based
renewal, old credentials keep their authority, and terminal kills stay terminal.
Restart preserves mode, binding, deadline, credentials and revocation. Older
binaries ignore the new mode and can refresh supervised leases on admissions:
**quiesce protected execution before rollback**. Back up and validate migration;
see [operations](OPERATIONS.md) and [compatibility](COMPATIBILITY.md).

The gate stops new admissions; a lost heartbeat does not itself terminate a
running tool. Combine it with an executor that actually owns and cancels the
work, such as the separately tested [owned-process reference](EXECUTOR-CANCELLATION.md).
That helper keeps status polling read-only; arrange heartbeat independently in
trusted lifecycle code. No implicit renewal was added to it. Same-user file
access, unrouted effects, remote jobs and subprocesses escaping the owned group
require OS/process isolation and separate cleanup. Supervision does not turn
policy into a sandbox or independently measure spend.

## Reproduce the checks

```sh
cargo test --locked --test supervision
cargo test --locked --lib serve::credential_tests
cargo build --release --locked --bin deadbolt
python -m pip install build==1.3.0
python tests/python_package_acceptance.py --install-sdk
python tests/supervision_client_acceptance.py --binary target/release/deadbolt
```

Use `target/release/deadbolt.exe` on Windows. The installed-client acceptance uses
synthetic credentials, actual 60-second idle expiry, a busy admission loop,
root/child marker callbacks, restart, credential refusal, a fixed deadline that
wins despite heartbeats, misleading success responses and an unavailable
sidecar. It connects no real accounts. CI exercises the installed client on
Linux, macOS and Windows. Cross-platform gate tests are distinct from the
POSIX-only process cancellation helper's platform support.
