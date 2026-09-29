"""External consumer tests: python3 tests/client_contract.py (requires Node)."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import shutil
import subprocess
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

    def test_import_dest_and_spend(self):
        self.assertEqual(self.node('console.log(JSON.stringify(Object.keys(d).sort()))'),
                         sorted(["admit", "ensure", "policy", "registerChild", "spend", "status"]))
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
                self.wfile.write(self.response)
            def log_message(self, *args): pass
        fake = HTTPServer(("127.0.0.1", 0), Handler)
        worker = Thread(target=fake.serve_forever)
        worker.start()
        try:
            target = f"127.0.0.1:{fake.server_port}"
            self.env["DEADBOLT_SOCK"] = os.environ["DEADBOLT_SOCK"] = target
            for status, body in [(200, b'null'), (200, b'[]'), (200, b'{"ok":true}'), (500, b'{"decision":"allow"}'), (401, b'{"decision":"allow"}')]:
                Handler.status, Handler.response = status, body
                self.assertEqual(client.admit("agent", "shell")["decision"], "deny")
                self.assertEqual(self.node('d.admit("agent", "shell").then(x => console.log(JSON.stringify(x)))')["decision"], "deny")
        finally:
            fake.shutdown()
            worker.join()
            fake.server_close()

if __name__ == "__main__":
    unittest.main()
