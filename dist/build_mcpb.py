"""Build a MCPB from the four checksum-verified published v2.0.1 archives."""
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
    'aarch64-apple-darwin': 'e4f204744f65a45a944761bb9597d7d17a637d714c1c56eec8ed69d5347839df',
    'x86_64-apple-darwin': '0075e54fe2dbc5f3081be751b793f6ad30e2238bbba59f42cfdb358e4b4bec06',
    'x86_64-unknown-linux-gnu': 'c2340e58a74ac79d97644912d9bf03bf7f0cadd51b9cc0c802ec0a1fadf02856',
    'x86_64-pc-windows-msvc': 'cdfb9dbf35c4544317640c07bd0887de1422d8129c53330eda0f4d45d69c1874',
}

def build(downloads, out):
    out.mkdir(parents=True, exist_ok=True)
    staging = out / 'bundle'
    if staging.exists():
        raise RuntimeError('Use a fresh output directory; existing bundle will not be overwritten')
    shutil.copytree(ROOT / 'dist/mcpb', staging)
    provenance = {'engine_version': '2.0.1', 'engine_release': 'https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.1', 'binaries': {}}
    for target in TARGETS:
        name = f'deadbolt-2.0.1-{target}.zip'
        archive = downloads / name
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if digest != DIGESTS[target]:
            raise RuntimeError(f'Archive checksum mismatch: {target}')
        with zipfile.ZipFile(archive) as z:
            prefix = name[:-4] + '/'
            meta = json.loads(z.read(prefix + 'BUILD.json'))
            if meta['version'] != '2.0.1' or meta['target'] != target or meta['source_revision'] != 'e87519f700be03c27a564ed274907649224d256f':
                raise RuntimeError('Release metadata mismatch')
            executable = 'deadbolt.exe' if 'windows' in target else 'deadbolt'
            content = z.read(prefix + executable)
            if hashlib.sha256(content).hexdigest() != meta['binary_sha256']:
                raise RuntimeError('Binary checksum mismatch')
            native = staging / 'native' / target
            native.mkdir(parents=True)
            (native / executable).write_bytes(content)
            (native / executable).chmod(0o755)
            (native / 'BUILD.json').write_bytes((json.dumps(meta, indent=2) + '\n').encode('utf-8'))
            provenance['binaries'][target] = {**meta, 'archive_sha256': digest, 'archive_url': f'https://github.com/seanebones-lang/deadbolt/releases/download/v2.0.1/{name}'}
            if target == TARGETS[0]:
                for file in ('LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES.txt', 'THIRD-PARTY.md'):
                    (staging / file).write_bytes(z.read(prefix + file))
                for file in ('docs/TRUST.md', 'docs/INSTALL.md', 'docs/INTEGRATION.md', 'docs/DESTINATION-POLICY.md', 'docs/LEASE-DEADLINES.md', 'docs/SUPERVISION.md', 'docs/MIGRATING-V2.md'):
                    dst = staging / 'engine-docs' / Path(file).name
                    dst.parent.mkdir(exist_ok=True)
                    dst.write_bytes(z.read(prefix + file))
    (staging / 'PROVENANCE.json').write_bytes((json.dumps(provenance, indent=2) + '\n').encode('utf-8'))
    return staging

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--downloads', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    print(build(args.downloads.resolve(), args.out.resolve()))
