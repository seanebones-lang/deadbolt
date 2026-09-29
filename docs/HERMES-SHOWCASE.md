# Independent showcase: Hermes and a reference MCP server

Deadbolt can sit between Hermes' stdio MCP client and a trusted MCP server.
The acceptance test uses real Hermes transport code and the reference filesystem
server. It needs no model credentials or paid model calls, and creates a temporary
Hermes home, state store and filesystem workspace. It does not change an installed
Hermes profile.

## What passed

On 2026-09-29, macOS arm64, Python 3.12.13 and the installed Deadbolt binary built
from `fecb6f74b9957186fe698c921516c17a12a5f003` passed 13 checks. The merge commit
`84b8c135b44e32e17ac54d8bde37f984a8c9e490` contains that implementation.

Hermes source: `5eb1381f3965fd96454d1c32f6bbefcd99cda347` from
[NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent).
Reference server: `@modelcontextprotocol/server-filesystem@2026.8.31` from
[the MCP reference servers](https://github.com/modelcontextprotocol/servers).

Four permitted writes created the expected file content. Nine denied attempts
created no file: off-list tool, approval required, approval consumed, killed parent,
killed child, attempted re-ensure of a killed ID, sidecar outage, revocation after
restart and wrong authentication token. An unrelated identity worked before and
after restart. An incident export also completed.

The [recorded results](evidence/hermes-mcp-2026-09-29.json) distinguish decision
codes from actual body execution. The runnable test is
[`tests/hermes_mcp_acceptance.py`](../tests/hermes_mcp_acceptance.py).

The candidate executable repeats the same acceptance, with its binary hash and
server version recorded in [candidate results](evidence/hermes-mcp-rc.1-2026-09-29.json).
The release-candidate workflow also runs this path on Linux; consult its actual
job result before claiming that platform passed.

The stable 1.0.0 executable also passed the 13 checks locally; its version and
binary hash are in [v1 results](evidence/hermes-mcp-v1-2026-09-29.json).
The final release workflow repeats this acceptance on Linux at the selected
release source revision.

## Reproduce

Use an isolated directory and a Python 3.12 virtual environment. Install the
tested Python dependencies from `tests/hermes-acceptance-requirements.txt` into
that environment; do not install them into your production Hermes environment.
For example, from the Deadbolt checkout:

```sh
git clone https://github.com/NousResearch/hermes-agent.git /tmp/deadbolt-hermes
git -C /tmp/deadbolt-hermes checkout 5eb1381f3965fd96454d1c32f6bbefcd99cda347
python3.12 -m venv /tmp/deadbolt-hermes-venv
/tmp/deadbolt-hermes-venv/bin/python -m pip install -r tests/hermes-acceptance-requirements.txt
npm install --prefix /tmp/deadbolt-mcp-reference --ignore-scripts --no-audit --no-fund @modelcontextprotocol/server-filesystem@2026.8.31
cargo build --release --locked --bin deadbolt
/tmp/deadbolt-hermes-venv/bin/python tests/hermes_mcp_acceptance.py \
  --hermes /tmp/deadbolt-hermes \
  --binary target/release/deadbolt \
  --filesystem-server /tmp/deadbolt-mcp-reference/node_modules/@modelcontextprotocol/server-filesystem/dist/index.js
```

Choose different paths if those temporary directories already contain work.
The test exits unsuccessfully if an expected denial runs its body or a permitted
body fails. Temporary state and files are removed on exit. The test uses Hermes
internal transport APIs, so its pinned revision matters; future Hermes revisions
need fresh acceptance. npm's transitive resolution is not frozen by the package
version alone; preserve the generated package lock for your rerun.

## Configure the routed MCP connection

For a deliberately selected trusted filesystem server, the shape of Hermes'
`mcp_servers` entry is:

```yaml
mcp_servers:
  deadbolt_filesystem:
    command: /absolute/path/to/deadbolt
    args:
      - mcp-proxy
      - --agent
      - operator-assigned-run-id
      - --
      - /absolute/path/to/node
      - /absolute/path/to/server-filesystem/dist/index.js
      - /absolute/path/to/disposable-workspace
    env:
      DEADBOLT_DB: /absolute/path/to/private-state/deadbolt.db
      DEADBOLT_EVENTS: /absolute/path/to/private-state/events.jsonl
```

This example embeds admission in the proxy. The automated acceptance instead
uses `--serve-sock` with a tokened loopback sidecar so it can also test outages.
Configure policy through the operator before executing calls. Policy names are
the original server tool names, such as `write_file`, not Hermes' prefixed names.
Assign a new ID to each intentional new run; reuse the same ID on reconnection.

## Exact scope

This establishes handshake, tool discovery and actual tool-call behavior through
Hermes' real MCP transport. It is not a full Hermes conversation, a Hermes
maintainer endorsement, or independent human validation. Deadbolt's authoring
team ran the test against independent software.

Only tool calls routed through this connection are gated. Hermes built-in
terminal, browser, file tools, code execution, delegation and other MCP
connections remain outside it. The MCP server is trusted and inherits the
proxy's OS access; startup and other MCP methods are not action gates. Do not
present this configuration as containment of the whole Hermes agent.

For a whole-agent integration, first audit every actual dispatch route, identity
assignment and child path. A plugin can supplement the routing proof, but a
disabled, unloaded or bypassed plugin cannot establish mandatory enforcement.
