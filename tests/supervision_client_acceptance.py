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
import sys
import sysconfig
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from http.server import BaseHTTPRequestHandler, HTTPServer
from threading import Thread

import deadbolt_client as client
from deadbolt_supervisor import SupervisorHeartbeat, HeartbeatLost

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
            def mcp_call(label, denied=None):
                effect=root / (label+'-mcp-effect')
                child="import sys,json,pathlib; " \
                      "[(pathlib.Path(sys.argv[1]).write_text('ran'), print(json.dumps({'jsonrpc':'2.0','id':json.loads(line)['id'],'result':{'content':[{'type':'text','text':'ran read'}]}}),flush=True)) for line in sys.stdin]"
                frame={'jsonrpc':'2.0','id':label,'method':'tools/call','params':{'name':'read','arguments':{}}}
                result=subprocess.run([str(binary),'mcp-proxy','--agent','run','--serve-sock',endpoint,'--',
                                       sys.executable,'-c',child,str(effect)],
                                      env={**os.environ,'DEADBOLT_ADMISSION_TOKEN':worker},
                                      input=json.dumps(frame)+'\n',capture_output=True,text=True,timeout=10)
                require(result.returncode==0, 'MCP process completes')
                reply=json.loads(result.stdout)
                require(reply['id']==label and 'error' not in reply, 'tool result preserves request id')
                if denied:
                    require(reply['result']['isError'] is True, 'denial is model-facing tool result')
                    require(reply['result']['_meta']['io.github.seanebones-lang/deadbolt']=={'code':denied,'retry':'never'}, 'terminal denial hint')
                    require(reply['result']['content'][0]['text']=='DeadBolt denied: '+denied+'. Do not retry this run.', 'exact model reason')
                    require(not effect.exists(), 'denied MCP body withheld')
                else:
                    require(effect.read_text()=='ran' and not reply['result'].get('isError',False), 'live MCP effect')
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
            with SupervisorHeartbeat('run',secret,should_continue=lambda:True,interval_secs=1) as heartbeat:
                heartbeat.check()
                mcp_call('s1')
            expiry = client.status("run")["agents"][0]["expires_at"]
            os.environ["DEADBOLT_ADMISSION_TOKEN"] = worker
            while time.time() < expiry:
                decision = client.admit("run", "read")
                require(decision.get("decision") == "allow" or decision.get("code") == "lease_expired",
                        "busy workload decision: " + str(decision.get("code", decision.get("decision"))))
                time.sleep(.5)
            for agent in ["run", "child"]:
                os.environ.pop("DEADBOLT_ADMISSION_TOKEN", None)
                effect = root / f"{agent}-after-expiry"
                result = client.dispatch(agent,"write",lambda:effect.write_text("wrong"))
                require(not result["executed"] and result["decision"]["code"] == "lease_expired" and not effect.exists(), "lost supervisor withholds effect")
            cases.extend(["busy_agent_cannot_refresh_idle_ttl", "expired_root_and_child_effects_withheld"])
            mcp_call('s2','lease_expired')
            require(client.renew_supervised("run", secret).get("ok") is False, "no expired revival")
            mcp_call('s3','lease_expired')
            cases.append("s1_s2_s3_model_readable_mcp_results_and_withheld_effects")
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
            # The installed helper scopes renewal to a trusted finite job.
            server = start()
            operator("supervise", "--agent", "helper")
            operator("supervisor", "issue", "--agent", "helper", "--id", "helper-sup", "--out", str(root / "helper-sup"))
            helper_secret = (root / "helper-sup").read_text()
            marker = root / "helper-effect"
            try:
                with SupervisorHeartbeat("helper", helper_secret, should_continue=lambda:True, interval_secs=.05) as heartbeat:
                    heartbeat.check()
                    result=client.dispatch("helper","read",lambda:marker.write_text("ok"))
                    require(result["executed"] and marker.read_text()=="ok", "helper permits live admitted marker")
                    operator("supervisor", "revoke", "--id", "helper-sup")
                    require(heartbeat._lost.wait(5), "helper detects revoked credential")
                    heartbeat.check()
            except HeartbeatLost:
                require(not heartbeat._thread.is_alive(), "helper thread cleaned up")
            else:
                raise RuntimeError("helper allowed continuation after renewal loss")
            stopped_count=len([json.loads(line) for line in operator("export","--agent","helper").splitlines()
                               if json.loads(line).get("kind")=="supervisor_renew"])
            time.sleep(.1)
            require(len([json.loads(line) for line in operator("export","--agent","helper").splitlines()
                         if json.loads(line).get("kind")=="supervisor_renew"])==stopped_count, "helper has no detached renewals")
            cli_run=subprocess.run([str(Path(sysconfig.get_path('scripts')) / ('deadbolt-supervisor-heartbeat.exe' if os.name=='nt' else 'deadbolt-supervisor-heartbeat')),
                                    "--agent","helper","--credential-file",str(root / "helper-sup"),"--max-secs","1"],
                                   capture_output=True,text=True,timeout=10)
            require(cli_run.returncode!=0 and helper_secret not in cli_run.stdout+cli_run.stderr, "finite CLI refuses revoked secret without printing it")
            operator("supervise", "--agent", "helper-cli")
            operator("supervisor", "issue", "--agent", "helper-cli", "--id", "helper-cli-sup", "--out", str(root / "helper-cli-sup"))
            cli_secret=(root / "helper-cli-sup").read_text()
            cli_run=subprocess.run([str(Path(sysconfig.get_path('scripts')) / ('deadbolt-supervisor-heartbeat.exe' if os.name=='nt' else 'deadbolt-supervisor-heartbeat')),
                                    "--agent","helper-cli","--credential-file",str(root / "helper-cli-sup"),"--max-secs",".2","--interval-secs",".05"],
                                   capture_output=True,text=True,timeout=10)
            require(cli_run.returncode==0 and cli_secret not in cli_run.stdout+cli_run.stderr,
                    "finite CLI maintains live run privately; exit="+str(cli_run.returncode))
            before_cli=operator("export","--agent","helper-cli")
            require('supervisor_renew' in before_cli, "finite CLI performs renewal")
            time.sleep(.15)
            require(operator("export","--agent","helper-cli")==before_cli, "finite CLI stops without detached heartbeat")
            cases.append("installed_heartbeat_job_scope_and_revocation")
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
