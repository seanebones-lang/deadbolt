"""External consumer tests: python3 tests/client_contract.py (requires Node)."""
import importlib.util
import asyncio
import json
import os
from pathlib import Path
import socket
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer
from threading import Thread

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("DEADBOLT_BIN", ROOT / "target/debug/deadbolt"))
spec = importlib.util.spec_from_file_location("deadbolt_client", ROOT / "examples/deadbolt_client.py")
client = importlib.util.module_from_spec(spec)
spec.loader.exec_module(client)

class ClientContract(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        self.env = {**os.environ, "DEADBOLT_SOCK": f"127.0.0.1:{port}",
                    "DEADBOLT_TOKEN": "test-only-token", "DEADBOLT_DB": self.temp.name + "/db",
                    "DEADBOLT_EVENTS": self.temp.name + "/events.jsonl"}
        self.old = dict(os.environ)
        os.environ.update(self.env)
        self.server = subprocess.Popen([str(BINARY), "serve", "--bind", self.env["DEADBOLT_SOCK"]], env=self.env,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        for _ in range(100):
            if client.ensure("agent").get("ok") is True:
                break
            if self.server.poll() is not None:
                self.fail(self.server.stderr.read().decode())
            time.sleep(.02)
        else:
            self.fail("sidecar did not become ready")

    def tearDown(self):
        self.server.terminate()
        self.server.wait(timeout=5)
        self.server.stderr.close()
        os.environ.clear()
        os.environ.update(self.old)
        self.temp.cleanup()

    def node(self, expression):
        script = f"const d = require({json.dumps(str(ROOT / 'examples/deadbolt_client.js'))}); {expression}"
        out = subprocess.run(["node", "-e", script], env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(out.returncode, 0, out.stderr)
        return json.loads(out.stdout)

    def test_scoped_python_node_and_mcp_keep_control_with_operator(self):
        root = Path(self.temp.name)
        self.assertTrue(client.ensure("other")["ok"])
        self.assertTrue(client.policy("agent", tools=["write_file", "read"])["ok"])
        issued = subprocess.run([str(BINARY), "credential", "issue", "--agent", "agent", "--id", "key",
                                 "--out", str(root / "key")], env=self.env, check=True, capture_output=True, text=True)
        secret = (root / "key").read_text()
        self.assertNotIn(secret, issued.stdout + issued.stderr)
        self.env["DEADBOLT_ADMISSION_TOKEN"] = os.environ["DEADBOLT_ADMISSION_TOKEN"] = secret
        # Operator token remains deliberately present: workload mode must take precedence.
        self.assertEqual(client.admit("agent", "read")["decision"], "allow")
        self.assertEqual(self.node('d.admit("agent", "read").then(x => console.log(JSON.stringify(x)))')["decision"], "allow")
        self.assertEqual(client.admit("other", "read")["decision"], "deny")
        for result in [client.ensure("agent"), client.policy("agent", tools=["send"]), client.spend("agent", -1),
                       client.register_child("agent", "child"), client.status("agent")]:
            self.assertNotEqual(result.get("ok"), True)
            self.assertNotIn("agents", result)
        self.assertEqual(self.node('d.policy("agent", {tools:["send"]}).then(x => console.log(JSON.stringify(x)))')["decision"], "deny")
        effect = root / "effect"
        self.assertTrue(client.dispatch("agent", "write_file", lambda: effect.write_text("allowed"))["executed"])
        # Real proxy routes one tools/call to a child that reports its environment.
        child = "import sys,json,os; [print(json.dumps({'jsonrpc':'2.0','id':json.loads(line)['id'],'result':{'secrets_inherited':any(k in os.environ for k in ['DEADBOLT_TOKEN','DEADBOLT_ADMISSION_TOKEN'])}}),flush=True) for line in sys.stdin]"
        proxy = subprocess.run([str(BINARY), "mcp-proxy", "--agent", "agent", "--serve-sock", self.env["DEADBOLT_SOCK"],
                                "--", sys.executable, "-c", child], input=json.dumps({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read","arguments":{}}})+"\n",
                               env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(proxy.returncode, 0, proxy.stderr)
        self.assertFalse(json.loads(proxy.stdout)["result"]["secrets_inherited"])
        no_remote = subprocess.run([str(BINARY), "mcp-proxy", "--agent", "agent", "--", sys.executable, "-c", "raise Exception('must not start')"], env=self.env, capture_output=True, text=True, timeout=10)
        self.assertNotEqual(no_remote.returncode, 0)
        self.assertIn("bad_request", no_remote.stderr)
        subprocess.run([str(BINARY), "credential", "revoke", "--id", "key"], env=self.env, check=True, capture_output=True)
        self.assertFalse(client.dispatch("agent", "write_file", lambda: effect.write_text("wrong"))["executed"])
        self.assertFalse(self.node(f'd.dispatch("agent", "write_file", () => require("fs").writeFileSync({json.dumps(str(effect))}, "wrong")).then(x => console.log(JSON.stringify(x)))')["executed"])
        self.assertEqual(effect.read_text(), "allowed")
        # Empty workload credentials must not fall back to the still-valid operator token.
        self.env["DEADBOLT_ADMISSION_TOKEN"] = os.environ["DEADBOLT_ADMISSION_TOKEN"] = ""
        self.assertEqual(client.admit("agent", "read")["decision"], "deny")
        self.assertEqual(self.node('d.admit("agent", "read").then(x => console.log(JSON.stringify(x)))')["decision"], "deny")

    def test_mcp_child_failure_returns_while_host_input_is_open(self):
        proxy = subprocess.Popen([str(BINARY), "mcp-proxy", "--agent", "failed-child", "--",
                                  sys.executable, "-c", "import sys; sys.exit(7)"],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, env=self.env)
        try:
            self.assertNotEqual(proxy.wait(timeout=5), 0)
            self.assertIn("store_unavailable", proxy.stderr.read().decode())
        finally:
            if proxy.poll() is None:
                proxy.kill()
                proxy.wait(timeout=5)
            for stream in (proxy.stdin, proxy.stdout, proxy.stderr):
                stream.close()

    def test_import_dest_and_spend(self):
        self.assertEqual(self.node('console.log(JSON.stringify(Object.keys(d).sort()))'),
                         sorted(["admit", "dispatch", "ensure", "policy", "registerChild", "spend", "status"]))
        self.assertTrue(client.policy("agent", tools=["fetch"], dest=["example.com"], spend_cap=2)["ok"])
        self.assertEqual(client.admit("agent", "fetch", "example.com")["decision"], "allow")
        self.assertEqual(self.node('d.admit("agent", "fetch", "other.com").then(x => console.log(JSON.stringify(x)))')["code"], "purpose_exceeded")
        self.assertEqual(self.node('d.admit("agent", "fetch", "example.com").then(x => console.log(JSON.stringify(x)))')["decision"], "allow")
        self.assertEqual(self.node('d.spend("agent", 2).then(x => console.log(JSON.stringify(x)))')["code"], "spend_cap")
        self.assertEqual(client.admit("agent", "fetch", "example.com")["code"], "spend_cap")

    def test_kill_and_down(self):
        self.assertEqual(client.admit("agent", "shell")["decision"], "allow")
        subprocess.run([str(BINARY), "kill", "--agent", "agent"], env=self.env, check=True, capture_output=True)
        self.assertEqual(client.admit("agent", "shell")["code"], "killed")
        self.assertEqual(self.node('d.admit("agent", "shell").then(x => console.log(JSON.stringify(x)))')["code"], "killed")
        self.server.terminate()
        self.server.wait(timeout=5)
        self.assertEqual(client.admit("agent", "shell")["code"], "store_unavailable")
        self.assertEqual(self.node('d.admit("agent", "shell").then(x => console.log(JSON.stringify(x)))')["code"], "store_unavailable")

    def test_quiescent_backup_restore_preserves_revocation(self):
        self.assertTrue(client.ensure("unrelated")["ok"])
        subprocess.run([str(BINARY), "kill", "--agent", "agent"], env=self.env,
                       check=True, capture_output=True)
        self.server.terminate()
        self.server.wait(timeout=5)
        self.server.stderr.close()
        with tempfile.TemporaryDirectory() as recovery:
            restored = Path(recovery) / "restored"
            shutil.copytree(self.temp.name, restored)
            self.env.update({"DEADBOLT_DB": str(restored / "db"),
                             "DEADBOLT_EVENTS": str(restored / "events.jsonl")})
            os.environ.update(self.env)
            self.server = subprocess.Popen(
                [str(BINARY), "serve", "--bind", self.env["DEADBOLT_SOCK"]],
                env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            try:
                for _ in range(100):
                    if client.ensure("agent").get("ok") is True:
                        break
                    time.sleep(.02)
                else:
                    self.fail("restored sidecar did not become ready")
                self.assertEqual(client.admit("agent", "shell")["code"], "killed")
                self.assertEqual(client.admit("unrelated", "shell")["decision"], "allow")
                self.assertEqual(self.node('d.admit("agent", "shell").then(x => console.log(JSON.stringify(x)))')["code"], "killed")
            finally:
                self.server.terminate()
                self.server.wait(timeout=5)

    def test_invalid_response_denies(self):
        class Handler(BaseHTTPRequestHandler):
            response = b'null'
            status = 200
            def do_POST(self):
                self.send_response(self.status)
                self.send_header("Content-Length", str(len(self.response)))
                self.end_headers()
                try: self.wfile.write(self.response)
                except OSError: pass
            def log_message(self, *args): pass
        fake = HTTPServer(("127.0.0.1", 0), Handler)
        worker = Thread(target=fake.serve_forever)
        worker.start()
        try:
            target = f"127.0.0.1:{fake.server_port}"
            self.env["DEADBOLT_SOCK"] = os.environ["DEADBOLT_SOCK"] = target
            for status, body in [(200, b'null'), (200, b'[]'), (200, b'{"ok":true}'), (500, b'{"decision":"allow"}'), (401, b'{"decision":"allow"}'), (200, b'{"decision":"allow","padding":"' + b'x' * (1024 * 1024) + b'"}')]:
                Handler.status, Handler.response = status, body
                self.assertEqual(client.admit("agent", "shell")["decision"], "deny")
                self.assertEqual(self.node('d.admit("agent", "shell").then(x => console.log(JSON.stringify(x)))')["decision"], "deny")
        finally:
            fake.shutdown()
            worker.join()
            fake.server_close()

    def test_dispatch_effects_and_one_shot_across_languages(self):
        root = Path(self.temp.name)
        self.assertTrue(client.policy("agent", tools=["write_file"], irreversible=["write_file"])["ok"])
        blocked = root / "blocked.txt"
        result = client.dispatch("agent", "write_file", lambda: blocked.write_text("wrong"))
        self.assertFalse(result["executed"])
        self.assertFalse(blocked.exists())
        subprocess.run([str(BINARY), "approve", "--agent", "agent", "--tool", "write_file"], env=self.env, check=True, capture_output=True)
        allowed = root / "allowed.txt"
        result = self.node(f'd.dispatch("agent", "write_file", async () => {{require("fs").writeFileSync({json.dumps(str(allowed))}, "once"); return 42}}).then(x => console.log(JSON.stringify(x)))')
        self.assertTrue(result["executed"])
        self.assertEqual(result["result"], 42)
        self.assertEqual(allowed.read_text(), "once")
        self.assertFalse(client.dispatch("agent", "write_file", lambda: blocked.write_text("wrong"))["executed"])
        self.assertFalse(blocked.exists())
        subprocess.run([str(BINARY), "kill", "--agent", "agent"], env=self.env, check=True, capture_output=True)
        denied = self.node(f'd.dispatch("agent", "write_file", () => require("fs").writeFileSync({json.dumps(str(blocked))}, "wrong")).then(x => console.log(JSON.stringify(x)))')
        self.assertFalse(denied["executed"])
        self.assertEqual(denied["decision"]["code"], "killed")
        self.assertFalse(blocked.exists())

    def test_invalid_configuration_and_payload_deny_without_effect(self):
        def forbidden(): self.fail("invalid configuration ran a body")
        for target, token in [("127.0.0.1:99999", "test-only-token"), (self.env["DEADBOLT_SOCK"], "invalid\nheader")]:
            self.env.update({"DEADBOLT_SOCK": target, "DEADBOLT_TOKEN": token})
            os.environ.update(self.env)
            self.assertFalse(client.dispatch("agent", "shell", forbidden)["executed"])
            self.assertFalse(self.node('d.dispatch("agent", "shell", () => {throw new Error("body ran")}).then(x => console.log(JSON.stringify(x)))')["executed"])
        self.assertEqual(client.admit(object(), "shell")["decision"], "deny")
        self.assertEqual(self.node('d.admit(1n, "shell").then(x => console.log(JSON.stringify(x)))')["decision"], "deny")

    def test_mcp_eof_terminates_noncooperative_child(self):
        started = time.monotonic()
        result = subprocess.run([str(BINARY), "mcp-proxy", "--agent", "eof-probe", "--", sys.executable,
                                 "-c", "import sys,time; sys.stdin.read(); time.sleep(30)"],
                                env=self.env, input="", capture_output=True, text=True, timeout=6)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertLess(time.monotonic() - started, 5)

    def test_async_python_dispatch_and_tool_errors(self):
        file = Path(self.temp.name) / "async-effect"
        async def body():
            await asyncio.sleep(0)
            file.write_text("once")
            return 42
        with self.assertRaises(TypeError):
            client.dispatch("agent", "write_file", body)
        self.assertFalse(file.exists())
        result = asyncio.run(client.dispatch_async("agent", "write_file", body))
        self.assertTrue(result["executed"])
        self.assertEqual(result["result"], 42)
        self.assertEqual(file.read_text(), "once")
        def failure(): raise ValueError("tool_error")
        with self.assertRaisesRegex(ValueError, "tool_error"):
            client.dispatch("agent", "write_file", failure)
        self.assertEqual(self.node('d.dispatch("agent", "write_file", async () => {throw new Error("tool_error")}).then(() => {throw new Error("error swallowed")}).catch(e => console.log(JSON.stringify({error:e.message})))')["error"], "tool_error")
        subprocess.run([str(BINARY), "kill", "--agent", "agent"], env=self.env, check=True, capture_output=True)
        file.unlink()
        result = asyncio.run(client.dispatch_async("agent", "write_file", body))
        self.assertFalse(result["executed"])
        self.assertFalse(file.exists())

    def test_partial_response_does_not_renew_node_deadline(self):
        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                self.send_response(200)
                self.send_header("Content-Length", "1000")
                self.end_headers()
                try:
                    for _ in range(100):
                        self.wfile.write(b" ")
                        self.wfile.flush()
                        time.sleep(.1)
                except OSError: pass
            def log_message(self, *args): pass
        fake = HTTPServer(("127.0.0.1", 0), Handler)
        worker = Thread(target=fake.serve_forever)
        worker.start()
        try:
            self.env["DEADBOLT_SOCK"] = f"127.0.0.1:{fake.server_port}"
            started = time.monotonic()
            result = self.node('d.dispatch("agent", "shell", () => {throw new Error("body ran")}).then(x => console.log(JSON.stringify(x)))')
            self.assertFalse(result["executed"])
            self.assertLess(time.monotonic() - started, 7)
        finally:
            fake.shutdown()
            worker.join()
            fake.server_close()

if __name__ == "__main__":
    unittest.main()
