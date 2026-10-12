# Stop work owned by a trusted executor

This **unreleased source addition** supplies `deadbolt_supervisor.run_owned` in
source-built Python wheels. The published `n11-deadbolt-client==1.1.0rc1` wheel
predates it. Install from this checkout using the [Python build instructions](PYTHON.md).
A source check does not upgrade an existing native binary, plugin or registry package.

The core gate denies the next connected admission. It does not stop a callback or
process that has already been admitted. This optional executor reference supplies
that second step for processes it starts: a fresh admission, a new POSIX session,
read-only lease monitoring, then TERM and KILL to the owned process group. A small owned group leader stays alive until cleanup finishes; the real command
is its child. The helper reaps that leader and stops lingering group members even
when the command finishes.
Nonzero exit status is preserved, and commands are never retried.

## Supported hosts

Linux with `os.waitid` and `WNOWAIT`, or macOS with Python 3.13 or newer.
Windows and older macOS Python refuse before admission or process creation.
The base HTTP client still supports Python 3.9+ on its documented hosts. Windows
process-tree cancellation needs a separate executor using Job Objects; this
reference does not substitute a direct-child kill for that guarantee.

The supervisor must own its child's wait lifecycle. No other thread, SIGCHLD
handler, framework child reaper or auto-reaping signal disposition may reap it.
The helper retains an alive group leader until the last signal and uses a
non-reaping check for unexpected leader exits. It never accepts an outside PID or a cached process group.
See Python's [waitid and WNOWAIT documentation](https://docs.python.org/3/library/os.html#os.waitid)
and [new-session process creation](https://docs.python.org/3/library/subprocess.html#subprocess.Popen).

## Small integration

Provision a fresh lease and policies through your existing operator setup first.
Run this in the **trusted supervisor**, with the operator transport credential in
that process only. Do not expose this module, `status`, or operator credentials as
model-callable tools. Admission credentials cannot read `/status`; they remain
restricted to their admission routes. The child receives no inherited environment,
stdin or extra file descriptors. Explicit child environment entries must exclude
all `DEADBOLT_` variables; do not put secrets into argv or output files either.

```python
import sys
import deadbolt_client as client
from deadbolt_supervisor import run_owned

result = run_owned(
    "review-run", "read", [sys.executable, "-c", "print('disposable work')"],
    admit=client.admit,
    status=client.status,
    max_secs=120,
)
print(result)  # executed, reason, returncode; no child output captured by default
```

`admit(agent_id, tool, dest)` must make exactly one fresh admission for the reviewed
operation. `status(agent_id)` must return fresh authenticated `/status` JSON with
bounded I/O timeouts and a strict JSON decoder. The included Python transport
rejects duplicate keys and nonfinite JSON constants. A missing, ambiguous, wrong-agent
or malformed status stops execution. Status from an older binary without
`hard_deadline_at` is refused; use an engine with the fixed-deadline status contract.
Neither callback may execute the tool body. These are trusted callbacks, not model
output. Callbacks that hang, fabricate state or reap children violate this
reference's contract; status callbacks must also never refresh TTL. The single
fresh admission can refresh the idle timer once, as the core contract specifies.

The executor checks status before admission and immediately before starting work.
An admission and process creation are not an atomic operation: a revocation between
them can still start a process, which the next check stops. One-shot approvals are
consumed at admission even if the subsequent status check or spawn fails. This
helper takes an ordinary tool admission; use a separately reviewed exact-action
admission callback for actions requiring bound arguments, and freeze the command
and its argument mapping as part of that review.

## Stop conditions and results

- `killed`, `paused` or `clipped`: stop the owned work when the operator changes that state.
- `deadline_exceeded` or `lease_expired`: stop when the authenticated lease's fixed
  deadline or idle expiry is reached using the host wall clock.
- `status_unavailable`: stop on unreadable, invalid, unauthorized or unavailable status.
- `executor_timeout`: stop at the executor's separate monotonic maximum runtime.
- `completed`: retain the command's reported exit code, while cleaning lingering group members.
- `admission_denied`: start nothing. A pre-spawn status refusal also has `executed=False`.

Read-only status never renews idle TTL or spends another approval. A long command
can therefore expire while running: choose the lease's operator-provisioned TTL
for the intended job, or build a separate trusted liveness supervisor. Supervisor-only
renewal is a separate future feature, not something this helper silently enables.

Cancellation latency includes status I/O, polling, host scheduling, the TERM grace
and process cleanup. The included transport has five-second socket timeouts, which
are not a strict whole-response deadline against a slow-drip peer. Process creation
and uninterruptible OS work can also delay completion. `max_secs` starts after spawn
and cannot interrupt a blocking callback. Do not advertise an instantaneous stop.
Cleanup errors propagate; a returned result means the signals were issued and the
owned group leader was reaped, not that every descendant was independently reaped.
A stopped command has `returncode=None` because its final status was not observed;
`executed=True` means the supervisor released the command to start, not proof of
any specific effect.

Ordinary exceptions and `KeyboardInterrupt` clean up before propagating. In a
main-thread CLI, translate SIGTERM into an exception in your supervisor's own signal
handler if you need graceful termination. This library does not install global
signal handlers. A supervisor SIGKILL, crash or second interrupt during cleanup
requires an OS service/container process-lifetime boundary; Python finally blocks
cannot provide that guarantee.

## Trust limits

This is process lifecycle management, **not an arbitrary-code sandbox**. A child
that creates a different session, delegates work to another service, reaches a
writable operator store or steals credentials is outside the group guarantee.
It runs with the supervisor user's privileges. Use separate users, read-only
mounts, OS egress policy and appropriate container/service process boundaries for
untrusted code. `dest_allow` still checks declared destinations, not real network
traffic. Admission spend reports are still trusted harness input, not provider billing.
The helper does not roll back effects performed before cancellation and does not
cancel existing Python threads, coroutines or unrelated processes.

Use absolute executable paths and `shell=False`. Output defaults to DEVNULL;
pass caller-owned files when needed. Undrained PIPE output is refused so a verbose
child cannot block this reference through a full output pipe. Limit file growth in
your executor if outputs may be large.

## Reproduce the disposable acceptance

Build the binary and source wheel, install that wheel outside the checkout, then:

```sh
python tests/executor_acceptance.py --binary /absolute/path/to/deadbolt
```

The acceptance starts a temporary loopback sidecar, uses synthetic agents and
marker-writing processes, and tests operator kill, fixed deadline, unavailable
status, TERM-resistant work, interrupted supervision, ordinary completion,
parent-exit descendant cleanup and an unrelated group that must survive. No real
accounts, shell payloads, remote tools or user files are involved. CI tests Linux
and modern macOS process cleanup and Windows' explicit refusal separately.
