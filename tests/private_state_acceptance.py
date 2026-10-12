"""Actual Unix executable state creation under permissive subprocess umasks."""
import argparse
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--ownership-only', action='store_true', help='Disposable root-owned Unix fixture; requires root')
    parser.add_argument('--acl-only', action='store_true', help='Linux POSIX ACL fixtures; requires setfacl')
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if os.name != 'posix':
        print(json.dumps({'passed': True, 'coverage': 'Unix modes only; Windows ACLs operator-managed'}))
        return
    cases = []
    with tempfile.TemporaryDirectory(prefix='db-state-') as temporary:
        root = Path(temporary)
        base = {k: v for k, v in os.environ.items() if not k.startswith('DEADBOLT_')}
        if args.acl_only:
            command = shutil.which('setfacl')
            assert command, 'install acl fixture utility'
            for target in ('parent', 'db', 'events', 'db-wal', 'db-shm', 'db-journal'):
                work = root / target
                work.mkdir(mode=0o700)
                env = dict(base, DEADBOLT_DB=str(work/'db'), DEADBOLT_EVENTS=str(work/'events'))
                subprocess.run([str(binary), 'ensure', '--agent', 'fixture'], env=env,
                               capture_output=True, check=True, timeout=10)
                if target == 'parent':
                    subprocess.run([command, '-m', 'd:u:65534:r-x', str(work)], check=True, timeout=10)
                else:
                    file = work / target
                    if not file.exists(): file.write_bytes(b'')
                    file.chmod(0o600)
                    subprocess.run([command, '-m', 'u:65534:r--', str(file)], check=True, timeout=10)
                    # Keep mode 0600 while retaining the ACL representation.
                    subprocess.run([command, '-m', 'm::---', str(file)], check=True, timeout=10)
                    assert file.stat().st_mode & 0o7777 == 0o600
                result = subprocess.run([str(binary), 'ensure', '--agent', 'unexpected'], env=env,
                                        capture_output=True, timeout=10)
                assert result.returncode != 0, target
                if target == 'parent': subprocess.run([command, '-k', str(work)], check=True, timeout=10)
                else: subprocess.run([command, '-b', str(file)], check=True, timeout=10)
                subprocess.run([str(binary), 'ensure', '--agent', 'control'], env=env,
                               capture_output=True, check=True, timeout=10)
                cases.append({'extended_acl': target, 'startup': 'refused', 'simple_acl_control': 'passed'})
            print(json.dumps({'passed': True, 'cases': cases, 'real_accounts_connected': False}))
            return
        if args.ownership_only:
            assert os.geteuid() == 0, 'requires root only for disposable chown fixture'
            for target in ('db', 'events', 'db-wal', 'db-shm', 'db-journal'):
                work = root / target
                work.mkdir(mode=0o700)
                env = dict(base, DEADBOLT_DB=str(work/'db'), DEADBOLT_EVENTS=str(work/'events'))
                subprocess.run([str(binary), 'ensure', '--agent', 'fixture'], env=env,
                               capture_output=True, check=True, timeout=10)
                file = work / target
                if not file.exists(): file.write_bytes(b'')
                file.chmod(0o600)
                before = file.read_bytes()
                os.chown(file, 65534, -1)
                result = subprocess.run([str(binary), 'ensure', '--agent', 'unexpected'], env=env,
                                        capture_output=True, timeout=10)
                assert result.returncode != 0, target
                assert file.read_bytes() == before, target
                cases.append({'foreign_owner': target, 'startup': 'refused', 'bytes': 'unchanged'})
            print(json.dumps({'passed': True, 'cases': cases, 'real_accounts_connected': False}))
            return
        for mask in (0o000, 0o022):
            for setup in ('default', 'custom', 'relative', 'expanded'):
                work = root / f'{mask}-{setup}'
                work.mkdir(mode=0o700)
                env = dict(base, HOME=str(work))
                if setup == 'default':
                    db, events = work / '.deadbolt/deadbolt.db', work / '.deadbolt/deadbolt-events.jsonl'
                else:
                    db, events = work / 'state/db', work / 'logs/events'
                    paths = {'custom': (str(db), str(events)), 'relative': ('state/db', 'logs/events'),
                             'expanded': ('~/state/db', '~/logs/events')}[setup]
                    env.update(DEADBOLT_DB=paths[0], DEADBOLT_EVENTS=paths[1])
                subprocess.run([str(binary), 'ensure', '--agent', 'fixture'], cwd=work, env=env,
                               umask=mask, capture_output=True, check=True, timeout=10)
                for file in (db, events):
                    assert file.stat().st_mode & 0o7777 == 0o600, setup
                    assert file.parent.stat().st_mode & 0o7777 == 0o700, setup
                endpoint = root / 's.sock'
                env['DEADBOLT_TOKEN'] = secrets.token_hex(32)
                server = subprocess.Popen([str(binary), 'serve', '--bind', str(endpoint)], cwd=work,
                    env=env, umask=mask, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
                try:
                    for _ in range(100):
                        if server.poll() is not None: raise RuntimeError('fixture server exited')
                        if endpoint.exists(): break
                        time.sleep(.02)
                    else: raise RuntimeError('fixture readiness timeout')
                    for suffix in ('-wal', '-shm'):
                        assert Path(str(db) + suffix).stat().st_mode & 0o7777 == 0o600
                finally:
                    server.terminate()
                    server.wait(timeout=10)
                    server.stderr.close()
                    endpoint.unlink(missing_ok=True)
                subprocess.run([str(binary), 'kill', '--agent', 'fixture'], cwd=work, env=env,
                               umask=mask, capture_output=True, check=True, timeout=10)
                result = subprocess.run([str(binary), 'status', '--agent', 'fixture'], cwd=work, env=env,
                    umask=mask, capture_output=True, text=True, check=True, timeout=10)
                assert 'killed' in result.stdout
                cases.append({'umask': oct(mask), 'paths': setup, 'files': '0600', 'parents': '0700',
                              'wal_shm': '0600', 'reopen': 'kill retained'})
    print(json.dumps({'passed': True, 'cases': cases, 'real_accounts_connected': False}))


if __name__ == '__main__': main()
