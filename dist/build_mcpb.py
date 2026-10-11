"""Build a MCPB from the four checksum-verified published v1.0.3 archives."""
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
    'aarch64-apple-darwin': 'd8af0582d347e56ae4d618f6ac2e4380d52b4b01ffc57a9e385e5cc33cc36a41',
    'x86_64-apple-darwin': '3534804199a637e75f53179399f9130e86c53206dcc7cbfe2253fc6eae224f04',
    'x86_64-unknown-linux-gnu': '6fbf51e9a58016a1a50b2dfd8496d8ddccc80b918d8ba59442b025fcb6029f5d',
    'x86_64-pc-windows-msvc': '6bdcb6901cf2c5e708d0d53e19bb97d4dc24354782de9b87a19883911604ebe0',
}

def build(downloads, out):
    out.mkdir(parents=True, exist_ok=True)
    staging = out / 'bundle'
    if staging.exists():
        raise RuntimeError('Use a fresh output directory; existing bundle will not be overwritten')
    shutil.copytree(ROOT / 'dist/mcpb', staging)
    provenance = {'engine_version': '1.0.3', 'engine_release': 'https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.3', 'binaries': {}}
    for target in TARGETS:
        name = f'deadbolt-1.0.3-{target}.zip'
        archive = downloads / name
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if digest != DIGESTS[target]:
            raise RuntimeError(f'Archive checksum mismatch: {target}')
        with zipfile.ZipFile(archive) as z:
            prefix = name[:-4] + '/'
            meta = json.loads(z.read(prefix + 'BUILD.json'))
            if meta['version'] != '1.0.3' or meta['target'] != target:
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
            provenance['binaries'][target] = {**meta, 'archive_sha256': digest, 'archive_url': f'https://github.com/seanebones-lang/deadbolt/releases/download/v1.0.3/{name}'}
            if target == TARGETS[0]:
                for file in ('LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES.txt', 'THIRD-PARTY.md'):
                    (staging / file).write_bytes(z.read(prefix + file))
                for file in ('docs/TRUST.md', 'docs/INSTALL.md', 'docs/INTEGRATION.md'):
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
