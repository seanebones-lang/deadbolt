"""Upgrade real legacy state to a candidate binary using disposable state."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "examples"))
import deadbolt_client as client


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old-binary", required=True, type=Path)
    parser.add_argument("--new-binary", required=True, type=Path)
    args = parser.parse_args()
    old, new = args.old_binary.resolve(strict=True), args.new_binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="deadbolt-upgrade-") as temp:
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            endpoint = f"127.0.0.1:{sock.getsockname()[1]}"
        env = {**os.environ, "DEADBOLT_SOCK": endpoint,
               "DEADBOLT_TOKEN": "disposable-upgrade-token",
               "DEADBOLT_DB": str(Path(temp) / "state.db"),
               "DEADBOLT_EVENTS": str(Path(temp) / "events.jsonl")}
        os.environ.update({k: v for k, v in env.items() if k.startswith("DEADBOLT_")})

        def operator(binary, *command):
            return subprocess.run([str(binary), *command], env=env, check=True,
                                  capture_output=True, text=True, timeout=10)

        def start(binary):
            process = subprocess.Popen([str(binary), "serve", "--bind", endpoint],
                                       env=env, stdout=subprocess.DEVNULL,
                                       stderr=subprocess.DEVNULL)
            for _ in range(100):
                if client.ensure("readiness").get("ok") is True:
                    return process
                if process.poll() is not None:
                    raise RuntimeError("upgrade sidecar exited")
                time.sleep(.02)
            process.terminate()
            process.wait(timeout=10)
            raise RuntimeError("upgrade sidecar readiness timeout")

        server = start(old)
        try:
            assert client.ensure("A").get("ok") is True
            assert client.ensure("B").get("ok") is True
            assert client.register_child("A", "child").get("live") is True
            assert client.spend("B", 1.25)["code"] == "ok"
        finally:
            server.terminate()
            server.wait(timeout=10)
        # No live connections while old operator writes are preparing the fixture.
        operator(old, "kill", "--agent", "A")
        operator(old, "policy", "--agent", "B", "--tools", "shell,fetch",
                 "--dest", "example.com", "--irreversible", "shell", "--spend-cap", "5")
        operator(old, "approve", "--agent", "B", "--tool", "shell")
        with sqlite3.connect(env["DEADBOLT_DB"]) as db:
            before = {row[0] for row in db.execute("SELECT cid FROM events")}
            assert db.execute("SELECT spend_usd FROM leases WHERE agent_id='B'").fetchone()[0] == 1.25
        server = start(new)
        try:
            assert client.ensure("A").get("ok") is True
            assert client.admit("A", "shell")["code"] == "killed"
            assert client.admit("child", "shell")["code"] == "killed"
            assert client.admit("B", "other")["code"] == "purpose_exceeded"
            assert client.admit("B", "fetch", "other.example")["code"] == "purpose_exceeded"
            assert client.admit("B", "fetch", "example.com")["decision"] == "allow"
            assert client.admit("B", "shell")["decision"] == "allow"
            assert client.admit("B", "shell")["code"] == "needs_human"
            assert client.spend("B", 3.75)["code"] == "spend_cap"
            assert client.admit("B", "fetch", "example.com")["code"] == "spend_cap"
            credential = Path(temp) / "credential"
            operator(new, "credential", "issue", "--agent", "B", "--id", "migrated-key", "--out", str(credential))
            os.environ["DEADBOLT_ADMISSION_TOKEN"] = credential.read_text()
            try:
                assert client.admit("B", "fetch", "example.com")["code"] == "spend_cap"
                assert client.admit("A", "shell")["code"] == "unauthorized"
                assert client.ensure("A")["code"] == "forbidden"
                operator(new, "credential", "revoke", "--id", "migrated-key")
                assert client.admit("B", "fetch", "example.com")["code"] == "unauthorized"
            finally:
                os.environ.pop("DEADBOLT_ADMISSION_TOKEN", None)
        finally:
            server.terminate()
            server.wait(timeout=10)
        with sqlite3.connect(env["DEADBOLT_DB"]) as db:
            after = {row[0] for row in db.execute("SELECT cid FROM events")}
            assert before <= after, "upgrade lost existing evidence"
            assert db.execute("SELECT parent_id FROM leases WHERE agent_id='child'").fetchone()[0] == "A"
        print(json.dumps({"new_version": operator(new, "--version").stdout.strip(),
                          "old_binary_sha256": hashlib.sha256(old.read_bytes()).hexdigest(),
                          "new_binary_sha256": hashlib.sha256(new.read_bytes()).hexdigest(),
                          "preserved": ["terminal revocation", "child lineage", "tool policy",
                                        "destination policy", "one-shot approval", "spend", "evidence",
                                        "credential migration and independent revocation"],
                          "result": "passed"}, indent=2))


if __name__ == "__main__":
    main()
