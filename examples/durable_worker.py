#!/usr/bin/env python3
"""Durable worker: a minimal stand-in for any durable engine or workspace.

The worker owns a task queue and a completion ledger on disk. Before each
task's effect it asks the DeadBolt sidecar for admission; the effect happens
only on an explicit allow. The effects ledger *is* the completion record:
each task's effect is a single fsync'd append of its task id, and on startup
the worker treats ids already in the ledger as done. Keeping the effect and
its completion record in one atomic append matters — recording completion in
a separate state file after the effect leaves a crash window in which the
effect happened but was not recorded, and the task runs twice on resume.
With the ledger as the record, a hard kill (SIGKILL) at any point leaves the
queue resumable with no duplicate effects: on restart the worker skips
completed tasks and continues with the rest. DeadBolt is never asked to
resume anything — the gate's job is admission and revocation; the engine's
job is the work.

Usage:
  DEADBOLT_SOCK=... DEADBOLT_TOKEN=... \
  python3 durable_worker.py --agent run-001 --workdir ./work
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__))))
from deadbolt_client import admit, ensure  # noqa: E402  (examples/deadbolt_client.py)


def _line(obj):
    json.dump(obj, sys.stdout)
    sys.stdout.write("\n")
    sys.stdout.flush()


def _load(path, default):
    try:
        with open(path) as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return default


def _save_atomic(path, obj):
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(obj, f)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)


def main(argv):
    parser = argparse.ArgumentParser(description="Durable worker gated by DeadBolt")
    parser.add_argument("--agent", required=True)
    parser.add_argument("--workdir", required=True)
    parser.add_argument("--tool", default="ledger_write")
    parser.add_argument("--interval", type=float, default=0.7)
    args = parser.parse_args(argv)

    os.makedirs(args.workdir, exist_ok=True)
    queue_path = os.path.join(args.workdir, "queue.json")
    state_path = os.path.join(args.workdir, "worker-state.json")
    effects_path = os.path.join(args.workdir, "effects.log")

    queue = _load(queue_path, None)
    if queue is None:
        return 64  # queue file must be provisioned by the operator/setup step

    # /ensure is trusted setup (operator route). A dispatcher holding only
    # a scoped admission credential cannot call it -- and must not: the
    # operator provisions the lease before handing over the credential
    # (docs/CREDENTIALS.md). Skip ensure in workload mode.
    if os.environ.get("DEADBOLT_ADMISSION_TOKEN") is None:
        ensured = ensure(args.agent)
        if ensured.get("ok") is not True:
            _line({"decision": "deny", "code": ensured.get("code") or "store_unavailable"})
            return 2

    # The effects ledger is the completion record (see module docstring).
    done = set()
    if os.path.exists(effects_path):
        with open(effects_path) as f:
            done = {ln.strip() for ln in f if ln.strip()}
    for task in queue:
        if task in done:
            continue
        decision = admit(args.agent, args.tool)
        if decision.get("decision") != "allow":
            _line({"decision": "deny", "code": decision.get("code"), "task": task})
            return 2
        # The effect: one fsync'd line in the durable effects ledger, which
        # is also the task's completion record.
        with open(effects_path, "a") as f:
            f.write(task + "\n")
            f.flush()
            os.fsync(f.fileno())
        done.add(task)
        _save_atomic(state_path, {"done": sorted(done)})
        _line({"decision": "allow", "task": task})
        time.sleep(args.interval)
    _line({"decision": "complete", "done": sorted(done)})
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
