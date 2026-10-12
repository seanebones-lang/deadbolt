"""Upgrade real legacy state to a candidate binary using disposable state."""

import argparse
import hashlib
import json
import os
import re
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

        old_help = operator(old, "--help").stdout
        old_scoped = bool(re.search(r"^\s+credential(?:\s|$)", old_help, re.MULTILINE))
        old_exact = bool(re.search(r"^\s+action(?:\s|$)", old_help, re.MULTILINE))
        legacy_credential = Path(temp) / "legacy-credential"
        legacy_actions = []

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
            if old_scoped:
                assert client.ensure("legacy-worker").get("ok") is True
            if old_exact:
                assert client.ensure("legacy-exact").get("ok") is True
        finally:
            server.terminate()
            server.wait(timeout=10)
        # No live connections while old operator writes are preparing the fixture.
        operator(old, "kill", "--agent", "A")
        operator(old, "policy", "--agent", "B", "--tools", "shell,fetch",
                 "--dest", "example.com", "--irreversible", "shell", "--spend-cap", "5")
        operator(old, "approve", "--agent", "B", "--tool", "shell")
        if old_scoped:
            operator(old, "credential", "issue", "--agent", "legacy-worker",
                     "--id", "old-worker", "--out", str(legacy_credential))
        if old_exact:
            for body in ["pending-before-upgrade", "consumed-before-upgrade"]:
                action = client.prepare_action("legacy-exact", "write", {"body": body})
                review = Path(temp) / "old-review.json"
                review.write_text(json.dumps(action))
                fingerprint = operator(old, "action", "inspect", "--file", str(review)).stdout.splitlines()[0].split("=", 1)[1]
                operator(old, "action", "approve", "--file", str(review), "--fingerprint", fingerprint)
                legacy_actions.append(action)
            server = start(old)
            try:
                assert client.dispatch_action(legacy_actions[1], lambda a: a["body"])["executed"]
            finally:
                server.terminate()
                server.wait(timeout=10)
        with sqlite3.connect(env["DEADBOLT_DB"]) as db:
            before = {row[0] for row in db.execute("SELECT cid FROM events")}
            assert db.execute("SELECT spend_usd FROM leases WHERE agent_id='B'").fetchone()[0] == 1.25
        server = start(new)
        try:
            if old_scoped:
                os.environ["DEADBOLT_ADMISSION_TOKEN"] = legacy_credential.read_text()
                try:
                    assert client.admit("legacy-worker", "read")["decision"] == "allow"
                    assert client.admit("A", "read")["code"] == "unauthorized"
                    assert client.ensure("legacy-worker")["code"] == "forbidden"
                    operator(new, "credential", "revoke", "--id", "old-worker")
                    assert client.admit("legacy-worker", "read")["code"] == "unauthorized"
                finally:
                    os.environ.pop("DEADBOLT_ADMISSION_TOKEN", None)
            if old_exact:
                assert client.dispatch_action(legacy_actions[0], lambda a: a["body"])["result"] == "pending-before-upgrade"
                assert not client.dispatch_action(legacy_actions[0], lambda a: (_ for _ in ()).throw(AssertionError("pending replay")))["executed"]
                assert not client.dispatch_action(legacy_actions[1], lambda a: (_ for _ in ()).throw(AssertionError("consumed replay")))["executed"]
            # New exact-action state can coexist with migrated legacy leases.
            assert client.ensure("exact-upgrade").get("ok") is True
            action=client.prepare_action("exact-upgrade","write",{"body":"reviewed"})
            file=Path(temp)/"review.json"; file.write_text(json.dumps(action))
            inspected=operator(new,"action","inspect","--file",str(file)).stdout
            fingerprint=inspected.splitlines()[0].split("=",1)[1]
            operator(new,"action","approve","--file",str(file),"--fingerprint",fingerprint)
            assert client.admit("exact-upgrade","write")["code"]=="needs_human"
            assert client.dispatch_action(action,lambda a:a["body"])["result"]=="reviewed"
            assert not client.dispatch_action(action,lambda a:(_ for _ in ()).throw(AssertionError("replay")))["executed"]
            assert client.ensure("A").get("ok") is True
            assert client.admit("A", "shell")["code"] == "killed"
            assert client.admit("child", "shell")["code"] == "killed"
            assert client.admit("B", "other")["code"] == "purpose_exceeded"
            assert client.admit("B", "fetch", "other.example")["code"] == "purpose_exceeded"
            assert client.admit("B", "fetch", "example.com")["decision"] == "allow"
            # Migrated destination policy defaults to no offline exceptions.
            assert client.admit("B", "shell")["code"] == "purpose_exceeded"
            operator(new, "policy", "--agent", "B", "--offline-tools", "shell")
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
            assert db.execute("SELECT hard_deadline_at, supervision_root FROM leases WHERE agent_id='B'").fetchone() == (None, None)
            if old_scoped:
                assert db.execute("SELECT scope, revoked_at IS NOT NULL FROM admission_credentials WHERE credential_id='old-worker'").fetchone() == ("admission", 1)
        print(json.dumps({"new_version": operator(new, "--version").stdout.strip(),
                          "old_binary_sha256": hashlib.sha256(old.read_bytes()).hexdigest(),
                          "new_binary_sha256": hashlib.sha256(new.read_bytes()).hexdigest(),
                          "preserved": ["terminal revocation", "child lineage", "tool policy",
                                        "destination policy", "one-shot approval", "spend", "evidence",
                                        "new scoped credential and independent revocation", "new exact-action state and replay refusal"],
                          "old_scoped_credential_preserved": old_scoped,
                          "old_pending_and_consumed_exact_grants_preserved": old_exact,
                          "result": "passed"}, indent=2))


if __name__ == "__main__":
    main()
