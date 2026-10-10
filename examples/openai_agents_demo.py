#!/usr/bin/env python3
"""One-command, disposable SDK function-tool demo. No API key or provider calls."""
import argparse
import asyncio
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time

from agents import Agent, Runner, set_tracing_disabled
from agents.exceptions import UserError
from agents.testing import ScriptedModel, ModelStep, function_call, assistant_message
from deadbolt_openai_agents import protected_tool, denial_code
import deadbolt_client as client


async def evaluate(binary, output):
    @protected_tool(agent_id="demo-run")
    async def write_file(content: str) -> str:
        """Write the disposable demonstration file."""
        output.write_text(content)
        return "written"

    def agent(content):
        return Agent(name="Offline demonstration", tools=[write_file], model=ScriptedModel([
            ModelStep(output=[function_call("write_file", {"content": content}, call_id="demo-call")]),
            ModelStep(output=[assistant_message("finished")]),
        ]))

    await Runner.run(agent("allowed"), "synthetic local demonstration")
    if output.read_text() != "allowed":
        raise RuntimeError("allowed effect was not verified")
    subprocess.run([str(binary), "kill", "--agent", "demo-run"], check=True,
                   capture_output=True, timeout=10)
    code = None
    try:
        await Runner.run(agent("must not execute"), "synthetic local demonstration")
    except UserError as error:
        code = denial_code(error)
        if code is None:
            raise
    if code != "killed" or output.read_text() != "allowed":
        raise RuntimeError("revoked body was not blocked")
    return {"sdk": "openai-agents==0.23.1", "provider_requests": 0,
            "allowed_file_effect_verified": True, "denied_code": code,
            "denied_body_effect_absent": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    set_tracing_disabled(True)
    previous = dict(os.environ)
    try:
        with tempfile.TemporaryDirectory(prefix="deadbolt-sdk-demo-") as state:
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", 0))
                port = sock.getsockname()[1]
            os.environ.update({"DEADBOLT_SOCK": f"127.0.0.1:{port}",
                               "DEADBOLT_TOKEN": secrets.token_hex(32),
                               "DEADBOLT_DB": str(Path(state) / "db"),
                               "DEADBOLT_EVENTS": str(Path(state) / "events.jsonl")})
            with subprocess.Popen([str(binary), "serve", "--bind", os.environ["DEADBOLT_SOCK"]],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.PIPE) as server:
                try:
                    for _ in range(100):
                        if client.ensure("demo-run").get("ok") is True:
                            break
                        if server.poll() is not None:
                            raise RuntimeError("demo sidecar exited")
                        time.sleep(.02)
                    else:
                        raise RuntimeError("demo sidecar did not become ready")
                    if client.policy("demo-run", tools=["write_file"]).get("ok") is not True:
                        raise RuntimeError("demo policy could not be set")
                    key = Path(state) / "admission-key"
                    subprocess.run([str(binary), "credential", "issue", "--agent", "demo-run", "--id", "demo-key",
                                    "--out", str(key)], check=True, capture_output=True, timeout=10)
                    os.environ["DEADBOLT_ADMISSION_TOKEN"] = key.read_text()
                    os.environ.pop("DEADBOLT_TOKEN", None)
                    print(json.dumps(asyncio.run(evaluate(binary, Path(state) / "effect")), indent=2))
                finally:
                    if server.poll() is None:
                        server.terminate()
                        try:
                            server.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            server.kill()
                            server.wait(timeout=5)
    finally:
        os.environ.clear()
        os.environ.update(previous)


if __name__ == "__main__":
    main()
