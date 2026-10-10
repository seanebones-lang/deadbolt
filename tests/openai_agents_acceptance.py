"""Installed-wheel, real SDK Runner/sidecar acceptance. No API/provider calls.

Install the candidate wheel and openai-agents==0.23.1 in an isolated environment.
Run from any directory: python /absolute/path/tests/openai_agents_acceptance.py
"""
from __future__ import annotations

import asyncio
import importlib.metadata
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest

from agents import Agent, RunConfig, Runner, RunContextWrapper, function_tool
from agents import ToolGuardrailFunctionOutput, tool_input_guardrail, set_tracing_disabled
from agents.testing import ScriptedModel, ModelStep, assistant_message, function_call
from agents.exceptions import UserError, ModelBehaviorError
from deadbolt_openai_agents import DeadboltDenied, protected_tool, denial_code
import deadbolt_client as client

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("DEADBOLT_BIN", ROOT / "target/debug/deadbolt")).resolve()
set_tracing_disabled(True)


class OpenAIAcceptance(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.assertEqual(importlib.metadata.version("openai-agents"), "0.23.1")
        self.assertFalse(Path(client.__file__).resolve().is_relative_to(ROOT), "install the wheel first")
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        self.old = dict(os.environ)
        self.addCleanup(self.restore_env)
        os.environ.update({"DEADBOLT_SOCK": f"127.0.0.1:{port}",
                           "DEADBOLT_TOKEN": "synthetic-test-token",
                           "DEADBOLT_DB": self.temp.name + "/db",
                           "DEADBOLT_EVENTS": self.temp.name + "/events.jsonl"})
        self.server = subprocess.Popen([str(BINARY), "serve", "--bind", os.environ["DEADBOLT_SOCK"]],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        self.addCleanup(self.stop_server)
        for _ in range(100):
            if client.ensure("worker").get("ok") is True:
                break
            if self.server.poll() is not None:
                self.fail(self.server.stderr.read().decode())
            time.sleep(.02)
        else:
            self.fail("sidecar did not start")
        self.file = Path(self.temp.name) / "effect"

    def restore_env(self):
        os.environ.clear()
        os.environ.update(self.old)

    def stop_server(self):
        if self.server.poll() is None:
            self.server.terminate()
            self.server.wait(timeout=5)
        self.server.stderr.close()

    def operator(self, *args):
        subprocess.run([str(BINARY), *args], check=True, capture_output=True, timeout=10)

    def make_tool(self, **options):
        async def write_file(content: str) -> str:
            self.file.write_text(content)
            return "written:" + content
        return protected_tool(agent_id="worker", **options)(write_file)

    def make_agent(self, tool, arguments=None, call_id="call-1"):
        model = ScriptedModel([
            ModelStep(output=[function_call(tool.name, arguments or {"content": "allowed"}, call_id=call_id)]),
            ModelStep(output=[assistant_message("finished")]),
        ])
        return Agent(name="Synthetic executor", tools=[tool], model=model)

    async def run_tool(self, tool, arguments=None, **kwargs):
        return await Runner.run(self.make_agent(tool, arguments), "synthetic test", **kwargs)

    async def test_runner_allow_and_policy_denial_have_real_effects(self):
        tool = self.make_tool()
        self.assertEqual((await self.run_tool(tool)).final_output, "finished")
        self.assertEqual(self.file.read_text(), "allowed")
        self.assertTrue(client.policy("worker", tools=["different_tool"])["ok"])
        with self.assertRaises(UserError) as denied:
            await self.run_tool(tool, {"content": "forbidden"})
        self.assertEqual(denial_code(denied.exception), "purpose_exceeded")
        self.assertEqual(self.file.read_text(), "allowed")

    async def test_pause_kill_and_sidecar_failure_leave_body_untouched(self):
        tool = self.make_tool()
        self.operator("pause", "--agent", "worker")
        with self.assertRaises(UserError) as paused:
            await self.run_tool(tool)
        self.assertIsNotNone(denial_code(paused.exception))
        self.assertFalse(self.file.exists())
        self.operator("resume", "--agent", "worker")
        self.operator("kill", "--agent", "worker")
        with self.assertRaises(UserError) as killed:
            await self.run_tool(tool)
        self.assertEqual(denial_code(killed.exception), "killed")
        # Constructing/calling tools never ensures or revives a revoked run.
        self.assertEqual(client.status("worker")["agents"][0]["state"], "killed")
        self.server.terminate()
        self.server.wait(timeout=5)
        with self.assertRaises(UserError) as down:
            await self.run_tool(tool)
        self.assertEqual(denial_code(down.exception), "store_unavailable")
        self.assertFalse(self.file.exists())

    async def test_sdk_approval_resume_rechecks_revocation(self):
        tool = self.make_tool(needs_approval=True)
        agent = self.make_agent(tool)
        result = await Runner.run(agent, "synthetic test")
        self.assertEqual(len(result.interruptions), 1)
        self.assertFalse(self.file.exists())
        state = result.to_state()
        state.approve(result.interruptions[0])
        self.operator("kill", "--agent", "worker")
        with self.assertRaises(UserError) as denied:
            await Runner.run(agent, state)
        self.assertEqual(denial_code(denied.exception), "killed")
        self.assertFalse(self.file.exists())

    async def test_preapproval_guardrails_do_not_consume_one_shot(self):
        calls = []
        @tool_input_guardrail
        def inspect_input(data):
            calls.append(data.context.tool_call_id)
            return ToolGuardrailFunctionOutput.allow()
        self.assertTrue(client.policy("worker", irreversible=["write_file"])["ok"])
        self.operator("approve", "--agent", "worker", "--tool", "write_file")
        from agents import ToolExecutionConfig
        config = RunConfig(tool_execution=ToolExecutionConfig(pre_approval_tool_input_guardrails=True))
        tool = self.make_tool(needs_approval=True, tool_input_guardrails=[inspect_input])
        agent = self.make_agent(tool)
        pending = await Runner.run(agent, "synthetic test", run_config=config)
        self.assertEqual(len(pending.interruptions), 1)
        self.assertEqual(len(calls), 1)
        self.assertFalse(self.file.exists())
        state = pending.to_state()
        state.approve(pending.interruptions[0])
        result = await Runner.run(agent, state, run_config=config)
        self.assertEqual(result.final_output, "finished")
        self.assertEqual(len(calls), 2)
        self.assertEqual(self.file.read_text(), "allowed")
        with self.assertRaises(UserError) as denied:
            await self.run_tool(self.make_tool(), {"content": "twice"})
        self.assertEqual(denial_code(denied.exception), "needs_human")
        self.assertEqual(self.file.read_text(), "allowed")

    async def test_competing_runners_execute_only_one_approved_body(self):
        self.assertTrue(client.policy("worker", irreversible=["write_file"])["ok"])
        self.operator("approve", "--agent", "worker", "--tool", "write_file")
        outcomes = await asyncio.gather(*[self.run_tool(self.make_tool(), {"content": str(i)})
                                         for i in range(8)], return_exceptions=True)
        self.assertEqual(sum(not isinstance(x, Exception) for x in outcomes), 1)
        self.assertEqual(sum(isinstance(x, Exception) and denial_code(x) == "needs_human"
                             for x in outcomes), 7)
        self.assertIn(self.file.read_text(), [str(i) for i in range(8)])

    async def test_callable_approval_preserves_sdk_default_argument_inspection(self):
        inspected = []
        async def review(ctx, args, call_id):
            inspected.append(args)
            return False
        async def write_file(content: str = "default") -> str:
            self.file.write_text(content)
            return content
        tool = protected_tool(agent_id="worker", needs_approval=review)(write_file)
        # Missing default changes parsed arguments: SDK requires manual approval.
        agent = self.make_agent(tool, "{}")
        pending = await Runner.run(agent, "synthetic test")
        self.assertEqual(len(pending.interruptions), 1)
        self.assertEqual(inspected, [])
        self.assertFalse(self.file.exists())
        state = pending.to_state()
        state.approve(pending.interruptions[0])
        await Runner.run(agent, state)
        self.assertEqual(self.file.read_text(), "default")

    async def test_context_schema_and_sync_tool_result_are_preserved(self):
        def write_file(ctx: RunContextWrapper[dict], content: str) -> str:
            """Write the synthetic content."""
            self.file.write_text(ctx.context["prefix"] + content)
            return "sync-result"
        original = function_tool(write_file, failure_error_function=None)
        guarded = protected_tool(agent_id="worker")(write_file)
        self.assertEqual(guarded.params_json_schema, original.params_json_schema)
        self.assertEqual(guarded.description, original.description)
        result = await self.run_tool(guarded, context={"prefix": "context:"})
        self.assertEqual(self.file.read_text(), "context:allowed")
        outputs = [i.raw_item for i in result.new_items if i.type == "tool_call_output_item"]
        self.assertEqual(outputs[0]["output"], "sync-result")

    async def test_destination_is_resolved_from_validated_arguments(self):
        self.assertTrue(client.policy("worker", dest=["example.com"])["ok"])
        async def fetch(host: str) -> str:
            self.file.write_text(host)
            return host
        tool = protected_tool(agent_id="worker", destination=lambda args: args["host"])(fetch)
        await self.run_tool(tool, {"host": "example.com"})
        with self.assertRaises(UserError) as denied:
            await self.run_tool(tool, {"host": "other.com"})
        self.assertEqual(denial_code(denied.exception), "purpose_exceeded")
        self.assertEqual(self.file.read_text(), "example.com")

    async def test_resolver_failure_stops_before_admission_and_body(self):
        self.assertTrue(client.policy("worker", irreversible=["write_file"])["ok"])
        self.operator("approve", "--agent", "worker", "--tool", "write_file")
        invalid = self.make_tool(destination=lambda args: None)
        with self.assertRaises(UserError) as invalid_dest:
            await self.run_tool(invalid)
        self.assertIsInstance(invalid_dest.exception.__cause__, ValueError)
        self.assertFalse(self.file.exists())
        # Resolver failure did not spend the one-shot admission.
        await self.run_tool(self.make_tool())
        self.assertEqual(self.file.read_text(), "allowed")

    async def test_tool_exception_is_not_retried_or_returned_to_model(self):
        calls = []
        async def write_file(content: str) -> str:
            calls.append(content)
            raise RuntimeError("synthetic-tool-failure")
        tool = protected_tool(agent_id="worker")(write_file)
        with self.assertRaisesRegex(UserError, "synthetic-tool-failure") as failure:
            await self.run_tool(tool)
        self.assertIsInstance(failure.exception.__cause__, RuntimeError)
        self.assertIsNone(denial_code(failure.exception))
        self.assertEqual(calls, ["allowed"])
        self.assertFalse(self.file.exists())

    async def test_renamed_tool_uses_same_identity_for_policy_and_sdk(self):
        self.assertTrue(client.policy("worker", tools=["protected_write"])["ok"])
        tool = self.make_tool(name_override="protected_write")
        self.assertEqual(tool.name, "protected_write")
        await self.run_tool(tool)
        self.assertEqual(self.file.read_text(), "allowed")
        with self.assertRaises(UserError) as wrong_name:
            await self.run_tool(self.make_tool(), {"content": "wrong"})
        self.assertEqual(denial_code(wrong_name.exception), "purpose_exceeded")
        self.assertEqual(self.file.read_text(), "allowed")

    async def test_schema_validation_precedes_admission(self):
        from typing import Annotated
        from pydantic import Field
        # Use resolved annotations on this local function for postponed annotations.
        async def write_file(content):
            self.file.write_text(content)
            return content
        write_file.__annotations__ = {"content": Annotated[str, Field(min_length=3)], "return": str}
        tool = protected_tool(agent_id="worker")(write_file)
        self.assertEqual(tool.params_json_schema["properties"]["content"]["minLength"], 3)
        self.assertTrue(client.policy("worker", irreversible=["write_file"])["ok"])
        self.operator("approve", "--agent", "worker", "--tool", "write_file")
        with self.assertRaises(ModelBehaviorError) as invalid:
            await self.run_tool(tool, {"content": "x"})
        self.assertIsNone(denial_code(invalid.exception))
        self.assertFalse(self.file.exists())
        await self.run_tool(tool)
        self.assertEqual(self.file.read_text(), "allowed")

    async def test_async_resolver_and_deferred_sync_body_are_rejected(self):
        async def resolver(args):
            return "example.com"
        with self.assertRaises(UserError) as invalid:
            await self.run_tool(self.make_tool(destination=resolver))
        self.assertIsInstance(invalid.exception.__cause__, TypeError)
        async def effect():
            self.file.write_text("deferred")
        def write_file(content: str):
            return effect()
        with self.assertRaises(UserError) as deferred:
            await self.run_tool(protected_tool(agent_id="worker")(write_file))
        self.assertIsInstance(deferred.exception.__cause__, TypeError)
        self.assertFalse(self.file.exists())

    def test_setup_rejects_unsupported_inputs(self):
        with self.assertRaises(TypeError):
            protected_tool(agent_id="worker")(self.make_tool())
        with self.assertRaises(ValueError):
            protected_tool(agent_id="worker", failure_error_function=lambda *args: "retry")
        with self.assertRaises(ValueError):
            protected_tool(agent_id="model supplied id")


if __name__ == "__main__":
    unittest.main(verbosity=2)
