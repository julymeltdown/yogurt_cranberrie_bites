"""Owned finite-round utilities; each guard independently observes live membership."""
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time
import membership

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = Path(__file__).resolve().parent
GIB, MIB = 1024**3, 1024**2
ROOTS = [ROOT, Path('/Users/lhs/Desktop/mcr/.git'), Path('/Users/lhs/Desktop/mcr/frontend/node_modules'),
         Path('/Users/lhs/Desktop/mcr-tools'), Path('/Applications/Google Chrome.app'),
         Path('/Library/Frameworks/Python.framework'), membership.BASE]
MUTABLE = [ROOT/'frontend/src/features/paradise-city-map', ROOT/'frontend/src/app/venue-entry', ROOT/'frontend/e2e/venue-map',
           ROOT/'docs/mcr/frontend/paradise-city-map.md', EVIDENCE, membership.OWN]


def size(paths, required=False):
    seen, total = set(), 0
    for path in paths:
        if required:
            assert path.exists() and not path.is_symlink(), 'required custody root missing: '+str(path)
        for entry in path.rglob('*') if path.is_dir() and not path.is_symlink() else [path]:
            try:
                if not entry.is_file() or entry.is_symlink():
                    continue
                stat = entry.stat(); key = stat.st_dev, stat.st_ino
                if key not in seen:
                    seen.add(key); total += max(stat.st_size, stat.st_blocks*512)
            except FileNotFoundError:
                # Full historical MAX is retained. A required root itself may never disappear.
                assert not required or path.exists(), 'required custody root disappeared'
    return total


def free():
    value = os.statvfs(ROOT); return value.f_bavail*value.f_frsize


def dependencies():
    base = ROOT/'frontend/node_modules'
    assert base.is_dir() and not base.is_symlink(), 'existing real retained dependencies required'
    expected = json.loads((EVIDENCE/'retained-dependencies.json').read_text())
    rows = []
    for p in sorted(base.rglob('*')):
        if p.is_symlink(): rows.append(dict(path=p.relative_to(base).as_posix(), link=os.readlink(p)))
        elif p.is_file(): rows.append(dict(path=p.relative_to(base).as_posix(), bytes=p.stat().st_size, sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
    assert rows == expected, 'retained dependency drift'
    return len(rows)


def pins():
    readset = EVIDENCE/'readset.json'
    assert hashlib.sha256(readset.read_bytes()).hexdigest() == os.environ['MCR_VENU_READSET_SHA']
    for row in json.loads(readset.read_text())['pins']:
        assert hashlib.sha256((ROOT/row['path']).read_bytes()).hexdigest() == row['sha256'], 'read-set drift: '+row['path']


class Round:
    def __init__(self, role):
        self.role, self.mode = role, os.environ['MCR_VENU_MODE']
        self.lease = os.environ['MCR_VENU_LEASE']; self.out = Path(os.environ['MCR_VENU_OUTPUT']); self.tmp = Path(os.environ['MCR_VENU_TMP'])
        self.accepted = float(os.environ['MCR_VENU_ACCEPT_EPOCH'])
        self.work = self.accepted+int(os.environ['MCR_VENU_WORK_SECONDS']); self.hard = self.accepted+int(os.environ['MCR_VENU_FULL_SECONDS'])
        self.cap = (128 if self.mode == 'source' else int(os.environ.get('MCR_VENU_NATIVE_FAMILY_MIB', '200')))*MIB
        assert self.cap in ((128*MIB,) if self.mode == 'source' else (200*MIB,256*MIB,320*MIB,352*MIB,384*MIB,416*MIB,448*MIB))
        self.future = int(os.environ.get('MCR_VENU_FUTURE_NEW_UPPER', self.cap))
        assert 0 < self.future <= self.cap
        self.pids, self.rss = (8, int(2.5*GIB)) if self.mode == 'source' else (24, int(3.5*GIB))
        self.port = int(os.environ.get('MCR_VENU_PORT', '0'))
        self.log = EVIDENCE/(self.out.name+'-controller.jsonl') if role == 'controller' else self.out/'guardian.jsonl'
        self.seen = {}; self.peak = dict(processes=0, rssBytes=0, newBytes=0, tmpBytes=0, wholeBytes=6642925568)
        self.minimum = free(); self.carry = 6642925568; self.spent_tmp = 0; self.new_carry = 0

    def event(self, value):
        with self.log.open('a') as f: f.write(json.dumps(value)+'\n')
        print(json.dumps(value), flush=True)

    def resource(self):
        return dict(ownerThreadId=membership.OWNER, offerId=self.lease, state='ACCEPTED', acceptedAtEpoch=self.accepted,
                    workDeadlineEpoch=self.work, fullDeadlineEpoch=self.hard, kind='source' if self.mode=='source' else 'browser',
                    processCap=self.pids, rssCapBytes=self.rss, remainingNewUpperBytes=self.future,
                    controller=membership.identity(os.getpid(), EVIDENCE/'controller.py', self.out), guardControllers=[],
                    receiptPaths=dict(accept=str(EVIDENCE/(self.out.name+'-controller.jsonl')), full=str(EVIDENCE/(self.out.name+'-controller.jsonl'))))

    def port_closed(self):
        if self.mode != 'native': return True
        with socket.socket() as s:
            s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
            try: s.bind(('127.0.0.1',self.port)); s.listen(1); return True
            except OSError: return False

    def initialize(self):
        assert self.mode in ('source','native') and Path.cwd()==ROOT
        assert self.out.is_relative_to(EVIDENCE) and self.tmp.as_posix().startswith('/private/tmp/mcr-paradise-city-map-')
        assert self.hard-self.work>=60 and time.time()<self.work
        assert self.port_closed(), 'strict port occupied'
        if self.role=='controller': assert not self.out.exists() and not self.tmp.exists() and not self.log.exists()
        else: self.out.mkdir(); self.tmp.mkdir(mode=0o700)
        for name in ('controller.py','guardian.py'):
            assert hashlib.sha256((EVIDENCE/name).read_bytes()).hexdigest()==os.environ['MCR_VENU_'+name.split('.')[0].upper()+'_SHA']
        pins(); count = dependencies()
        prior, prior_new = 6642925568, 40919040
        for p in EVIDENCE.glob('*-controller.jsonl'):
            for line in p.read_text().splitlines():
                row=json.loads(line); prior=max(prior,row.get('peak',{}).get('wholeBytes',0)); prior_new=max(prior_new,row.get('peak',{}).get('newBytes',0))
                if row.get('type')=='FULL' and row.get('role')=='controller': self.spent_tmp+=row.get('peak',{}).get('tmpBytes',0)
        self.carry=max(prior,size(ROOTS,True)+34*MIB)
        self.new_carry=max(prior_new,34*MIB+size(MUTABLE)+self.spent_tmp,105992192 if self.mode=='native' else 40919040)
        if 'MCR_VENU_FUTURE_NEW_UPPER' in os.environ:
            assert self.future <= self.cap-self.new_carry, 'future reservation exceeds actual cumulative remainder'
        self.peak['wholeBytes']=self.carry
        assert self.carry+self.cap<=8*GIB, 'complete whole carry plus worst-case NEW'
        floor=int(os.environ['MCR_VENU_START_FLOOR']); other=int(os.environ['MCR_VENU_OTHER_NEW'])
        assert floor>=11*GIB and free()>=floor and free()-other-self.cap>=9*GIB
        self.event(dict(type='ACCEPT',role=self.role,lease=self.lease,at=time.time(),workDeadline=self.work,fullDeadline=self.hard,
                        cumulativeNewCeilingBytes=self.cap,futureNewUpperBytes=self.future,
                        carryBytes=self.carry,newCarryBytes=self.new_carry,priorSpentTemporaryBytes=self.spent_tmp,dependencyRecords=count,freeBytes=free(),otherNewUpperBytes=other))
        self.check(initial=True)

    def check(self, initial=False):
        sample = membership.sample()
        membership.publish_census(EVIDENCE/(self.out.name+'-controller-census.json') if self.role=='controller' else self.out/'guardian-census.json',sample)
        if initial or not hasattr(self,'owners') or self.owners != sample['owners']:
            self.event(dict(type='INITIAL_GLOBAL' if initial else 'GLOBAL',sample=sample))
        self.owners=sample['owners']
        rows=membership.processes(); receipt=json.loads(membership.OWN.read_text())
        roots={c['pid'] for c in [receipt.get('controller')]+receipt.get('guardControllers',[]) if c and c['pid'] in rows and rows[c['pid']]['birth']==' '.join(c['birth'].split())}
        # An observed exact child remains ours if the OS reparents it before exit.
        tree=set(roots)|{pid for pid,birth in self.seen.items() if pid in rows and rows[pid]['birth']==birth}
        while True:
            extra={pid for pid,row in rows.items() if row['parent'] in tree}-tree
            if not extra: break
            tree|=extra
        self.seen.update({pid:rows[pid]['birth'] for pid in tree if pid!=os.getpid()})
        temp=size([self.tmp]); counters=dict(processes=len(tree)+2,rssBytes=sum(rows[p]['rss'] for p in tree)+24*MIB,
            newBytes=max(self.new_carry,34*MIB+size(MUTABLE)+self.spent_tmp)+temp,tmpBytes=temp,wholeBytes=max(self.carry,size(ROOTS,True)+34*MIB)+temp)
        for key,value in counters.items(): self.peak[key]=max(self.peak[key],value)
        self.minimum=min(self.minimum,free())
        assert counters['processes']<=self.pids and counters['rssBytes']<=self.rss, 'own ALL PID/RSS cap'
        assert counters['newBytes']<=self.cap and counters['wholeBytes']<=8*GIB, 'NEW/WHOLE cap'
        assert free()>=8*GIB and time.time()<self.work, 'live floor/WORK deadline'

    def cleanup(self, child=None):
        signal.signal(signal.SIGTERM,signal.SIG_IGN)
        rows=membership.processes()
        own_controller=json.loads(membership.OWN.read_text()).get('controller',{}).get('pid')
        targets=[pid for pid,birth in self.seen.items() if pid in rows and rows[pid]['birth']==birth and pid not in (os.getpid(),own_controller)]
        for pid in targets:
            try: os.kill(pid,signal.SIGTERM)
            except ProcessLookupError: pass
        # Let the inner guard finish its own cleanup and dependency receipt.
        # Reap our direct child before checking final process membership.
        deadline=time.monotonic()+20
        while time.monotonic()<deadline:
            if child is not None: child.poll()
            current=membership.processes()
            if not any(pid in current and current[pid]['birth']==self.seen[pid] for pid in targets): break
            time.sleep(.1)
        rows=membership.processes()
        for pid in targets:
            if pid in rows and rows[pid]['birth']==self.seen[pid]:
                try: os.kill(pid,signal.SIGKILL)
                except ProcessLookupError: pass
        if child is not None: child.wait(timeout=5)

    def release(self, code, failure, child=None):
        import shutil
        self.cleanup(child)
        if self.tmp.exists(): shutil.rmtree(self.tmp)
        preserved=dependencies()
        rows=membership.processes(); live=[p for p,b in self.seen.items() if p in rows and rows[p]['birth']==b and p!=json.loads(membership.OWN.read_text()).get('controller',{}).get('pid')]
        value=dict(type='FULL',role=self.role,lease=self.lease,at=time.time(),exitCode=code,failure=failure,peak=self.peak,minimumFreeBytes=self.minimum,
                   livePids=live,resources=len(live)+(0 if self.port_closed() else 1),privateTmpAbsent=not self.tmp.exists(),portClosed=self.port_closed(),retainedDependencyRecords=preserved)
        if self.role=='guardian':
            self.out.mkdir(exist_ok=True)
            (self.out/'release.json').write_text(json.dumps(value,indent=2)+'\n')
        self.event(value)
        if self.role=='controller':
            resource=json.loads(membership.OWN.read_text()); resource.update(state='FULL',actualFullEpoch=time.time(),remainingNewUpperBytes=0,resources=value['resources'],actual=value); membership.publish(resource)
        return value
