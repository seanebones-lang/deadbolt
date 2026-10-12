#!/usr/bin/env python3
"""Run the recipe outside the source tree with the installed wheel."""
import argparse
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import deadbolt_client

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if Path(deadbolt_client.__file__).resolve().is_relative_to(ROOT / "examples"):
        raise RuntimeError("install the wheel; source imports cannot prove consumer installation")
    if importlib.metadata.version("fastmcp") != "4.1.0":
        raise RuntimeError("install the pinned framework contract")
    clean_env = {key: value for key, value in os.environ.items()
                 if not key.startswith("DEADBOLT_") and key != "PYTHONPATH"}
    with tempfile.TemporaryDirectory(prefix="deadbolt-fastmcp-consumer-") as directory:
        consumer = Path(directory)
        for filename in ("fastmcp_demo.py", "fastmcp_server.py"):
            shutil.copyfile(ROOT / "examples" / filename, consumer / filename)
        completed = subprocess.run([sys.executable, str(consumer / "fastmcp_demo.py"), "--binary", str(binary)],
                                   cwd=consumer, env=clean_env, check=True, capture_output=True, text=True, timeout=90)
        report = json.loads(completed.stdout)
        if report.get("passed") is not True or report.get("count") != 17:
            raise RuntimeError("not all stdio effect cases passed")
        for name, settings in (
            ("missing_sidecar_refuses_startup", {"DEADBOLT_ADMISSION_TOKEN": "synthetic"}),
            ("missing_admission_key_refuses_startup", {"DEADBOLT_SOCK": "127.0.0.1:1"}),
            ("operator_key_refuses_startup", {"DEADBOLT_SOCK": "127.0.0.1:1", "DEADBOLT_ADMISSION_TOKEN": "synthetic", "DEADBOLT_TOKEN": "synthetic"}),
        ):
            refused = subprocess.run([sys.executable, str(consumer / "fastmcp_server.py"), "--agent", "test", "--output", str(consumer / "note")],
                                     cwd=consumer, env={**clean_env, **settings}, capture_output=True, text=True, timeout=10)
            if refused.returncode != 2 or (consumer / "note").exists():
                raise RuntimeError(f"{name}: startup was not refused")
            report["cases"].append(name)
        report["count"] = len(report["cases"])
        report["installed_client_outside_examples"] = True
        report["copied_recipe_outside_checkout"] = True
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
