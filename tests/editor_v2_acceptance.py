"""Install the public editor helper outside the checkout and exercise its exports."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from urllib.parse import parse_qs, urlparse


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    helper = root / "docs/assets/editor_setup_v2.py"
    checks = []
    with tempfile.TemporaryDirectory(prefix="deadbolt-editor-consumer-") as tmp:
        work = Path(tmp).resolve()
        copied = work / "editor_setup.py"
        shutil.copyfile(helper, copied)
        install = work / "private path with spaces"
        def run(*argv, ok=True, optimized=False):
            command = [sys.executable] + (["-O"] if optimized else []) + [str(copied), *map(str, argv)]
            result = subprocess.run(command, cwd=work, capture_output=True, text=True, timeout=90)
            assert (result.returncode == 0) == ok, result.stderr
            return result
        # Failed downloads cannot create a partial installation or mask failure.
        bad = work / "bad.mcpb"
        bad.write_bytes(b"not the released bundle")
        run("prepare", "--bundle", bad, "--out", install, ok=False)
        assert not install.exists()
        checks.append({"case": "checksum_mismatch_no_installation", "passed": True})
        run("prepare", "--bundle", args.bundle.resolve(), "--out", install)
        sentinel = install / "retained.txt"
        sentinel.write_text("keep existing installation")
        run("prepare", "--bundle", args.bundle.resolve(), "--out", install, ok=False)
        assert sentinel.read_text() == "keep existing installation"
        checks.append({"case": "existing_installation_preserved", "passed": True})
        cursor = json.loads((install / "cursor-mcp.json").read_text())["mcpServers"]["deadbolt-rc2-setup"]
        portable = json.loads((install / "portable-mcp.json").read_text())["mcpServers"]["deadbolt-rc2-setup"]
        vscode = json.loads((install / "vscode-mcp.json").read_text())["servers"]["deadbolt-rc2-setup"]
        assert cursor == portable and vscode == {"type": "stdio", **cursor}
        assert len(cursor["args"]) == 1 and Path(cursor["args"][0]).is_absolute()
        uri = urlparse((install / "cursor-install-url.txt").read_text().strip())
        query = parse_qs(uri.query)
        assert uri.scheme == "cursor" and uri.netloc == "anysphere.cursor-deeplink" and uri.path == "/mcp/install"
        assert query["name"] == ["deadbolt-rc2-setup"]
        assert json.loads(base64.b64decode(query["config"][0])) == cursor
        # Exercise both exported client commands, not just the helper's internal
        # test command: this catches paths/escaping and shape mismatches.
        request = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}) + "\n"
        for label, entry in (("cursor", cursor), ("vscode", vscode)):
            result = subprocess.run([entry["command"], *entry["args"]], input=request,
                                    capture_output=True, text=True, timeout=20)
            assert result.returncode == 0, result.stderr
            assert json.loads(result.stdout)["result"]["serverInfo"]["name"] == "deadbolt-setup"
            checks.append({"case": label + "_exported_command_real_stdio", "passed": True})
        report = json.loads(run("verify", "--out", install, optimized=True).stdout)
        assert report["passed"] and len(report["checks"]) == 9
        checks.append({"case": "optimized_interpreter_effect_verification", "passed": True, "checks": report["checks"]})
        demo = Path(json.loads(run("demo", "--out", install).stdout)["demo"])
        config = json.loads((demo / "operator.json").read_text())
        assert config["version"] == 2 and config["renewal_mode"] == "activity"
        assert config["offline_tools"] == ["demo_write_marker", "demo_publish_marker"]
        assert type(config["hard_deadline_at"]) is int
        assert config["irreversible_tools"] == ["demo_publish_marker"]
        assert config["tools"] == ["demo_write_marker", "demo_publish_marker"]
        assert not Path(config["database"]).exists(), "Preparation must not start a lease/server"
        assert not (demo / "effects.txt").exists()
        second = Path(json.loads(run("demo", "--out", install).stdout)["demo"])
        assert json.loads((second / "operator.json").read_text())["agent"] != config["agent"]
        run("operator", "--out", install, "--demo", work, "kill", ok=False)
        checks.append({"case": "fresh_demo_identity_no_effects_until_connection_and_operator_scope", "passed": True})
        data = json.loads((install / "installation.json").read_text())
        binary = install / "bundle/native" / data["target"] / ("deadbolt.exe" if sys.platform == "win32" else "deadbolt")
        binary.write_bytes(b"corrupt")
        run("verify", "--out", install, ok=False)
        checks.append({"case": "installed_binary_corruption_refused", "passed": True})
    output = {"passed": True, "helper_sha256": hashlib.sha256(helper.read_bytes()).hexdigest(),
              "scope": "Fresh external directory; checksum-pinned published RC2 MCPB; exported editor commands and disposable effects. No GUI/model/account test.", "checks": checks}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
