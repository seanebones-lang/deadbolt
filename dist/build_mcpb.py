"""Build a MCPB from the four checksum-verified published v2.0.0-rc.2 archives."""
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
    'aarch64-apple-darwin': '26c33da31ba36b0753bfdc89e43913d1bd62c1ee102a59009b29a7b09967d689',
    'x86_64-apple-darwin': 'abdf66e8a12ad120f88686a24fd76828e430dc9a39728ba078a89b8afe0abf3e',
    'x86_64-unknown-linux-gnu': 'e8f1c581a5e75d81f1240574eb5b29a203aa7fbd4461adf2de77e60e6d1dbd9e',
    'x86_64-pc-windows-msvc': '1981299078bacdc9e82a2da1bc9b693da085ffdec316de556857b321e01140ef',
}

def build(downloads, out):
    out.mkdir(parents=True, exist_ok=True)
    staging = out / 'bundle'
    if staging.exists():
        raise RuntimeError('Use a fresh output directory; existing bundle will not be overwritten')
    shutil.copytree(ROOT / 'dist/mcpb', staging)
    provenance = {'engine_version': '2.0.0-rc.2', 'engine_release': 'https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0-rc.2', 'binaries': {}}
    for target in TARGETS:
        name = f'deadbolt-2.0.0-rc.2-{target}.zip'
        archive = downloads / name
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if digest != DIGESTS[target]:
            raise RuntimeError(f'Archive checksum mismatch: {target}')
        with zipfile.ZipFile(archive) as z:
            prefix = name[:-4] + '/'
            meta = json.loads(z.read(prefix + 'BUILD.json'))
            if meta['version'] != '2.0.0-rc.2' or meta['target'] != target or meta['source_revision'] != 'd166c7b039800bbd6d8acb1bc5c66f9add472f5f':
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
            provenance['binaries'][target] = {**meta, 'archive_sha256': digest, 'archive_url': f'https://github.com/seanebones-lang/deadbolt/releases/download/v2.0.0-rc.2/{name}'}
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
