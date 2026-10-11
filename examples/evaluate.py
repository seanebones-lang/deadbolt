#!/usr/bin/env python3
"""Disposable HTTP evaluation with real file effects; Python stdlib only."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import tempfile
import time

import deadbolt_client as client


def require(condition, message):
    # Keep checks active even when Python is invoked with -O.
    if not condition:
        raise RuntimeError(message)


def evaluate(binary):
    checks = []
    previous = dict(os.environ)
    server = None
    with tempfile.TemporaryDirectory(prefix="deadbolt-evaluation-") as directory:
        root = Path(directory)
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        os.environ.update({
            "DEADBOLT_DB": str(root / "state.db"),
            "DEADBOLT_EVENTS": str(root / "events.jsonl"),
            "DEADBOLT_SOCK": f"127.0.0.1:{port}",
            "DEADBOLT_TOKEN": secrets.token_hex(32),
        })

        def operator(*args):
            return subprocess.run([str(binary), *args], check=True,
                                  capture_output=True, text=True, timeout=15).stdout

        def start():
            process = subprocess.Popen(
                [str(binary), "serve", "--bind", os.environ["DEADBOLT_SOCK"]],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                for _ in range(100):
                    require(process.poll() is None, "sidecar exited before becoming ready")
                    if client.ensure("worker").get("ok") is True:
                        return process
                    time.sleep(0.05)
                raise RuntimeError("sidecar did not become ready")
            except BaseException:
                stop(process)
                raise

        def stop(process):
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)

        def dispatch(name, agent="worker", tool="write_file", expected="allow"):
            path = root / name
            outcome = client.dispatch(agent, tool, lambda: path.write_text(name, encoding="utf-8"))
            decision = outcome["decision"]
            observed = ("allow" if decision.get("decision") == "allow"
                        else decision.get("code"))
            require(observed == expected, f"{name}: expected {expected}, got {decision}")
            require(path.exists() == (expected == "allow"), f"{name}: unexpected body effect")
            require(outcome["executed"] == path.exists(), f"{name}: wrong execution status")
            if path.exists():
                require(path.read_text(encoding="utf-8") == name, f"{name}: wrong content")
            checks.append({"case": name, "decision": observed, "body_ran": path.exists()})

        try:
            version = operator("--version").strip()
            server = start()
            operator("policy", "--agent", "worker", "--tools", "write_file")
            dispatch("allowed.txt")
            dispatch("off-policy.txt", tool="delete_file", expected="purpose_exceeded")
            operator("policy", "--agent", "worker", "--tools", "write_file",
                     "--irreversible", "write_file")
            dispatch("unapproved.txt", expected="needs_human")
            operator("approve", "--agent", "worker", "--tool", "write_file")
            dispatch("approved-once.txt")
            dispatch("approval-consumed.txt", expected="needs_human")
            require(client.register_child("worker", "child").get("live") is True,
                    "child registration failed")
            operator("policy", "--agent", "child", "--tools", "write_file")
            require(client.ensure("unrelated").get("ok") is True, "unrelated ensure failed")
            operator("policy", "--agent", "unrelated", "--tools", "write_file")
            operator("kill", "--agent", "worker")
            dispatch("killed-parent.txt", expected="killed")
            dispatch("killed-child.txt", agent="child", expected="killed")
            dispatch("unrelated.txt", agent="unrelated")
            stop(server)
            server = None
            dispatch("outage.txt", agent="unrelated", expected="store_unavailable")
            server = start()
            dispatch("restart-revoked.txt", expected="killed")
            dispatch("restart-unrelated.txt", agent="unrelated")
            return {"passed": True, "binary_version": version,
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "integration": "Python HTTP client with temporary file dispatcher",
                    "checks": checks,
                    "scope": "This demonstration dispatcher only; no model or whole-agent containment."}
        finally:
            stop(server)
            os.environ.clear()
            os.environ.update(previous)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="deadbolt", help="Executable path or command on PATH")
    args = parser.parse_args()
    executable = shutil.which(args.binary)
    if executable is None:
        parser.error("Deadbolt executable not found; install it or pass --binary with its path")
    try:
        report = evaluate(Path(executable).resolve())
    except Exception as exc:
        print(json.dumps({"passed": False, "error": str(exc)}))
        return 1
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
