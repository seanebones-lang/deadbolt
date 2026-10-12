"""Installed job-scoped heartbeat: no accounts, credentials or real workloads."""
import threading
import time
import unittest
from deadbolt_supervisor import HeartbeatLost, SupervisorHeartbeat


class HeartbeatContract(unittest.TestCase):
    def test_initial_failure_prevents_work_and_does_not_retry(self):
        for response in [None, [], {"ok": False}, {"ok": "true"}, {"ok": 1}, {"ok": True, "code": "deny"}]:
            calls=[]
            with self.assertRaises(HeartbeatLost):
                with SupervisorHeartbeat('dummy', 'dummy-secret', should_continue=lambda:True,
                                         renew=lambda *args:calls.append(args) or response):
                    self.fail('work started without confirmed renewal')
            self.assertEqual(len(calls),1)

    def test_job_scope_stops_heartbeats_and_cannot_restart(self):
        calls=[]; repeated=threading.Event()
        def renew(*args):
            calls.append(args)
            if len(calls)>1: repeated.set()
            return {'ok':True}
        h=SupervisorHeartbeat('dummy', 'dummy-secret', should_continue=lambda:True,
                              interval_secs=.01, renew=renew)
        with h:
            h.check()
            self.assertTrue(repeated.wait(1))
        count=len(calls)
        self.assertGreater(count,1)
        time.sleep(.03)
        self.assertEqual(len(calls),count)
        with self.assertRaises(HeartbeatLost): h.check()
        with self.assertRaises(HeartbeatLost): h.__enter__()

    def test_lifecycle_loss_stops_renewals_without_retry(self):
        live=threading.Event(); live.set(); calls=[]
        h=SupervisorHeartbeat('dummy', 'dummy-secret', should_continue=live.is_set,
                              interval_secs=.01, renew=lambda *args:calls.append(args) or {'ok':True})
        with self.assertRaises(HeartbeatLost):
            with h:
                live.clear()
                self.assertTrue(h._lost.wait(1))
                h.check()
        self.assertEqual(len(calls),1)

    def test_transport_exception_is_not_echoed_and_thread_stops(self):
        calls=[]
        def renew(*args):
            calls.append(args)
            if len(calls)>1: raise RuntimeError('private-secret-value')
            return {'ok':True}
        h=SupervisorHeartbeat('dummy','private-secret-value',should_continue=lambda:True,
                              interval_secs=.01,renew=renew)
        with self.assertRaisesRegex(HeartbeatLost,'^supervisor liveness lost$'):
            with h:
                self.assertTrue(h._lost.wait(1))
                h.check()
        self.assertEqual(len(calls),2)
        self.assertNotIn('private-secret-value',repr(h))

    def test_invalid_interval_and_untrusted_continue_value(self):
        for interval in [0,True,float('inf'),21]:
            with self.assertRaises(ValueError):
                SupervisorHeartbeat('dummy','dummy-secret',should_continue=lambda:True,interval_secs=interval)
        calls=[]
        with self.assertRaises(HeartbeatLost):
            with SupervisorHeartbeat('dummy','dummy-secret',should_continue=lambda:'yes',
                                     renew=lambda *args:calls.append(args) or {'ok':True}): pass
        self.assertEqual(calls,[])


if __name__=='__main__': unittest.main()
