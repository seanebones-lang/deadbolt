"""Build a MCPB from the four checksum-verified published v2.0.0-rc.3 archives."""
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
    'aarch64-apple-darwin': 'e3156c03b9979ad68c58f6de17b503f2fdc20c037e4c159a518cd2ef3b247291',
    'x86_64-apple-darwin': 'ccc044f85977b4d320c5bdb3e0a146c17fba1edc63b407f19d7945ba4c1bc276',
    'x86_64-unknown-linux-gnu': '7ce46035c1b16b15836a06c4717062e3a74b93630c834fab5ce7f27d4e993911',
    'x86_64-pc-windows-msvc': 'ce0a48ba1bdc05f3deaa0dc7024866cc7b2238ddfacd3ce729bc8d34d416efb7',
}

def build(downloads, out):
    out.mkdir(parents=True, exist_ok=True)
    staging = out / 'bundle'
    if staging.exists():
        raise RuntimeError('Use a fresh output directory; existing bundle will not be overwritten')
    shutil.copytree(ROOT / 'dist/mcpb', staging)
    provenance = {'engine_version': '2.0.0-rc.3', 'engine_release': 'https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0-rc.3', 'binaries': {}}
    for target in TARGETS:
        name = f'deadbolt-2.0.0-rc.3-{target}.zip'
        archive = downloads / name
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if digest != DIGESTS[target]:
            raise RuntimeError(f'Archive checksum mismatch: {target}')
        with zipfile.ZipFile(archive) as z:
            prefix = name[:-4] + '/'
            meta = json.loads(z.read(prefix + 'BUILD.json'))
            if meta['version'] != '2.0.0-rc.3' or meta['target'] != target or meta['source_revision'] != '1e879b994d4115ed0cd0c6668c13ee208dd1b351':
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
            provenance['binaries'][target] = {**meta, 'archive_sha256': digest, 'archive_url': f'https://github.com/seanebones-lang/deadbolt/releases/download/v2.0.0-rc.3/{name}'}
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
