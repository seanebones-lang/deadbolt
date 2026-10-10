"""Developer ID sign a macOS candidate ZIP without changing its source identity.

Submit the resulting ZIP itself to Apple's notary service, then publish these
exact bytes and their checksum only after Apple reports Accepted.
"""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tempfile
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--identity", required=True, help="Developer ID Application certificate SHA-1")
    args = parser.parse_args()
    source = args.input.resolve(strict=True)
    destination = args.output.resolve()
    if source == destination:
        parser.error("output must differ from input")
    if destination.suffix != ".zip":
        parser.error("output must be a ZIP archive")
    if destination.exists() or destination.with_suffix(".zip.sha256").exists():
        parser.error("output ZIP or checksum already exists")
    if not re.fullmatch(r"[0-9A-Fa-f]{40}", args.identity):
        parser.error("identity must be a certificate SHA-1 fingerprint")

    with zipfile.ZipFile(source) as archive:
        members = archive.infolist()
        names = [member.filename for member in members]
        if len(names) != len(set(names)):
            raise ValueError("duplicate archive members")
        for name in names:
            parts = PurePosixPath(name).parts
            if not parts or name.startswith("/") or ".." in parts or "\\" in name:
                raise ValueError(f"unsafe archive member: {name}")
        roots = {PurePosixPath(name).parts[0] for name in names}
        if len(roots) != 1:
            raise ValueError("expected one archive root")
        root = roots.pop()
        # Cargo prereleases are real candidate identities. Keep the complete
        # version (including optional build metadata) bound to BUILD.json.
        version = r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
        match = re.fullmatch(rf"deadbolt-({version})-((?:aarch64|x86_64)-apple-darwin)", root)
        if not match:
            raise ValueError("expected a versioned macOS release archive")
        binary_name = f"{root}/deadbolt"
        build_name = f"{root}/BUILD.json"
        if names.count(binary_name) != 1 or names.count(build_name) != 1:
            raise ValueError("archive must contain one binary and BUILD.json")
        build = json.loads(archive.read(build_name))
        if build.get("target") != match.group(2) or build.get("version") != match.group(1):
            raise ValueError("BUILD.json target differs from archive name")
        original_binary = archive.read(binary_name)
        if hashlib.sha256(original_binary).hexdigest() != build.get("binary_sha256"):
            raise ValueError("unsigned binary hash differs from BUILD.json")

        with tempfile.TemporaryDirectory(prefix="deadbolt-sign-") as temp:
            binary = Path(temp) / "deadbolt"
            binary.write_bytes(original_binary)
            binary.chmod(0o755)
            expected_arch = "arm64" if match.group(2).startswith("aarch64") else "x86_64"
            actual_arch = subprocess.check_output(["lipo", "-archs", str(binary)], text=True).strip()
            if actual_arch != expected_arch:
                raise ValueError(f"binary architecture {actual_arch} differs from {expected_arch}")
            subprocess.run(["codesign", "--force", "--options", "runtime", "--timestamp",
                            "--sign", args.identity, str(binary)], check=True)
            subprocess.run(["codesign", "--verify", "--strict", str(binary)], check=True)
            details = subprocess.run(["codesign", "-dv", "--verbose=4", str(binary)],
                                     capture_output=True, text=True, check=True).stderr
            if "Authority=Developer ID Application:" not in details or "flags=0x10000(runtime)" not in details:
                raise RuntimeError("Developer ID signature or hardened runtime missing")
            team = re.search(r"^TeamIdentifier=([A-Z0-9]+)$", details, re.MULTILINE)
            cdhash = re.search(r"^CDHash=([0-9a-f]+)$", details, re.MULTILINE)
            timestamp = re.search(r"^Timestamp=(.+)$", details, re.MULTILINE)
            if not (team and cdhash and timestamp):
                raise RuntimeError("signature team, CDHash or secure timestamp missing")
            signed_binary = binary.read_bytes()
            build["binary_sha256"] = hashlib.sha256(signed_binary).hexdigest()
            build["distribution"] = "Developer ID signed; see release notes for notarization status"
            build["signing_team_id"] = team.group(1)
            build["signing_cdhash"] = cdhash.group(1)

            destination.parent.mkdir(parents=True, exist_ok=True)
            staged = Path(temp) / "signed.zip"
            with zipfile.ZipFile(staged, "w") as output:
                for member in members:
                    if member.is_dir():
                        continue
                    if member.filename == binary_name:
                        data = signed_binary
                    elif member.filename == build_name:
                        data = (json.dumps(build, indent=2) + "\n").encode()
                    else:
                        data = archive.read(member.filename)
                    output.writestr(member, data)
            with zipfile.ZipFile(staged) as check:
                if check.testzip() is not None:
                    raise RuntimeError("signed archive CRC verification failed")
                if hashlib.sha256(check.read(binary_name)).hexdigest() != build["binary_sha256"]:
                    raise RuntimeError("signed archive binary hash mismatch")
            shutil.move(staged, destination)

    digest = hashlib.sha256(destination.read_bytes()).hexdigest()
    destination.with_suffix(".zip.sha256").write_text(f"{digest}  {destination.name}\n")
    print(json.dumps({"archive": str(destination), "sha256": digest,
                      "binary_sha256": build["binary_sha256"],
                      "team_id": build["signing_team_id"], "cdhash": build["signing_cdhash"]}))


if __name__ == "__main__":
    main()
