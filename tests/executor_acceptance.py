#!/usr/bin/env python3
"""Installed helper, owned disposable processes, and real sidecar. No accounts."""
import argparse
from contextlib import ExitStack
import importlib.metadata
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

import deadbolt_client as client
import deadbolt_supervisor as supervisor

ROOT = Path(__file__).resolve().parents[1]
BINARY = None


def active(agent='run'):
    return {'agents':[{'agent_id':agent,'state':'active','expires_at':int(time.time())+60,
                      'hard_deadline_at':None,'clips':[]}]}


class ExecutorAcceptance(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='deadbolt-owned-')
        self.root = Path(self.temp.name)
        self.options = dict(admit=lambda *args:{'decision':'allow'}, status=lambda _:active(),
                            poll_secs=.03, grace_secs=.05, max_secs=5)

    def tearDown(self):
        self.temp.cleanup()

    def run_body(self, code, **options):
        return supervisor.run_owned('run', 'read', [sys.executable, '-c', code],
                                    **{**self.options, **options})

    def writer(self, marker):
        return ('import time,pathlib,signal; signal.signal(signal.SIGTERM,signal.SIG_IGN); '
                'p=pathlib.Path('+repr(str(marker))+'); '
                '\nwhile True:\n p.write_text(str(time.monotonic())); time.sleep(.015)')

    def assert_stopped(self, marker):
        self.assertTrue(marker.exists(), 'fixture never started')
        value = marker.read_text()
        time.sleep(.15)
        self.assertEqual(marker.read_text(), value, 'work continued after cancellation')

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_completion_nonzero_and_minimal_environment(self):
        result = self.run_body('import os,sys; assert not any(k.startswith("DEADBOLT_") for k in os.environ); sys.exit(7)')
        self.assertEqual((result['reason'], result['returncode']), ('completed',7))
        self.assertEqual(self.run_body('pass')['returncode'], 0)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_spawn_failure_is_explicit_and_output_file_works(self):
        with self.assertRaises(RuntimeError):
            supervisor.run_owned('run','read',[str(self.root/'missing')],**self.options)
        output=self.root/'stdout'
        with output.open('wb') as file:
            self.assertEqual(self.run_body('print("observed output")',stdout=file)['returncode'],0)
        self.assertEqual(output.read_text().strip(),'observed output')

    def test_unsupported_refuses_before_callbacks_or_spawn(self):
        with patch.object(supervisor, 'supported', return_value=False), patch.object(subprocess, 'Popen') as spawn:
            with self.assertRaises(NotImplementedError):
                self.run_body('pass', status=lambda _:self.fail('status called'), admit=lambda *a:self.fail('admit called'))
            spawn.assert_not_called()

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_denied_and_revoked_before_spawn(self):
        with patch.object(subprocess, 'Popen') as spawn:
            self.assertFalse(self.run_body('pass', admit=lambda *a:{'decision':'deny'})['executed'])
            dead=active(); dead['agents'][0]['state']='killed'
            self.assertEqual(self.run_body('pass',status=lambda _:dead)['reason'],'killed')
            spawn.assert_not_called()

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_revocation_after_anchor_ready_withholds_command(self):
        count=[0]; marker=self.root/'never-started'
        def status(_):
            count[0]+=1; data=active()
            if count[0]>=3: data['agents'][0]['state']='killed'
            return data
        result=self.run_body('import pathlib; pathlib.Path('+repr(str(marker))+').write_text("bad")',status=status)
        self.assertFalse(result['executed'])
        self.assertEqual(result['reason'],'killed')
        self.assertFalse(marker.exists())

    def test_status_validation(self):
        bad=[None, {}, {'agents':[]}, {'agents':[active()['agents'][0]]*2}]
        for key,value in [('agent_id','other'),('state','unknown'),('expires_at',True),('clips','read'),('hard_deadline_at',False)]:
            row=active()['agents'][0]; row[key]=value; bad.append({'agents':[row]})
        row=active()['agents'][0]; del row['hard_deadline_at']; bad.append({'agents':[row]})
        for data in bad:
            self.assertEqual(supervisor.lease_stop_reason(data,'run','read'),'status_unavailable')
        for key,value,reason in [('state','paused','paused'),('state','killed','killed'),('expires_at',0,'lease_expired'),
                                  ('hard_deadline_at',int(time.time())-1,'deadline_exceeded'),('clips',['read'],'clipped')]:
            data=active(); data['agents'][0][key]=value
            self.assertEqual(supervisor.lease_stop_reason(data,'run','read'),reason)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_configuration_refused_before_spawn(self):
        for kwargs in [{'max_secs':float('nan')},{'poll_secs':0},{'child_env':{'DEADBOLT_TOKEN':'dummy'}},
                       {'stdout':subprocess.PIPE},{'grace_secs':-1}]:
            with self.assertRaises(ValueError): self.run_body('pass', **kwargs)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_timeout_escalates_for_term_ignoring_work(self):
        marker=self.root/'timeout'
        result=self.run_body(self.writer(marker),max_secs=.3)
        self.assertEqual(result['reason'],'executor_timeout')
        self.assertIsNone(result['returncode'])
        self.assert_stopped(marker)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_status_exception_cancels_running_work(self):
        marker=self.root/'unavailable'
        def status(_):
            if marker.exists(): raise OSError('disconnected')
            return active()
        self.assertEqual(self.run_body(self.writer(marker),status=status)['reason'],'status_unavailable')
        self.assert_stopped(marker)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_malformed_status_cancels_running_work(self):
        marker=self.root/'malformed'
        def status(_):
            return {'agents':[]} if marker.exists() else active()
        self.assertEqual(self.run_body(self.writer(marker),status=status)['reason'],'status_unavailable')
        self.assert_stopped(marker)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_term_grace_reaches_real_command(self):
        ready=self.root/'ready'; stopped=self.root/'term'
        code=('import signal,pathlib,time,sys; '
              'signal.signal(signal.SIGTERM,lambda *a:(pathlib.Path('+repr(str(stopped))+').write_text("stopped"),sys.exit(0))); '
              'pathlib.Path('+repr(str(ready))+').write_text("ready"); time.sleep(30)')
        def status(_):
            data=active()
            if ready.exists(): data['agents'][0]['state']='killed'
            return data
        self.assertEqual(self.run_body(code,status=status,grace_secs=.2)['reason'],'killed')
        self.assertEqual(stopped.read_text(),'stopped')

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_supervisor_interrupt_cleans_up(self):
        marker=self.root/'interrupt'
        def status(_):
            if marker.exists(): raise KeyboardInterrupt()
            return active()
        with self.assertRaises(KeyboardInterrupt): self.run_body(self.writer(marker),status=status)
        self.assert_stopped(marker)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_finished_parent_cleans_lingering_descendant_and_preserves_exit(self):
        marker=self.root/'descendant'
        code=('import subprocess,sys,pathlib,time; subprocess.Popen([sys.executable,"-c",'+repr(self.writer(marker))+']); '
              '\nwhile not pathlib.Path('+repr(str(marker))+').exists(): time.sleep(.01)\nsys.exit(4)')
        result=self.run_body(code)
        self.assertEqual((result['reason'],result['returncode']),('completed',4))
        self.assert_stopped(marker)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_unrelated_group_survives(self):
        marker=self.root/'unrelated'
        other=subprocess.Popen([sys.executable,'-c',self.writer(marker)],start_new_session=True)
        try:
            self.run_body('import time; time.sleep(10)',max_secs=.15)
            self.assertIsNone(other.poll())
            before=marker.read_text(); time.sleep(.1); self.assertNotEqual(before,marker.read_text())
        finally:
            os.killpg(other.pid,9); other.wait(timeout=5)

    @unittest.skipUnless(supervisor.supported(), 'POSIX waitid required')
    def test_real_sidecar_kill_deadline_and_outage(self):
        original=dict(os.environ)
        try:
            with ExitStack() as cleanup:
                for key in list(os.environ):
                    if key.startswith('DEADBOLT_'): del os.environ[key]
                with socket.socket() as sock:
                    sock.bind(('127.0.0.1',0)); endpoint=f'127.0.0.1:{sock.getsockname()[1]}'
                os.environ.update(DEADBOLT_SOCK=endpoint,DEADBOLT_TOKEN=secrets.token_hex(32),
                                  DEADBOLT_DB=str(self.root/'db'),DEADBOLT_EVENTS=str(self.root/'events'))
                server=subprocess.Popen([str(BINARY),'serve','--bind',endpoint],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
                def stop():
                    if server.poll() is None: server.terminate(); server.wait(timeout=5)
                cleanup.callback(stop)
                for _ in range(100):
                    if client.ensure('ready').get('ok') is True: break
                    time.sleep(.03)
                else: self.fail('sidecar unavailable')
                def operator(*args):
                    subprocess.run([str(BINARY),*args],check=True,timeout=10,capture_output=True)
                options=dict(admit=client.admit,status=client.status,max_secs=8)
                for case in ['kill','deadline','outage']:
                    agent=case; marker=self.root/case
                    descendant=self.root/(case+'-descendant')
                    if case=='deadline': operator('issue','--agent',agent,'--hard-deadline-at',str(int(time.time())+2))
                    else: self.assertTrue(client.ensure(agent)['ok'])
                    calls=[]
                    def admit(*args): calls.append(args); return client.admit(*args)
                    changed=[False]
                    def status(a):
                        if marker.exists() and descendant.exists() and not changed[0] and case!='deadline':
                            changed[0]=True
                            if case=='kill': operator('kill','--agent',agent)
                            else: stop()
                        return client.status(a)
                    code=('import subprocess,sys; subprocess.Popen([sys.executable,"-c",'+repr(self.writer(descendant))+']); '+self.writer(marker))
                    result=supervisor.run_owned(agent,'read',[sys.executable,'-c',code],
                                                **{**options,'admit':admit,'status':status,'poll_secs':.03,'grace_secs':.05})
                    self.assertEqual(result['reason'],{'kill':'killed','deadline':'deadline_exceeded','outage':'status_unavailable'}[case])
                    self.assertEqual(len(calls),1,'watcher must not re-admit/renew')
                    self.assert_stopped(marker)
                    self.assert_stopped(descendant)
        finally:
            os.environ.clear(); os.environ.update(original)


def main():
    global BINARY
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('--binary',required=True,type=Path)
    BINARY=parser.parse_args().binary.resolve(strict=True)
    if Path(supervisor.__file__).resolve().is_relative_to(ROOT/'examples'):
        raise RuntimeError('install helper outside checkout before acceptance')
    suite=unittest.defaultTestLoader.loadTestsFromTestCase(ExecutorAcceptance)
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    print(json.dumps({'passed':result.wasSuccessful(),'tests':result.testsRun,'skipped':len(result.skipped),
                      'supported':supervisor.supported(),'installed_version':importlib.metadata.version('n11-deadbolt-client'),
                      'real_accounts_connected':False}))
    if not result.wasSuccessful(): sys.exit(1)

if __name__=='__main__': main()
