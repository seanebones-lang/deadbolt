"""Verify Cargo's real file selection excludes ignored local build material."""
import json
from pathlib import Path, PurePosixPath
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    # A clean CI checkout would miss basename-pattern pollution. Populate an
    # ignored local build folder with harmless files before asking Cargo.
    build = ROOT / "build"
    build.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="crate-boundary-", dir=build) as directory:
        probe = Path(directory)
        for name in ("README.md", "LICENSE", "NOTICE", "SECURITY.md", "Cargo.toml", "MANIFEST.in"):
            (probe / name).write_text("synthetic local build material; must not ship\n")
        result = subprocess.run(["cargo", "package", "--list", "--locked"], cwd=ROOT,
                                capture_output=True, text=True, timeout=90)
        if result.returncode:
            raise RuntimeError("Cargo file-selection check failed:\n" + result.stderr[-2000:])
        files = set(result.stdout.splitlines())
        assert not any(name.startswith("build/") for name in files), "local build files entered the package"
        assert not any(name.startswith("docs/assets/") or name.endswith(".html") for name in files), "hosted media entered the crate"
        required = {"Cargo.lock", "LICENSE", "NOTICE", "README.md", "SECURITY.md",
                    "src/lib.rs", "src/main.rs", "examples/build_in.rs", "examples/deadbolt_supervisor.py",
                    "docs/MIGRATING-V2.md", "docs/PRIVACY.md", "docs/SUPERVISION.md",
                    "tests/published_versions.json"}
        assert required <= files, "missing developer package files: " + repr(sorted(required - files))
        print(json.dumps({"passed": True, "package_file_count": len(files),
                          "ignored_build_sentinels_excluded": True,
                          "required_developer_files_present": True}))


if __name__ == "__main__":
    main()
