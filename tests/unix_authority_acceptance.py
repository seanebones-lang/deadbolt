"""Default Unix HTTP authentication and explicit legacy migration acceptance."""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time


def require(condition, label):
    if not condition:
        raise RuntimeError(label)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--scenario", choices=["all", "strict"], default="all")
    args = parser.parse_args()
    binary = args.binary.resolve()
    if os.name != "posix":
        print(json.dumps({"ok": True, "coverage": "Unix-only; TCP covered separately"}))
        return
    cases = []
    with tempfile.TemporaryDirectory(prefix="deadbolt-unix-auth-") as temporary:
        root = Path(temporary)
        env = dict(os.environ, DEADBOLT_DB=str(root / "db"), DEADBOLT_EVENTS=str(root / "events"))
        env.pop("DEADBOLT_TOKEN", None)
        env.pop("DEADBOLT_ADMISSION_TOKEN", None)
        endpoint = str(root / "sidecar.sock")

        def operator(*commands):
            result = subprocess.run([str(binary), *commands], env=env, capture_output=True, timeout=10)
            require(result.returncode == 0, "trusted CLI setup failed")

        def request(method, route, payload=None, headers=None):
            body = json.dumps(payload or {}) if method == "POST" else ""
            extra = "".join(f"{name}: {value}\r\n" for name, value in (headers or {}).items())
            wire = f"{method} {route} HTTP/1.1\r\nhost: local\r\n{extra}content-length: {len(body)}\r\nconnection: close\r\n\r\n{body}"
            with socket.socket(socket.AF_UNIX) as connection:
                connection.settimeout(3)
                connection.connect(endpoint)
                connection.sendall(wire.encode())
                connection.shutdown(socket.SHUT_WR)
                result = b""
                while chunk := connection.recv(8192):
                    result += chunk
            head, reply = result.decode().split("\r\n\r\n", 1)
            return int(head.split()[1]), json.loads(reply)

        @contextmanager
        def server(*flags, token=None):
            server_env = dict(env)
            if token is not None:
                server_env["DEADBOLT_TOKEN"] = token
            process = subprocess.Popen([str(binary), "serve", "--bind", endpoint, *flags],
                env=server_env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            try:
                for _ in range(100):
                    if process.poll() is not None or Path(endpoint).exists():
                        break
                    time.sleep(.02)
                yield process
            finally:
                if process.poll() is None:
                    process.terminate()
                process.wait(timeout=10)
                process.stderr.close()
                Path(endpoint).unlink(missing_ok=True)

        operator("ensure", "--agent", "run")
        operator("policy", "--agent", "run", "--tools", "read")
        for missing in [None, "", "   "]:
            with server(token=missing) as process:
                # Baseline permits this mutation; corrected default refuses
                # before listening, so socket possession cannot widen policy.
                if process.poll() is None:
                    status, _ = request("POST", "/policy", {"agent_id": "run", "tools": ["shell"]})
                    require(status == 401, "unauthenticated socket holder widened policy")
                require(process.poll() is not None and process.returncode != 0,
                        "default un-tokened Unix listener started")
        cases.append("default_refuses_un_tokened_unix_without_operator_authority")
        if args.scenario == "strict":
            print(json.dumps({"ok": True, "cases": cases}))
            return

        token = secrets.token_hex(32)
        with server(token=token) as process:
            require(process.poll() is None, "authenticated Unix startup failed")
            for method, route, body in [
                ("POST", "/ensure", {"agent_id": "unissued"}),
                ("POST", "/policy", {"agent_id": "run", "tools": ["shell"]}),
                ("POST", "/spend", {"agent_id": "run", "usd": 1}),
                ("POST", "/admit", {"agent_id": "run", "tool": "read"}),
                ("GET", "/status", None),
            ]:
                require(request(method, route, body)[0] == 401, "anonymous route not refused")
                require(request(method, route, body, {"X-Deadbolt-Token": "wrong"})[0] == 401,
                        "wrong operator token not refused")
            auth = {"X-Deadbolt-Token": token}
            require(request("POST", "/admit", {"agent_id": "run", "tool": "read"}, auth)[1]
                    == {"decision": "allow"}, "authenticated control failed")
            key = root / "worker.key"
            operator("credential", "issue", "--agent", "run", "--id", "worker", "--ttl-secs", "600", "--out", str(key))
            worker = {"X-Deadbolt-Admission": key.read_text()}
            require(request("POST", "/admit", {"agent_id": "run", "tool": "read"}, worker)[1]
                    == {"decision": "allow"}, "scoped Unix control failed")
            require(request("POST", "/policy", {"agent_id": "run", "tools": ["shell"]}, worker)[0] == 403,
                    "worker gained operator authority")
            require(request("POST", "/admit", {"agent_id": "run", "tool": "read"}, {**auth, **worker})[0] == 401,
                    "mixed credentials accepted")
        cases.append("tokened_unix_and_scoped_auth_preserve_roles")

        with server("--legacy-unauthenticated-unix") as process:
            require(process.poll() is None, "explicit legacy listener failed")
            require(request("GET", "/status")[0] == 401, "legacy mode restored anonymous access after credential issuance")
            operator("credential", "revoke", "--id", "worker")
            require(request("GET", "/status")[0] == 401, "revocation restored anonymous authority")
        with server("--legacy-unauthenticated-unix"):
            require(request("GET", "/status")[0] == 401, "restart restored anonymous authority")
        cases.append("legacy_opt_in_cannot_erase_persisted_credential_boundary")

        # Fresh store is the sole documented legacy compatibility path.
        env["DEADBOLT_DB"] = str(root / "legacy-db")
        env["DEADBOLT_EVENTS"] = str(root / "legacy-events")
        with server("--legacy-unauthenticated-unix"):
            require(request("POST", "/ensure", {"agent_id": "legacy"})[1].get("ok") is True,
                    "explicit legacy setup failed")
        with server("--legacy-unauthenticated-unix", token=token):
            require(request("GET", "/status")[0] == 401, "legacy flag bypassed configured token")
        cases.append("legacy_is_explicit_and_configured_token_always_wins")
    print(json.dumps({"ok": True, "cases": cases}))


if __name__ == "__main__":
    main()
