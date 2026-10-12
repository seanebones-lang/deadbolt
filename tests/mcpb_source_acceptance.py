"""Run bundle marker acceptance with this host's freshly built source engine.

The published-engine suite remains a compatibility test. This disposable overlay
closes the separate source-engine/launcher integration gap; it is not a release
bundle, a graphical-client test, or evidence for other hosts' engine binaries.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--mcpb-module", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    target = {("Darwin", "arm64"): "aarch64-apple-darwin",
              ("Darwin", "x86_64"): "x86_64-apple-darwin",
              ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
              ("Windows", "AMD64"): "x86_64-pc-windows-msvc"}[(platform.system(), platform.machine())]
    binary = args.binary.resolve(strict=True)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    assert subprocess.check_output([str(binary), "--version"], text=True).strip() == "deadbolt " + version
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    source_diff = subprocess.check_output(["git", "diff", "HEAD", "--"], cwd=ROOT)
    source_dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT).strip())
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    with tempfile.TemporaryDirectory(prefix="deadbolt-source-bundle-") as directory:
        bundle = Path(directory) / "bundle"
        shutil.copytree(args.bundle.resolve(strict=True), bundle)
        name = "deadbolt.exe" if target.endswith("msvc") else "deadbolt"
        destination = bundle / "native" / target / name
        shutil.copy2(binary, destination)
        metadata = {"version": version, "source_revision": revision,
                    "target": target, "binary_sha256": digest,
                    "distribution": "test-only source overlay, not a published archive"}
        (destination.parent / "BUILD.json").write_text(json.dumps(metadata, indent=2) + "\n")
        provenance = json.loads((bundle / "PROVENANCE.json").read_text())
        provenance["engine_version"] = version
        provenance["engine_release"] = "test-only source overlay"
        provenance["binaries"][target] = metadata
        (bundle / "PROVENANCE.json").write_text(json.dumps(provenance, indent=2) + "\n")
        output = Path(directory) / "acceptance.json"
        subprocess.run([sys.executable, str(ROOT / "tests/mcpb_acceptance.py"),
                        "--bundle", str(bundle), "--out", str(output),
                        "--expect-damaged-store-refusal",
                        "--mcpb-module", str(args.mcpb_module.resolve(strict=True))],
                       check=True, timeout=600)
        report = json.loads(output.read_text())
        report.update(source_commit=revision, source_worktree_dirty=source_dirty,
                      source_diff_sha256=hashlib.sha256(source_diff).hexdigest(),
                      binary_sha256=digest, engine_version=version,
                      artifact_kind="test-only source overlay", graphical_client_tested=False)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
