"""Verify a fresh native release ZIP and installed Python wheel, without real accounts."""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import tomllib
import zipfile

from packaging.version import Version
import deadbolt_client as client

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    archive_source=parser.add_mutually_exclusive_group(required=True)
    archive_source.add_argument('--archive',type=Path)
    archive_source.add_argument('--target', help="Select this source version's freshly built native archive")
    parser.add_argument("--published-upgrades", action="store_true",
                        help="Also verify both checksum-pinned published versions upgrade to the extracted executable")
    args = parser.parse_args()
    expected = tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
    archive=(args.archive or ROOT/'release-artifacts'/f'deadbolt-{expected}-{args.target}.zip').resolve(strict=True)
    commit = subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    assert Version(importlib.metadata.version('n11-deadbolt-client')) == Version(expected)
    assert not Path(client.__file__).resolve().is_relative_to(ROOT/'examples'), 'install the wheel first'
    checks=[]
    with tempfile.TemporaryDirectory(prefix='deadbolt-native-candidate-') as directory:
        root=Path(directory)
        with zipfile.ZipFile(archive) as z:
            names=z.namelist()
            prefix=archive.stem+'/'
            assert names and all(name.startswith(prefix) and '..' not in Path(name).parts and not Path(name).is_absolute() for name in names)
            assert all(not stat.S_ISLNK(entry.external_attr >> 16) for entry in z.infolist()), 'no archive symlinks'
            meta=json.loads(z.read(prefix+'BUILD.json'))
            assert meta['version']==expected
            if args.target: assert meta['target']==args.target
            assert meta['source_revision']==commit, 'archive must match exact reviewed source'
            executable='deadbolt.exe' if 'windows' in meta['target'] else 'deadbolt'
            content=z.read(prefix+executable)
            assert hashlib.sha256(content).hexdigest()==meta['binary_sha256']
            assert z.read(prefix+'examples/deadbolt_client.py')==Path(client.__file__).read_bytes(), 'installed wheel must match native source client'
            assert z.read(prefix+'examples/deadbolt_client.js')==(ROOT/'examples/deadbolt_client.js').read_bytes()
            assert z.read(prefix+'docs/MIGRATING-V2.md')==(ROOT/'docs/MIGRATING-V2.md').read_bytes()
            z.extractall(root)
        binary=root/archive.stem/executable
        binary.chmod(0o755)
        env={key:value for key,value in os.environ.items() if key not in ('GH_TOKEN','GITHUB_TOKEN')}
        env['DEADBOLT_BIN']=str(binary)
        for key in list(env):
            if key.startswith('DEADBOLT_') and key!='DEADBOLT_BIN': del env[key]
        result=subprocess.run([str(binary),'--version'],env=env,capture_output=True,text=True,check=True,timeout=10)
        assert result.stdout.strip()=='deadbolt '+expected
        subprocess.run([str(binary),'drill'],env=env,capture_output=True,check=True,timeout=15)
        checks.append('extracted version, drill, source identity and hashes')
        for script, tail in [
            ('client_contract.py', []),
            ('deadline_client_acceptance.py',['--binary',str(binary)]),
            ('supervision_client_acceptance.py',['--binary',str(binary)]),
            ('executor_acceptance.py',['--binary',str(binary)]),
        ]:
            result=subprocess.run([sys.executable,str(ROOT/'tests'/script),*tail],cwd=root,env=env,capture_output=True,text=True,timeout=180)
            if result.returncode != 0:
                raise RuntimeError(script+' failed:\n'+(result.stdout+result.stderr)[-3000:])
            checks.append(script)
        if args.published_upgrades:
            result=subprocess.run([sys.executable,str(ROOT/'tests/published_upgrade_acceptance.py'),
                                   '--binary',str(binary),'--target',meta['target']],
                                  cwd=root,env={**os.environ,'DEADBOLT_BIN':str(binary)},capture_output=True,text=True,timeout=300)
            if result.returncode:
                raise RuntimeError('published upgrades failed:\n'+(result.stdout+result.stderr)[-3000:])
            upgrade_report=json.loads(result.stdout)
            assert upgrade_report['passed'] is True
            checks.append(upgrade_report)
        print(json.dumps({'passed':True,'version':expected,'source_commit':commit,'target':meta['target'],
                          'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),
                          'binary_sha256':meta['binary_sha256'],'checks':checks,'real_accounts_connected':False}))


if __name__=='__main__': main()
