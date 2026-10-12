#!/usr/bin/env python3
"""One protected local FastMCP tool; operator setup belongs in another process."""
import argparse
import asyncio
import os
from pathlib import Path
from typing import Annotated

from fastmcp import FastMCP
from fastmcp.exceptions import ToolError
from pydantic import Field
from deadbolt_client import dispatch_async


def create_server(agent_id: str, output: Path) -> FastMCP:
    server = FastMCP("deadbolt_notes_mcp", mask_error_details=True)

    @server.tool(annotations={"readOnlyHint": False, "destructiveHint": True,
                              "idempotentHint": True, "openWorldHint": False})
    async def demo_write_note(
        content: Annotated[str, Field(min_length=1, max_length=4096)],
    ) -> str:
        """Replace the operator-selected demo note. Denial leaves it unchanged.

        content: Non-empty note text, at most 4096 characters. The output path
        and run identity are fixed by the operator, not MCP call arguments.
        """
        async def effect() -> str:
            await asyncio.to_thread(output.write_text, content, encoding="utf-8")
            return "written"

        outcome = await dispatch_async(agent_id, "demo_write_note", effect)
        if not outcome["executed"]:
            code = outcome["decision"].get("code", "unknown")
            raise ToolError(f"deadbolt:{code}; the note was not written. Contact the operator.")
        return outcome["result"]

    return server


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    # Refuse operator credentials in this workload process, even if the client
    # would otherwise prefer its scoped admission credential.
    if "DEADBOLT_TOKEN" in os.environ:
        parser.error("keep the operator token outside the MCP server process")
    if not os.environ.get("DEADBOLT_SOCK") or not os.environ.get("DEADBOLT_ADMISSION_TOKEN"):
        parser.error("operator-supplied sidecar address and admission credential required")
    output = args.output.resolve()
    if not output.parent.is_dir():
        parser.error("the operator-selected output directory must already exist")
    create_server(args.agent, output).run(transport="stdio", show_banner=False)


if __name__ == "__main__":
    main()
