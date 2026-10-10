"""Build sdist -> wheel, install outside checkout, verify dependency-free import.

Requires Python 3.12+, build==1.3.0 and packaging. No registry publication.
"""
import email
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import venv
import zipfile

from packaging.version import Version

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "release-artifacts" / "python"


def run(command, **kwargs):
    subprocess.run(command, check=True, timeout=180, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install-sdk", action="store_true",
                        help="Install the verified wheel plus its SDK extra into this Python environment")
    args = parser.parse_args()
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    pyproject = tomllib.loads((ROOT / "pyproject.toml").read_text())
    version = pyproject["project"]["version"]
    if Version(cargo) != Version(version):
        raise RuntimeError("Cargo and Python source candidate versions differ")
    OUT.mkdir(parents=True, exist_ok=True)
    run([sys.executable, "-m", "build", "--sdist", "--outdir", str(OUT)], cwd=ROOT,
        stdout=subprocess.DEVNULL)
    sdist = OUT / ("n11_deadbolt_client-" + version + ".tar.gz")
    with tempfile.TemporaryDirectory(prefix="deadbolt-python-consumer-") as temp:
        temp = Path(temp)
        unpacked = temp / "source"
        with tarfile.open(sdist) as archive:
            archive.extractall(unpacked, filter="data")
        source = unpacked / ("n11_deadbolt_client-" + version)
        wheels = temp / "wheel"
        run([sys.executable, "-m", "build", "--wheel", "--outdir", str(wheels)], cwd=source,
            stdout=subprocess.DEVNULL)
        wheel = wheels / ("n11_deadbolt_client-" + version + "-py3-none-any.whl")
        with zipfile.ZipFile(wheel) as archive:
            metadata_name = next(n for n in archive.namelist() if n.endswith(".dist-info/METADATA"))
            metadata = email.message_from_bytes(archive.read(metadata_name))
            requirements = metadata.get_all("Requires-Dist", [])
            if any("extra ==" not in r for r in requirements):
                raise RuntimeError("base client acquired a runtime dependency")
            if not any(n.endswith("/licenses/LICENSE") for n in archive.namelist()):
                raise RuntimeError("wheel lacks its license")
            if not any(n.endswith("/licenses/NOTICE") for n in archive.namelist()):
                raise RuntimeError("wheel lacks its notice")
        environment = temp / "environment"
        venv.EnvBuilder(with_pip=True).create(environment)
        python = environment / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        command = environment / ("Scripts/deadbolt-client.exe" if os.name == "nt" else "bin/deadbolt-client")
        run([str(python), "-m", "pip", "install", "--no-deps", "--no-index", str(wheel)],
            cwd=temp, stdout=subprocess.DEVNULL)
        env = {k: v for k, v in os.environ.items() if k not in ("PYTHONPATH", "PYTHONHOME")}
        env["DEADBOLT_SOCK"] = str(temp / "nonexistent.sock")
        smoke = '''
import importlib.util, pathlib, deadbolt_client, deadbolt_openai_agents
assert importlib.util.find_spec("agents") is None
assert pathlib.Path(deadbolt_client.__file__).is_relative_to(pathlib.Path(__import__("sys").prefix))
result = deadbolt_client.dispatch("synthetic", "write_file", lambda: (_ for _ in ()).throw(AssertionError("body ran")))
assert result == {"executed": False, "decision": {"decision": "deny", "code": "store_unavailable"}}, result
print("installed base wheel: import, fail-closed body, no SDK dependency passed")
'''
        run([str(python), "-I", "-c", smoke], cwd=temp, env=env)
        run([str(command), "--help"], cwd=temp, env=env, stdout=subprocess.DEVNULL)
        # Retain the artifact actually built from the unpacked sdist.
        (OUT / wheel.name).write_bytes(wheel.read_bytes())
        report = {"version": version, "cargo_version": cargo, "base_runtime_dependencies": [],
                  "wheel_sha256": hashlib.sha256(wheel.read_bytes()).hexdigest(),
                  "sdist_sha256": hashlib.sha256(sdist.read_bytes()).hexdigest(),
                  "checks": ["sdist-to-wheel outside checkout", "license and notice",
                             "isolated install without dependencies", "import without SDK",
                             "unavailable sidecar leaves body untouched", "installed CLI"]}
        (OUT / "package-acceptance.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
        if args.install_sdk:
            run([sys.executable, "-m", "pip", "install", "--force-reinstall", "--no-deps",
                 str(OUT / wheel.name)], cwd=temp)
            run([sys.executable, "-m", "pip", "install", "-r", str(ROOT / "tests/openai-agents-requirements.txt")], cwd=temp)


if __name__ == "__main__":
    main()
