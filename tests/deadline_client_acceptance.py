#!/usr/bin/env python3
"""Real loopback sidecar: installed Python and copied Node callbacks stop at a fixed deadline."""
import argparse
from contextlib import ExitStack
import importlib.metadata
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

import deadbolt_client as client

ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def stop(server):
    if server is not None and server.poll() is None:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    binary = parser.parse_args().binary.resolve(strict=True)
    require(not Path(client.__file__).resolve().is_relative_to(ROOT / "examples"), "install the client outside examples")
    client_version = importlib.metadata.version("n11-deadbolt-client")
    node = shutil.which("node")
    require(node is not None, "Node is required for this acceptance")
    original = dict(os.environ)
    server = None
    cases = []
    try:
        with tempfile.TemporaryDirectory(prefix="deadbolt-deadline-consumer-") as directory, ExitStack() as cleanup:
            root = Path(directory)
            for key in list(os.environ):
                if key.startswith("DEADBOLT_"):
                    del os.environ[key]
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", 0))
                endpoint = f"127.0.0.1:{sock.getsockname()[1]}"
            token = secrets.token_hex(32)
            os.environ.update(DEADBOLT_DB=str(root / "state.db"), DEADBOLT_EVENTS=str(root / "events.jsonl"),
                              DEADBOLT_SOCK=endpoint, DEADBOLT_TOKEN=token)

            def operator(*args, expect_ok=True):
                result = subprocess.run([str(binary), *args], capture_output=True, text=True, timeout=15)
                require(result.returncode == 0 if expect_ok else result.returncode != 0, f"operator result for {args[0]}")
                return result.stdout

            def http(path, body, expected=200, admission=None):
                headers = {"Content-Type": "application/json"}
                headers["X-Deadbolt-Admission" if admission else "X-Deadbolt-Token"] = admission or token
                request = Request(f"http://{endpoint}{path}", json.dumps(body).encode(), headers, method="POST")
                try:
                    response = urlopen(request, timeout=5)
                except HTTPError as error:
                    response = error
                with response:
                    require(response.status == expected, f"HTTP status for {path}")
                    return json.load(response)

            server = subprocess.Popen([str(binary), "serve", "--bind", endpoint], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            # Stop the server before TemporaryDirectory removes open SQLite files on Windows.
            cleanup.callback(stop, server)
            for _ in range(100):
                require(server.poll() is None, "sidecar exited")
                if client.ensure("readiness").get("ok") is True:
                    break
                time.sleep(.05)
            else:
                raise RuntimeError("sidecar not ready")
            deadline = int(time.time()) + 4
            operator("issue", "--agent", "run", "--hard-deadline-at", str(deadline))
            require(client.register_child("run", "child").get("live") is True, "child setup")
            key = root / "worker-key"
            operator("credential", "issue", "--agent", "run", "--id", "worker", "--ttl-secs", "600", "--out", str(key))
            admission = key.read_text()
            for agent in ["run", "child"]:
                effect = root / f"{agent}-allowed"
                outcome = client.dispatch(agent, "write", lambda: effect.write_text("allowed"))
                require(outcome["executed"] and effect.read_text() == "allowed", "allowed callback")
                cases.append(f"{agent}_effect_before_deadline")
            operator("issue", "--agent", "run", "--hard-deadline-at", str(deadline+600), expect_ok=False)
            cases.append("cli_cannot_extend_existing_run")
            require(http("/issue", {"agent_id":"new", "hard_deadline_at":deadline+600}, 403, admission)["code"] == "forbidden", "scoped control refused")
            cases.append("scoped_issue_refused")
            while time.time() < deadline:
                # Actual admissions keep the idle timer active until the fixed cutoff.
                decision = client.admit("run", "read")
                require(decision.get("decision") == "allow" or decision.get("code") == "deadline_exceeded", "loop decision")
                time.sleep(.03)
            cases.append("active_loop_reaches_fixed_deadline")
            for agent in ["run", "child"]:
                effect = root / f"{agent}-denied"
                outcome = client.dispatch(agent, "write", lambda: effect.write_text("wrong"))
                require(not outcome["executed"] and outcome["decision"]["code"] == "deadline_exceeded" and not effect.exists(), "deadline withheld Python effect")
                cases.append(f"{agent}_python_effect_withheld")
            os.environ["DEADBOLT_ADMISSION_TOKEN"] = admission
            del os.environ["DEADBOLT_TOKEN"]
            effect = root / "scoped-python-denied"
            outcome = client.dispatch("run", "write", lambda: effect.write_text("wrong"))
            require(not outcome["executed"] and outcome["decision"]["code"] == "deadline_exceeded" and not effect.exists(), "scoped Python effect")
            cases.append("scoped_python_effect_withheld")
            shutil.copyfile(ROOT / "examples" / "deadbolt_client.js", root / "client.cjs")
            (root / "node-check.cjs").write_text('''
const client = require('./client.cjs');
const fs = require('fs');
client.dispatch('run', 'write', () => fs.writeFileSync('node-effect', 'wrong')).then(result => {
  if (result.executed || result.decision.code !== 'deadline_exceeded' || fs.existsSync('node-effect')) process.exitCode=1;
}).catch(() => { process.exitCode=1; });
''')
            subprocess.run([node, str(root / "node-check.cjs")], cwd=root, capture_output=True, text=True, check=True, timeout=10)
            require(not (root / "node-effect").exists(), "Node effect withheld")
            cases.append("scoped_node_effect_withheld")
            operator("resume", "--agent", "run", expect_ok=False)
            cases.append("resume_after_deadline_refused")
            os.environ.pop("DEADBOLT_ADMISSION_TOKEN")
            os.environ["DEADBOLT_TOKEN"] = token
            require(client.ensure("run").get("ok") is True, "ensure reconciliation")
            require(client.admit("run", "read")["code"] == "deadline_exceeded", "ensure cannot extend")
            cases.append("ensure_cannot_extend_deadline")
            operator("issue", "--agent", "fresh", "--hard-deadline-at", str(int(time.time())+60))
            fresh = root / "fresh-effect"
            require(client.dispatch("fresh", "write", lambda: fresh.write_text("allowed"))["executed"], "fresh operator run")
            cases.append("fresh_authorized_run_effect")
            print(json.dumps({"passed":True,"count":len(cases),"cases":cases,"python_client_version":client_version,
                              "installed_client_outside_examples":True,"node_client_copied_outside_checkout":True,"real_accounts_connected":False}))
    finally:
        stop(server)
        os.environ.clear()
        os.environ.update(original)


if __name__ == "__main__":
    main()
