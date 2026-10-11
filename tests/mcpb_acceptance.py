"""Exercise an unpacked MCPB through real stdio and disposable tool effects."""
import argparse
from contextlib import closing
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
    assert manifest['tools_generated'] and len(manifest['tools']) == 2
    assert not manifest['user_config']['operator_config']['required']
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
 const setup = await getMcpConfigForManifest({...base,userConfig:{}});
 if (!setup || setup.args.length !== 2 || !['', '${user_config.operator_config}'].includes(setup.args[1])) throw new Error('blank config does not resolve to setup mode');
 const c = await getMcpConfigForManifest({...base,userConfig:{operator_config:process.argv[4]}});
 console.log(JSON.stringify(c));
})().catch(()=>process.exit(1));
'''
        resolved = json.loads(subprocess.check_output([node, '-e', resolver, str(args.mcpb_module.resolve()),
                                                       str(bundle/'manifest.json'), str(bundle), str(config)], text=True))
        # The official resolver normalizes bundle paths to forward slashes,
        # including on Windows. Check equivalent native paths, not separators.
        assert len(resolved['args']) == 2
        assert [Path(value).resolve() for value in resolved['args']] == [launcher.resolve(), config.resolve()]
        checks.append({'case':'official_mcpb_host_resolution_and_optional_setup_config', 'passed':True})
        # Inherited proxy settings must not choose the real store or use tokens.
        env = {**os.environ, 'DEADBOLT_DB': str(w / 'wrong.db'), 'DEADBOLT_EVENTS': str(w / 'wrong-events'),
               'DEADBOLT_TOKEN': 'disposable-never-forwarded', 'DEADBOLT_ADMISSION_TOKEN': 'disposable-bad-token'}
        # Mirror hosted Node: import the entry point, bridge JS stdout writes,
        # and expose an execPath that cannot launch an ordinary Node subprocess.
        hosted = r'''
const {pathToFileURL} = require('node:url');
const entry = process.argv[1], config = process.argv[2];
process.argv = ['node', entry, config];
Object.defineProperty(process, 'execPath', {value:require('node:path').join(require('node:path').dirname(config),'not-a-node-executable')});
const write = process.stdout.write.bind(process.stdout);
let pending = '';
process.stdout.write = (chunk, encoding, callback) => {
 pending += chunk.toString();
 let end;
 while ((end = pending.indexOf('\n')) >= 0) {
  write('HOST:' + pending.slice(0,end+1)); pending = pending.slice(end+1);
 }
 if (typeof encoding === 'function') encoding(); else if (callback) callback();
 return true;
};
import(pathToFileURL(entry).href).catch(()=>process.exit(1));
'''
        def launch(file=config, embedded=False):
            command = [node, '-e', hosted, resolved['args'][0], str(file)] if embedded else [node, resolved['args'][0], str(file)]
            process = subprocess.Popen(command, env=env, stdin=subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
            process.responses = queue.Queue()
            def read():
                for line in process.stdout:
                    process.responses.put(line)
            threading.Thread(target=read, daemon=True).start()
            process.embedded = embedded
            return process
        def operator(*cmd):
            op_env = {k:v for k,v in os.environ.items() if not k.startswith('DEADBOLT_')}
            op_env.update(DEADBOLT_DB=c['database'], DEADBOLT_EVENTS=c['events'])
            return subprocess.run([str(exe), *cmd], env=op_env, check=True, capture_output=True, text=True, timeout=10)
        proc = None
        seq = 0
        def rpc(method, params=None):
            nonlocal seq
            seq += 1
            proc.stdin.write(json.dumps({'jsonrpc':'2.0','id':seq,'method':method,'params': params or {}})+'\n')
            proc.stdin.flush()
            line = proc.responses.get(timeout=10)
            if proc.embedded:
                assert line.startswith('HOST:'), 'native output bypassed the host bridge'
                line = line[5:]
            response = json.loads(line)
            assert response['id'] == seq
            return response
        # Setup mode must be useful with zero operator configuration or effects.
        for embedded in (False, True):
            proc = launch('${user_config.operator_config}' if embedded else '', embedded=embedded)
            try:
                assert rpc('initialize', {'protocolVersion':'2025-06-18'})['result']['serverInfo']['name'] == 'deadbolt-setup'
                definitions = rpc('tools/list')['result']['tools']
                assert [(t['name'], t['description']) for t in definitions] == [(t['name'], t['description']) for t in manifest['tools']]
                for t in definitions:
                    assert t['annotations'] == {'readOnlyHint':True, 'destructiveHint':False, 'idempotentHint':True, 'openWorldHint':False}
                    assert t['outputSchema']['type'] == 'object'
                    assert all(v.get('description') for v in t['inputSchema']['properties'].values())
                def setup_call(name, arguments):
                    result = rpc('tools/call', {'name':name, 'arguments':arguments})['result']
                    data = result['structuredContent']
                    schema = next(t['outputSchema'] for t in definitions if t['name']==name)
                    assert set(data) == set(schema['required'])
                    types = {'string': str, 'boolean': bool, 'array': list}
                    for key, prop in schema['properties'].items():
                        assert isinstance(data[key], types[prop['type']])
                        if 'const' in prop: assert data[key] == prop['const']
                    assert json.loads(result['content'][0]['text']) == data
                    return data
                verified = setup_call('deadbolt_check_bundle', {})
                assert verified['checksum_matches'] and verified['active_protection'] is False
                assert verified['target'] == target and verified['engine_version'] == '1.0.3'
                for route in ('claude_desktop', 'stdio_client', 'rust', 'http'):
                    g = setup_call('deadbolt_get_integration_guide', {'route':route})
                    assert g['route'] == route and 'no tools are protected' in g['boundary']
                    assert json.loads(g['operator_config_example'])['tools']
                for name, arguments in [('deadbolt_get_integration_guide', {}), ('deadbolt_get_integration_guide', {'route':'unknown'}), ('deadbolt_check_bundle', {'path': str(w)}), ('deadbolt_check_bundle', [])]:
                    assert rpc('tools/call', {'name':name, 'arguments':arguments})['result']['isError']
                assert 'error' in rpc('tools/call', {'name':'approve', 'arguments':{}})
                assert 'error' in rpc('resources/read', {'uri':'file://'+str(w)})
                assert not starts.exists() and not effects.exists() and not Path(c['database']).exists()
            finally:
                proc.stdin.close()
                proc.wait(timeout=10)
                assert proc.returncode == 0, proc.stderr.read()
            checks.append({'case':'read_only_setup_and_typed_tools_embedded' if embedded else 'read_only_setup_and_typed_tools', 'passed': True})
        # Supplying a missing configuration must fail, never fall back to setup.
        invalid = launch(w/'missing-config.json')
        stdout, stderr = invalid.communicate(timeout=10)
        assert invalid.returncode != 0 and not stdout and not starts.exists()
        checks.append({'case':'supplied_missing_config_never_falls_back_to_setup', 'passed':True})
        proc = launch()
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
            with closing(sqlite3.connect(c['database'])) as db:
                with db:
                    db.execute('UPDATE leases SET expires_at=0 WHERE agent_id=?', (c['agent'],))
            call('echo', 'lease_expired')
        finally:
            proc.stdin.close()
            proc.wait(timeout=10)
        checks.append({'case': 'expiry_forced_by_disposable_operator_store', 'passed': True})
        c['agent'] = 'embedded-host-test'
        config.write_text(json.dumps(c))
        proc = launch(embedded=True)
        try:
            assert 'result' in rpc('initialize')
            assert len(rpc('tools/list')['result']['tools']) == 3
            call('echo')
            call('off_list', 'purpose_exceeded')
            call('write_marker', 'needs_human')
            operator('kill', '--agent', c['agent'])
            call('echo', 'killed')
        finally:
            proc.stdin.close()
            proc.wait(timeout=10)
            assert proc.returncode == 0, proc.stderr.read()
        count = starts.read_text()
        dead = launch(embedded=True)
        dead.communicate(timeout=10)
        assert dead.returncode != 0 and starts.read_text() == count
        checks.append({'case': 'embedded_import_host_stream_bridge_no_self_exec_and_killed_restart', 'passed': True})
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
