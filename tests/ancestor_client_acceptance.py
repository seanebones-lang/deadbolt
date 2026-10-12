#!/usr/bin/env python3
"""Installed Python client and both MCP gate backends with three-level trees.

Only disposable markers and credentials are used. SQL expiry fixtures isolate
ancestor liveness without a long sleep; they do not test resistance to store edits.
"""
import argparse
from contextlib import closing, ExitStack
import json
import os
from pathlib import Path
import secrets
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time

import deadbolt_client as client

ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def stop(process):
    if process is not None and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    binary = parser.parse_args().binary.resolve(strict=True)
    require(not Path(client.__file__).resolve().is_relative_to(ROOT / "examples"),
            "install the client outside the checkout first")
    original = dict(os.environ)
    cases = []
    server = None
    try:
        with tempfile.TemporaryDirectory(prefix="deadbolt-ancestor-consumer-") as directory, ExitStack() as cleanup:
            root = Path(directory)
            for key in list(os.environ):
                if key.startswith("DEADBOLT_"):
                    del os.environ[key]
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", 0))
                endpoint = f"127.0.0.1:{sock.getsockname()[1]}"
            os.environ.update(DEADBOLT_DB=str(root / "state.db"),
                              DEADBOLT_EVENTS=str(root / "events.jsonl"),
                              DEADBOLT_SOCK=endpoint, DEADBOLT_TOKEN=secrets.token_hex(32))

            def operator(*args):
                result = subprocess.run([str(binary), *args], capture_output=True,
                                        text=True, timeout=15)
                require(result.returncode == 0, f"operator {args[0]} failed")

            server = subprocess.Popen([str(binary), "serve", "--bind", endpoint],
                                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            # Release SQLite handles before deleting state; Windows does not
            # allow unlinking an open database, including on assertion failure.
            cleanup.callback(stop, server)
            for _ in range(100):
                require(server.poll() is None, "sidecar exited")
                if client.status("readiness").get("agents") == []:
                    break
                time.sleep(.05)
            else:
                raise RuntimeError("sidecar not ready")

            for supervised in [False, True]:
                mode = "supervised" if supervised else "activity"
                ancestor, parent, grandchild = [f"{mode}-{name}" for name in ["root", "parent", "grandchild"]]
                if supervised:
                    operator("supervise", "--agent", ancestor)
                else:
                    require(client.ensure(ancestor).get("ok") is True, "activity root setup")
                require(client.register_child(ancestor, parent).get("live") is True, "parent setup")
                require(client.register_child(parent, grandchild).get("live") is True, "grandchild setup")
                key_path = root / (mode + ".key")
                operator("credential", "issue", "--agent", grandchild, "--id", mode,
                         "--ttl-secs", "600", "--out", str(key_path))
                worker = key_path.read_text()

                def mcp_call(label, backend, denied=None):
                    effect = root / f"{mode}-{label}-{backend}"
                    downstream = (
                        "import sys,json,pathlib; "
                        "[(pathlib.Path(sys.argv[1]).write_text('ran'), "
                        "print(json.dumps({'jsonrpc':'2.0','id':json.loads(line)['id'],"
                        "'result':{'content':[{'type':'text','text':'ran'}]}}),flush=True)) "
                        "for line in sys.stdin]"
                    )
                    args = [str(binary), "mcp-proxy", "--agent", grandchild]
                    if backend == "socket":
                        args += ["--serve-sock", endpoint]
                    args += ["--", sys.executable, "-c", downstream, str(effect)]
                    env = dict(os.environ)
                    if backend == "socket":
                        env["DEADBOLT_ADMISSION_TOKEN"] = worker
                    frame = {"jsonrpc":"2.0", "id":label, "method":"tools/call",
                             "params":{"name":"read", "arguments":{}}}
                    result = subprocess.run(args, env=env, input=json.dumps(frame) + "\n",
                                            capture_output=True, text=True, timeout=10)
                    require(result.returncode == 0, "MCP proxy completion")
                    reply = json.loads(result.stdout)
                    require(reply.get("id") == label and "error" not in reply, "MCP tool envelope")
                    if denied:
                        require(reply["result"].get("isError") is True, "model-readable denial")
                        require(reply["result"]["_meta"]["io.github.seanebones-lang/deadbolt"]["code"] == denied,
                                "ancestor denial reason")
                        require(not effect.exists(), "denied downstream effect withheld")
                    else:
                        require(effect.read_text() == "ran", "healthy downstream effect")

                for backend in ["embedded", "socket"]:
                    mcp_call("healthy", backend)
                operator("pause", "--agent", parent)
                for scoped in [False, True]:
                    if scoped:
                        os.environ["DEADBOLT_ADMISSION_TOKEN"] = worker
                    effect = root / f"{mode}-paused-{scoped}"
                    result = client.dispatch(grandchild, "read", lambda: effect.write_text("wrong"))
                    require(not result["executed"] and result["decision"].get("code") == "paused"
                            and not effect.exists(), "HTTP ancestor pause withheld effect")
                    os.environ.pop("DEADBOLT_ADMISSION_TOKEN", None)
                for backend in ["embedded", "socket"]:
                    mcp_call("paused", backend, "paused")
                operator("resume", "--agent", parent)
                for backend in ["embedded", "socket"]:
                    mcp_call("resumed", backend)
                with closing(sqlite3.connect(root / "state.db")) as sql, sql:
                    sql.execute("UPDATE leases SET expires_at=0 WHERE agent_id=?", (parent,))
                require(client.status(grandchild)["agents"][0]["state"] == "active", "status is own-lease state")
                for backend in ["embedded", "socket"]:
                    mcp_call("expired-parent", backend, "lease_expired")
                cases.append(mode + "_http_scoped_and_operator_pause_resume")
                cases.append(mode + "_embedded_and_socket_mcp_pause_resume_and_expiry")
            print(json.dumps({"ok":True, "cases":cases}))
    finally:
        stop(server)
        os.environ.clear()
        os.environ.update(original)


if __name__ == "__main__":
    main()
