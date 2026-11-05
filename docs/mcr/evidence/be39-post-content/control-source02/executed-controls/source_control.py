"""Finite SOURCE01/02 controls; real outer accounting, synthetic decoder inputs."""
import contextlib
import gzip
import hashlib
import json
import os
from pathlib import Path
import runpy
import signal
import subprocess
import sys
import time
import traceback
import accounting
import budget

O = Path(__file__).parent
OWNER = '01a1a21a-a088-7c31-83e3-19de119ba873'
INDEX = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')
BOOK_PATH = O / 'source-budget-amendment.json'
BOOK = json.loads(BOOK_PATH.read_text()) if BOOK_PATH.exists() else {'shared_new_bytes':16*1024**2,'prior_uncertified_reservation_bytes':0}
CAP = BOOK['shared_new_bytes']
GIB = 1024**3
ACCEPT_BY = 1793847600  # SOURCE02 extension 2026-11-05T03:00Z
PREP_BY = 1793847000  # ordinary preparation through02:50Z


def put(path, value):
    budget.write(path.parent, path.name, value)


def actual():
    d = budget.identity(os.getpid(), O / 'source_control.py', O)
    d.update(cwd=os.getcwd(), OScomm=subprocess.check_output(['/bin/ps', '-p', str(os.getpid()), '-o', 'comm='], text=True).strip())
    return d


def history(old):
    result = old.get('closedHistory', [])
    assert isinstance(result, list)
    if old['state'] == 'FULL' and old.get('kind') != 'ordinary':
        fp = old['receiptPaths']['full']
        full = json.loads(Path(fp).read_text())
        epoch = full.get('epoch', full.get('actualFullEpoch'))
        result = result + [{'state': 'FULL', 'ownerThreadId': OWNER, 'offerId': old['offerId'], 'acceptedAtEpoch': old['acceptedAtEpoch'], 'actualFullEpoch': epoch, 'resourcesReleased': True, 'fullPath': fp, 'fullSHA256': budget.digest(fp)}]
    return result


def metadata(a):
    try:
        sample = budget.processes({**a, 'process_cap': 4, 'rss_cap_bytes': GIB, 'kind': 'source'})
    except BaseException as error:
        detail = {'epoch':time.time(),'sampler_pid':os.getpid(),'failure':type(error).__name__+': '+str(error),'traceback':traceback.format_exc()}
        if hasattr(error,'receipt'):detail['accounting_receipt']=error.receipt()
        put(O/('source-global-failure-'+str(os.getpid())+'.json'),detail)
        raise
    free = os.statvfs(O); free = free.f_bavail * free.f_frsize
    assert free >= 8 * GIB, 'live free floor'
    return sample, free


def prepare():
    assert not(O/'source-baseline.json.gz').exists(),'fixed baseline never recaptured'
    assert time.time() < PREP_BY
    old = json.loads((INDEX / (OWNER + '.json')).read_text())
    assert old['state'] in ['FULL', 'DECLINED']
    epoch = time.time(); ident = actual(); suffix = str(time.time_ns())
    rec = {'ownerThreadId': OWNER, 'offerId': 'POST-CONTROL-ORDINARY-PREP-' + suffix, 'state': 'ACCEPTED', 'kind': 'ordinary', 'acceptedAtEpoch': epoch, 'workDeadlineEpoch': ACCEPT_BY, 'fullDeadlineEpoch': None, 'processCap': None, 'rssCapBytes': None, 'remainingNewUpperBytes': 64*1024**2, 'controller': ident, 'guardControllers': [], 'receiptPaths': {'accept': str(O / ('source-prep-' + suffix + '-accept.json')), 'full': str(O / ('source-prep-' + suffix + '-FULL.json'))}, 'closedHistory': history(old), 'authorization': 'Existing nonreset TOTAL64MiB ordinary preparation; no SOURCE ACCEPT or application workload'}
    put(Path(rec['receiptPaths']['accept']), rec); put(INDEX / (OWNER + '.json'), rec)
    failure = None
    try:
        prepared = json.loads((O / 'accounting-roots-prepared.json').read_text())
        roots = {k: v for k, v in prepared['directory_roots'].items() if k != 'target'}
        roots['evidence'] = str(O)
        roots['previous_compile_unit03_history'] = '/private/tmp/mcr-post21a-be39-compile-unit03-0111'
        identities = {k: accounting.RootIdentity.capture(v) for k, v in roots.items()}
        a = {'output_directory': str(O), 'controller_pid': os.getpid(), 'global_roster_path': str(O / 'global-roster.json')}
        last = 0; cached = None
        def check():
            nonlocal last, cached
            if time.time()-last > .3:
                cached = metadata(a); last = time.time()
            sample, free = cached
            assert time.time() < ACCEPT_BY
            assert free >= 11*GIB and free - sample['other_accepted_remaining_new_bytes'] - int(2.5*GIB) - CAP >= 9*GIB, 'fresh ALL bookings fit'
        initial = sum(p.stat().st_blocks * 512 for p in O.iterdir() if p.is_file())
        check()
        base = accounting.collect(identities, deadline=time.monotonic()+45, checkpoint=check, attempts=3)
        seen = {(dev, ino) for _, dev, ino, _ in base['inodes']}
        for path in prepared['file_roots']:
            st = os.lstat(path)
            if (st.st_dev, st.st_ino) not in seen:
                base['inodes'].append((path, st.st_dev, st.st_ino, st.st_blocks*512)); seen.add((st.st_dev, st.st_ino))
        whole = sum(size for _, _, _, size in base['inodes'])
        carry = max(43312521216, whole)
        assert carry + CAP <= BOOK.get('whole_limit_bytes',48*GIB)
        with gzip.open(O / 'source-baseline.json.gz', 'wt') as f: json.dump(base, f)
        plan = {'roots': roots, 'file_roots': prepared['file_roots'], 'initial_prepared_bytes': initial, 'measured_complete_union_bytes': whole, 'whole_carry_bytes': carry, 'baseline_epoch': time.time(), 'target_excluded_and_never_read': True}
        put(O / 'source-plan.json', plan)
        print(json.dumps({'state': 'SOURCE_BASELINE_PREPARED_NOT_ACCEPTED', **plan}), flush=True)
    except BaseException as e:
        failure = type(e).__name__ + ': ' + str(e)
        raise
    finally:
        full = {'state': 'FULL_ORDINARY_PREPARATION_CLOSED', 'actualFullEpoch': time.time(), 'acceptedAtEpoch': epoch, 'failure': failure, 'resourcesReleased': True, 'remainingNewUpperBytes': 0, 'execution_workflows': 0, 'target_access': 'NONE'}
        put(Path(rec['receiptPaths']['full']), full)
        rec.update(state='FULL', remainingNewUpperBytes=0, actualFullEpoch=full['actualFullEpoch'])
        put(INDEX / (OWNER + '.json'), rec)


class SourceGuard:
    def __init__(self, a):
        self.a = a
        self.plan = json.loads((O / 'source-plan.json').read_text())
        self.roots = {k: accounting.RootIdentity.capture(v) for k, v in self.plan['roots'].items()}
        with gzip.open(O / 'source-baseline.json.gz', 'rt') as f: base = json.load(f)
        self.baseline = {(dev, ino): size for _, dev, ino, size in base['inodes']}
        self.high = dict(self.baseline)
        for p in O.glob('source-high-*.json.gz'):
            with gzip.open(p, 'rt') as f:
                for dev, ino, size in json.load(f): self.high[(dev, ino)] = max(self.high.get((dev, ino), 0), size)
        self.last = 0; self.sampled = {}; self.peak = {}

    def fast(self):
        run=Path(self.a['receiptPaths']['accept']).parent if 'receiptPaths' in self.a else None
        cleanup=run is not None and (run/'cleanup').exists()
        stop=self.a['fullDeadlineEpoch'] if cleanup else self.a.get('workDeadlineEpoch',self.a['fullDeadlineEpoch'])
        assert time.time() < stop - 5, 'fixed source WORK/FULL deadline'
        for path, sha in self.a.get('immutable_file_pins', {}).items():
            assert budget.digest(path) == sha, 'immutable source changed: ' + path
        if time.time() - self.last > .3:
            self.sampled, free = metadata({**self.a, 'output_directory': str(O), 'controller_pid': self.a['controller']['pid'], 'global_roster_path': str(O / 'global-roster.json')})
            self.last = time.time()
            assert free - self.sampled['other_accepted_remaining_new_bytes'] - int(2.5*GIB) - CAP >= 9*GIB, 'ALL current bookings residual floor'
            self.peak['minimum_free'] = min(self.peak.get('minimum_free', free), free)
            for key in ['own_processes','own_rss','global_processes','global_rss','global_workflows']:
                self.peak[key] = max(self.peak.get(key, 0), self.sampled[key])
        return self.sampled

    def sample(self):
        self.fast()
        try:
            receipt = accounting.collect(self.roots, deadline=min(time.monotonic()+35, time.monotonic()+self.a['fullDeadlineEpoch']-time.time()-10), checkpoint=self.fast, attempts=3)
        except BaseException as error:
            detail={'epoch':time.time(),'sampler_pid':os.getpid(),'failure':type(error).__name__+': '+str(error),'traceback':traceback.format_exc()}
            if hasattr(error,'receipt'):detail['accounting_receipt']=error.receipt()
            put(O/('source-accounting-failure-'+str(os.getpid())+'.json'),detail)
            raise
        assert receipt['complete']
        for _, dev, ino, size in receipt['inodes']:
            self.high[(dev, ino)] = max(self.high.get((dev, ino), 0), size)
        for path in self.plan['file_roots']:
            st = os.lstat(path); key = (st.st_dev, st.st_ino); self.high[key] = max(self.high.get(key, 0), st.st_blocks*512)
        growth = BOOK.get('prior_uncertified_reservation_bytes',0) + self.plan['initial_prepared_bytes'] + sum(max(0, size-self.baseline.get(key, 0)) for key, size in self.high.items())
        self.peak['cumulative_new'] = max(BOOK.get('minimum_cumulative_carry_bytes',0),self.a.get('minimum_cumulative_carry_bytes',0),self.peak.get('cumulative_new',0),growth)
        self.peak['whole'] = max(self.a.get('minimum_whole_highwater_bytes',0),self.plan.get('minimum_whole_highwater_bytes',0),self.plan['whole_carry_bytes'] + self.peak['cumulative_new'])
        # Same inode is overwritten; the retained high map includes deleted inode maxima.
        state_path=O / ('source-high-' + str(os.getpid()) + '.json.gz')
        with gzip.open(state_path, 'wt') as f:
            json.dump([(dev, ino, size) for (dev, ino), size in self.high.items()], f)
        st=os.lstat(state_path);key=(st.st_dev,st.st_ino);self.high[key]=max(self.high.get(key,0),st.st_blocks*512)
        growth = BOOK.get('prior_uncertified_reservation_bytes',0) + self.plan['initial_prepared_bytes'] + sum(max(0,size-self.baseline.get(key,0)) for key,size in self.high.items())
        self.peak['cumulative_new']=max(BOOK.get('minimum_cumulative_carry_bytes',0),self.a.get('minimum_cumulative_carry_bytes',0),self.peak.get('cumulative_new',0),growth)
        self.peak['whole']=max(self.a.get('minimum_whole_highwater_bytes',0),self.plan.get('minimum_whole_highwater_bytes',0),self.plan['whole_carry_bytes']+self.peak['cumulative_new'])
        assert self.peak['cumulative_new'] <= CAP, 'shared cumulative SOURCE01/02 NEW cap exceeded'
        assert self.peak['whole'] <= BOOK.get('whole_limit_bytes',48*GIB), 'WHOLE ceiling exceeded'
        return {'epoch': time.time(), 'complete': True, 'actual_live_metadata': self.fast(), 'peak': self.peak, 'target_access': 'NONE'}


def guardian(a, run):
    put(run / 'guard-identity.json', actual())
    failure = None
    try:
        end = min(time.time()+10, a['workDeadlineEpoch'])
        while not (run / 'identities-published').exists():
            assert time.time() < end; time.sleep(.05)
        g = SourceGuard(a)
        put(run / 'guard-live.json', g.sample())
        last = time.time()
        while not (run / 'stop').exists():
            g.fast()
            if time.time()-last > 3:
                put(run / 'guard-live.json', g.sample()); last = time.time()
            time.sleep(.1)
    except BaseException as e:
        failure = type(e).__name__ + ': ' + str(e)
        put(run / 'guard-failure.json', {'failure': failure, 'epoch': time.time()})
        try: os.kill(a['controller']['pid'], signal.SIGTERM)
        except ProcessLookupError: pass
    finally:
        put(run / 'guard-exit.json', {'failure': failure, 'epoch': time.time(), 'peak': g.peak if 'g' in locals() else {}})


def controller(number):
    run = O / ('source-run-' + number)
    old = json.loads((INDEX / (OWNER + '.json')).read_text()); assert old['state'] in ['FULL','DECLINED']
    assert time.time() < ACCEPT_BY
    pins = json.loads((O / 'source-pins.json').read_text())
    for path, sha in pins.items(): assert budget.digest(path) == sha, 'source pin changed: ' + path
    admission=json.loads((O/'source-admission.json').read_text())
    assert admission['state']=='METADATA_ONLY_PASS_NOT_RUNTIME_QUALIFICATION' and admission['complete_source_admission']['complete']
    assert time.time()-admission['epoch']<120,'fresh complete source admission required'
    assert admission['free']>=11*GIB
    run.mkdir(exist_ok=False)
    now = time.time(); a = {'ownerThreadId': OWNER, 'offerId': 'POST-CONTROL-SOURCE' + number + '-0204', 'state': 'ACCEPTED', 'kind': 'source', 'acceptedAtEpoch': now, 'workDeadlineEpoch': now+120, 'fullDeadlineEpoch': now+180, 'processCap': 4, 'rssCapBytes': GIB, 'remainingNewUpperBytes': CAP, 'controller': actual(), 'guardControllers': [], 'receiptPaths': {'accept': str(run/'accept.json'), 'full': str(run/'FULL.json')}, 'closedHistory': history(old), 'immutable_file_pins': pins, 'minimum_cumulative_carry_bytes':admission['complete_source_admission']['peak']['cumulative_new'],'minimum_whole_highwater_bytes':admission['complete_source_admission']['peak']['whole']}
    put(run/'accept.json', a); put(INDEX/(OWNER+'.json'), a)
    def interrupted(_s, _f): raise RuntimeError('finite SOURCE interrupted')
    signal.signal(signal.SIGTERM, interrupted)
    guard = None; failure = None; cleanup_failure = None; test_code = None
    try:
        guard = subprocess.Popen([sys.executable, '-B', str(O/'source_control.py'), 'guard', str(run/'accept.json')], start_new_session=True)
        end = min(time.time()+12, a['workDeadlineEpoch'])
        while not (run/'guard-identity.json').exists():
            assert guard.poll() is None and time.time() < end; time.sleep(.05)
        a['guardControllers'] = [json.loads((run/'guard-identity.json').read_text())]
        put(run/'accept-identities.json', a); put(INDEX/(OWNER+'.json'), a); (run/'identities-published').write_text(str(time.time()))
        g = SourceGuard(a)
        end = min(time.time()+50, a['workDeadlineEpoch']-20)
        while not (run/'guard-live.json').exists():
            assert guard.poll() is None and time.time() < end; g.fast(); time.sleep(.1)
        live = json.loads((run/'guard-live.json').read_text()); assert live['complete'] and time.time()-live['epoch'] < 40
        put(run/'controller-live.json', g.sample())
        assert guard.poll() is None and not(run/'guard-failure.json').exists() and time.time() < a['workDeadlineEpoch']-10
        saved = sys.argv
        with (run/'fixtures.stdout').open('w') as so, (run/'fixtures.stderr').open('w') as se, contextlib.redirect_stdout(so), contextlib.redirect_stderr(se):
            sys.argv = [str(O/'test_process_receipts.py')]
            try: runpy.run_path(str(O/'test_process_receipts.py'), run_name='__main__'); test_code = 0
            except SystemExit as e: test_code = e.code or 0
            finally: sys.argv = saved
        assert test_code == 0, 'synthetic production decoder fixture failures'
        assert time.time() < a['workDeadlineEpoch']
        put(run/'controller-final-live.json', g.sample())
    except BaseException as e:
        failure = type(e).__name__ + ': ' + str(e)
    finally:
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        (run/'cleanup').write_text(str(time.time()))
        (run/'stop').write_text(str(time.time()))
        if guard:
            try: guard.wait(timeout=8)
            except subprocess.TimeoutExpired: guard.terminate(); guard.wait(timeout=5)
        if 'g' in locals():
            try: g.sample()
            except BaseException as e: cleanup_failure = type(e).__name__ + ': ' + str(e)
        result = {'state': 'PASS' if not failure and not cleanup_failure else 'FAILED', 'failure': failure, 'cleanup_failure': cleanup_failure, 'test_exit': test_code, 'fixture_inputs': 'SYNTHETIC_ONLY_ACTUAL_PRODUCTION_FUNCTION', 'outer_accounting': 'REAL_LIVE_NOT_MOCKED', 'guard_reaped': guard is None or guard.poll() is not None, 'peak': g.peak if 'g' in locals() else {}, 'target_Cargo_PG_browser': 'NOT_RUN', 'epoch': time.time()}
        if (run/'guard-exit.json').exists():
            result['guardian'] = json.loads((run/'guard-exit.json').read_text())
            if result['guardian']['failure']: result['state'] = 'FAILED'
        put(run/'controller-result.json', result); print(json.dumps(result), flush=True)
    return 0 if result['state'] == 'PASS' else 1


if __name__ == '__main__':
    if sys.argv[1] == 'prepare': prepare()
    elif sys.argv[1] == 'guard':
        p = Path(sys.argv[2]); guardian(json.loads(p.read_text()), p.parent)
    else: sys.exit(controller(sys.argv[1]))
