# Supervisor-only liveness (current source, unreleased)

Use this optional mode when the trusted supervisor's continued participation,
not the agent's activity, should keep a run live. Build the current reviewed
source and install its client wheel. Existing engine downloads, PyPI packages,
MCPB bundles and editor plugins do **not** contain this addition. Record the
source revision; the unchanged candidate version string is not its identity.

Two independent controls answer different questions:

| Control | What it bounds | What can extend it |
| --- | --- | --- |
| Idle TTL, legacy mode | Time since the last live admission, including a denial | Live admissions |
| Idle TTL, supervised mode | Time since issuance or a valid supervisor heartbeat | A separate renewal credential, while still live |
| Optional fixed `hard_deadline_at` | The latest time a run may receive a new admission | Nothing; a new run requires a fresh operator-issued ID |

The default CLI idle TTL is 60 seconds. Embedded Rust hosts configure
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
deadbolt supervisor revoke --id review-supervisor-002
```

Configure policy and register children before handing admission access to the
trusted dispatcher. Supervisor credentials can be issued only for a live
supervised **root**. Their independent expiration is 1..86400 seconds and never
slides. The CLI writes a fresh credential file without printing or overwriting
it. Unix mode is 0600; Windows operators must provision an owner-protected
containing-directory ACL. SQLite stores only the SHA-256 hash. Renewal IDs and
admission IDs share an immutable namespace; revoked IDs cannot be reused.

Issuance cannot convert an existing lease's mode. Rust exposes
`issue_supervised_agent(agent_id, optional_deadline)`; trusted HTTP setup uses
`POST /issue-supervised`. `ensure` remains legacy setup and cannot acknowledge
requested supervision. A fresh registered child inherits its parent's
`supervision_root` and deadline, starts no later than its parent's idle expiry,
and does not inherit its policy. Admissions also check the supervision root's
liveness. Keep all workload paths behind fresh admission.

## Run the heartbeat only in trusted supervision

The installed Python source-candidate client provides an explicit call:

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
the same supervision root, in one writer transaction. It cannot revive an
expired or killed root or child, unpause a lease, clear clips, change policy,
consume approvals, admit tools or extend a fixed deadline. Paused leases can
retain their idle window while paused; renewal does not permit their execution.
Revoking a supervisor credential stops its future heartbeats, not the existing
idle window or another authorized supervisor credential. Use operator kill for
immediate admission revocation.

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
  duplicate role headers return 401. Anonymous Unix authority is disabled after
  **any** scoped credential issuance, including supervisor-only issuance; it
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
