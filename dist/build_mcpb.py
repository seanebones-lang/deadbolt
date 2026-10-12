"""Build a MCPB from the four checksum-verified published v2.0.0-rc.1 archives."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc')
# Pinned independently of a mutable downloaded SHA256SUMS file.
DIGESTS = {
    'aarch64-apple-darwin': '2a55ad536473ef9900510a516fb9407fd96e43429d8ba2b51cc378da4c6cd88b',
    'x86_64-apple-darwin': '44d96961ddea1baf90a1888aa2693b3a201091e08a863ee35ca07ced9864409d',
    'x86_64-unknown-linux-gnu': '0559183a38ca8e08c95e20133462847cf42d2dc1f87f8c09f7248cee098cc229',
    'x86_64-pc-windows-msvc': '3ccaf85bdea9f901c6a98385057d6b8a5d4745b30f034487ef27d4c7573d2af4',
}

def build(downloads, out):
    out.mkdir(parents=True, exist_ok=True)
    staging = out / 'bundle'
    if staging.exists():
        raise RuntimeError('Use a fresh output directory; existing bundle will not be overwritten')
    shutil.copytree(ROOT / 'dist/mcpb', staging)
    provenance = {'engine_version': '2.0.0-rc.1', 'engine_release': 'https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0-rc.1', 'binaries': {}}
    for target in TARGETS:
        name = f'deadbolt-2.0.0-rc.1-{target}.zip'
        archive = downloads / name
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if digest != DIGESTS[target]:
            raise RuntimeError(f'Archive checksum mismatch: {target}')
        with zipfile.ZipFile(archive) as z:
            prefix = name[:-4] + '/'
            meta = json.loads(z.read(prefix + 'BUILD.json'))
            if meta['version'] != '2.0.0-rc.1' or meta['target'] != target or meta['source_revision'] != 'd14f81f974c556446e520d896acf7761f73861e6':
                raise RuntimeError('Release metadata mismatch')
            executable = 'deadbolt.exe' if 'windows' in target else 'deadbolt'
            content = z.read(prefix + executable)
            if hashlib.sha256(content).hexdigest() != meta['binary_sha256']:
                raise RuntimeError('Binary checksum mismatch')
            native = staging / 'native' / target
            native.mkdir(parents=True)
            (native / executable).write_bytes(content)
            (native / executable).chmod(0o755)
            (native / 'BUILD.json').write_text(json.dumps(meta, indent=2) + '\n')
            provenance['binaries'][target] = {**meta, 'archive_sha256': digest, 'archive_url': f'https://github.com/seanebones-lang/deadbolt/releases/download/v2.0.0-rc.1/{name}'}
            if target == TARGETS[0]:
                for file in ('LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES.txt', 'THIRD-PARTY.md'):
                    (staging / file).write_bytes(z.read(prefix + file))
                for file in ('docs/TRUST.md', 'docs/INSTALL.md', 'docs/INTEGRATION.md', 'docs/DESTINATION-POLICY.md', 'docs/LEASE-DEADLINES.md', 'docs/SUPERVISION.md', 'docs/MIGRATING-V2.md'):
                    dst = staging / 'engine-docs' / Path(file).name
                    dst.parent.mkdir(exist_ok=True)
                    dst.write_bytes(z.read(prefix + file))
    (staging / 'PROVENANCE.json').write_text(json.dumps(provenance, indent=2) + '\n')
    return staging

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--downloads', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    print(build(args.downloads.resolve(), args.out.resolve()))
