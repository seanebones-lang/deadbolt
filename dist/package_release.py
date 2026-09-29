"""Package and smoke-test a native release binary; uses Python 3.11+ stdlib."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import tomllib
import zipfile

from collect_licenses import collect

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "release-artifacts")
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9_-]+", args.target):
        parser.error("target must be an architecture/OS label")
    binary = args.binary.resolve(strict=True)
    host = subprocess.check_output(["rustc", "-vV"], text=True)
    if f"host: {args.target}\n" not in host:
        raise RuntimeError("native builder does not match the archive target")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    expected = f"deadbolt {version}"
    actual = subprocess.check_output([str(binary), "--version"], text=True).strip()
    if actual != expected:
        raise RuntimeError(f"binary/source version mismatch: {actual} != {expected}")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)
    if dirty.strip():
        raise RuntimeError("commit candidate source before packaging its source identity")
    args.out.mkdir(parents=True, exist_ok=True)
    name = f"deadbolt-{version}-{args.target}"
    archive = args.out / f"{name}.zip"
    with tempfile.TemporaryDirectory(prefix="deadbolt-package-") as temp:
        staging = Path(temp) / name
        staging.mkdir()
        executable = "deadbolt.exe" if os.name == "nt" else "deadbolt"
        shutil.copy2(binary, staging / executable)
        for file in ("README.md", "LICENSE", "NOTICE", "SECURITY.md", "CHANGELOG.md",
                     "CONTRIBUTING.md", "Cargo.toml", "Cargo.lock", "Dockerfile"):
            shutil.copy2(ROOT / file, staging / file)
        shutil.copy2(ROOT / "THIRD-PARTY.md", staging / "THIRD-PARTY.md")
        shutil.copytree(ROOT / "third-party", staging / "third-party")
        collect(staging / "THIRD-PARTY-NOTICES.txt")
        shutil.copytree(ROOT / "docs", staging / "docs")
        shutil.copytree(ROOT / "examples", staging / "examples",
                        ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
        shutil.copytree(ROOT / "src", staging / "src")
        shutil.copytree(ROOT / "tests", staging / "tests",
                        ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
        (staging / "dist").mkdir()
        for file in ("deadbolt.service", "docker-compose.yml", "deadbolt.env.example",
                     "package_release.py", "sign_macos_archive.py", "collect_licenses.py"):
            shutil.copy2(ROOT / "dist" / file, staging / "dist" / file)
        metadata = {"version": version, "source_revision": revision,
                    "target": args.target, "builder_platform": platform.platform(),
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "distribution": "unsigned native binary; verify checksums and source"}
        (staging / "BUILD.json").write_text(json.dumps(metadata, indent=2) + "\n")
        # Upstream license files can predate ZIP's 1980 timestamp minimum.
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, strict_timestamps=False) as package:
            for file in sorted(staging.rglob("*")):
                if file.is_file():
                    package.write(file, file.relative_to(staging.parent))
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        archive.with_suffix(".zip.sha256").write_text(f"{digest}  {archive.name}\n")
        extracted = Path(temp) / "extracted"
        with zipfile.ZipFile(archive) as package:
            package.extractall(extracted)
        installed = extracted / name / executable
        if os.name != "nt":
            installed.chmod(0o755)
        env = {**os.environ, "DEADBOLT_DB": str(Path(temp) / "state.db"),
               "DEADBOLT_EVENTS": str(Path(temp) / "events.jsonl")}
        subprocess.run([str(installed), "--version"], check=True, env=env)
        subprocess.run([str(installed), "drill"], check=True, env=env, timeout=30)
        subprocess.run([str(installed), "--help"], check=True, env=env,
                       stdout=subprocess.DEVNULL)
    print(f"Packaged and verified {archive.name}: {digest}")


if __name__ == "__main__":
    main()
