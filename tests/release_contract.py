"""Archive signing preflight tests; no certificate, signing or notarization calls."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("sign_macos_archive", ROOT / "dist/sign_macos_archive.py")
signer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(signer)


class SigningPreflight(unittest.TestCase):
    def run_archive(self, version, *, metadata_version=None, corrupt_hash=False, extra=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, destination = root / "input.zip", root / "output.zip"
            name = f"deadbolt-{version}-aarch64-apple-darwin"
            binary = b"synthetic test fixture, not executable"
            build = {"version": metadata_version or version, "target": "aarch64-apple-darwin",
                     "binary_sha256": hashlib.sha256(b"wrong" if corrupt_hash else binary).hexdigest()}
            with zipfile.ZipFile(source, "w") as archive:
                archive.writestr(name + "/deadbolt", binary)
                archive.writestr(name + "/BUILD.json", json.dumps(build))
                if extra:
                    archive.writestr(extra, "synthetic fixture")
            # Synthetic fingerprint only satisfies CLI argument shape. All tool
            # execution is blocked; reaching lipo proves complete ZIP preflight.
            fingerprint = hashlib.sha1(b"synthetic certificate fingerprint").hexdigest()
            argv = ["sign_macos_archive.py", "--input", str(source), "--output", str(destination),
                    "--identity", fingerprint]
            with patch.object(sys, "argv", argv), patch.object(signer.subprocess, "check_output",
                    side_effect=RuntimeError("reached architecture verification")) as architecture, \
                    patch.object(signer.subprocess, "run") as signing:
                try:
                    signer.main()
                finally:
                    signing.assert_not_called()
                    self.assertFalse(destination.exists())
                    if architecture.called:
                        self.assertEqual(architecture.call_args.args[0][0], "lipo")

    def test_stable_and_candidate_archives_reach_architecture_check(self):
        for version in ("1.0.3", "1.1.0-rc.1", "1.1.0-rc.1+build.7"):
            with self.subTest(version=version), self.assertRaisesRegex(RuntimeError, "reached architecture"):
                self.run_archive(version)

    def test_version_mismatch_rejected_before_tools(self):
        with self.assertRaisesRegex(ValueError, "BUILD.json target differs"):
            self.run_archive("1.1.0-rc.1", metadata_version="1.1.0")

    def test_binary_hash_mismatch_rejected_before_tools(self):
        with self.assertRaisesRegex(ValueError, "unsigned binary hash differs"):
            self.run_archive("1.1.0-rc.1", corrupt_hash=True)

    def test_unsafe_and_multiple_roots_rejected_before_tools(self):
        for extra in ("../outside", "other-root/NOTICE"):
            with self.subTest(extra=extra), self.assertRaises(ValueError):
                self.run_archive("1.1.0-rc.1", extra=extra)


if __name__ == "__main__":
    unittest.main()
