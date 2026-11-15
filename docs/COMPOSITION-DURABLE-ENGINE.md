# Composition recipe: DeadBolt in front of a durable engine

DeadBolt does not resume work. It is a gate: it admits or refuses each
action, remembers kills and one-use approvals in a private store, and keeps
the evidence. Restart recovery — picking work up after a crash without
repeating what already ran — belongs to the engine or workspace behind the
gate. This recipe is the composition that gives you both: the engine owns
the work and its completion records; DeadBolt owns admission and
revocation. Neither half can do the other's job, and the recipe fails
honestly if you ask it to.

## What persists, what resumes, what the operator does

| Piece | Persists where | Survives a crash of |
|---|---|---|
| Gate state: leases, policy, kills, consumed approvals, evidence log | DeadBolt's private store (database + events file) | the agent process, the engine process, and the sidecar itself (restart on the same store) |
| Work state: sessions, queues, completion records | The engine's own durable store | the engine process |
| An action in flight at the moment of the crash | Nowhere — it is interrupted | nothing; the operator re-drives it (below) |

The one rule that makes the composition sound: **the engine's completion
record and the effect must be the same atomic write, or the effect must be
idempotent.** If an engine records "done" in a separate step after doing
the work, a crash between the two runs the task twice on resume. The
worked example below keeps the effect and its record in a single fsync'd
append for exactly this reason.

## Recipe A — the shipped pairing: Harness

Harness persists sessions outside the process and bundles the DeadBolt
gate (on by default). The composition is already assembled; operating it
after a crash is the recipe:

1. Run work in a session as usual. Every completed turn is in the session
   store; gate leases and decisions are in the gate store
   (`harness deadbolt status` lists them — they outlive the process).
2. After a crash or kill of the host process, relaunch and resume:
   `harness --resume <session>` (a session id prefix is enough). Completed
   turns are not repeated; the session history, including tool results, is
   carried into the next turn.
3. Re-drive the interrupted step with your next prompt. A turn that was in
   flight when the process died is **not** persisted — Harness saves a
   session at turn completion — so the step the crash interrupted runs
   once, when you ask again, and the gate admits or refuses it like any
   other action.
4. Kills and approvals behave as the operator left them: a run killed at
   the gate stays killed across the restart (start a new run rather than
   reviving it), and an approval consumed before the crash is not
   replayed after it — the next identical call is held again.

Verified 2026-11-14 against the v1.4.0-beta.2 binary with a scripted
provider: turn 1 completed and persisted; the host process was SIGKILLed
mid-turn-2 during a provider wait; a new process resumed the session,
executed the interrupted step exactly once, and the gate store showed both
processes' leases and a continuous event record across the kill.

## Recipe B — sidecar in front of any durable engine

The same composition with the standalone sidecar and your own engine,
queue, or workspace. `examples/durable_worker.py` (next to
`examples/deadbolt_client.py`) is a complete minimal engine: a task queue
whose effects ledger is also its completion record.

Operator setup (one terminal, private state per
[private state](PRIVATE-STATE.md)):

```sh
mkdir -p .state && chmod 700 .state
# create .state/operator.token (mode 0600) as in the quickstart
export DEADBOLT_TOKEN=$(cat .state/operator.token)
export DEADBOLT_DB="$PWD/.state/deadbolt.db"
export DEADBOLT_EVENTS="$PWD/.state/events.jsonl"
export DEADBOLT_SOCK="$PWD/.state/deadbolt.sock"
deadbolt serve --bind "$DEADBOLT_SOCK" &
```

Provision the run and its policy before the worker starts (policy needs an
existing lease, so ensure first — the operator, never the worker, holds
this authority; see [credentials](CREDENTIALS.md)):

```sh
python3 -c "from deadbolt_client import ensure; ensure('run-001')"
deadbolt policy --agent run-001 --tools ledger_write
deadbolt credential issue --agent run-001 --id run-001-cred \
  --ttl-secs 3600 --out .state/run-001.cred
printf '["task-1","task-2","task-3"]' > work/queue.json
```

Run the worker with only the scoped admission credential — never the
operator token ([credentials](CREDENTIALS.md)):

```sh
DEADBOLT_SOCK="$DEADBOLT_SOCK" \
DEADBOLT_ADMISSION_TOKEN=$(cat .state/run-001.cred) \
  python3 examples/durable_worker.py --agent run-001 --workdir work
```

The worker asks for admission before each task's effect and stops at the
first denial. After a crash — the worker, the host, or the sidecar — the
recovery procedure is always the same:

1. Restart the sidecar on the **same** store, if it died. Gate state,
   including kills, is exactly where you left it.
2. Restart the worker with the same `--agent` and `--workdir`. It reads
   the effects ledger, skips completed tasks, and continues.
3. If the run was killed at the gate, do not try to revive the id —
   `ensure` and `resume` do not undo a kill. Provision a new run id
   (`run-002`, same policy) against the same workdir; the new run is
   admitted and finishes the remaining queue.

Verified 2026-11-14 against the v2.0.2 release binary: a six-task queue
was SIGKILLed after two effects, resumed and completed two more, was then
killed at the gate mid-queue (next admission denied `killed`, no further
effects); the sidecar itself was SIGKILLed and restarted on the same
store and the kill still held; a fresh run id finished the queue. Final
ledger: each task exactly once, no duplicates; 21 evidence records across
the two runs' exports.

## The boundary, stated plainly

- DeadBolt still does not stop an action that is already running; it
  refuses the *next* admission. The engine's own cancellation story owns
  the in-flight action.
- If the engine's store is lost, the work is lost — the gate's evidence
  log can tell you what was admitted and decided, not reconstruct the
  work.
- A killed run is a revoked identity, not a paused one. Recovery means a
  new run against durable work state, never a resurrection.
