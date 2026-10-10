"""Optional OpenAI Agents SDK adapter. No provider calls or automatic lease creation.

Use protected_tool instead of function_tool for each protected Python body.
This is admission, not a sandbox. See docs/PYTHON.md for the integration boundary.
"""

import asyncio
from functools import wraps
import inspect
import re
from typing import get_type_hints

from deadbolt_client import dispatch_async


class DeadboltDenied(RuntimeError):
    """A protected body was not invoked. Carries no arguments or credentials."""

    def __init__(self, code):
        self.code = code
        super().__init__("deadbolt:" + code)


def denial_code(error):
    """Read a DeadBolt denial through SDK exception causes, or return None.

    The SDK Runner wraps body errors in UserError. Do not parse its text, which
    may also describe an ordinary body failure. No arguments are returned here.
    """
    seen = set()
    while isinstance(error, BaseException) and id(error) not in seen:
        if isinstance(error, DeadboltDenied):
            return error.code
        seen.add(id(error))
        error = error.__cause__
    return None


def _token(value, label):
    if (not isinstance(value, str) or not value or len(value) > 128
            or re.fullmatch(r"[A-Za-z0-9_.:/-]+", value) is None):
        raise ValueError("invalid DeadBolt " + label)
    return value


def protected_tool(*, agent_id, destination=None, **sdk_options):
    """Create an SDK FunctionTool whose validated Python body requires admission.

    agent_id is a fixed, executor-assigned run ID. destination is a fixed host
    token or a trusted synchronous resolver receiving a dict of validated Python
    arguments (including context when present). Resolvers must match the host
    the actual body uses and must not perform the protected action themselves.

    Other keyword arguments go to agents.function_tool, including name_override,
    needs_approval and tool guardrails. Error-as-output handlers are unsupported:
    denial and tool errors propagate, stopping the run rather than encouraging
    model retries. This wrapper never calls ensure; ordinary admission keeps
    the gate's existing sliding TTL for live leases, but cannot revive dead runs.
    """
    _token(agent_id, "agent ID")
    if destination is not None and not callable(destination):
        _token(destination, "destination")
    if sdk_options.get("failure_error_function") is not None:
        raise ValueError("protected_tool requires failure_error_function=None")
    sdk_options = {**sdk_options, "failure_error_function": None}

    def decorate(func):
        if not inspect.isfunction(func) or inspect.isasyncgenfunction(func) or inspect.isgeneratorfunction(func):
            raise TypeError("protected_tool requires a Python function, not an existing SDK tool")
        # The SDK builds its own schema/approval preparation from this signature.
        # Evaluate annotations in the original function's namespace before wraps
        # transfers them into this module (important for postponed annotations).
        signature = inspect.signature(func)
        annotations = get_type_hints(func, include_extras=True)
        name = _token(sdk_options.get("name_override") or func.__name__, "tool name")

        @wraps(func)
        async def guarded(*args, **kwargs):
            dest = destination
            if callable(destination):
                bound = signature.bind(*args, **kwargs)
                bound.apply_defaults()
                dest = destination(dict(bound.arguments))
                if inspect.isawaitable(dest):
                    if inspect.iscoroutine(dest):
                        dest.close()
                    raise TypeError("destination resolver must be synchronous")
                # A configured resolver cannot silently disable destination checks.
                _token(dest, "resolved destination")

            async def invoke():
                if inspect.iscoroutinefunction(func):
                    return await func(*args, **kwargs)
                result = await asyncio.to_thread(func, *args, **kwargs)
                if inspect.isawaitable(result):
                    if inspect.iscoroutine(result):
                        result.close()
                    raise TypeError("use an async function for an awaitable tool body")
                return result

            outcome = await dispatch_async(agent_id, name, invoke, dest)
            if not outcome["executed"]:
                raise DeadboltDenied(outcome["decision"]["code"])
            return outcome["result"]

        guarded.__annotations__ = annotations
        # Import only when constructing an SDK tool. Base client remains stdlib-only.
        from agents import function_tool
        return function_tool(guarded, **sdk_options)

    return decorate
