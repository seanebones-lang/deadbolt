"""Prepare local editor configuration from the checksum-pinned DeadBolt MCP bundle.

No profile is edited, no downstream process starts during preparation, and no
network requests are made. Python 3.9+ and a local Node.js 20+ are required.
"""
import argparse
import base64
import hashlib
import html
import json
import os
from pathlib import Path, PurePosixPath
import platform
import queue
import re
import shutil
import stat
import subprocess
import tempfile
import threading
import time
from urllib.parse import urlencode
import uuid
import zipfile

BUNDLE_SHA256 = "a0634e69da89c6e2c38bf72b8c1a1b8ed75e30e77df218841692808998c7b95e"
TARGETS = {("Darwin", "arm64"): "aarch64-apple-darwin",
           ("Darwin", "x86_64"): "x86_64-apple-darwin",
           ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
           ("Windows", "AMD64"): "x86_64-pc-windows-msvc"}

# Trusted, disposable demonstration only. Each admitted call appends its tool
# name to one fixed local marker file. No arguments select a path or destination.
FIXTURE = r'''
const fs = require('node:fs');
const readline = require('node:readline');
const names = ['demo_write_marker', 'demo_publish_marker', 'demo_off_list'];
readline.createInterface({input: process.stdin}).on('line', line => {
 let r;
 try { r = JSON.parse(line); } catch { return; }
 if (!Object.hasOwn(r, 'id')) return;
 let result;
 if (r.method === 'initialize') result = {protocolVersion:r.params?.protocolVersion || '2025-03-26',capabilities:{tools:{}},serverInfo:{name:'deadbolt-disposable-demo',version:'1.0.0'}};
 else if (r.method === 'tools/list') result = {tools:names.map(name=>({name,title:name,description:'Disposable local marker demonstration; no accounts or external effects',inputSchema:{type:'object',properties:{},additionalProperties:false},annotations:{readOnlyHint:false,destructiveHint:false,openWorldHint:false}}))};
 else if (r.method === 'tools/call' && names.includes(r.params?.name)) {
  fs.appendFileSync(process.argv[2], r.params.name + '\n');
  result = {content:[{type:'text',text:'Disposable marker written: ' + r.params.name}]};
 } else { process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:r.id,error:{code:-32601,message:'Unsupported demo method or tool'}})+'\n'); return; }
 process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:r.id,result})+'\n');
});
'''


def write_json(file, data):
    file.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    file.chmod(0o600)


def runtime(node=None):
    node = str(Path(node or shutil.which("node") or "").resolve())
    if not Path(node).is_file():
        raise ValueError("Install Node.js 20+ or supply --node with its absolute executable path")
    result = subprocess.run([node, "--version"], capture_output=True, text=True,
                            check=True, timeout=10)
    version = result.stdout.strip()
    if not re.fullmatch(r"v\d+\.\d+\.\d+", version) or int(version[1:].split(".")[0]) < 20:
        raise ValueError("Node.js 20+ is required")
    target = TARGETS.get((platform.system(), platform.machine()))
    if target is None:
        raise ValueError("Unsupported host: use the source integration instead")
    if platform.system() == "Linux" and platform.libc_ver()[0] != "glibc":
        raise ValueError("The published Linux executable requires glibc")
    return node, target


def export(directory, name, node, launcher, config=None):
    args = [str(launcher)] + ([str(config)] if config else [])
    entry = {"command": node, "args": args}
    write_json(directory / "cursor-mcp.json", {"mcpServers": {name: entry}})
    write_json(directory / "portable-mcp.json", {"mcpServers": {name: entry}})
    write_json(directory / "vscode-mcp.json", {"servers": {name: {"type": "stdio", **entry}}})
    encoded = base64.b64encode(json.dumps(entry).encode("utf-8")).decode("ascii")
    uri = "cursor://anysphere.cursor-deeplink/mcp/install?" + urlencode({"name": name, "config": encoded})
    (directory / "cursor-install-url.txt").write_text(uri + "\n", encoding="utf-8")
    (directory / "install.html").write_text(
        '<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">'
        '<title>DeadBolt editor installation</title><body style="font:18px/1.6 system-ui;max-width:760px;margin:3rem auto;padding:1rem">'
        '<h1>DeadBolt editor installation</h1><p>' +
        ('Disposable demo tools only. Review the local command before adding it. The demo writes local marker files; it has no accounts connected.' if config else
         'Read-only setup tools only. This connection provides no active protection and starts no downstream server.') +
        '</p><p><a href="' + html.escape(uri, quote=True) + '">Add ' + html.escape(name) +
        ' to Cursor</a></p><p>For VS Code, use MCP: Open User Configuration and merge the entry from vscode-mcp.json. '
        'For a new portable workspace configuration use portable-mcp.json as .mcp.json. Preserve existing servers.</p>'
        '<h2>Review the command</h2><pre style="white-space:pre-wrap;overflow-wrap:anywhere">' +
        html.escape(json.dumps(entry, indent=2)) + '</pre><p>Operator approval and kill remain outside the agent. '
        'Built-in editor terminal/file tools and other MCP connections are outside this route.</p></body></html>', encoding="utf-8")
    return entry


def prepare(bundle, out, node=None):
    node, target = runtime(node)
    if hashlib.sha256(bundle.read_bytes()).hexdigest() != BUNDLE_SHA256:
        raise ValueError("Bundle checksum mismatch; no installation written")
    # Preflight before creating anything. The checksum pins the entire published
    # bundle; additionally reject traversal, links and unexpected large archives.
    with zipfile.ZipFile(bundle) as archive:
        infos = archive.infolist()
        if len(infos) > 3000 or sum(i.file_size for i in infos) > 300_000_000:
            raise ValueError("Bundle exceeds extraction bounds")
        for info in infos:
            p = PurePosixPath(info.filename)
            if p.is_absolute() or ".." in p.parts or "\\" in info.filename or ":" in info.filename or stat.S_ISLNK(info.external_attr >> 16):
                raise ValueError("Unsafe bundle path")
        out.parent.mkdir(parents=True, exist_ok=True)
        out.mkdir(mode=0o700)  # Never reuse/overwrite an existing installation.
        root = out / "bundle"
        archive.extractall(root)
    binary = root / "native" / target / ("deadbolt.exe" if os.name == "nt" else "deadbolt")
    if os.name != "nt":
        binary.chmod(0o755)
    export(out, "deadbolt-rc2-setup", node, root / "server" / "launcher.cjs")
    write_json(out / "installation.json", {"bundle_version": "2.0.0-rc.2", "engine_version": "2.0.0-rc.2",
               "bundle_sha256": BUNDLE_SHA256, "node": node, "target": target})
    return {"prepared": str(out), "mode": "read-only setup; no active protection", "profile_modified": False}


def installation(out):
    data = json.loads((out / "installation.json").read_text(encoding="utf-8"))
    node, target = runtime(data["node"])
    if target != data["target"] or data["bundle_sha256"] != BUNDLE_SHA256:
        raise ValueError("Installation does not match this host or supported bundle")
    root = out / "bundle"
    binary = root / "native" / target / ("deadbolt.exe" if os.name == "nt" else "deadbolt")
    provenance = json.loads((root / "PROVENANCE.json").read_text(encoding="utf-8"))
    if hashlib.sha256(binary.read_bytes()).hexdigest() != provenance["binaries"][target]["binary_sha256"]:
        raise ValueError("Installed executable checksum mismatch")
    return node, root / "server" / "launcher.cjs", binary


def demo_config(directory, node):
    directory.mkdir(mode=0o700)
    fixture = directory / "disposable-server.cjs"
    fixture.write_text(FIXTURE, encoding="utf-8")
    config = {"version": 2, "hard_deadline_at": int(time.time()) + 600,
              "renewal_mode": "activity", "offline_tools": ["demo_write_marker", "demo_publish_marker"], "agent": "editor-demo-" + uuid.uuid4().hex,
              "database": str(directory / "state.db"), "events": str(directory / "events.jsonl"),
              "command": node, "args": [str(fixture), str(directory / "effects.txt")],
              "tools": ["demo_write_marker", "demo_publish_marker"], "destinations": [],
              "irreversible_tools": ["demo_publish_marker"], "spend_cap_usd": 10}
    write_json(directory / "operator.json", config)
    return config


def operator(binary, config, action):
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith("DEADBOLT_")}
    env.update(DEADBOLT_DB=config["database"], DEADBOLT_EVENTS=config["events"])
    args = [str(binary), action, "--agent", config["agent"]]
    if action == "approve":
        args += ["--tool", "demo_publish_marker"]
    return subprocess.run(args, env=env, check=True, capture_output=True, text=True, timeout=10).stdout


class Session:
    def __init__(self, command):
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, text=True, encoding="utf-8", bufsize=1)
        self.responses = queue.Queue()
        self.sequence = 0
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        for line in self.process.stdout:
            self.responses.put(line)

    def rpc(self, method, params=None):
        self.sequence += 1
        request = {"jsonrpc": "2.0", "id": self.sequence, "method": method, "params": params or {}}
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        response = json.loads(self.responses.get(timeout=15))
        if response.get("id") != self.sequence:
            raise ValueError("Unexpected MCP response ID")
        return response

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=10)
        if self.process.returncode != 0:
            raise ValueError("Demo server exited unsuccessfully: " + self.process.stderr.read()[:500])


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(out):
    node, launcher, binary = installation(out)
    checks = []
    setup = Session([node, str(launcher)])
    try:
        require(setup.rpc("initialize")["result"]["serverInfo"]["name"] == "deadbolt-setup", "Unexpected setup server")
        names = [t["name"] for t in setup.rpc("tools/list")["result"]["tools"]]
        require(names == ["deadbolt_get_integration_guide", "deadbolt_check_bundle"], "Unexpected setup tools")
        checked = setup.rpc("tools/call", {"name": "deadbolt_check_bundle", "arguments": {}})
        require("result" in checked and not checked["result"].get("isError"), "Bundle checksum tool failed")
        checks.append({"case": "read_only_setup_and_bundle_checksum", "passed": True})
    finally:
        setup.close()
    with tempfile.TemporaryDirectory(prefix="deadbolt-editor-verify-") as tmp:
        directory = Path(tmp).resolve() / "demo"
        config = demo_config(directory, node)
        effects = directory / "effects.txt"
        session = Session([node, str(launcher), str(directory / "operator.json")])
        try:
            require(session.rpc("initialize")["result"]["serverInfo"]["name"] == "deadbolt-disposable-demo", "Unexpected demo server")
            require(len(session.rpc("tools/list")["result"]["tools"]) == 3, "Missing demo tools")
            def call(name, denied=None):
                before = effects.read_text() if effects.exists() else ""
                response = session.rpc("tools/call", {"name": name, "arguments": {}})
                after = effects.read_text() if effects.exists() else ""
                if denied:
                    require("error" not in response, "Policy denial must be a model-facing tool result")
                    result = response.get("result", {})
                    require(result.get("isError") is True and result.get("_meta", {}).get("io.github.seanebones-lang/deadbolt", {}).get("code") == denied, "Expected tool denial: " + denied)
                    require(after == before, "A denied tool body ran")
                else:
                    require("result" in response, "Expected admitted tool result")
                    require(after == before + name + "\n", "Allowed body did not run exactly once")
                checks.append({"case": denied or "allowed_marker", "passed": True, "body_ran": before != after})
            call("demo_write_marker")
            call("demo_off_list", "purpose_exceeded")
            call("demo_publish_marker", "needs_human")
            operator(binary, config, "approve")
            call("demo_publish_marker")
            call("demo_publish_marker", "needs_human")
            operator(binary, config, "pause")
            call("demo_write_marker", "paused")
            operator(binary, config, "resume")
            call("demo_write_marker")
            operator(binary, config, "kill")
            call("demo_write_marker", "killed")
        finally:
            session.close()
    report = {"passed": True, "host": platform.platform(), "checks": checks,
              "scope": "Real published bundle and stdio protocol; disposable marker effects. No GUI or model invocation."}
    write_json(out / "verification.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prep = commands.add_parser("prepare", help="Verify/extract the pinned bundle; export configuration without editing profiles")
    prep.add_argument("--bundle", type=Path, required=True)
    prep.add_argument("--out", type=Path, required=True, help="New private installation directory outside agent-editable workspaces")
    prep.add_argument("--node", help="Absolute Node.js 20+ executable; otherwise resolve PATH")
    for name in ("demo", "verify", "operator"):
        command = commands.add_parser(name)
        command.add_argument("--out", type=Path, required=True)
        if name == "operator":
            command.add_argument("--demo", type=Path, required=True, help="Disposable demo directory created by this script")
            command.add_argument("action", choices=["status", "approve", "pause", "resume", "kill"])
    args = parser.parse_args()
    out = args.out.expanduser().resolve()
    if args.command == "prepare":
        result = prepare(args.bundle.expanduser().resolve(), out, args.node)
    elif args.command == "verify":
        result = verify(out)
    elif args.command == "demo":
        node, launcher, _ = installation(out)
        directory = out / ("demo-" + uuid.uuid4().hex[:12])
        demo_config(directory, node)
        export(directory, "deadbolt-rc2-demo", node, launcher, directory / "operator.json")
        result = {"demo": str(directory), "mode": "Disposable marker effects; no connected accounts", "lease": "60 seconds idle, 10 minutes absolute. Explicit activity mode for this short dummy demo; fresh ordinary roots default to supervisor renewal. Use demo again for a fresh run."}
    else:
        _, _, binary = installation(out)
        directory = args.demo.expanduser().resolve()
        if directory.parent != out or not directory.name.startswith("demo-"):
            raise ValueError("Operator helper only accepts a disposable demo from this installation")
        config = json.loads((directory / "operator.json").read_text(encoding="utf-8"))
        result = {"action": args.action, "output": operator(binary, config, args.action)}
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError, zipfile.BadZipFile, queue.Empty) as error:
        raise SystemExit("DeadBolt editor setup: " + str(error))
