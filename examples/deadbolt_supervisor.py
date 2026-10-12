"""Trusted job-scoped heartbeat and owned POSIX executor references.

Heartbeat uses Python's cross-platform standard library. Process cancellation
requires waitid(WNOWAIT): Linux, or macOS with Python 3.13+. No other thread/signal
handler may reap the executor's child. This is not a sandbox; see
EXECUTOR-CANCELLATION.md and SUPERVISION.md.
"""
import json
import math
import os
import signal
import socket
import subprocess
import sys
import time
import threading


class HeartbeatLost(RuntimeError):
    """Trusted supervision stopped; do not continue protected dispatch."""


class SupervisorHeartbeat:
    """Job-scoped heartbeat, separate from admission and owned-process monitoring.

    Enter before dispatch: the first renewal must succeed synchronously. Use
    check() before each new dispatch, and cancel owned work on HeartbeatLost.
    should_continue is trusted lifecycle code, never agent output. It and any
    injected renew callback must be bounded. The default client uses private
    renewal-only authentication. One failed heartbeat stops this helper without
    retries or reissuing a run. Exiting stops heartbeats, not the remaining TTL.
    """
    def __init__(self, agent_id, secret, *, should_continue, interval_secs=10, renew=None):
        _number(interval_secs, "interval_secs")
        if interval_secs > 20:
            raise ValueError("use an interval of at most 20 seconds for the default 60-second TTL")
        if not callable(should_continue):
            raise ValueError("provide a bounded trusted lifecycle callback")
        if type(agent_id) is not str or not agent_id or type(secret) is not str or not secret.strip():
            raise ValueError("agent and private supervisor credential required")
        if renew is None:
            from deadbolt_client import renew_supervised
            renew = renew_supervised
        if not callable(renew):
            raise ValueError("renew must be a bounded trusted callback")
        self._agent, self._secret = agent_id, secret
        self._continue, self._renew = should_continue, renew
        self._interval = interval_secs
        self._stop, self._lost = threading.Event(), threading.Event()
        self._thread, self._started = None, False

    def _beat(self):
        try:
            if self._stop.is_set() or self._continue() is not True or self._stop.is_set():
                raise HeartbeatLost("supervisor liveness lost")
            result = self._renew(self._agent, self._secret)
            if type(result) is not dict or set(result) != {"ok"} or result["ok"] is not True:
                raise HeartbeatLost("supervisor liveness lost")
            return True
        except Exception:
            self._lost.set()
            self._stop.set()
            return False

    def _loop(self):
        while not self._stop.wait(self._interval):
            if not self._beat():
                return

    def __enter__(self):
        if self._started or self._stop.is_set():
            raise HeartbeatLost("heartbeat contexts cannot be restarted")
        self._started = True
        if not self._beat():
            raise HeartbeatLost("initial supervisor renewal failed")
        self._thread = threading.Thread(target=self._loop, name="deadbolt-heartbeat", daemon=True)
        self._thread.start()
        return self

    def check(self):
        if not self._started or self._lost.is_set() or self._stop.is_set():
            raise HeartbeatLost("supervisor liveness lost")

    def close(self):
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=6)
            if self._thread.is_alive():
                self._lost.set()
                raise HeartbeatLost("heartbeat callback did not stop; verify trusted supervisor cleanup")

    def __exit__(self, exc_type, exc, traceback):
        self.close()
        if exc_type is None and self._lost.is_set():
            raise HeartbeatLost("supervisor liveness lost")


def heartbeat_cli():
    """Finite operator-side helper; no credential output or agent subprocess."""
    import argparse
    from pathlib import Path
    parser = argparse.ArgumentParser(description="Trusted job-scoped supervisor heartbeat; never revives expired runs")
    parser.add_argument("--agent", required=True)
    parser.add_argument("--credential-file", required=True, type=Path)
    parser.add_argument("--interval-secs", type=float, default=10)
    parser.add_argument("--max-secs", type=float, required=True)
    args = parser.parse_args()
    try:
        _number(args.max_secs, "max_secs")
        if os.name == "posix" and args.credential_file.stat().st_mode & 0o077:
            raise ValueError("credential file must be private (0600)")
        secret = args.credential_file.read_text().strip()
        end = time.monotonic() + args.max_secs
        # This finite command's scope owns the thread; it is not a detached
        # renewal daemon. close() stops it before the command returns.
        with SupervisorHeartbeat(args.agent, secret, should_continue=lambda: True,
                                 interval_secs=args.interval_secs) as heartbeat:
            print("supervisor heartbeat active; stopping at the requested duration", flush=True)
            while time.monotonic() < end:
                heartbeat.check()
                time.sleep(min(.1, max(0, end-time.monotonic())))
        return 0
    except (HeartbeatLost, ValueError, OSError):
        print("supervisor heartbeat stopped or unavailable; verify operator state before continuing", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 0


def supported():
    return os.name == "posix" and all(hasattr(os, name) for name in
        ("waitid", "WNOWAIT", "WEXITED", "WNOHANG", "P_PID", "killpg"))


def lease_stop_reason(snapshot, agent_id, tool, now=None):
    """Validate one authenticated /status response; None means still live.

    The trusted callback must return fresh status, never cached/fabricated data.
    It must enforce a bounded transport timeout and reject ambiguous JSON.
    """
    if type(snapshot) is not dict or type(snapshot.get("agents")) is not list:
        return "status_unavailable"
    rows = snapshot["agents"]
    if len(rows) != 1 or type(rows[0]) is not dict:
        return "status_unavailable"
    row = rows[0]
    if row.get("agent_id") != agent_id or row.get("state") not in (
            "active", "paused", "killed", "expired", "deadline_exceeded"):
        return "status_unavailable"
    expiry = row.get("expires_at")
    deadline = row.get("hard_deadline_at")
    clips = row.get("clips")
    if (type(expiry) is not int or expiry < 0 or expiry > 9007199254740991
            or "hard_deadline_at" not in row
            or (deadline is not None and (type(deadline) is not int or deadline < 0
                                         or deadline > 9007199254740991))
            or type(clips) is not list or any(type(x) is not str for x in clips)):
        return "status_unavailable"
    if row["state"] == "killed":
        return "killed"
    current = time.time() if now is None else now
    if deadline is not None and current >= deadline:
        return "deadline_exceeded"
    if current >= expiry:
        return "lease_expired"
    if row["state"] in ("expired", "deadline_exceeded"):
        return "status_unavailable"  # Effective state disagrees with its timestamps.
    if row["state"] == "paused":
        return "paused"
    if tool in clips:
        return "clipped"
    return None


def _number(value, name, positive=True):
    if type(value) not in (int, float) or not math.isfinite(value) or value < 0 or (positive and value == 0):
        raise ValueError(name + " must be a finite positive number")


def _signal_group(pid, sig):
    try:
        os.killpg(pid, sig)
    except ProcessLookupError:
        pass


# This owned group leader stays alive through cleanup. In particular, Darwin can
# return EPERM for a group containing only zombies. An alive anchor also retains
# the process-group identity while the real command's descendants finish.
_ANCHOR = r"""
import json, os, signal, socket, subprocess, sys, time
signal.signal(signal.SIGTERM, lambda *_: None)
channel = socket.socket(fileno=int(sys.argv[1]))
channel.sendall(b"ready\n")
if channel.recv(1) != b"G":
    sys.exit(1)
try:
    command = subprocess.Popen(sys.argv[2:], close_fds=True, stdin=subprocess.DEVNULL)
    code = command.wait()
    channel.sendall(json.dumps({"returncode":code}).encode("ascii") + b"\n")
except Exception:
    channel.sendall(b'{"error":"spawn_failed"}\n')
while True:
    time.sleep(3600)
"""


def _cleanup(process, grace_secs, ready):
    # The anchor ignores TERM and stays alive until this final group KILL.
    # No poll()/wait() before the last signal; do not expose its PID to callers.
    failure = None
    if ready:
        try:
            _signal_group(process.pid, signal.SIGTERM)
        except OSError as error:
            failure = error
        time.sleep(grace_secs)
    try:
        _signal_group(process.pid, signal.SIGKILL)
    except OSError as error:
        failure = failure or error
    # Try the final signal and direct-child reap even after a failed TERM.
    # Report the failure rather than claiming that cancellation succeeded.
    code = process.wait(timeout=5)
    if failure is not None:
        raise failure
    return code


def run_owned(agent_id, tool, argv, *, admit, status, dest=None, child_env=None,
              cwd=None, poll_secs=0.1, grace_secs=0.2, max_secs=300,
              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL):
    """Fresh admission -> new session -> read-only monitoring -> group cleanup.

    admit(agent_id, tool, dest) and status(agent_id) are trusted, bounded callbacks.
    All three stop timers (poll, grace, max) are local executor settings; they do
    not change a lease. Callbacks must not execute the command; status must never
    renew the lease. The single fresh admission can refresh its idle TTL.
    Output may use caller-owned files; PIPE is refused to avoid undrained pipes.
    No inherited environment or stdin, shell, retries, or externally supplied PID.
    """
    if not supported():
        raise NotImplementedError("requires POSIX waitid(WNOWAIT); macOS needs Python 3.13+")
    if signal.getsignal(signal.SIGCHLD) == signal.SIG_IGN:
        raise ValueError("supervisor must own child reaping; SIGCHLD cannot be ignored")
    if any(type(value) is not str or not value or "\0" in value for value in (agent_id, tool)):
        raise ValueError("agent_id and tool must be nonempty strings")
    for value, name in ((poll_secs, "poll_secs"), (max_secs, "max_secs")):
        _number(value, name)
    _number(grace_secs, "grace_secs", positive=False)
    if type(argv) not in (list, tuple) or not argv or any(type(x) is not str or not x or "\0" in x for x in argv):
        raise ValueError("argv must be a nonempty sequence of nonempty strings")
    command = tuple(argv)  # Freeze before admission can yield to another thread.
    if not os.path.isabs(command[0]):
        raise ValueError("use an absolute executable path")
    env = dict(child_env or {})
    if any(type(k) is not str or type(v) is not str or k.upper().startswith("DEADBOLT_") for k, v in env.items()):
        raise ValueError("explicit child environment must exclude DeadBolt settings and credentials")
    if stdout == subprocess.PIPE or stderr == subprocess.PIPE:
        raise ValueError("use caller-owned output files, not undrained PIPE")

    def check():
        try:
            return lease_stop_reason(status(agent_id), agent_id, tool)
        except Exception:
            return "status_unavailable"

    reason = check()
    if reason:
        return {"executed": False, "reason": reason, "returncode": None}
    decision = admit(agent_id, tool, dest)
    if type(decision) is not dict or decision.get("decision") != "allow":
        return {"executed": False, "reason": "admission_denied", "decision": decision, "returncode": None}
    reason = check()
    if reason:
        return {"executed": False, "reason": reason, "returncode": None}
    parent, child = socket.socketpair()
    process = None
    ready = False
    try:
        process = subprocess.Popen([sys.executable, "-I", "-c", _ANCHOR, str(child.fileno()), *command],
                                   shell=False, start_new_session=True, close_fds=True,
                                   pass_fds=(child.fileno(),), env=env, cwd=cwd,
                                   stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
        child.close()
        # The anchor launches nothing until our GO, and acknowledges that its
        # TERM handler is installed before grace-based group cleanup is used.
        ready_until = time.monotonic() + 5
        announcement = b""
        while b"\n" not in announcement and len(announcement) < 64:
            remaining = ready_until - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("executor anchor readiness timeout")
            parent.settimeout(remaining)
            chunk = parent.recv(64 - len(announcement))
            if not chunk:
                break
            announcement += chunk
        if announcement != b"ready\n":
            raise RuntimeError("executor anchor unavailable")
        ready = True
        reason = check()
        if reason:
            return {"executed": False, "reason": reason, "returncode": None}
        parent.sendall(b"G")
        parent.setblocking(False)
        started = time.monotonic()
        output = b""
        code = None
        while True:
            reason = check()
            if reason:
                break
            if time.monotonic() - started >= max_secs:
                reason = "executor_timeout"
                break
            try:
                chunk = parent.recv(128)
            except BlockingIOError:
                chunk = None
            if chunk == b"":
                raise RuntimeError("executor anchor disconnected")
            if chunk:
                output += chunk
                if len(output) > 128:
                    raise RuntimeError("invalid executor completion")
                if b"\n" in output:
                    message = json.loads(output)
                    if (type(message) is not dict or set(message) != {"returncode"}
                            or type(message["returncode"]) is not int):
                        raise RuntimeError("command could not start or invalid completion")
                    code = message["returncode"]
                    reason = "completed"
                    break
            if os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None:
                raise RuntimeError("executor anchor exited unexpectedly")
            time.sleep(poll_secs)
    finally:
        try:
            if process is not None:
                _cleanup(process, grace_secs, ready)
        finally:
            parent.close()
            child.close()
    return {"executed": True, "reason": reason, "returncode": code}
