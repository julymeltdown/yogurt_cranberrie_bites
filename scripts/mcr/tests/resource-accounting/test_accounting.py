"""New functional fault boundaries and real filesystem churn; no feature matrix."""
from pathlib import Path
import errno
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
import accounting as api


RESULTS = []
REAL_POPEN = subprocess.Popen


class AccountingChecks(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ['RCR_TEMP'])
        self.base = Path(self.temporary.name)
        self.root = self.base / 'owned'
        self.root.mkdir()
        self.identity = api.RootIdentity.capture(self.root)

    def tearDown(self):
        self.temporary.cleanup()

    def scan(self, **options):
        return api.complete_allocated(self.identity, deadline=time.monotonic() + 2,
                                      checkpoint=lambda: None, **options)

    def script(self, bodies):
        """Actual bounded child faults; stdout is intentionally an unsafe subtotal."""
        count = [0]
        def spawn(_command, **options):
            code = bodies[min(count[0], len(bodies) - 1)]
            count[0] += 1
            return REAL_POPEN([sys.executable, '-c', code], **options)
        return spawn, count

    def success(self, kib=19):
        return f'import sys;sys.stdout.write({str(kib)+chr(9)+str(self.root)+chr(10)!r})'

    def missing(self, suffix='/heartbeat.next', extra=''):
        error = f'du: {self.root}{suffix}: No such file or directory\n' + extra
        return f'import sys;sys.stdout.write({"1"+chr(9)+str(self.root)+chr(10)!r});sys.stderr.write({error!r});sys.exit(1)'

    def expect_error(self, code, reason, attempts=3):
        spawn, count = self.script([code])
        with patch.object(api.subprocess, 'Popen', spawn):
            with self.assertRaises(api.AccountingError) as caught:
                self.scan(attempts=attempts)
        self.assertEqual(caught.exception.reason, reason)
        self.assertTrue(all(x.get('reaped') for x in caught.exception.attempts))
        RESULTS.append({'test': self.id(), 'failure': caught.exception.receipt(), 'children': count[0]})
        return count[0], caught.exception

    def test_complete_fresh_retry_never_uses_failed_subtotal(self):
        spawn, count = self.script([self.missing(), self.success()])
        with patch.object(api.subprocess, 'Popen', spawn):
            result = self.scan()
        self.assertEqual(result['bytes'], 19 * 1024)
        self.assertEqual(count[0], 2)
        self.assertIn('heartbeat.next', result['attempts'][0]['stderr'])
        self.assertTrue(all(x['reaped'] for x in result['attempts']))
        RESULTS.append({'test': self.id(), 'sample': result})

    def test_continuous_disappearance_is_bounded_and_fails_closed(self):
        count, error = self.expect_error(self.missing(), 'allocation incomplete after bounded retries')
        self.assertEqual(count, 3)
        self.assertFalse(error.receipt()['complete'])

    def test_permission_error_not_retried(self):
        count, _ = self.expect_error("import sys;sys.stderr.write('du: permission denied\\n');sys.exit(1)", 'allocation nontransient error')
        self.assertEqual(count, 1)

    def test_mixed_error_not_retried(self):
        count, _ = self.expect_error(self.missing(extra='du: other: Permission denied\n'), 'allocation nontransient error')
        self.assertEqual(count, 1)

    def test_nonowned_enoent_not_retried(self):
        code = "import sys;sys.stderr.write('du: /other/root/file: No such file or directory\\n');sys.exit(1)"
        self.assertEqual(self.expect_error(code, 'allocation nontransient error')[0], 1)

    def test_root_enoent_not_retried(self):
        self.assertEqual(self.expect_error(self.missing(suffix=''), 'allocation nontransient error')[0], 1)

    def test_wrong_exit_not_retried(self):
        self.assertEqual(self.expect_error(self.missing().replace('sys.exit(1)', 'sys.exit(2)'), 'allocation nontransient error')[0], 1)

    def test_malformed_success_fails_closed(self):
        self.expect_error("print('not an allocation')", 'allocation output invalid')

    def test_diagnostic_overflow_is_bounded_and_fails_closed(self):
        _,error=self.expect_error("import sys;sys.stderr.write('x'*100000)", 'allocation diagnostics overflow')
        self.assertLessEqual(len(error.attempts[0]['stderr']),65536)
        self.assertTrue(error.attempts[0]['diagnosticsTruncated'])

    def test_timeout_reaps_actual_child(self):
        spawn, _ = self.script(['import time;time.sleep(3)'])
        with patch.object(api.subprocess, 'Popen', spawn):
            with self.assertRaises(api.AccountingError) as caught:
                self.scan(timeout=.06)
        self.assertEqual(caught.exception.reason, 'allocation subprocess timeout')
        self.assertTrue(caught.exception.attempts[0]['reaped'])
        RESULTS.append({'test': self.id(), 'failure': caught.exception.receipt()})

    def test_work_deadline_is_not_renewed_by_retry(self):
        spawn, _ = self.script([self.missing()])
        start = time.monotonic()
        with patch.object(api.subprocess, 'Popen', spawn):
            with self.assertRaises(api.AccountingError) as caught:
                api.complete_allocated(self.identity, deadline=start + .04,
                    checkpoint=lambda: None, retry_delay=.05)
        self.assertEqual(caught.exception.reason, 'scan deadline')
        self.assertLess(time.monotonic() - start, .6)
        RESULTS.append({'test': self.id(), 'failure': caught.exception.receipt()})

    def test_checkpoint_can_stop_during_live_du_child(self):
        spawn, _ = self.script(['import time;time.sleep(3)'])
        observations = []
        def checkpoint():
            observations.append(time.monotonic())
            if len(observations) >= 3:
                raise RuntimeError('actual live limit reached')
        with patch.object(api.subprocess, 'Popen', spawn):
            with self.assertRaises(api.AccountingError) as caught:
                api.complete_allocated(self.identity, deadline=time.monotonic()+1,
                                       checkpoint=checkpoint)
        self.assertIn('actual live limit', caught.exception.attempts[0]['cause'])
        self.assertTrue(caught.exception.attempts[0]['reaped'])
        RESULTS.append({'test': self.id(), 'failure': caught.exception.receipt()})

    def test_replaced_root_fails_identity_check(self):
        self.root.rename(self.base/'old')
        self.root.mkdir()
        with self.assertRaises(api.AccountingError) as caught:
            self.scan()
        self.assertIn('identity changed', caught.exception.attempts[0]['cause'])

    def test_root_symlink_is_rejected(self):
        link = self.base/'link'
        link.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            api.RootIdentity.capture(link)

    def test_real_du_does_not_follow_child_symlink_or_double_count_hardlink(self):
        (self.root/'data').write_bytes(os.urandom(131072))
        before = self.scan()['bytes']
        os.link(self.root/'data', self.root/'hardlink')
        (self.base/'external').write_bytes(os.urandom(262144))
        (self.root/'symlink').symlink_to(self.base/'external')
        after = self.scan()['bytes']
        self.assertLessEqual(after, before + 16384)
        RESULTS.append({'test': self.id(), 'before': before, 'after': after})

    def test_actual_atomic_heartbeat_churn_keeps_complete_samples(self):
        stop = threading.Event()
        def writer():
            while not stop.is_set():
                (self.root/'heartbeat.next').write_bytes(b'current')
                (self.root/'heartbeat.next').replace(self.root/'heartbeat')
                (self.root/'retired').write_bytes(b'old')
                (self.root/'retired').unlink()
                stop.wait(.004)
        thread = threading.Thread(target=writer)
        thread.start()
        try:
            samples = [self.scan() for _ in range(30)]
        finally:
            stop.set();thread.join(timeout=1)
        self.assertFalse(thread.is_alive())
        self.assertTrue(all(x['complete'] for x in samples))
        RESULTS.append({'test': self.id(), 'actualDuSamples': samples})

    def test_nonshrinking_carry_and_positive_growth(self):
        carry = api.Carry({'target':100, 'outside':50}, retained_new=20,
                          prior_whole=200, prior_new=30, reservation=8)
        first = carry.observe({'target':{'complete':True,'bytes':140},'outside':{'complete':True,'bytes':50}})
        second = carry.observe({'target':{'complete':True,'bytes':100},'outside':{'complete':True,'bytes':80}})
        self.assertEqual(first['new'],68)
        self.assertEqual(second['new'],98)
        self.assertGreaterEqual(second['whole'],200)
        with self.assertRaises(ValueError):
            carry.observe({'target':{'complete':False,'bytes':1},'outside':{'complete':True,'bytes':0}})
        with self.assertRaises(ValueError):
            carry.observe({'target':{'complete':'incomplete','bytes':1},'outside':{'complete':True,'bytes':0}})
        with self.assertRaises(ValueError):
            api.Carry({'root':-1},0,0,0)

    def union(self, **options):
        return api.collect({'owned':self.identity}, deadline=time.monotonic()+2,
                           checkpoint=lambda: None, **options)

    def test_union_deduplicates_overlaps_hardlinks_and_pinned_alias(self):
        nested = self.root/'nested'
        nested.mkdir()
        data = nested/'data'
        data.write_bytes(os.urandom(131072))
        os.link(data, self.root/'hardlink')
        link = self.base/'readonly'
        link.symlink_to(self.root, target_is_directory=True)
        alias = api.RootIdentity.capture(link, read_only_target=self.root)
        roots = {'owned':self.identity, 'nested':api.RootIdentity.capture(nested), 'alias':alias}
        sample = api.collect(roots, deadline=time.monotonic()+2, checkpoint=lambda:None)
        expected = sum(x.lstat().st_blocks*512 for x in (self.root,nested,data))
        self.assertEqual(sample['bytes'],expected)
        self.assertEqual(sample['scopes']['nested']['bytes'],0)
        self.assertEqual(sample['scopes']['alias']['bytes'],0)
        RESULTS.append({'test':self.id(),'sample':sample})

    def test_pinned_link_replacement_fails_closed(self):
        link = self.base/'readonly'
        link.symlink_to(self.root, target_is_directory=True)
        identity = api.RootIdentity.capture(link, read_only_target=self.root)
        other = self.base/'other'
        other.mkdir()
        link.unlink();link.symlink_to(other, target_is_directory=True)
        with self.assertRaises(api.AccountingError):
            api.collect({'alias':identity},deadline=time.monotonic()+2,checkpoint=lambda:None)

    def test_native_actual_child_removal_retains_subtotal_then_fresh_retry(self):
        data = self.root/'vanishing'
        data.write_bytes(b'old')
        real_stat = api.os.stat
        removed = []
        def disappearing(path,*args,**kwargs):
            if path == 'vanishing' and not removed:
                data.unlink();removed.append(True)
            return real_stat(path,*args,**kwargs)
        with patch.object(api.os,'stat',disappearing):
            sample = self.union()
        self.assertEqual(len(sample['attempts']),2)
        self.assertFalse(sample['attempts'][0]['complete'])
        self.assertEqual(sample['attempts'][0]['races'][0]['errno'],errno.ENOENT)
        self.assertEqual(sample['bytes'],self.root.stat().st_blocks*512)
        RESULTS.append({'test':self.id(),'sample':sample})

    def test_native_persistent_enoent_retains_known_bytes_fails_closed(self):
        (self.root/'vanishing').write_bytes(b'old')
        real_stat = api.os.stat
        def missing(path,*args,**kwargs):
            if path == 'vanishing':
                raise FileNotFoundError(errno.ENOENT,'controlled persistent ENOENT',path)
            return real_stat(path,*args,**kwargs)
        with patch.object(api.os,'stat',missing):
            with self.assertRaises(api.AccountingError) as caught:
                self.union()
        self.assertEqual(len(caught.exception.attempts),3)
        self.assertTrue(all(not x['complete'] for x in caught.exception.attempts))
        RESULTS.append({'test':self.id(),'failure':caught.exception.receipt()})

    def test_native_permission_io_unknown_errors_never_retry(self):
        (self.root/'data').write_bytes(b'old')
        real_stat = api.os.stat
        for code in (errno.EACCES,errno.EIO,errno.EINVAL):
            def fault(path,*args,**kwargs):
                if path == 'data':
                    raise OSError(code,'controlled fault',path)
                return real_stat(path,*args,**kwargs)
            with patch.object(api.os,'stat',fault):
                with self.assertRaises(api.AccountingError) as caught:
                    self.union()
            self.assertEqual(len(caught.exception.attempts),1)
            self.assertEqual(caught.exception.attempts[0]['cause']['errno'],code)
            RESULTS.append({'test':self.id(),'failure':caught.exception.receipt()})

    def test_native_entry_bound_and_deadline_never_admit_partial(self):
        (self.root/'data').write_bytes(b'old')
        with self.assertRaises(api.AccountingError) as caught:
            self.union(max_entries=1)
        self.assertEqual(caught.exception.reason,'scan entry bound')
        with self.assertRaises(api.AccountingError) as caught:
            api.collect({'owned':self.identity},deadline=time.monotonic()-1,checkpoint=lambda:None)
        self.assertEqual(caught.exception.reason,'scan deadline')

    def test_native_actual_churn_complete_samples(self):
        stop = threading.Event()
        faults = []
        def writer():
            try:
                while not stop.is_set():
                    (self.root/'heartbeat.next').write_bytes(b'current')
                    (self.root/'heartbeat.next').replace(self.root/'heartbeat')
                    (self.root/'retired').write_bytes(b'old')
                    (self.root/'retired').unlink()
                    stop.wait(.004)
            except Exception as error:
                faults.append(repr(error))
        thread=threading.Thread(target=writer)
        thread.start()
        try:
            samples=[self.union(attempts=5) for _ in range(30)]
        finally:
            stop.set();thread.join(timeout=1)
        self.assertFalse(thread.is_alive())
        self.assertFalse(faults)
        self.assertTrue(all(x['complete'] for x in samples))
        RESULTS.append({'test':self.id(),'nativeSamples':samples})

    def test_actual_resource_boundary_stops_and_reaps_scan_child(self):
        for reason in ('free','new','whole','processes','rss','deadline'):
            # Each fault is a real small observation against a deliberately low
            # test cap. No disk filling, huge RSS allocation or process storm.
            (self.root/'growth').write_bytes(os.urandom(8192))
            sample=self.union()
            proc=REAL_POPEN(['/bin/ps','-o','rss=','-p',str(os.getpid())],stdout=subprocess.PIPE)
            raw,_=proc.communicate(timeout=1)
            rss=int(raw.strip())*1024
            free=os.statvfs(self.root).f_bavail*os.statvfs(self.root).f_frsize
            actual={'free':free,'new':sample['bytes'],'whole':sample['bytes'],
                    'processes':2,'rss':rss}
            caps={'free_floor':0, 'new':2**40,'whole':2**40,'processes':1000,'rss':2**40}
            if reason == 'free': caps['free_floor']=free+1
            elif reason != 'deadline': caps[reason]=actual[reason]-1
            calls=[]
            spawn,_=self.script(['import time;time.sleep(3)'])
            def checkpoint():
                calls.append(time.monotonic())
                if len(calls)>1:
                    proc=REAL_POPEN(['/bin/ps','-axo','pid=,ppid=,rss='],stdout=subprocess.PIPE)
                    output,_=proc.communicate(timeout=1)
                    rows=[tuple(map(int,line.split())) for line in output.splitlines()]
                    owned={os.getpid()}
                    while True:
                        expanded=owned|{pid for pid,ppid,_ in rows if ppid in owned}
                        if expanded==owned: break
                        owned=expanded
                    actual['processes']=sum(pid in owned for pid,_,_ in rows)
                    actual['rss']=sum(size*1024 for pid,_,size in rows if pid in owned)
                    space=os.statvfs(self.root)
                    actual['free']=space.f_bavail*space.f_frsize
                    if reason=='free': caps['free_floor']=actual['free']+1
                    elif reason!='deadline': caps[reason]=actual[reason]-1
                    why=api.limit_reason(actual,caps,now=time.monotonic(),
                        deadline=0 if reason=='deadline' else time.monotonic()+1)
                    if why: raise RuntimeError('boundary '+why)
            with patch.object(api.subprocess,'Popen',spawn):
                with self.assertRaises(api.AccountingError) as caught:
                    api.complete_allocated(self.identity,deadline=time.monotonic()+2,checkpoint=checkpoint)
            row=caught.exception.attempts[0]
            self.assertIn('boundary '+reason,row['cause'])
            self.assertTrue(row['reaped'])
            RESULTS.append({'test':self.id(),'boundary':reason,'sample':actual,'caps':caps,
                            'failure':caught.exception.receipt()})


if __name__ == '__main__':
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(AccountingChecks)
    outcome = unittest.TextTestRunner(verbosity=2).run(suite)
    destination = Path(os.environ['RCR_OUTPUT'])
    destination.write_text(json.dumps({'status':'PASS' if outcome.wasSuccessful() else 'FAIL',
        'testsRun':outcome.testsRun,'failures':[str(x) for x in outcome.failures],
        'errors':[str(x) for x in outcome.errors],'observations':RESULTS},indent=2)+'\n')
    raise SystemExit(0 if outcome.wasSuccessful() else 1)
