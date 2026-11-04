"""Finite dual-guard SOURCE qualification. No product/build/dependency activity."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import time

REPO=Path(__file__).resolve().parents[4]
HELPER=REPO/'scripts/mcr/resource-accounting'
sys.path.insert(0,str(HELPER))
import accounting as api


def write(path,value):
    temporary=path.with_suffix('.next')
    temporary.write_text(json.dumps(value,indent=2)+'\n')
    temporary.replace(path)


def processes(parent):
    raw=subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,rss='],timeout=1,text=True)
    rows=[tuple(map(int,line.split())) for line in raw.splitlines()]
    owned={parent}
    while True:
        expanded=owned|{pid for pid,ppid,_ in rows if ppid in owned}
        if expanded==owned: break
        owned=expanded
    return {'pids':sorted(pid for pid,_,_ in rows if pid in owned),
            'processes':sum(pid in owned for pid,_,_ in rows),
            'rss':sum(rss*1024 for pid,_,rss in rows if pid in owned)}


class Guard:
    def __init__(self,config):
        self.config=config
        self.roots={key:api.RootIdentity(**value) for key,value in config['roots'].items()}
        self.carry=api.Carry(config['baseline'],config['retainedNew'],config['priorWhole'],
                             config['retainedNew'],config['reservation'])
        self.peak={key:0 for key in ('new','whole','processes','rss')}
        self.minimum_free=None
        self.last=0
        self.samples=0

    def live(self,force=False):
        now=time.monotonic()
        if now>=self.config['deadline']: raise RuntimeError('deadline')
        if not force and now-self.last<.025: return
        sample=processes(self.config['parent'])
        self.last=time.monotonic()
        space=os.statvfs(REPO)
        sample.update(free=space.f_bavail*space.f_frsize,new=self.carry.prior_new,whole=self.carry.prior_whole)
        self.minimum_free=sample['free'] if self.minimum_free is None else min(self.minimum_free,sample['free'])
        for key in self.peak: self.peak[key]=max(self.peak[key],sample[key])
        why=api.limit_reason(sample,self.config['caps'],now=now,deadline=self.config['deadline'])
        if why: raise RuntimeError(why)
        return sample

    def sample(self):
        self.live(True)
        sample=api.collect(self.roots,deadline=min(self.config['deadline'],time.monotonic()+4),checkpoint=self.live,attempts=5)
        self.carry.observe(sample['scopes'])
        self.samples+=1
        self.live(True)

    def receipt(self,failure):
        return {'failure':failure,'samples':self.samples,'peak':self.peak,'minimumFree':self.minimum_free}


def independent(config_path):
    config=json.loads(config_path.read_text())
    run=config_path.parent
    guard=Guard(config)
    failure=None
    try:
        guard.sample()
        write(run/'guardian-ready.json',guard.receipt(None))
        while not (run/'stop').exists():
            guard.sample()
            time.sleep(.05)
    except BaseException as error:
        failure=str(error)
        write(run/'guardian-failure.json',{'error':failure})
        try: os.kill(config['parent'],signal.SIGTERM)
        except ProcessLookupError: pass
    finally:
        write(run/'guardian-full.json',guard.receipt(failure))
    return 1 if failure else 0


def main(args):
    run=Path(args.run_dir).absolute()
    run.mkdir(exist_ok=False)
    started=time.monotonic()
    work_deadline=started+args.work_seconds
    full_deadline=work_deadline+args.cleanup_seconds
    config={'offer':args.offer,'parent':os.getpid(),'acceptedEpoch':time.time(),
            'deadline':work_deadline,'fullDeadline':full_deadline,
            'retainedNew':41158065,'priorWhole':1901942193,'reservation':args.reservation,
            'caps':{'free_floor':8*1024**3,'new':args.new_cap,'whole':args.whole_cap,
                    'processes':args.pid_cap,'rss':args.rss_cap}}
    roots={'checkout':api.RootIdentity.capture(REPO),
           'git':api.RootIdentity.capture('/Users/lhs/Desktop/mcr/.git'),
           'private':api.RootIdentity.capture(run)}
    config['roots']={key:value.__dict__ for key,value in roots.items()}
    pins={str(path.relative_to(REPO)):hashlib.sha256(path.read_bytes()).hexdigest()
          for path in sorted(HELPER.glob('*.py'))+sorted(Path(__file__).parent.glob('*.py'))}
    config['pins']=pins
    config['source']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip()
    # Baseline collection is itself bounded with live process/free/deadline checks.
    def initial_checkpoint():
        now=time.monotonic()
        if now>=work_deadline: raise RuntimeError('deadline during baseline')
        space=os.statvfs(REPO)
        live=processes(os.getpid())
        if space.f_bavail*space.f_frsize<config['caps']['free_floor']: raise RuntimeError('free during baseline')
        if live['processes']>args.pid_cap or live['rss']>args.rss_cap: raise RuntimeError('process/RSS during baseline')
    last=[0]
    def bounded_initial():
        if time.monotonic()-last[0]>.025:
            initial_checkpoint();last[0]=time.monotonic()
    baseline=api.collect(roots,deadline=min(work_deadline,started+8),checkpoint=bounded_initial,attempts=5)
    config['baseline']={key:value['bytes'] for key,value in baseline['scopes'].items()}
    write(run/'config.json',config)
    guard=Guard(config)
    guardian=child=None
    failure=None
    code=None
    def interrupted(*_): raise RuntimeError('signal or independent guard')
    signal.signal(signal.SIGTERM,interrupted)
    signal.signal(signal.SIGINT,interrupted)
    try:
        guard.sample()
        guardian=subprocess.Popen([sys.executable,__file__,'--guardian',str(run/'config.json')],start_new_session=True)
        while not (run/'guardian-ready.json').exists():
            guard.live(True)
            if guardian.poll() is not None: raise RuntimeError('guardian failed before child')
            time.sleep(.025)
        env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','PYTHONPATH':str(HELPER),
             'RCR_TEMP':str(run),'RCR_OUTPUT':str(run/'tests.json'),'TMPDIR':str(run)}
        with (run/'tests.log').open('wb') as log:
            child=subprocess.Popen([sys.executable,str(Path(__file__).with_name('test_accounting.py'))],
                stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
            while child.poll() is None:
                guard.sample()
                if guardian.poll() is not None: raise RuntimeError('independent guard stopped')
                time.sleep(.025)
            code=child.wait(timeout=1)
        if code: raise RuntimeError('functional tests exit '+str(code))
        guard.sample()
        for name,digest in pins.items():
            if hashlib.sha256((REPO/name).read_bytes()).hexdigest()!=digest: raise RuntimeError('source readset changed')
    except BaseException as error:
        failure=str(error)
        if isinstance(error,api.AccountingError): write(run/'scan-failure.json',error.receipt())
    finally:
        if child is not None and child.poll() is None:
            os.killpg(child.pid,signal.SIGTERM)
            try: child.wait(timeout=.5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=1)
        (run/'stop').write_text('stop\n')
        if guardian is not None:
            try: guardian.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(guardian.pid,signal.SIGTERM)
                try: guardian.wait(timeout=.5)
                except subprocess.TimeoutExpired:
                    os.killpg(guardian.pid,signal.SIGKILL);guardian.wait(timeout=1)
            if guardian.returncode: failure=failure or 'guardian exit '+str(guardian.returncode)
        remaining=[pid for pid in processes(os.getpid())['pids'] if pid!=os.getpid()]
        # ps in the measurement has already exited and been reaped.
        alive=[]
        for pid in remaining:
            try: os.kill(pid,0);alive.append(pid)
            except ProcessLookupError: pass
        if alive: failure=failure or 'remaining owned processes'
        if time.monotonic()>full_deadline: failure=failure or 'FULL deadline'
        write(run/'FULL.json',{'status':'FAIL' if failure else 'PASS','failure':failure,
            'offer':args.offer,'source':config['source'],'actualFullEpoch':time.time(),
            'workflow':0,'remainingNewReservation':0,'ownedChildren':alive,'testExit':code,
            'primary':guard.receipt(failure),'guardianExit':None if guardian is None else guardian.returncode,
            'withinFullDeadline':time.monotonic()<=full_deadline})
    return 1 if failure else 0


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--guardian')
    parser.add_argument('--run-dir')
    parser.add_argument('--offer')
    parser.add_argument('--work-seconds',type=float,default=120)
    parser.add_argument('--cleanup-seconds',type=float,default=60)
    parser.add_argument('--new-cap',type=int,default=64*1024**2)
    parser.add_argument('--whole-cap',type=int,default=4*1024**3)
    parser.add_argument('--pid-cap',type=int,default=16)
    parser.add_argument('--rss-cap',type=int,default=1024**3)
    parser.add_argument('--reservation',type=int,default=4*1024**2)
    args=parser.parse_args()
    if args.guardian:
        raise SystemExit(independent(Path(args.guardian)))
    try:
        result=main(args)
    except Exception as error:
        # Failures before guardian creation (including baseline/custody failure)
        # also preserve a truthful failed receipt instead of disappearing.
        run=Path(args.run_dir)
        if run.is_dir() and not (run/'FULL.json').exists():
            failure={'status':'FAIL','failure':repr(error),'offer':args.offer,
                     'actualFullEpoch':time.time(),'workflow':0,
                     'remainingNewReservation':0,'ownedChildren':[],
                     'phase':'baseline/pre-child; no workload admitted'}
            if isinstance(error,api.AccountingError): failure['accounting']=error.receipt()
            write(run/'FULL.json',failure)
        raise
    raise SystemExit(result)
