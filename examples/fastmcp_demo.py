#!/usr/bin/env python3
"""Disposable stdio FastMCP integration acceptance; no model or real account."""
import argparse
import asyncio
import importlib.metadata
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import tempfile
import time

from fastmcp import Client
from fastmcp.client.transports import StdioTransport
import deadbolt_client as operator_client

TOOL = "demo_write_note"
RUN = "fastmcp-demo-run"


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


async def evaluate(binary, state, sidecar, operator_env, endpoint, credential):
    effect = state / "note.txt"
    server = Path(__file__).with_name("fastmcp_server.py")
    cases = []

    def operator(*args):
        subprocess.run([str(binary), *args], env=operator_env, check=True,
                       capture_output=True, timeout=10)

    def connection(token=credential, run=RUN):
        return Client(StdioTransport(
            command=sys.executable, args=[str(server), "--agent", run, "--output", str(effect)],
            env={"DEADBOLT_SOCK": endpoint, "DEADBOLT_ADMISSION_TOKEN": token},
            keep_alive=False,
        ), timeout=10)

    async def check(client, name, content, denied=None):
        before = effect.read_bytes() if effect.exists() else None
        result = await client.call_tool(TOOL, {"content": content}, raise_on_error=False)
        if denied:
            text = " ".join(getattr(item, "text", "") for item in result.content)
            if not result.is_error or f"deadbolt:{denied};" not in text:
                raise RuntimeError(f"{name}: expected denial {denied}")
            after = effect.read_bytes() if effect.exists() else None
            if after != before:
                raise RuntimeError(f"{name}: denied body changed the note")
        elif result.is_error or effect.read_text(encoding="utf-8") != content:
            raise RuntimeError(f"{name}: allowed effect missing")
        cases.append(name)

    async with connection() as client:
        tools = await client.list_tools()
        if [tool.name for tool in tools] != [TOOL] or set(tools[0].input_schema["properties"]) != {"content"}:
            raise RuntimeError("tool schema exposed unexpected control arguments")
        cases.append("discovery_has_only_effect_tool_and_content_argument")
        operator("policy", "--agent", RUN, "--tools", "different_tool")
        await check(client, "off_policy_body_absent", "blocked", "purpose_exceeded")
        operator("policy", "--agent", RUN, "--tools", TOOL)
        await check(client, "allowed_file_effect", "allowed")
        before = effect.read_bytes()
        bad = await client.call_tool(TOOL, {"content": ""}, raise_on_error=False)
        if not bad.is_error or effect.read_bytes() != before:
            raise RuntimeError("invalid schema reached the body")
        cases.append("invalid_argument_body_absent")
        operator("pause", "--agent", RUN)
        await check(client, "paused_body_absent", "paused", "paused")
        operator("resume", "--agent", RUN)
        await check(client, "resumed_file_effect", "resumed")
        operator("policy", "--agent", RUN, "--tools", TOOL, "--irreversible", TOOL)
        await check(client, "approval_required_body_absent", "no grant", "needs_human")
        operator("approve", "--agent", RUN, "--tool", TOOL)
        await check(client, "one_shot_approved_effect", "approved")
        await check(client, "one_shot_replay_body_absent", "replay", "needs_human")
        operator("clip", TOOL, "--agent", RUN)
        await check(client, "clipped_body_absent", "clipped", "purpose_exceeded")
        operator("policy", "--agent", RUN, "--tools", TOOL)
        async with connection("invalid-synthetic-credential") as invalid:
            await check(invalid, "invalid_credential_body_absent", "invalid", "unauthorized")
        async with connection(run="different-run") as other_run:
            await check(other_run, "credential_cannot_change_run", "wrong run", "unauthorized")
        revoked_key = state / "revocable-key"
        operator("credential", "issue", "--agent", RUN, "--id", "revocable-key", "--out", str(revoked_key))
        async with connection(revoked_key.read_text().strip()) as revoked:
            operator("credential", "revoke", "--id", "revocable-key")
            await check(revoked, "revoked_credential_body_absent", "revoked key", "unauthorized")
        short_key = state / "short-key"
        operator("credential", "issue", "--agent", RUN, "--id", "short-key", "--ttl-secs", "1", "--out", str(short_key))
        async with connection(short_key.read_text().strip()) as expired:
            await asyncio.sleep(1.1)
            await check(expired, "expired_credential_body_absent", "expired key", "unauthorized")
        operator("kill", "--agent", RUN)
        await check(client, "killed_body_absent", "killed", "killed")

    async with connection() as reconnected:
        await check(reconnected, "kill_persists_after_mcp_reconnect", "reconnect", "killed")
        stop(sidecar)
        await check(reconnected, "sidecar_outage_body_absent", "outage", "store_unavailable")
    return {"passed": True, "framework": "fastmcp==4.1.2", "client": "n11-deadbolt-client==" + importlib.metadata.version("n11-deadbolt-client"),
            "transport": "stdio subprocess", "provider_requests": 0,
            "cases": cases, "count": len(cases), "operator_token_forwarded": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if importlib.metadata.version("fastmcp") != "4.1.2":
        parser.error("this recipe is tested with fastmcp==4.1.2")
    installed = importlib.metadata.version("n11-deadbolt-client")
    supported = {"1.1.0rc1": "deadbolt 1.1.0-rc.1", "2.0.0rc1": "deadbolt 2.0.0-rc.1",
                 "2.0.0rc2": "deadbolt 2.0.0-rc.2",
                 "2.0.0rc3": "deadbolt 2.0.0-rc.3", "2.0.0": "deadbolt 2.0.0"}
    if installed not in supported:
        parser.error("this recipe requires a matching 1.1.0rc1, 2.0.0rc1, 2.0.0rc2, 2.0.0rc3 or 2.0.0 client")
    version = subprocess.check_output([str(binary), "--version"], text=True, timeout=10).strip()
    if version != supported[installed]:
        parser.error("install the engine matching the tested client: " + supported[installed])
    previous = dict(os.environ)
    try:
        with tempfile.TemporaryDirectory(prefix="deadbolt-fastmcp-") as directory:
            state = Path(directory)
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", 0))
                endpoint = f"127.0.0.1:{sock.getsockname()[1]}"
            operator_env = {key: value for key, value in previous.items() if not key.startswith("DEADBOLT_")}
            operator_env.update({"DEADBOLT_SOCK": endpoint, "DEADBOLT_TOKEN": secrets.token_hex(32),
                                 "DEADBOLT_DB": str(state / "db"), "DEADBOLT_EVENTS": str(state / "events.jsonl")})
            os.environ.clear()
            os.environ.update(operator_env)
            sidecar = subprocess.Popen([str(binary), "serve", "--bind", endpoint], env=operator_env,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                for _ in range(100):
                    if operator_client.ensure(RUN).get("ok") is True:
                        break
                    if sidecar.poll() is not None:
                        raise RuntimeError("disposable sidecar exited")
                    time.sleep(.02)
                else:
                    raise RuntimeError("disposable sidecar did not become ready")
                key = state / "admission-key"
                subprocess.run([str(binary), "credential", "issue", "--agent", RUN, "--id", "demo-key",
                                "--out", str(key)], env=operator_env, check=True, capture_output=True, timeout=10)
                report = asyncio.run(evaluate(binary, state, sidecar, operator_env, endpoint, key.read_text().strip()))
                report["engine"] = version
                print(json.dumps(report, indent=2))
            finally:
                stop(sidecar)
    finally:
        os.environ.clear()
        os.environ.update(previous)


if __name__ == "__main__":
    main()
