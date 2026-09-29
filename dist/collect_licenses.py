"""Collect locked Rust dependency notices for a native binary distribution."""

import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def collect(output):
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT, text=True))
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    selected, pending = set(), [metadata["resolve"]["root"]]
    while pending:
        key = pending.pop()
        if key in selected:
            continue
        selected.add(key)
        for dep in nodes[key]["deps"]:
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                pending.append(dep["pkg"])
    sections = [
        "Third-party Rust dependency notices\n"
        "Collected from the locked runtime/build dependency closure across targets.\n"
        "Some listed dependencies are build-time or target-specific, not linked on every platform.\n"
        "Original notices and license choices remain applicable to their components.\n"
        "option-ext 0.2.0 source and its MPL-2.0 license are included under third-party/.\n"
        "Rust runtime notices are included under third-party/rust-1.85.0/.\n"
    ]
    packages = sorted((p for p in metadata["packages"] if p["id"] in selected and p["source"]),
                      key=lambda p: (p["name"], p["version"]))
    for package in packages:
        root = Path(package["manifest_path"]).parent.resolve()
        files = {f.resolve() for f in root.iterdir() if f.is_file() and
                 f.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "COPYRIGHT"))}
        if package["license_file"]:
            files.add((root / package["license_file"]).resolve())
        if not files:
            raise RuntimeError(f"missing license files: {package['name']} {package['version']}")
        sections.append(f"\n{'=' * 72}\n{package['name']} {package['version']}\n"
                        f"Declared license: {package['license']}\n"
                        f"Source archive: https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download\n")
        for file in sorted(files):
            if not file.is_relative_to(root) or not file.is_file():
                raise RuntimeError("license path outside packaged dependency")
            sections.append(f"\n--- {file.name} ---\n{file.read_text(encoding='utf-8')}\n")
    Path(output).write_text("".join(sections), encoding="utf-8")
    return len(packages)


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    print(f"Collected {collect(args.output)} locked dependency notices")
