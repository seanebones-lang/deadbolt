"""Exercise an unpacked MCPB through real stdio and disposable tool effects."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import queue
import shutil
import sqlite3
import subprocess
import tempfile
import threading

FIXTURE = r'''
const fs = require('node:fs');
const readline = require('node:readline');
fs.appendFileSync(process.argv[2], 'started\n');
readline.createInterface({input: process.stdin}).on('line', line => {
 const r = JSON.parse(line);
 if (!Object.hasOwn(r, 'id')) return;
 let result;
 if (r.method === 'initialize') result = {protocolVersion: '2025-03-26', capabilities: {tools:{}}, serverInfo:{name:'disposable-fixture', version:'1.0.0'}};
 else if (r.method === 'tools/list') result = {tools:['echo','write_marker','off_list'].map(name=>({name,description:'Disposable acceptance fixture',inputSchema:{type:'object',properties:{},additionalProperties:true}}))};
 else if (r.method === 'tools/call') {
   fs.appendFileSync(process.argv[3], r.params.name + '\n');
   result = {content:[{type:'text', text:'fixture body ran'}]};
 } else result = {};
 process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:r.id,result})+'\n');
});
'''

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--bundle', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--mcpb-module', type=Path, required=True, help='Installed official @anthropic-ai/mcpb package directory')
    args = p.parse_args()
    bundle = args.bundle.resolve()
    node = shutil.which('node')
    assert node
    manifest = json.loads((bundle / 'manifest.json').read_text())
    assert manifest['tools_generated'] and 'tools' not in manifest
    target = {('Darwin', 'arm64'): 'aarch64-apple-darwin', ('Darwin', 'x86_64'): 'x86_64-apple-darwin',
              ('Linux', 'x86_64'): 'x86_64-unknown-linux-gnu', ('Windows', 'AMD64'): 'x86_64-pc-windows-msvc'}[(platform.system(), platform.machine())]
    exe = bundle / 'native' / target / ('deadbolt.exe' if os.name == 'nt' else 'deadbolt')
    checks = []
    with tempfile.TemporaryDirectory(prefix='deadbolt-mcpb-') as d:
        w = Path(d)
        fixture = w / 'fixture.cjs'
        fixture.write_text(FIXTURE)
        starts, effects = w / 'starts', w / 'effects'
        c = {'version': 1, 'agent': 'bundle-test', 'database': str(w / 'state.db'), 'events': str(w / 'events.jsonl'),
             'command': node, 'args': [str(fixture), str(starts), str(effects)], 'tools': ['echo', 'write_marker'],
             'destinations': [], 'irreversible_tools': ['write_marker'], 'spend_cap_usd': 10}
        config = w / 'config.json'
        config.write_text(json.dumps(c))
        launcher = bundle / manifest['server']['entry_point']
        resolver = r'''
const fs = require('node:fs');
const {getMcpConfigForManifest} = require(process.argv[1]);
const manifest = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const base = {manifest, extensionPath:process.argv[3], systemDirs:{}, pathSeparator:require('node:path').sep};
(async()=>{
 if(await getMcpConfigForManifest({...base,userConfig:{}}) !== undefined) throw new Error('missing config accepted');
 const c = await getMcpConfigForManifest({...base,userConfig:{operator_config:process.argv[4]}});
 console.log(JSON.stringify(c));
})().catch(()=>process.exit(1));
'''
        resolved = json.loads(subprocess.check_output([node, '-e', resolver, str(args.mcpb_module.resolve()),
                                                       str(bundle/'manifest.json'), str(bundle), str(config)], text=True))
        assert resolved['args'] == [str(launcher), str(config)]
        checks.append({'case':'official_mcpb_host_resolution_and_required_config', 'passed':True})
        # Inherited proxy settings must not choose the real store or use tokens.
        env = {**os.environ, 'DEADBOLT_DB': str(w / 'wrong.db'), 'DEADBOLT_EVENTS': str(w / 'wrong-events'),
               'DEADBOLT_TOKEN': 'disposable-never-forwarded', 'DEADBOLT_ADMISSION_TOKEN': 'disposable-bad-token'}
        def launch(file=config):
            process = subprocess.Popen([node, resolved['args'][0], str(file)], env=env, stdin=subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
            process.responses = queue.Queue()
            def read():
                for line in process.stdout:
                    process.responses.put(line)
            threading.Thread(target=read, daemon=True).start()
            return process
        def operator(*cmd):
            op_env = {k:v for k,v in os.environ.items() if not k.startswith('DEADBOLT_')}
            op_env.update(DEADBOLT_DB=c['database'], DEADBOLT_EVENTS=c['events'])
            return subprocess.run([str(exe), *cmd], env=op_env, check=True, capture_output=True, text=True, timeout=10)
        proc = launch()
        seq = 0
        def rpc(method, params=None):
            nonlocal seq
            seq += 1
            proc.stdin.write(json.dumps({'jsonrpc':'2.0','id':seq,'method':method,'params': params or {}})+'\n')
            proc.stdin.flush()
            response = json.loads(proc.responses.get(timeout=10))
            assert response['id'] == seq
            return response
        def call(tool, deny=None):
            before = effects.read_text() if effects.exists() else ''
            response = rpc('tools/call', {'name': tool, 'arguments': {}})
            after = effects.read_text() if effects.exists() else ''
            if deny:
                assert deny in response['error']['message'], response
                assert before == after, 'denied body ran'
            else:
                assert 'result' in response, response
                assert after == before + tool + '\n'
            checks.append({'case': tool + ':' + (deny or 'allow'), 'body_ran': before != after})
        try:
            assert rpc('initialize')['result']['serverInfo']['name'] == 'disposable-fixture'
            assert len(rpc('tools/list')['result']['tools']) == 3
            call('echo')
            call('off_list', 'purpose_exceeded')
            call('write_marker', 'needs_human')
            operator('approve', '--agent', c['agent'], '--tool', 'write_marker')
            call('write_marker')
            call('write_marker', 'needs_human')
            operator('pause', '--agent', c['agent'])
            call('echo', 'paused')
            operator('resume', '--agent', c['agent'])
            call('echo')
            operator('clip', 'echo', '--agent', c['agent'])
            call('echo', 'purpose_exceeded')
            operator('resume', '--agent', c['agent'])
            operator('kill', '--agent', c['agent'])
            call('echo', 'killed')
        finally:
            proc.stdin.close()
            proc.wait(timeout=10)
            assert proc.returncode == 0, proc.stderr.read()
        assert not (w / 'wrong.db').exists()
        assert not (w / 'wrong-events').exists()
        checks.append({'case': 'inherited_state_and_credentials_ignored', 'passed': True})
        count = starts.read_text()
        dead = launch()
        _, stderr = dead.communicate(timeout=10)
        assert dead.returncode != 0 and starts.read_text() == count
        checks.append({'case': 'killed_restart_never_starts_configured_child', 'passed': True})
        assert 'disposable-never-forwarded' not in stderr and str(w) not in stderr
        for mutation in ({'tools': []}, {'command': 'node'}, {'args': 'echo hi'}, {'unexpected': True},
                         {'database': str(w/'missing'/'db')}, {'spend_cap_usd': -1}):
            bad = w / 'bad.json'
            bad.write_text(json.dumps({**c, **mutation}))
            invalid = launch(bad)
            invalid.communicate(timeout=10)
            assert invalid.returncode != 0 and starts.read_text() == count
        checks.append({'case': 'invalid_configuration_never_starts_configured_child', 'cases': 6, 'passed': True})
        c['agent'] = 'expiry-test'
        config.write_text(json.dumps(c))
        proc = launch()
        try:
            assert 'result' in rpc('initialize')
            call('echo')
            with sqlite3.connect(c['database']) as db:
                db.execute('UPDATE leases SET expires_at=0 WHERE agent_id=?', (c['agent'],))
            call('echo', 'lease_expired')
        finally:
            proc.stdin.close()
            proc.wait(timeout=10)
        checks.append({'case': 'expiry_forced_by_disposable_operator_store', 'passed': True})
        # A broken existing store must not start the configured child.
        c['database'] = str(w/'invalid-store.db')
        Path(c['database']).write_text('not sqlite')
        config.write_text(json.dumps(c))
        count = starts.read_text()
        invalid = launch()
        invalid.communicate(timeout=10)
        assert invalid.returncode != 0 and starts.read_text() == count
        checks.append({'case': 'unavailable_store_never_starts_configured_child', 'passed': True})
    report = {'host': platform.platform(), 'target': target, 'node_version': subprocess.check_output([node,'--version'],text=True).strip(),
              'engine_version': subprocess.check_output([str(exe),'--version'],text=True).strip(),
              'binary_sha256': hashlib.sha256(exe.read_bytes()).hexdigest(),
              'launcher_sha256': hashlib.sha256(launcher.read_bytes()).hexdigest(), 'checks': checks,
              'scope': 'Real unpacked-bundle launcher, stdio JSON-RPC host and disposable fixture effects; no model/account access or GUI client installation. Other OS wrappers not executed in this run.'}
    args.out.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report, indent=2))

if __name__ == '__main__':
    main()
