"""Optional independent-host acceptance. See docs/HERMES-SHOWCASE.md.

Uses Hermes' real MCP transport and the reference filesystem server. No model,
installed Hermes profile, or non-temporary filesystem workspace is used.
"""

import argparse
import asyncio
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


async def acceptance(args, work):
    # Set before importing Hermes; never use the user's real profile.
    os.environ["HERMES_HOME"] = str(work / "hermes-home")
    sys.path.insert(0, str(args.hermes))
    sys.path.insert(0, str(ROOT / "examples"))
    from tools.mcp_tool import MCPServerTask
    import deadbolt_client as client

    sandbox = work / "sandbox"
    sandbox.mkdir()
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        endpoint = f"127.0.0.1:{sock.getsockname()[1]}"
    env = {**os.environ, "DEADBOLT_SOCK": endpoint,
           "DEADBOLT_TOKEN": "disposable-acceptance-token",
           "DEADBOLT_DB": str(work / "state.db"),
           "DEADBOLT_EVENTS": str(work / "events.jsonl")}
    os.environ.update({k: v for k, v in env.items() if k.startswith("DEADBOLT_")})
    servers = []
    sidecar = None
    checks = []

    def operator(*command):
        return subprocess.run([str(args.binary), *command], env=env, check=True,
                              capture_output=True, text=True, timeout=10)

    def start_sidecar():
        process = subprocess.Popen([str(args.binary), "serve", "--bind", endpoint],
                                   env=env, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL)
        for _ in range(100):
            if client.ensure("readiness").get("ok") is True:
                return process
            if process.poll() is not None:
                raise RuntimeError("sidecar exited before readiness")
            time.sleep(.02)
        process.terminate()
        process.wait(timeout=10)
        raise RuntimeError("sidecar readiness timeout")

    async def host(identity, token=None):
        server = MCPServerTask(f"deadbolt-{identity}-{len(servers)}")
        servers.append(server)
        config = {"command": str(args.binary),
                  "args": ["mcp-proxy", "--agent", identity, "--serve-sock", endpoint,
                           "--", args.node, str(args.filesystem_server), str(sandbox)],
                  "env": {k: v for k, v in env.items() if k.startswith("DEADBOLT_")},
                  "connect_timeout": 15, "sampling": {"enabled": False},
                  "elicitation": {"enabled": False}}
        if token is not None:
            config["env"]["DEADBOLT_TOKEN"] = token
        await asyncio.wait_for(server.start(config), timeout=20)
        assert server.session is not None
        tools = await server.session.list_tools()
        assert "write_file" in {tool.name for tool in tools.tools}
        return server

    async def write(server, name, expected=None):
        path = sandbox / name
        assert not path.exists()
        try:
            result = await asyncio.wait_for(server.session.call_tool(
                "write_file", {"path": str(path), "content": name}), timeout=10)
        except Exception as exc:
            if expected is None:
                raise
            assert expected in str(exc), str(exc)
            assert not path.exists(), f"denied body ran: {name}"
        else:
            assert expected is None, f"expected {expected}, got {result}"
            assert not result.isError, result
            assert path.read_text() == name
        checks.append({"case": name, "decision": expected or "allow",
                       "body_ran": path.exists()})

    try:
        sidecar = start_sidecar()
        a = await host("A")
        await write(a, "allowed.txt")
        operator("policy", "--agent", "A", "--tools", "read_text_file")
        await write(a, "off-list.txt", "purpose_exceeded")
        operator("policy", "--agent", "A", "--tools", "write_file,read_text_file",
                 "--irreversible", "write_file")
        await write(a, "unapproved.txt", "needs_human")
        operator("approve", "--agent", "A", "--tool", "write_file")
        await write(a, "approved-once.txt")
        await write(a, "approval-consumed.txt", "needs_human")
        assert client.register_child("A", "child").get("live") is True
        child = await host("child")
        b = await host("B")
        operator("kill", "--agent", "A")
        await write(a, "killed-parent.txt", "killed")
        await write(child, "killed-child.txt", "killed")
        await write(b, "unrelated.txt")
        assert client.ensure("A").get("ok") is True
        await write(a, "ensure-does-not-resurrect.txt", "killed")
        sidecar.terminate()
        sidecar.wait(timeout=10)
        sidecar = None
        await write(b, "outage.txt", "store_unavailable")
        sidecar = start_sidecar()
        await write(a, "restart-revoked.txt", "killed")
        await write(b, "restart-unrelated.txt")
        wrong = await host("wrong-token", "incorrect-disposable-token")
        await write(wrong, "wrong-token.txt", "unauthorized")
        report = json.loads(operator("incident", "--agent", "A", "--json", "--children").stdout)
        assert report
        print(json.dumps({"hermes_revision": subprocess.check_output(
            ["git", "-C", str(args.hermes), "rev-parse", "HEAD"], text=True).strip(),
            "binary_version": operator("--version").stdout.strip(),
            "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
            "filesystem_server_version": json.loads(
                (args.filesystem_server.parent.parent / "package.json").read_text())["version"],
            "checks": checks, "scope": "Hermes MCP transport only; no model turn"}, indent=2))
    finally:
        for server in reversed(servers):
            await server.shutdown()
        if sidecar is not None:
            sidecar.terminate()
            sidecar.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hermes", required=True, type=Path)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--filesystem-server", required=True, type=Path)
    parser.add_argument("--node", default=shutil.which("node"))
    args = parser.parse_args()
    for name in ("hermes", "binary", "filesystem_server"):
        value = getattr(args, name).resolve()
        if not value.exists():
            parser.error(f"missing {name}: {value}")
        setattr(args, name, value)
    if not args.node:
        parser.error("Node is required")
    with tempfile.TemporaryDirectory(prefix="deadbolt-hermes-acceptance-") as temp:
        asyncio.run(acceptance(args, Path(temp).resolve()))


if __name__ == "__main__":
    main()
