"""Finite source controls; real outer accounting, synthetic decoder inputs."""
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
BOOK_PATH = O / 'source-budget-640-amendment.json'
BOOK = json.loads(BOOK_PATH.read_text()) if BOOK_PATH.exists() else {'shared_new_bytes':16*1024**2,'prior_uncertified_reservation_bytes':0}
CAP = BOOK['shared_new_bytes']
GIB = 1024**3
CUMULATIVE_CARRY_FLOOR = 547319808  # retained545222656 plus entire ordinary2MiB upper
WHOLE_CARRY_FLOOR = 58987859968
ACCEPT_BY = 1793852100  # distinct SOURCE07/08 accept-by04:15Z
PREP_BY = 1793852100


def put(path, value):
    if path == INDEX / (OWNER + '.json'):
        if path.exists() and 'admissionDisposition' not in value:
            pending=json.loads(path.read_text()).get('admissionDisposition')
            if pending and value.get('offerId')!=pending.get('offerId'):
                value['admissionDisposition']=pending
        incident = O / 'source02-ended-launch-identity.json'
        if incident.exists():
            value['historicalLaunchIdentityObservations'] = [json.loads(incident.read_text())]
    budget.write(path.parent, path.name, value)


def actual():
    d = budget.identity(os.getpid(), O / 'source_control.py', O)
    d.update(cwd=os.getcwd(), OScomm=subprocess.check_output(['/bin/ps', '-p', str(os.getpid()), '-o', 'comm='], text=True).strip(), actualInvocation=list(sys.argv))
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
        if (O/'source07-retained-inputs.json').exists():
            frozen=json.loads((O/'source07-retained-inputs.json').read_text())
            for entry in frozen['retainedMaps']:
                assert budget.digest(entry['path'])==entry['SHA256'], 'retained higher map changed'
        for p in O.glob('source-high-*.json.gz'):
            with gzip.open(p, 'rt') as f:
                for dev, ino, size in json.load(f): self.high[(dev, ino)] = max(self.high.get((dev, ino), 0), size)
        self.last = 0; self.sampled = {}; self.peak = {}

    def verify_pins(self):
        for path, sha in self.a.get('immutable_file_pins', {}).items():
            assert budget.digest(path) == sha, 'immutable source changed: ' + path

    def fast(self):
        run=Path(self.a['receiptPaths']['accept']).parent if 'receiptPaths' in self.a else None
        cleanup=run is not None and (run/'cleanup').exists()
        stop=self.a['fullDeadlineEpoch'] if cleanup else self.a.get('workDeadlineEpoch',self.a['fullDeadlineEpoch'])
        assert time.time() < stop - 5, 'fixed source WORK/FULL deadline'
        if time.time() - self.last > .3:
            self.verify_pins()
            self.sampled, free = metadata({**self.a, 'output_directory': str(O), 'controller_pid': self.a['controller']['pid'], 'global_roster_path': str(O / 'global-roster.json')})
            self.last = time.time()
            ordinary=self.sampled.get('other_accepted_ordinary_remaining_new_bytes',0)
            booked=self.sampled['other_accepted_remaining_new_bytes']-ordinary+max(ordinary,int(2.5*GIB),self.sampled.get('unquantified_ordinary_envelope_bytes',0))
            assert free - booked - CAP >= 9*GIB, 'ALL current bookings residual floor'
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


def bootstrap_guard(boot):
    # Publish the actual current invocation before waiting. The same process
    # becomes the independent execution sampler after a real ACCEPT is supplied.
    put(boot/'identity.json', actual())
    end = min(PREP_BY, time.time()+100)
    while not (boot/'accepted-path.json').exists():
        if (boot/'stop').exists(): return
        assert time.time()<end, 'bounded bootstrap wait'
        time.sleep(.05)
    path=Path(json.loads((boot/'accepted-path.json').read_text())['path'])
    guardian(json.loads(path.read_text()), path.parent)


def ordinary_admission(number):
    """Same actual Python image publishes before complete census; no shell child."""
    old = json.loads((INDEX / (OWNER + '.json')).read_text())
    assert old['state'] in ['FULL', 'DECLINED'] and time.time() < PREP_BY
    epoch = time.time(); suffix = str(time.time_ns()); ident = actual()
    rec = {'ownerThreadId':OWNER,'offerId':'POST-CONTROL-INPROCESS-PREP-'+suffix,'state':'ACCEPTED','kind':'ordinary','acceptedAtEpoch':epoch,'workDeadlineEpoch':PREP_BY,'fullDeadlineEpoch':None,'processCap':None,'rssCapBytes':None,'remainingNewUpperBytes':CAP,'controller':ident,'guardControllers':[],'receiptPaths':{'accept':str(O/('inprocess-'+suffix+'-accept.json')),'full':str(O/('inprocess-'+suffix+'-FULL.json'))},'closedHistory':history(old),'authorization':'Explicit SAME cumulative640MiB through04:15; lightweight source admission only, no preaccept filesystem census or fixture payload'}
    put(Path(rec['receiptPaths']['accept']),rec);put(INDEX/(OWNER+'.json'),rec)
    failure = None; guard = None; handed_off = False
    boot=O/('source-bootstrap-'+number+'-'+suffix)
    try:
        boot.mkdir()
        guard=subprocess.Popen([sys.executable,'-B',str(O/'source_control.py'),'guard-bootstrap',str(boot)],start_new_session=True)
        end=min(PREP_BY,time.time()+10)
        while not (boot/'identity.json').exists():
            assert guard.poll() is None and time.time()<end, 'bootstrap identity publication'
            time.sleep(.05)
        guard_ident=json.loads((boot/'identity.json').read_text())
        rec['guardControllers']=[guard_ident]
        rec['bootstrapDirectory']=str(boot)
        put(Path(rec['receiptPaths']['accept']),rec);put(INDEX/(OWNER+'.json'),rec)
        # Only lightweight admission metadata here. Complete filesystem
        # samplers run after this distinct execution's actual acceptance.
        sample,free=metadata({'controller_pid':os.getpid(),'output_directory':str(O),'global_roster_path':str(O/'global-roster.json')})
        ordinary=sample.get('other_accepted_ordinary_remaining_new_bytes',0)
        booked=sample['other_accepted_remaining_new_bytes']-ordinary+max(ordinary,int(2.5*GIB),sample.get('unquantified_ordinary_envelope_bytes',0))
        free = os.statvfs(O); free = free.f_bavail * free.f_frsize
        assert free >= 11*GIB and free-booked-CAP>=9*GIB
        result = {'state':'PROCESS_METADATA_ONLY_NOT_FILESYSTEM_OR_RUNTIME_QUALIFICATION','complete_process_admission':sample,'free':free,'epoch':time.time(),'sample':sample,'target_access':'NONE','minimumNEW':CUMULATIVE_CARRY_FLOOR,'minimumWHOLE':WHOLE_CARRY_FLOOR,'completeFilesystemCensus':'DEFERRED_UNTIL_ACTUAL_ACCEPT'}
        put(O/'source-admission.json',result)
        handed_off=True
        return result, guard, boot, guard_ident
    except BaseException as error:
        failure=type(error).__name__+': '+str(error)
        put(O/('inprocess-'+suffix+'-failure.json'),{'failure':failure,'traceback':traceback.format_exc(),'epoch':time.time()})
        raise
    finally:
        if not handed_off:
            if guard:
                (boot/'stop').write_text(str(time.time()))
                try: guard.wait(timeout=3)
                except subprocess.TimeoutExpired: guard.terminate();guard.wait(timeout=3)
            full={'state':'FULL_ORDINARY_METADATA_CLOSED','ownerThreadId':OWNER,'offerId':rec['offerId'],'acceptedAtEpoch':epoch,'actualFullEpoch':time.time(),'resourcesReleased':True,'resources':{'workflows':0,'guards':0,'children':0,'tests':0,'target':0},'remainingNewUpperBytes':0,'failure':failure}
            put(Path(rec['receiptPaths']['full']),full);rec.update(state='FULL',actualFullEpoch=full['actualFullEpoch'],remainingNewUpperBytes=0,resourcesReleased=True);put(INDEX/(OWNER+'.json'),rec)


def controller(number):
    assert number in ['07','08'], 'closed rounds never restart; distinct grant required'
    admission,guard,boot,guard_ident=ordinary_admission(number)
    try:
        return accepted_controller(number,admission,guard,boot,guard_ident)
    finally:
        if guard.poll() is None:
            (boot/'stop').write_text(str(time.time()))
            try: guard.wait(timeout=3)
            except subprocess.TimeoutExpired: guard.terminate();guard.wait(timeout=3)
        current=json.loads((INDEX/(OWNER+'.json')).read_text())
        if current['state']=='ACCEPTED' and current['kind']=='ordinary':
            full={'state':'FULL_ORDINARY_PREACCEPT_REFUSED','ownerThreadId':OWNER,'offerId':current['offerId'],'acceptedAtEpoch':current['acceptedAtEpoch'],'actualFullEpoch':time.time(),'resourcesReleased':True,'resources':{'workflows':0,'guards':0,'children':0,'tests':0,'target':0},'remainingNewUpperBytes':0}
            put(Path(current['receiptPaths']['full']),full);current.update(state='FULL',actualFullEpoch=full['actualFullEpoch'],resourcesReleased=True,remainingNewUpperBytes=0);put(INDEX/(OWNER+'.json'),current)


def current_publication_order(offer):
    if offer in ['POST-CONTROL-SOURCE07-0350','POST-CONTROL-SOURCE08-0350']:
        path=INDEX/('01a1a153-1227-7e72-801f-a2a6b91981e1.json')
        raw=path.read_bytes();record=json.loads(raw)
        expected='ROOT153-L-REGISTRATION-SOURCE03-0343'
        epoch=record.get('acceptedAtEpoch')
        actual=record.get('offerId')==expected and record.get('state') in ['ACCEPTED','FULL'] and type(epoch) in [int,float] and 0<epoch<=time.time()
        disposition=record.get('admissionDisposition',{})
        yielded=disposition.get('offerId')==expected and disposition.get('state')=='UNREADY_YIELD' and disposition.get('resourcesHeld') is False
        assert actual or yielded, 'direct allocator predecessor RootL03 unresolved'
        if offer.endswith('08-0350'):
            prior=json.loads((O/'source-run-07/FULL.json').read_text())
            assert prior.get('resourcesReleased') is True and prior.get('offerId')=='POST-CONTROL-SOURCE07-0350','repair requires literal prior FULL'
            assert json.loads((O/'source-run-07/controller-result.json').read_text())['state']=='FAILED','08 actual repair only'
        return {'directAllocatorAmendment':'0350','predecessor':expected,'actualAcceptedEpoch':epoch if actual else None,'unreadyYield':yielded,**budget.retain_owner_index(O,raw)}
    path=INDEX.parent/'resource-admission-current.json'
    raw=path.read_bytes();order=json.loads(raw);entries=order['sequence']
    matches=[i for i,e in enumerate(entries) if e['owner']==OWNER and e['offer']==offer]
    assert len(matches)==1, 'current offer absent from publication order'
    position=matches[0]
    own=json.loads((INDEX/(OWNER+'.json')).read_text())
    yielded=own.get('admissionDisposition',{}).get('state')=='UNREADY_YIELD' and own['admissionDisposition'].get('offerId')==offer
    required=entries[max(0,position-1):position]
    if yielded:required=required+entries[position+1:]
    facts=[]
    for entry in required:
        record_path=INDEX/(entry['owner']+'.json');record_raw=record_path.read_bytes();record=json.loads(record_raw)
        epoch=record.get('acceptedAtEpoch')
        actual=record.get('offerId')==entry['offer'] and record.get('state') in ['ACCEPTED','FULL'] and type(epoch) in [int,float] and 0<epoch<=time.time()
        disposition=record.get('admissionDisposition',{})
        unready=disposition.get('offerId')==entry['offer'] and disposition.get('state')=='UNREADY_YIELD' and disposition.get('resourcesHeld') is False
        assert actual or unready, 'publication predecessor/ready-first attempt not resolved: '+entry['offer']
        facts.append({'owner':entry['owner'],'offer':entry['offer'],'actualAcceptedEpoch':epoch if actual else None,'unreadyYield':unready,**budget.retain_owner_index(O,record_raw)})
    return {'path':str(path),'SHA256':hashlib.sha256(raw).hexdigest(),'position':position+1,'afterYieldOtherFirstAttemptsChecked':yielded,'facts':facts}


def accepted_controller(number,admission,guard,boot,guard_ident):
    run = O / ('source-run-' + number)
    old = json.loads((INDEX / (OWNER + '.json')).read_text()); assert old['state']=='ACCEPTED' and old['kind']=='ordinary' and old['bootstrapDirectory']==str(boot)
    assert time.time() < ACCEPT_BY
    pins = json.loads((O / 'source-pins.json').read_text())
    # Prospective owned controls may be refreshed before this round's ACCEPT.
    # Every other original pin, including the fixed accounting baseline, stays exact.
    for name in ['source_control.py', 'budget.py', 'test_process_receipts.py']:
        pins[str(O / name)] = budget.digest(O / name)
    pins[str(O / 'source-admission.json')] = budget.digest(O / 'source-admission.json')
    for path, sha in pins.items(): assert budget.digest(path) == sha, 'source pin changed: ' + path
    pins[str(O/'source-admission.json')] = budget.digest(O/'source-admission.json')
    put(O/'source-pins.json',pins)
    admission=json.loads((O/'source-admission.json').read_text())
    assert admission['state']=='PROCESS_METADATA_ONLY_NOT_FILESYSTEM_OR_RUNTIME_QUALIFICATION'
    assert time.time()-admission['epoch']<120,'fresh complete source admission required'
    assert admission['free']>=11*GIB
    routing=current_publication_order('POST-CONTROL-SOURCE'+number+'-0350')
    # Refresh the actual full reservation census immediately before publication;
    # routing facts are never treated as a resource release.
    current_sample,free=metadata({'controller_pid':os.getpid(),'output_directory':str(O),'global_roster_path':str(O/'global-roster.json')})
    ordinary=current_sample.get('other_accepted_ordinary_remaining_new_bytes',0)
    booked=current_sample['other_accepted_remaining_new_bytes']-ordinary+max(ordinary,int(2.5*GIB),current_sample.get('unquantified_ordinary_envelope_bytes',0))
    assert free>=11*GIB and free-booked-CAP>=9*GIB, 'fresh preaccept capacity'
    run.mkdir(exist_ok=False)
    now = time.time(); a = {'ownerThreadId': OWNER, 'offerId': 'POST-CONTROL-SOURCE' + number + '-0350', 'state': 'ACCEPTED', 'kind': 'source', 'acceptedAtEpoch': now, 'workDeadlineEpoch': now+120, 'fullDeadlineEpoch': now+180, 'processCap': 4, 'rssCapBytes': GIB, 'remainingNewUpperBytes': CAP, 'controller': actual(), 'guardControllers': [], 'receiptPaths': {'accept': str(run/'accept.json'), 'full': str(run/'FULL.json')}, 'closedHistory': history(old), 'immutable_file_pins': pins, 'minimum_cumulative_carry_bytes':CUMULATIVE_CARRY_FLOOR,'minimum_whole_highwater_bytes':WHOLE_CARRY_FLOOR}
    assert guard.poll() is None, 'bootstrap guardian must remain live'
    a['guardControllers']=[guard_ident]
    a['admissionRouting']=routing
    a['ordinaryAdmissionTransition']={'acceptPath':old['receiptPaths']['accept'],'acceptSHA256':budget.digest(old['receiptPaths']['accept']),'semantics':'Same actual controller and guardian change role; ordinary admission is not a literal resource release or a second execution acceptance; same cumulative family.'}
    put(run/'accept.json', a); put(INDEX/(OWNER+'.json'), a)
    print(json.dumps({'event':'ACTUAL_ACCEPT','offer':a['offerId'],'acceptedAtEpoch':now,'workDeadlineEpoch':a['workDeadlineEpoch'],'fullDeadlineEpoch':a['fullDeadlineEpoch'],'controller':a['controller']}),flush=True)
    def interrupted(_s, _f): raise RuntimeError('finite SOURCE interrupted')
    signal.signal(signal.SIGTERM, interrupted)
    failure = None; cleanup_failure = None; test_code = None
    try:
        put(boot/'accepted-path.json',{'path':str(run/'accept.json')})
        end = min(time.time()+12, a['workDeadlineEpoch'])
        while not (run/'guard-identity.json').exists():
            assert guard.poll() is None and time.time() < end; time.sleep(.05)
        a['guardControllers'] = [json.loads((run/'guard-identity.json').read_text())]
        put(run/'accept-identities.json', a); put(INDEX/(OWNER+'.json'), a); (run/'identities-published').write_text(str(time.time()))
        controls = run / 'executed-controls'; controls.mkdir()
        for filename, sha in pins.items():
            path = Path(filename)
            if path.parent == O and path.name != 'source-baseline.json.gz':
                data = path.read_bytes()
                assert hashlib.sha256(data).hexdigest() == sha
                (controls / path.name).write_bytes(data)
        put(controls / 'manifest.json', {'pins':pins,'fixed_baseline_not_copied':True})
        g = SourceGuard(a)
        end = min(time.time()+50, a['workDeadlineEpoch']-20)
        while not (run/'guard-live.json').exists():
            assert guard.poll() is None and time.time() < end; g.fast(); time.sleep(.1)
        live = json.loads((run/'guard-live.json').read_text()); assert live['complete'] and time.time()-live['epoch'] < 40
        put(run/'controller-live.json', g.sample())
        assert guard.poll() is None and not(run/'guard-failure.json').exists() and time.time() < a['workDeadlineEpoch']-10
        saved = sys.argv
        g.verify_pins()
        with (run/'fixtures.stdout').open('w') as so, (run/'fixtures.stderr').open('w') as se, contextlib.redirect_stdout(so), contextlib.redirect_stderr(se):
            sys.argv = [str(O/'test_process_receipts.py')]
            try: runpy.run_path(str(O/'test_process_receipts.py'), run_name='__main__'); test_code = 0
            except SystemExit as e: test_code = e.code or 0
            finally: sys.argv = saved
        assert test_code == 0, 'synthetic production decoder fixture failures'
        g.verify_pins()
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
    elif sys.argv[1] == 'guard-bootstrap': bootstrap_guard(Path(sys.argv[2]))
    else: sys.exit(controller(sys.argv[1]))
