# DeadBolt MCP with Docker

Use this route when your MCP client can launch Docker and you want a local
stdio container. **The source build is available now. Docker MCP Catalog
submission [#5627](https://github.com/docker/mcp-registry/pull/5627) is pending
review.** Use the source build below; catalog acceptance and a Docker-hosted
image are not established by the preview release.

The **Docker package 2.0.0-rc.2 developer preview** builds engine
**2.0.0-rc.2** from exact source
`d166c7b039800bbd6d8acb1bc5c66f9add472f5f`. It requires version 2 operator
configuration: a fixed future deadline, explicit renewal mode and deliberate
offline-tool exceptions. Stable Docker package 1.0.0 with engine 1.0.3 remains
available from `e5efc1595bc87683ccf96bc367448012a9e170ee`; old configuration
is not silently migrated.

This launcher supports broad one-shot tool approval; it strips inherited
`DEADBOLT_*` variables and does not provision exact-action grants. Use the
Rust/Python or raw-proxy integration when your executor needs exact-action
grants.
## Build the reviewed source

You need Git and a running Docker Linux engine. Node and Rust are supplied by
the image/build stages. The tested native architectures are Linux ARM64 and
AMD64, including Docker Desktop's Linux engine on macOS and Windows.

```sh
git clone https://github.com/seanebones-lang/deadbolt-docker-mcp.git
cd deadbolt-docker-mcp
git checkout v2.0.0-rc.2
docker build -t n11-deadbolt-docker:local .
```

This revision pins the engine source checksum, Cargo lockfile and official base
image digests. The first build downloads public sources and dependencies.
Startup does not download an engine or connect an account. The tag above names
your local build; it is not a public registry download.

## Start with read-only setup

For clients using an `mcpServers` configuration:

```json
{
  "mcpServers": {
    "deadbolt": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "--network", "none", "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "n11-deadbolt-docker:local"]
    }
  }
}
```

VS Code uses a `servers` key with `type: "stdio"`; retain the same command and
arguments. See the [editor guide](EDITORS.md) for client-specific configuration
and trust controls. Keep Docker running. Do not add `-t`, which changes the
stdio stream.

With no operator configuration, the container exposes two tools:

- `deadbolt_check_bundle`: check the bundled engine and report whether protection is active.
- `deadbolt_get_integration_guide`: read instructions for connecting a trusted server.

**Setup mode provides no active protection.** It needs no secrets or mounts.
The executable checksum detects corruption; it is not independent signature
verification or a security certification.

## Enable one protected downstream

Follow the pinned package's
[operator configuration and derived-image guide](https://github.com/seanebones-lang/deadbolt-docker-mcp/blob/v2.0.0-rc.2/README.md#put-a-trusted-server-behind-the-gate).
Your reviewed downstream runs **inside the same container**. Supply a private
read-only configuration mount and a separate writable state mount, outside
agent-editable workspaces. Set actual tool names, policy and a fresh run ID
deliberately. Invalid supplied configuration stops startup.

Placing DeadBolt beside another server in Docker MCP Toolkit does not gate
that server. Only routed `tools/call` requests in this configured container are
protected. The downstream remains trusted; its startup, other MCP methods,
existing tool bodies and OS/network access are outside the admission gate.
Operator approval, pause, resume, clip and kill remain local operator actions.
The preview requires a fixed future integer Unix `hard_deadline_at`. Activity
mode renews the 60-second idle TTL on live admission; supervisor mode requires
separate trusted operator renewal. Neither extends the fixed deadline. Restart
cannot change the deadline or mode, revive expiry/kill, or migrate an old run.
The example's zero deadline is an invalid placeholder that you must replace.

Configured mode installs the destination allow-list even if it is empty. Every
routed tool must declare an allowed destination or have an explicit operator
`offline_tools` exception. With both lists empty, no tool call is admitted.
An exception cannot override a declared foreign destination. This is declared
policy, not proof of a tool's actual network behavior; enforce egress separately.
Keep supervisor credentials outside the model-facing container; the setup entry
does not provision a renewal sidecar or heartbeat. Run state-writing operator
commands in the short-lived Linux operator container shown in the pinned README.
A separately provisioned supervisor sidecar must likewise use the same Docker
Linux engine and private state. Do not concurrently open/edit that SQLite state
with host-native macOS/Windows processes; the supported test path keeps database
locking and updates on the Linux side of Docker Desktop's mounts.

For a runnable operator-side example, use the separate
[RC2 container supervisor reference](https://github.com/seanebones-lang/deadbolt-docker-mcp/blob/8592bc61bac2246ea52b1e25fc50f1be36baa62f/examples/supervisor/README.md).
It builds an operator image with the matching checksum-pinned Python client,
puts the authenticated sidecar socket in a private Linux volume, and mounts
only that socket and one read-only renewal key into the finite heartbeat.
The downstream receives neither the control socket nor credential mounts.
This later example source does not replace the frozen RC2 MCP image/tag.
The downstream still shares writable state and remains trusted; the reference
does not make same-user state tamper-proof or cancel running tool bodies.

Use `--network none` for local-only tools. Remote services require deliberate
network configuration and credentials. Do not mount your home directory or
Docker socket, and do not use privileged mode. These limits are part of the
container configuration; DeadBolt is not a whole-agent sandbox.

## Verify and remove

From the pinned package checkout, with Python 3.11+ and the image built:

```sh
python3 tests/acceptance.py n11-deadbolt-docker:local
# Optional: requires Docker Desktop's MCP Toolkit.
python3 tests/gateway_acceptance.py
```

The RC2 acceptance suite checks 24 cases in disposable local state, including
23 gated calls: 8 allowed effects and 15 denials without effects. It covers
single-use approval/replay, pause/resume/clip, terminal kill/reconnect, explicit
destination/exception policy, immutable run bounds, supervisor non-renewal,
a real wall-clock deadline and running store outage. Denials must carry the
expected MCP tool-error reason code and retry hint. Idle expiry is
operator-forced; a live supervisor heartbeat is not exercised by this suite.
The gateway check uses a separate temporary catalog/configuration and does not
change your client connections. CI passed these checks on native AMD64 and
ARM64 with the official Docker MCP Gateway v0.42.0. These are protocol/container
checks, not model-mediated client testing or independent customer adoption.
[Published RC2 container evidence](evidence/docker-rc2-2026-10-12.json)
records exact source/image and CI checks.

The separate supervisor lifecycle suite crosses the actual default 60-second
idle expiry with real renewal, then stops its heartbeat while tools keep calling.
It checks idle-expiry denials without effects, refused late renewal/reconnect,
and the fixed deadline winning over continued heartbeats. It uses no forced
SQLite expiry edits. Follow the reference's own checkout/build commands; this
additional suite is not present in the earlier frozen Docker RC2 source tag.

Remove your client entry and stop its container to disconnect. `--rm` removes
the container; private bind-mounted state remains. Back up or deliberately
remove that state separately. Remove the local image with
`docker image rm n11-deadbolt-docker:local` when no container uses it.

[Package source, privacy and support](https://github.com/seanebones-lang/deadbolt-docker-mcp) ·
[All installation routes](INSTALL.md) · [Integration boundaries](INTEGRATION.md)
