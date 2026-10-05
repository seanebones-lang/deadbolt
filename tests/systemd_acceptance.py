"""Disposable Linux/systemd acceptance using a privileged Docker container.

Requires Docker with cgroup access. No host ports, operator tokens, or host state
are shared with the container. This is a bounded service test, not host-wide
production acceptance.
"""

import argparse
import datetime
import json
import os
import pathlib
import subprocess
import sys
import time


def run(*args, check=True):
    return subprocess.run(args, check=check, text=True, capture_output=True)


CLIENT = r'''
import json, pathlib, urllib.error, urllib.request
token = pathlib.Path('/etc/deadbolt/deadbolt.env').read_text().strip().split('=', 1)[1]
base = 'http://127.0.0.1:9782'
def request(path, data=None, secret=token):
    return urllib.request.Request(
        base + path,
        data=None if data is None else json.dumps(data).encode(),
        headers={'X-Deadbolt-Token': secret},
        method='GET' if data is None else 'POST',
    )
def call(path, data=None):
    return json.load(urllib.request.urlopen(request(path, data)))
def rejects_wrong_token():
    try:
        urllib.request.urlopen(request('/status', secret='wrong'))
    except urllib.error.HTTPError as error:
        assert error.code == 401
    else:
        raise AssertionError('wrong token admitted')
rejects_wrong_token()
assert call('/ensure', {'agent_id': 'systemd-a'})['ok'] is True
assert call('/policy', {'agent_id': 'systemd-a', 'tools': ['safe']})['ok'] is True
assert call('/admit', {'agent_id': 'systemd-a', 'tool': 'safe'})['decision'] == 'allow'
assert call('/admit', {'agent_id': 'systemd-a', 'tool': 'blocked'})['decision'] == 'deny'
'''

POST_RESTART = r'''
import json, pathlib, urllib.error, urllib.request
token = pathlib.Path('/etc/deadbolt/deadbolt.env').read_text().strip().split('=', 1)[1]
def call(path, data):
    request = urllib.request.Request(
        'http://127.0.0.1:9782' + path,
        data=json.dumps(data).encode(),
        headers={'X-Deadbolt-Token': token}, method='POST',
    )
    return json.load(urllib.request.urlopen(request))
def killed():
    return call('/admit', {'agent_id': 'systemd-a', 'tool': 'safe'}) == {'decision': 'deny', 'code': 'killed'}
assert killed()
call('/ensure', {'agent_id': 'systemd-a'})
assert killed()
assert call('/ensure', {'agent_id': 'systemd-b'})['ok'] is True
assert call('/admit', {'agent_id': 'systemd-b', 'tool': 'safe'})['decision'] == 'allow'
'''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', type=pathlib.Path)
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parent.parent
    image = 'deadbolt-systemd-acceptance:local'
    name = f'deadbolt-systemd-acceptance-{os.getpid()}'
    run('docker', 'build', '-q', '-f', str(root / 'tests/systemd.Dockerfile'), '-t', image, str(root))
    try:
        run('docker', 'run', '-d', '--name', name, '--privileged', '--tmpfs', '/run',
            '--tmpfs', '/run/lock', '-v', '/sys/fs/cgroup:/sys/fs/cgroup:rw', image)
        for _ in range(50):
            status = run('docker', 'exec', name, 'systemctl', 'is-system-running', check=False)
            if status.stdout.strip() in {'running', 'degraded'}:
                break
            time.sleep(0.1)
        else:
            raise RuntimeError('systemd did not boot')
        for _ in range(50):
            service = run('docker', 'exec', name, 'systemctl', 'is-active', 'deadbolt', check=False).stdout.strip()
            if service == 'failed':
                break
            time.sleep(0.1)
        else:
            raise RuntimeError(f'empty-token service did not fail: {service}')
        # Generate a disposable test token inside the container; no token is stored in source.
        run('docker', 'exec', name, 'python3', '-c',
            'import pathlib,secrets; '
            'p=pathlib.Path("/etc/deadbolt/deadbolt.env"); '
            'test_token=secrets.token_hex(32); '
            'p.write_text("{}={}\\n".format("DEADBOLT_TOKEN", test_token)); '
            'p.chmod(0o600)')
        run('docker', 'exec', name, 'systemctl', 'reset-failed', 'deadbolt')
        run('docker', 'exec', name, 'systemctl', 'start', 'deadbolt')
        assert run('docker', 'exec', name, 'systemctl', 'is-active', 'deadbolt').stdout.strip() == 'active', 'tokened service inactive'
        run('docker', 'exec', name, 'python3', '-c', CLIENT)
        run('docker', 'exec', name, 'runuser', '-u', 'deadbolt', '--', 'env',
            'DEADBOLT_DB=/var/lib/deadbolt/deadbolt.db',
            'DEADBOLT_EVENTS=/var/lib/deadbolt/deadbolt-events.jsonl',
            '/usr/local/bin/deadbolt', 'kill', '--agent', 'systemd-a')
        run('docker', 'exec', name, 'systemctl', 'restart', 'deadbolt')
        run('docker', 'exec', name, 'python3', '-c', POST_RESTART)
        revision = run('git', '-C', str(root), 'rev-parse', 'HEAD').stdout.strip()
        report = {
            'passed': True,
            'tested_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'source_revision': revision,
            'source_tree_clean': not run('git', '-C', str(root), 'status', '--porcelain').stdout.strip(),
            'image_id': run('docker', 'image', 'inspect', image, '--format', '{{.Id}}').stdout.strip(),
            'platform': run('docker', 'exec', name, 'uname', '-sm').stdout.strip(),
            'service': run('docker', 'exec', name, 'systemctl', 'is-active', 'deadbolt').stdout.strip(),
            'checks': [
                'empty token prevents startup', 'tokened service starts under systemd',
                'wrong token returns 401', 'policy permits listed tool and denies off-list tool',
                'operator kill persists after systemd restart and re-ensure',
                'unrelated agent remains admitted',
            ],
            'limit': 'Privileged Docker container with systemd on a Linux VM; not a native-host deployment or application integration.',
        }
        encoded = json.dumps(report, indent=2) + '\n'
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(encoded)
        print(encoded, end='')
    finally:
        run('docker', 'rm', '-f', name, check=False)


if __name__ == '__main__':
    try:
        main()
    except (AssertionError, RuntimeError, subprocess.CalledProcessError) as error:
        if isinstance(error, subprocess.CalledProcessError):
            print(error.stderr.strip() or error.stdout.strip(), file=sys.stderr)
        print(f'systemd acceptance failed: {error}', file=sys.stderr)
        sys.exit(1)
