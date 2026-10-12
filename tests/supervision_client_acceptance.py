#!/usr/bin/env python3
"""Installed client and real sidecar: separate heartbeat, lost liveness, fixed cutoff.

Uses disposable local state and marker callbacks only. Runs the actual default
60-second idle window rather than manipulating the lease store.
"""
import argparse
from contextlib import ExitStack
import importlib.metadata
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from http.server import BaseHTTPRequestHandler, HTTPServer
from threading import Thread

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
    require(not Path(client.__file__).resolve().is_relative_to(ROOT / "examples"), "install client first")
    original = dict(os.environ)
    server = None
    cases = []
    try:
        with tempfile.TemporaryDirectory(prefix="deadbolt-supervised-consumer-") as directory, ExitStack() as cleanup:
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
                require(result.returncode == 0 if expect_ok else result.returncode != 0,
                        f"operator result for {args[0]}")
                return result.stdout

            def http(path, body, header, expected):
                request = Request(f"http://{endpoint}{path}", json.dumps(body).encode(),
                                  {"Content-Type":"application/json", **header}, method="POST")
                try:
                    response = urlopen(request, timeout=5)
                except HTTPError as error:
                    response = error
                with response:
                    require(response.status == expected, f"HTTP status for {path}")
                    return json.load(response)

            def start():
                process = subprocess.Popen([str(binary), "serve", "--bind", endpoint],
                                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                cleanup.callback(stop, process)
                for _ in range(100):
                    require(process.poll() is None, "sidecar exited")
                    if client.status("readiness").get("agents") == []:
                        return process
                    time.sleep(.05)
                raise RuntimeError("sidecar not ready")

            server = start()
            operator("supervise", "--agent", "run")
            require(client.register_child("run", "child").get("live") is True, "child setup")
            operator("supervisor", "issue", "--agent", "run", "--id", "sup", "--ttl-secs", "600", "--out", str(root / "sup"))
            secret = (root / "sup").read_text()
            if os.name == "posix":
                require((root / "sup").stat().st_mode & 0o777 == 0o600, "private supervisor file")
            cases.append("private_separate_credential")
            operator("credential", "issue", "--agent", "run", "--id", "worker", "--ttl-secs", "600", "--out", str(root / "worker"))
            worker = (root / "worker").read_text()
            initial = client.status("run")["agents"][0]["expires_at"]
            os.environ["DEADBOLT_ADMISSION_TOKEN"] = worker
            require(client.renew_supervised("run", secret).get("ok") is True, "explicit supervisor auth with both environment tokens")
            cases.append("explicit_header_ignores_operator_and_workload_environment")
            for agent in ["run", "child"]:
                os.environ.pop("DEADBOLT_ADMISSION_TOKEN", None)  # Trusted operator fixture dispatch for child.
                effect = root / f"{agent}-allowed"
                require(client.dispatch(agent,"write",lambda:effect.write_text("allowed"))["executed"], "live callback")
            cases.append("root_and_child_effects_before_expiry")
            time.sleep(1.1)
            require(client.renew_supervised("run", secret).get("ok") is True, "heartbeat")
            expiry = client.status("run")["agents"][0]["expires_at"]
            require(expiry > initial, "heartbeat extends idle window")
            require(client.status("child")["agents"][0]["expires_at"] == expiry, "child renewal")
            cases.append("one_heartbeat_maintains_live_root_and_child")
            require(http("/admit",{"agent_id":"run","tool":"read"},{"X-Deadbolt-Supervisor":secret},403)["code"] == "forbidden", "supervisor cannot admit")
            require(http("/renew",{"agent_id":"run"},{"X-Deadbolt-Admission":worker},403)["code"] == "forbidden", "workload cannot renew")
            require(http("/renew",{"agent_id":"run"},{"X-Deadbolt-Token":token},404)["code"] == "not_found", "operator header cannot impersonate heartbeat")
            require(http("/policy",{"agent_id":"run","tools":[]},{"X-Deadbolt-Supervisor":secret},403)["code"] == "forbidden", "supervisor cannot change policy")
            require(http("/renew",{"agent_id":"child"},{"X-Deadbolt-Supervisor":secret},401)["code"] == "unauthorized", "supervisor root binding")
            cases.extend(["supervisor_cannot_admit_or_change_policy", "workload_cannot_renew", "operator_header_cannot_renew", "root_binding_enforced"])
            stop(server)
            server = start()
            require(client.status("run")["agents"][0]["supervision_root"] == "run", "mode persists after restart")
            require(client.renew_supervised("run", secret).get("ok") is True, "credential persists after restart")
            expiry = client.status("run")["agents"][0]["expires_at"]
            cases.append("mode_and_credential_survive_restart")
            os.environ["DEADBOLT_ADMISSION_TOKEN"] = worker
            while time.time() < expiry:
                decision = client.admit("run", "read")
                require(decision.get("decision") == "allow" or decision.get("code") == "lease_expired", "busy workload decision")
                time.sleep(.15)
            for agent in ["run", "child"]:
                os.environ.pop("DEADBOLT_ADMISSION_TOKEN", None)
                effect = root / f"{agent}-after-expiry"
                result = client.dispatch(agent,"write",lambda:effect.write_text("wrong"))
                require(not result["executed"] and result["decision"]["code"] == "lease_expired" and not effect.exists(), "lost supervisor withholds effect")
            cases.extend(["busy_agent_cannot_refresh_idle_ttl", "expired_root_and_child_effects_withheld"])
            require(client.renew_supervised("run", secret).get("ok") is False, "no expired revival")
            operator("resume", "--agent", "run", expect_ok=False)
            operator("supervise", "--agent", "run", expect_ok=False)
            cases.append("expired_identity_cannot_be_renewed_resumed_or_reissued")
            deadline = int(time.time()) + 3
            operator("supervise", "--agent", "fixed", "--hard-deadline-at", str(deadline))
            operator("supervisor", "issue", "--agent", "fixed", "--id", "fixed-sup", "--out", str(root / "fixed-sup"))
            fixed_secret = (root / "fixed-sup").read_text()
            while time.time() < deadline:
                client.renew_supervised("fixed",fixed_secret)
                require(client.status("fixed")["agents"][0]["expires_at"] <= deadline, "deadline unchanged")
                time.sleep(.05)
            effect = root / "fixed-effect"
            result = client.dispatch("fixed","write",lambda:effect.write_text("wrong"))
            require(not result["executed"] and result["decision"]["code"] == "deadline_exceeded" and not effect.exists(), "heartbeat cannot defeat deadline")
            cases.append("fixed_deadline_wins_despite_heartbeats")
            operator("supervise", "--agent", "fresh")
            operator("supervisor", "issue", "--agent", "fresh", "--id", "fresh-sup", "--out", str(root / "fresh-sup"))
            fresh_secret = (root / "fresh-sup").read_text()
            operator("supervisor", "revoke", "--id", "fresh-sup")
            require(client.renew_supervised("fresh",fresh_secret).get("ok") is False, "revoked credential")
            cases.append("revoked_supervisor_cannot_renew")
            events = (root / "events.jsonl").read_text()
            require(all(value not in events for value in [secret,worker,fixed_secret,fresh_secret]), "evidence contains no bearer")
            rows = [json.loads(line) for line in operator("export","--agent","run").splitlines()]
            require(any(row.get("supervision_root") == "run" for row in rows), "issuance export")
            require(any(row.get("renewed") == ["run","child"] and row.get("expires_at") for row in rows), "renewal export")
            cases.append("observed_export_without_bearers")
            stop(server)
            require(client.renew_supervised("fresh",fresh_secret).get("ok") is False, "sidecar loss")
            cases.append("sidecar_loss_cannot_report_liveness")
            # A malformed or misleading success body cannot renew the supervisor's view.
            response = [b'{"ok":true}']
            observed = []
            class Handler(BaseHTTPRequestHandler):
                def do_POST(self):
                    observed.append(dict(self.headers))
                    self.send_response(200)
                    self.send_header("Content-Length",str(len(response[0])))
                    self.end_headers()
                    self.wfile.write(response[0])
                def log_message(self,*args): pass
            fake = HTTPServer(("127.0.0.1",0),Handler)
            thread = Thread(target=fake.serve_forever); thread.start()
            try:
                os.environ.update(DEADBOLT_SOCK=f"127.0.0.1:{fake.server_port}", DEADBOLT_ADMISSION_TOKEN=worker)
                for raw in [b'{"ok":"true"}',b'{"ok":1}',b'{"ok":false,"ok":true}',b'{"ok":true,"cost":NaN}',b'[]']:
                    response[0] = raw
                    require(client.renew_supervised("fresh",fresh_secret).get("ok") is False, "malformed heartbeat success")
                require(all("X-Deadbolt-Supervisor" in headers and "X-Deadbolt-Token" not in headers and "X-Deadbolt-Admission" not in headers for headers in observed), "only explicit supervisor header")
            finally:
                fake.shutdown(); thread.join(); fake.server_close()
            cases.append("malformed_success_and_auth_fallback_refused")
            print(json.dumps({"passed":True,"count":len(cases),"cases":cases,
                              "python_client_version":importlib.metadata.version("n11-deadbolt-client"),
                              "installed_client_outside_examples":True,"actual_idle_ttl_seconds":60,
                              "real_accounts_connected":False}))
    finally:
        stop(server)
        os.environ.clear(); os.environ.update(original)


if __name__ == "__main__":
    main()
