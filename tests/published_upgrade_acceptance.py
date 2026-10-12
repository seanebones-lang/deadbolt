"""Checksum-pinned published native archives -> current extracted candidate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import zipfile

ROOT=Path(__file__).resolve().parents[1]


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--target',required=True)
    args=p.parse_args()
    binary=args.binary.resolve(strict=True)
    pinned=json.loads((ROOT/'tests/published_versions.json').read_text())
    reports=[]
    with tempfile.TemporaryDirectory(prefix='deadbolt-published-upgrades-') as directory:
        root=Path(directory)
        for version, release in pinned.items():
            name=f'deadbolt-{version}-{args.target}.zip'
            expected=release['archives'][name]
            destination=root/version;destination.mkdir()
            subprocess.run(['gh','release','download','v'+version,'--repo','seanebones-lang/deadbolt','--pattern',name,'--dir',str(destination)],check=True,timeout=90)
            archive=destination/name
            assert hashlib.sha256(archive.read_bytes()).hexdigest()==expected,'published archive mismatch'
            with zipfile.ZipFile(archive) as z:
                prefix=archive.stem+'/'
                assert all(i.filename.startswith(prefix) and '..' not in Path(i.filename).parts and not stat.S_ISLNK(i.external_attr>>16) for i in z.infolist())
                meta=json.loads(z.read(prefix+'BUILD.json'))
                assert meta['version']==version and meta['target']==args.target
                assert meta['source_revision']==release['source_commit']
                exe='deadbolt.exe' if 'windows' in args.target else 'deadbolt'
                assert hashlib.sha256(z.read(prefix+exe)).hexdigest()==meta['binary_sha256']
                z.extractall(destination)
            old=destination/archive.stem/exe;old.chmod(0o755)
            result=subprocess.run([sys.executable,str(ROOT/'tests/upgrade_acceptance.py'),'--old-binary',str(old),'--new-binary',str(binary)],capture_output=True,text=True,timeout=90)
            if result.returncode:
                raise RuntimeError('upgrade from '+version+' failed:\n'+(result.stdout+result.stderr)[-3000:])
            reports.append({'old_version':version,'old_archive_sha256':expected,'result':json.loads(result.stdout)})
        print(json.dumps({'passed':True,'target':args.target,'upgrades':reports,'real_accounts_connected':False}))


if __name__=='__main__':main()
