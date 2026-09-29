#!/usr/bin/env python3
"""Bolt-on client for `deadbolt serve`. Stdlib only. Not a model tool."""

import argparse
import http.client
import json
import os
import socket
import sys


def sock_path():
    raw = os.environ.get("DEADBOLT_SOCK")
    if raw:
        return raw
    return os.path.join(os.path.expanduser("~"), ".deadbolt", "deadbolt.sock")


def _tcp_target(raw):
    text = raw
    if text.startswith("http://"):
        text = text[len("http://") :]
    elif text.startswith("https://"):
        raise SystemExit("deadbolt:bind_refused")
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
        raise SystemExit("deadbolt:bind_refused")
    return host, int(port)


class _UnixHTTPConnection(http.client.HTTPConnection):
    def __init__(self, path):
        super().__init__("localhost", timeout=5)
        self._path = path

    def connect(self):
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(self.timeout)
        sock.connect(self._path)
        self.sock = sock


def _headers():
    headers = {"Content-Type": "application/json", "Connection": "close"}
    token = os.environ.get("DEADBOLT_TOKEN")
    if token:
        headers["X-Deadbolt-Token"] = token
    return headers


def _down(path):
    if path.startswith("/status"):
        return {"code": "store_unavailable"}
    return {"decision": "deny", "code": "store_unavailable"}


def _call(method, path, body=None):
    try:
        payload = None if body is None else json.dumps(body).encode("utf-8")
        target = _tcp_target(sock_path())
        if target:
            host, port = target
            conn = http.client.HTTPConnection(host, port, timeout=5)
        else:
            conn = _UnixHTTPConnection(sock_path())
        conn.request(method, path, body=payload, headers=_headers())
        resp = conn.getresponse()
        raw = resp.read()
        status = resp.status
        conn.close()
        if status >= 500 or not raw:
            return _down(path)
        return json.loads(raw.decode("utf-8"))
    except Exception:
        return _down(path)


def ensure(agent_id):
    return _call("POST", "/ensure", {"agent_id": agent_id})


def admit(agent_id, tool):
    return _call("POST", "/admit", {"agent_id": agent_id, "tool": tool})


def register_child(parent, child, swarm_task_id=None):
    body = {"parent": parent, "child": child}
    if swarm_task_id:
        body["swarm_task_id"] = swarm_task_id
    return _call("POST", "/register_child", body)


def status(agent_id=None):
    path = "/status" if not agent_id else "/status?agent=" + agent_id
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
        _print(admit(args.agent, args.tool))
    elif args.cmd == "register-child":
        _print(register_child(args.parent, args.child, args.swarm_task))
    elif args.cmd == "status":
        _print(status(args.agent))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
