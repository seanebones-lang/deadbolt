#!/usr/bin/env python3
"""Disposable raw-proxy setup checks; no accounts or model requests."""
import argparse
from contextlib import ExitStack
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import tempfile
from threading import Thread
import time

import deadbolt_client as client


def require(value, message):
    if not value:
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
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--scenario", choices=["all", "missing", "policy"], default="all")
    options = parser.parse_args()
    binary = options.binary.resolve(strict=True)
    original = dict(os.environ)
    try:
        with tempfile.TemporaryDirectory(prefix="deadbolt-proxy-setup-") as directory, ExitStack() as cleanup:
            root = Path(directory)
            for name in list(os.environ):
                if name.startswith("DEADBOLT_"):
                    del os.environ[name]
            with socket.socket() as socket_probe:
                socket_probe.bind(("127.0.0.1", 0))
                endpoint = f"127.0.0.1:{socket_probe.getsockname()[1]}"
            os.environ.update(DEADBOLT_DB=str(root / "state.db"), DEADBOLT_EVENTS=str(root / "events.jsonl"),
                              DEADBOLT_TOKEN=secrets.token_hex(32), DEADBOLT_SOCK=endpoint)
            server = subprocess.Popen([str(binary), "serve", "--bind", endpoint],
                                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            cleanup.callback(stop, server)
            for _ in range(100):
                require(server.poll() is None, "sidecar exited")
                if client.status("ready").get("agents") == []:
                    break
                time.sleep(.05)
            else:
                raise RuntimeError("sidecar did not start")

            def call(agent, tool, backend, label, denial=None, env=None, arguments=None, address=None):
                marker = root / label
                child = (
                    "import sys,json,pathlib; "
                    "[(pathlib.Path(sys.argv[1]).write_text('ran'),print(json.dumps("
                    "{'jsonrpc':'2.0','id':json.loads(line)['id'],'result':{'content':"
                    "[{'type':'text','text':'ran'}]}}),flush=True)) for line in sys.stdin]"
                )
                args = [str(binary), "mcp-proxy", "--agent", agent]
                if backend == "socket":
                    args += ["--serve-sock", address or endpoint]
                args += ["--", sys.executable, "-c", child, str(marker)]
                frame = {"jsonrpc":"2.0", "id":label, "method":"tools/call",
                         "params":{"name":tool, "arguments":arguments or {}}}
                result = subprocess.run(args, input=json.dumps(frame)+"\n", capture_output=True,
                                        text=True, timeout=10, env=env)
                require(result.returncode == 0, "proxy completion")
                reply = json.loads(result.stdout)
                if denial:
                    require(reply["result"].get("isError") is True, "missing setup must deny")
                    require(reply["result"]["_meta"]["io.github.seanebones-lang/deadbolt"]["code"] == denial,
                            "actual setup reason")
                    require(not marker.exists(), "denied body ran")
                else:
                    require(marker.read_text() == "ran", "configured body did not run")

            def operator(*args):
                return subprocess.run([str(binary), *args], check=True, capture_output=True, text=True)

            cases = []
            for backend in ["embedded", "socket"]:
                control = "control-" + backend
                require(client.ensure(control).get("ok") is True, "control lease setup")
                require(client.policy(control, tools=["read"]).get("ok") is True, "control policy setup")
                call(control, "read", backend, control+"-allowed")
                missing = "missing-" + backend
                if options.scenario in ["all", "missing"]:
                    call(missing, "shell", backend, missing, "no_lease")
                    require(client.status(missing).get("agents") == [], "proxy must not create a lease")
                if options.scenario == "missing":
                    continue
                agent = "issued-" + backend
                require(client.ensure(agent).get("ok") is True, "explicit operator setup")
                call(agent, "shell", backend, agent+"-unset", "policy_required")
                require(client.policy(agent, tools=["read"]).get("ok") is True, "explicit policy")
                call(agent, "shell", backend, agent+"-off-list", "purpose_exceeded")
                call(agent, "read", backend, agent+"-allowed")
                require(client.policy(agent, tools=[]).get("ok") is True, "deny-all policy")
                call(agent, "read", backend, agent+"-empty", "purpose_exceeded")
                cases.append(backend+"_missing_unset_configured_and_deny_all")
            if options.scenario == "all":
                # Exact grants must not bypass setup, with either operator or scoped authority.
                for backend, scoped in [("embedded", False), ("socket", False), ("socket", True)]:
                    agent = f"exact-{backend}-{scoped}"
                    require(client.ensure(agent).get("ok") is True, "exact setup")
                    action = client.prepare_action(agent, "write", {"body":"reviewed"})
                    review = root / (agent+".json")
                    review.write_text(json.dumps(action))
                    fingerprint = operator("action", "inspect", "--file", str(review)).stdout.splitlines()[0].split("=",1)[1]
                    operator("action", "approve", "--file", str(review), "--fingerprint", fingerprint)
                    grant = root / (agent+"-grant.json")
                    grant.write_text(json.dumps({"write":{k:action[k] for k in ["nonce","expires_at"]}}))
                    env = {**os.environ, "DEADBOLT_ACTION_GRANTS":str(grant)}
                    if scoped:
                        key = root / (agent+"-key")
                        operator("credential", "issue", "--agent", agent, "--id", agent, "--out", str(key))
                        env["DEADBOLT_ADMISSION_TOKEN"] = key.read_text()
                        call(agent, "read", backend, agent+"-scoped-unset", "policy_required", env=env)
                    call(agent, "write", backend, agent+"-unset", "policy_required", env=env, arguments=action["arguments"])
                    require(client.policy(agent, tools=["write", "read"]).get("ok") is True, "exact policy")
                    call(agent, "write", backend, agent+"-allowed", env=env, arguments=action["arguments"])
                    call(agent, "write", backend, agent+"-replay", "needs_human", env=env, arguments=action["arguments"])
                    cases.append(agent+"_policy_before_exact_grant")

                # Every proxy backend rejects ambiguous destinations before
                # spending or consuming a broad/exact approval. The downstream
                # is an inert marker process, not a real network client.
                mixed = {"a":{"url":"https://allowed.example"},
                         "b":{"host":"foreign.example"}}
                variants = [mixed,
                            {"url":"https://allowed.example", "host":"foreign.example"},
                            {"url":"https://allowed.example", "endpoint":"HTTPS://foreign.example"},
                            {"targets":[{"uri":"https://foreign.example"},
                                        {"endpoint":"https://allowed.example"}]}]
                for destination_backend, destination_scoped in [("embedded", False), ("socket", False), ("socket", True)]:
                    destination_agent = f"dest-{destination_backend}-{destination_scoped}"
                    require(client.ensure(destination_agent).get("ok") is True, "destination setup")
                    require(client.policy(destination_agent, tools=["read", "write"],
                                          dest=["allowed.example"], offline_tools=["read", "write"],
                                          irreversible=["read"]).get("ok") is True, "destination policy")
                    destination_env = dict(os.environ)
                    if destination_scoped:
                        destination_key = root / (destination_agent+"-key")
                        operator("credential", "issue", "--agent", destination_agent, "--id", destination_agent,
                                 "--out", str(destination_key))
                        destination_env["DEADBOLT_ADMISSION_TOKEN"] = destination_key.read_text()
                    operator("approve", "--agent", destination_agent, "--tool", "read")
                    for number, destination_arguments in enumerate(variants):
                        call(destination_agent, "read", destination_backend,
                             destination_agent+f"-mixed-{number}", "bad_request", env=destination_env,
                             arguments=destination_arguments)
                    call(destination_agent, "read", destination_backend, destination_agent+"-foreign",
                         "purpose_exceeded", env=destination_env, arguments={"url":"https://foreign.example"})
                    # The broad approval remains available exactly once.
                    call(destination_agent, "read", destination_backend, destination_agent+"-broad-allowed",
                         env=destination_env, arguments={"url":"HTTPS://Allowed.Example", "host":"allowed.example"})
                    call(destination_agent, "read", destination_backend, destination_agent+"-broad-replay",
                         "needs_human", env=destination_env, arguments={"url":"https://allowed.example"})
                    # A legitimate exact grant remains usable after rejection.
                    destination_action = client.prepare_action(destination_agent, "write", {"body":"reviewed"})
                    destination_review = root / (destination_agent+"-review.json")
                    destination_review.write_text(json.dumps(destination_action))
                    destination_fingerprint = operator("action", "inspect", "--file", str(destination_review)).stdout.splitlines()[0].split("=",1)[1]
                    operator("action", "approve", "--file", str(destination_review), "--fingerprint", destination_fingerprint)
                    destination_grant = root / (destination_agent+"-grant.json")
                    destination_grant.write_text(json.dumps({"write":{key:destination_action[key] for key in ["nonce","expires_at"]}}))
                    destination_env["DEADBOLT_ACTION_GRANTS"] = str(destination_grant)
                    call(destination_agent, "write", destination_backend, destination_agent+"-exact-mixed",
                         "bad_request", env=destination_env, arguments={**destination_action["arguments"], **mixed})
                    call(destination_agent, "write", destination_backend, destination_agent+"-exact-allowed",
                         env=destination_env, arguments=destination_action["arguments"])
                    call(destination_agent, "write", destination_backend, destination_agent+"-exact-replay",
                         "needs_human", env=destination_env, arguments=destination_action["arguments"])
                    cases.append(destination_agent+"_ambiguous_denial_preserves_grants")

                # An older sidecar may allow legacy routes, but must not downgrade MCP.
                paths = []
                class OldSidecar(BaseHTTPRequestHandler):
                    def do_POST(self):
                        self.rfile.read(int(self.headers.get("Content-Length", "0")))
                        paths.append(self.path)
                        body = b'{"decision":"allow"}'
                        self.send_response(404 if self.path.startswith("/admit-mcp") else 200)
                        self.send_header("Content-Length", str(len(body)))
                        self.end_headers()
                        self.wfile.write(body)
                    def log_message(self, *args):
                        pass
                old = HTTPServer(("127.0.0.1", 0), OldSidecar)
                thread = Thread(target=old.serve_forever)
                thread.start()
                try:
                    address = f"127.0.0.1:{old.server_port}"
                    call("old", "read", "socket", "old-regular", "store_unavailable", address=address)
                    call("old", "write", "socket", "old-exact", "store_unavailable", address=address, env=env, arguments=action["arguments"])
                    require(paths == ["/admit-mcp", "/admit-mcp-action"], "legacy sidecar fallback or implicit setup")
                    cases.append("old_sidecar_never_downgrades_or_provisions")
                finally:
                    old.shutdown()
                    thread.join()
                    old.server_close()
        print(json.dumps({"ok":True, "cases":cases}))
    finally:
        os.environ.clear()
        os.environ.update(original)


if __name__ == "__main__":
    main()
