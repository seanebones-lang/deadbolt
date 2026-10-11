#!/usr/bin/env python3
"""Bolt-on client for `deadbolt serve`. Stdlib only. Not a model tool."""

import argparse
import asyncio
import http.client
import inspect
import json
import math
import re
import secrets
import time
import os
import socket
import sys
from urllib.parse import quote

MAX_RESPONSE_BYTES = 1024 * 1024


def sock_path():
    raw = os.environ.get("DEADBOLT_SOCK")
    if raw:
        return raw
    if os.name == "nt":
        return "127.0.0.1:9782"
    return os.path.join(os.path.expanduser("~"), ".deadbolt", "deadbolt.sock")


def _tcp_target(raw):
    text = raw
    if text.startswith("http://"):
        text = text[len("http://") :]
    elif text.startswith("https://"):
        raise ValueError("deadbolt:bind_refused")
    if text.startswith("/") or text.startswith(".") or "/" in text:
        return None
    host = None
    port = None
    if text.startswith("[") and "]:" in text:
        host, _, rest = text[1:].partition("]")
        port = rest[1:]
    elif ":" in text:
        host, port = text.rsplit(":", 1)
    if not host or not port or not port.isdigit():
        return None
    if host not in ("127.0.0.1", "::1"):
        raise ValueError("deadbolt:bind_refused")
    return host, int(port)


class _UnixHTTPConnection(http.client.HTTPConnection):
    def __init__(self, path):
        super().__init__("localhost", timeout=5)
        self._path = path

    def connect(self):
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(self.timeout)
        try:
            sock.connect(self._path)
        except Exception:
            sock.close()
            raise
        self.sock = sock


def _headers():
    headers = {"Content-Type": "application/json", "Connection": "close"}
    workload = os.environ.get("DEADBOLT_ADMISSION_TOKEN")
    if workload is not None:
        headers["X-Deadbolt-Admission"] = workload
        return headers
    token = os.environ.get("DEADBOLT_TOKEN")
    if token:
        headers["X-Deadbolt-Token"] = token
    return headers


def _down(path, code="store_unavailable"):
    if path.startswith("/status"):
        return {"code": code}
    return {"decision": "deny", "code": code}


def _call(method, path, body=None):
    conn = None
    try:
        payload = None if body is None else json.dumps(body, ensure_ascii=False, allow_nan=False, separators=(",", ":")).encode("utf-8")
        target = _tcp_target(sock_path())
        if target:
            host, port = target
            conn = http.client.HTTPConnection(host, port, timeout=5)
        else:
            conn = _UnixHTTPConnection(sock_path())
        conn.request(method, path, body=payload, headers=_headers())
        resp = conn.getresponse()
        raw = resp.read(MAX_RESPONSE_BYTES + 1)
        status = resp.status
        if status in (401, 403):
            return _down(path, "unauthorized" if status == 401 else "forbidden")
        if not 200 <= status < 300 or not raw or len(raw) > MAX_RESPONSE_BYTES:
            return _down(path)
        return json.loads(raw.decode("utf-8"))
    except Exception:
        return _down(path)
    finally:
        if conn is not None:
            conn.close()


def ensure(agent_id):
    return _call("POST", "/ensure", {"agent_id": agent_id})


def admit(agent_id, tool, dest=None):
    body = {"agent_id": agent_id, "tool": tool}
    if dest is not None:
        body["dest"] = dest
    result = _call("POST", "/admit", body)
    if not isinstance(result, dict) or result.get("decision") not in ("allow", "deny"):
        return _down("/admit")
    return result



def _json_snapshot(value, depth=0, budget=None):
    """Copy only interoperable JSON values. Never use user conversion hooks."""
    if budget is None:
        budget = [4096]
    if depth > 32 or budget[0] <= 0:
        raise ValueError("invalid action JSON limits")
    budget[0] -= 1
    if value is None or type(value) is bool:
        return value
    if type(value) is str:
        value.encode("utf-8")  # Lone surrogates have no interoperable encoding.
        return value
    if type(value) in (int, float):
        if not math.isfinite(value) or abs(value) > 9007199254740991 or (value == 0 and math.copysign(1, value) < 0):
            raise ValueError("invalid action number")
        return value
    if type(value) is list:
        return [_json_snapshot(v, depth + 1, budget) for v in value]
    if type(value) is dict and all(type(k) is str for k in value):
        return {_json_snapshot(k): _json_snapshot(v, depth + 1, budget) for k, v in value.items()}
    raise TypeError("exact actions require JSON-native values")


def _action_snapshot(action):
    action = _json_snapshot(action)
    fields = {"version", "agent_id", "tool", "dest", "arguments", "nonce", "expires_at"}
    if type(action) is not dict or set(action) != fields or type(action["version"]) is not int or action["version"] != 1:
        raise ValueError("invalid action envelope")
    for key in ("agent_id", "tool", "nonce", "dest"):
        value = action[key]
        if key == "dest" and value is None:
            continue
        if type(value) is not str or re.fullmatch(r"[A-Za-z0-9_.:/-]{1,128}", value) is None:
            raise ValueError("invalid action token")
    if type(action["arguments"]) is not dict or type(action["expires_at"]) is not int or action["expires_at"] <= 0:
        raise ValueError("invalid action arguments/deadline")
    if len(json.dumps(action, ensure_ascii=False, allow_nan=False, separators=(",", ":")).encode("utf-8")) > 32768:
        raise ValueError("action too large")
    return action


def prepare_action(agent_id, tool, arguments, dest=None, ttl_secs=300):
    """Prepare private review data locally. Does not grant or renew permission."""
    if type(ttl_secs) is not int or not 1 <= ttl_secs <= 86400:
        raise ValueError("invalid action lifetime")
    return _action_snapshot({"version": 1, "agent_id": agent_id, "tool": tool, "dest": dest,
                             "arguments": arguments, "nonce": secrets.token_hex(32),
                             "expires_at": int(time.time()) + ttl_secs})


def admit_action(action):
    """Consume one exact approval. Unsupported/old servers fail closed."""
    try:
        result = _call("POST", "/admit-action", _action_snapshot(action))
    except Exception:
        return _down("/admit-action", "bad_request")
    if not isinstance(result, dict) or result.get("decision") not in ("allow", "deny"):
        return _down("/admit-action")
    return result


def dispatch_action(action, body):
    """Admit a private snapshot then pass its exact arguments to the trusted body."""
    if not callable(body) or inspect.iscoroutinefunction(body):
        raise TypeError("use dispatch_action_async for asynchronous bodies")
    try:
        snapshot = _action_snapshot(action)
    except Exception:
        return {"executed": False, "decision": _down("/admit-action", "bad_request")}
    decision = admit_action(snapshot)
    if decision.get("decision") != "allow":
        return {"executed": False, "decision": decision}
    result = body(snapshot["arguments"])
    if inspect.isawaitable(result):
        if inspect.iscoroutine(result):
            result.close()
        raise TypeError("use dispatch_action_async for asynchronous bodies")
    return {"executed": True, "decision": decision, "result": result}


async def dispatch_action_async(action, body):
    """Snapshot before waiting; never execute changed caller-owned arguments."""
    if not callable(body):
        raise TypeError("action body must be callable")
    try:
        snapshot = _action_snapshot(action)
    except Exception:
        return {"executed": False, "decision": _down("/admit-action", "bad_request")}
    decision = await asyncio.to_thread(admit_action, snapshot)
    if decision.get("decision") != "allow":
        return {"executed": False, "decision": decision}
    result = body(snapshot["arguments"])
    if inspect.isawaitable(result):
        result = await result
    return {"executed": True, "decision": decision, "result": result}


def policy(agent_id, **fields):
    return _call("POST", "/policy", {**fields, "agent_id": agent_id})


def dispatch(agent_id, tool, body, dest=None):
    """Run a trusted synchronous callback only on a fresh explicit allow.

    Returns executed, decision, and (on execution) result. Tool exceptions
    propagate normally; they are not sidecar errors and are never retried.
    """
    if not callable(body) or inspect.iscoroutinefunction(body):
        raise TypeError("deadbolt dispatch requires a synchronous callable; use dispatch_async for async bodies")
    decision = admit(agent_id, tool, dest)
    if decision.get("decision") != "allow":
        return {"executed": False, "decision": decision}
    result = body()
    if inspect.isawaitable(result):
        if inspect.iscoroutine(result):
            result.close()
        raise TypeError("deadbolt dispatch received an awaitable; use dispatch_async")
    return {"executed": True, "decision": decision, "result": result}


async def dispatch_async(agent_id, tool, body, dest=None):
    """Check in a worker thread, then call/await the trusted body once.

    Admission does not block the event loop. Requires Python 3.9+.
    """
    if not callable(body):
        raise TypeError("deadbolt dispatch requires a callable body")
    decision = await asyncio.to_thread(admit, agent_id, tool, dest)
    if decision.get("decision") != "allow":
        return {"executed": False, "decision": decision}
    result = body()
    if inspect.isawaitable(result):
        result = await result
    return {"executed": True, "decision": decision, "result": result}


def spend(agent_id, usd):
    return _call("POST", "/spend", {"agent_id": agent_id, "usd": usd})


def register_child(parent, child, swarm_task_id=None):
    body = {"parent": parent, "child": child}
    if swarm_task_id:
        body["swarm_task_id"] = swarm_task_id
    return _call("POST", "/register_child", body)


def status(agent_id=None):
    path = "/status" if not agent_id else "/status?agent=" + quote(agent_id, safe="")
    return _call("GET", path)


def _print(obj):
    json.dump(obj, sys.stdout)
    sys.stdout.write("\n")


def main(argv):
    parser = argparse.ArgumentParser(description="Deadbolt client")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_ensure = sub.add_parser("ensure")
    p_ensure.add_argument("--agent", required=True)
    p_admit = sub.add_parser("admit")
    p_admit.add_argument("--agent", required=True)
    p_admit.add_argument("--tool", required=True)
    p_admit.add_argument("--dest")
    p_reg = sub.add_parser("register-child")
    p_reg.add_argument("--parent", required=True)
    p_reg.add_argument("--child", required=True)
    p_reg.add_argument("--swarm-task")
    p_status = sub.add_parser("status")
    p_status.add_argument("--agent")
    args = parser.parse_args(argv)
    if args.cmd == "ensure":
        _print(ensure(args.agent))
    elif args.cmd == "admit":
        _print(admit(args.agent, args.tool, args.dest))
    elif args.cmd == "register-child":
        _print(register_child(args.parent, args.child, args.swarm_task))
    elif args.cmd == "status":
        _print(status(args.agent))
    return 0


def cli():
    return main(sys.argv[1:])


if __name__ == "__main__":
    raise SystemExit(cli())
