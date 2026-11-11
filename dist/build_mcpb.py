"""Build a MCPB from the four checksum-verified published v2.0.2 archives."""
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
    'x86_64-pc-windows-msvc': 'fd32eabb75a3571c040576f2d8a5a4a3654e8b6cbf1b08def7e8caae94bc9502',
    'x86_64-unknown-linux-gnu': 'c2e1e50fba85a5014f2c2ac9eafeceadc8c06ec2c24aef7f8b121ad836639d9f',
    'aarch64-apple-darwin': 'cc82167d1d0e52f43c4e95971af9324a93b5cde16d7c7f1540ec7dd39270c3a7',
    'x86_64-apple-darwin': '83d456de19d58a0d0d1490d1bc365497ea49d8317e9432e13966b33538881321',
}

def build(downloads, out):
    out.mkdir(parents=True, exist_ok=True)
    staging = out / 'bundle'
    if staging.exists():
        raise RuntimeError('Use a fresh output directory; existing bundle will not be overwritten')
    shutil.copytree(ROOT / 'dist/mcpb', staging)
    provenance = {'engine_version': '2.0.2', 'engine_release': 'https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.2', 'binaries': {}}
    for target in TARGETS:
        name = f'deadbolt-2.0.2-{target}.zip'
        archive = downloads / name
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if digest != DIGESTS[target]:
            raise RuntimeError(f'Archive checksum mismatch: {target}')
        with zipfile.ZipFile(archive) as z:
            prefix = name[:-4] + '/'
            meta = json.loads(z.read(prefix + 'BUILD.json'))
            if meta['version'] != '2.0.2' or meta['target'] != target or meta['source_revision'] != '7d130e882d03b9ba82556fb072f646d4191bbe8a':
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
            provenance['binaries'][target] = {**meta, 'archive_sha256': digest, 'archive_url': f'https://github.com/seanebones-lang/deadbolt/releases/download/v2.0.2/{name}'}
            if target == TARGETS[0]:
                for file in ('LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES.txt', 'THIRD-PARTY.md'):
                    (staging / file).write_bytes(z.read(prefix + file))
                # Preserve the runtime notices and source referenced by the
                # native distribution's third-party documentation.
                for member in z.namelist():
                    if member.startswith(prefix + 'third-party/') and not member.endswith('/'):
                        relative = Path(member.removeprefix(prefix))
                        if '..' in relative.parts or relative.is_absolute():
                            raise RuntimeError('Unsafe third-party notice path')
                        dst = staging / relative
                        dst.parent.mkdir(parents=True, exist_ok=True)
                        dst.write_bytes(z.read(member))
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
