"""PI08 finite owner job; independent pre-child guard and immutable FULL receipt."""
from pathlib import Path
import argparse, hashlib, json, os, signal, subprocess, sys, time, shutil
REPO=Path(__file__).resolve().parents[4]
sys.path.insert(0,str(REPO/'scripts/mcr/resource-accounting'))
import accounting
import global_census


def write(path,data):
    temporary=path.with_suffix('.next')
    temporary.write_text(json.dumps(data,indent=2)+'\n');temporary.replace(path)


def owned(parent):
    raw=subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,rss='],text=True,timeout=1)
    rows=[tuple(map(int,line.split())) for line in raw.splitlines()]
    ids={parent}
    while True:
        expanded=ids|{pid for pid,ppid,_ in rows if ppid in ids}
        if expanded==ids:break
        ids=expanded
    return {'pids':[pid for pid,_,_ in rows if pid in ids],
            'processes':sum(pid in ids for pid,_,_ in rows),
            'rss':sum(rss*1024 for pid,_,rss in rows if pid in ids)}


class Guard:
    def __init__(self,config):
        self.c=config;self.last=0;self.samples=0
        self.roots={key:accounting.RootIdentity(**{**value,'link_identity':tuple(value['link_identity']) if value.get('link_identity') is not None else None}) for key,value in config['roots'].items()}
        self.ledger=accounting.Carry(config['baseline'],config['retainedNew'],config['priorWhole'],config['retainedNew'],8*1024**2)
        self.peak={key:0 for key in ('new','whole','processes','rss')};self.minimum=None;self.observed=set()
        self.globalLast=0;self.globalPeak={'processes':0,'rss':0,'workflows':0};self.globalValue=None
        self.censusSelftest=global_census.selftest()
    def live(self,force=False):
        now=time.monotonic()
        if now>=self.c['deadline']:raise RuntimeError('fixed work deadline')
        if not force and now-self.last<.025:return
        value=owned(self.c['parent']);self.observed.update(value['pids']);self.last=time.monotonic()
        if force or self.last-self.globalLast>=.25:
            self.globalValue=global_census.sample();self.globalLast=time.monotonic()
            role='primary' if os.getpid()==self.c['parent'] else 'guardian'
            write(Path(self.c['runDir'])/('global-'+role+'.json'),{'epoch':time.time(),'samplerPID':os.getpid(),**self.globalValue})
            for key in self.globalPeak:self.globalPeak[key]=max(self.globalPeak[key],self.globalValue[key])
        s=os.statvfs(REPO);value.update(free=s.f_bavail*s.f_frsize,new=self.ledger.prior_new,whole=self.ledger.prior_whole)
        for key in self.peak:self.peak[key]=max(self.peak[key],value[key])
        self.minimum=value['free'] if self.minimum is None else min(self.minimum,value['free'])
        why=accounting.limit_reason(value,self.c['caps'],now=time.monotonic(),deadline=self.c['deadline'])
        if why:raise RuntimeError(why)
    def sample(self):
        self.live(True)
        snap=accounting.collect(self.roots,deadline=min(self.c['deadline'],time.monotonic()+4),checkpoint=self.live,attempts=5)
        self.ledger.observe(snap['scopes']);self.samples+=1;self.live(True)
    def receipt(self,error):return {'failure':error,'samples':self.samples,'peaks':self.peak,'minimumFree':self.minimum,'globalCensusSelftest':self.censusSelftest,'globalPeaks':self.globalPeak,'lastGlobal':self.globalValue,'observedPIDs':sorted(self.observed)}


def guardian(path):
    c=json.loads(path.read_text());run=path.parent;g=Guard(c);failure=None
    try:
        g.sample();write(run/'guardian-ready.json',g.receipt(None))
        while not (run/'stop').exists():g.sample();time.sleep(.05)
    except BaseException as error:
        failure=str(error)
        if isinstance(error,accounting.AccountingError):write(run/'guardian-scan-failure.json',error.receipt())
        write(run/'guardian-failure.json',g.receipt(failure))
        try:os.kill(c['parent'],signal.SIGTERM)
        except ProcessLookupError:pass
    finally:write(run/'guardian-FULL.json',g.receipt(failure))
    return 1 if failure else 0


def verify_pins(path,expected):
    data=Path(path).read_bytes()
    if hashlib.sha256(data).hexdigest()!=expected:raise RuntimeError('frozen readset manifest changed')
    manifest=json.loads(data)
    for row in manifest['files']:
        candidate=Path(row['path'])
        if hashlib.sha256(candidate.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('frozen source/input changed: '+str(candidate))
    return manifest


def main(a):
    run=Path(a.run_dir).absolute();run.mkdir(exist_ok=False)
    private=Path(a.scratch).absolute();private.mkdir(exist_ok=False)
    write(private/'ownership.json',{'pid':os.getpid(),'offer':a.offer})
    accept=time.time();start=time.monotonic();end=start+a.work
    birth=subprocess.check_output(['/bin/ps','-p',str(os.getpid()),'-o','lstart='],text=True).strip()
    write(run/'ACCEPT.json',{'offer':a.offer,'acceptedEpoch':accept,'workEpoch':accept+a.work,'fullEpoch':accept+a.work+60,
      'parentPID':os.getpid(),'parentBirth':birth,'kind':a.kind,'root':str(REPO),'private':str(private),
      'caps':{'processes':a.pids,'rss':a.rss,'new':a.new_cap,'whole':a.whole},'pinsSha256':a.pins_sha256})
    owner_record={'ownerThreadId':global_census.OWNER,'offerId':a.offer,'state':'ACCEPTED','acceptedAtEpoch':accept,
      'workDeadlineEpoch':accept+a.work,'fullDeadlineEpoch':accept+a.work+60,'kind':'browser' if a.kind=='native' else 'source',
      'processCap':a.pids,'rssCapBytes':a.rss,'remainingNewUpperBytes':max(0,a.new_cap-a.retained_new),
      'controller':{'pid':os.getpid(),'birth':birth,'executable':sys.executable,'script':__file__,'workingDirectory':str(REPO),'outputRoot':str(run)},
      'guardControllers':[],'receiptPaths':{'accept':str(run/'ACCEPT.json'),'full':str(run/'FULL.json')}}
    write(global_census.DIRECTORY/(global_census.OWNER+'.json'),owner_record)
    pins=verify_pins(a.pins,a.pins_sha256)
    c={'offer':a.offer,'kind':a.kind,'parent':os.getpid(),'runDir':str(run),'acceptedEpoch':accept,'workEpoch':accept+a.work,
       'fullEpoch':accept+a.work+60,'deadline':end,'retainedNew':a.retained_new,'priorWhole':a.prior_whole,
       'pins':a.pins,'pinsSha256':a.pins_sha256,'frozen':pins,
       'caps':{'new':a.new_cap,'whole':a.whole,'processes':a.pids,'rss':a.rss,'free_floor':8*1024**3}}
    roots={'checkout':REPO,'git':Path('/Users/lhs/Desktop/mcr/.git'),
           'dependencies':Path(a.dependencies)/'node_modules','foundation':Path(a.foundation),
           'node':Path(a.node).resolve().parent,'python':next(parent for parent in Path(sys.executable).resolve().parents if parent.name=='Python.framework'),'private':private,
           'ownerReceipts':global_census.DIRECTORY}
    if a.kind=='native':roots['chrome']=Path('/Applications/Google Chrome.app')
    identities={}
    for name,path in roots.items():
        identities[name]=accounting.RootIdentity.capture(path,**({'read_only_target':str(path.resolve())} if path.is_symlink() else {}))
    c['roots']={name:identity.__dict__ for name,identity in identities.items()}
    last=[0]
    def baseline_live():
        if time.monotonic()>=end:raise RuntimeError('work deadline during baseline')
        if time.monotonic()-last[0]<.025:return
        s=os.statvfs(REPO);p=owned(os.getpid());last[0]=time.monotonic()
        if s.f_bavail*s.f_frsize<8*1024**3 or p['processes']>a.pids or p['rss']>a.rss:raise RuntimeError('baseline live resource cap')
    s=os.statvfs(REPO);free=s.f_bavail*s.f_frsize
    remaining=max(0,a.new_cap-a.retained_new)
    if free<(12 if a.kind=='native' else 11)*1024**3 or free-remaining-a.other<9*1024**3:raise RuntimeError('fresh source start/after-ALL admission floor')
    c['admission']={'free':free,'remainingNew':remaining,'otherOwners':a.other,'afterAll':free-remaining-a.other}
    snap=accounting.collect(identities,deadline=min(end,start+8),checkpoint=baseline_live,attempts=5)
    c['baseline']={name:value['bytes'] for name,value in snap['scopes'].items()}
    write(run/'config.json',c);g=Guard(c)
    child=watch=None;failure=None;code=None;link=REPO/'frontend/node_modules'
    def interrupted(*_):raise RuntimeError('signal or independent guard')
    signal.signal(signal.SIGTERM,interrupted);signal.signal(signal.SIGINT,interrupted)
    try:
        assert not link.exists() and not link.is_symlink()
        g.sample();watch=subprocess.Popen([sys.executable,__file__,'--guardian',str(run/'config.json')],start_new_session=True)
        while not (run/'guardian-ready.json').exists():
            g.live(True)
            if watch.poll() is not None:raise RuntimeError('guardian not ready')
            time.sleep(.025)
        guardian_birth=subprocess.check_output(['/bin/ps','-p',str(watch.pid),'-o','lstart='],text=True).strip()
        owner_record['guardControllers']=[{'pid':watch.pid,'birth':guardian_birth,'executable':sys.executable,'script':__file__}]
        write(global_census.DIRECTORY/(global_census.OWNER+'.json'),owner_record)
        link.symlink_to(roots['dependencies'],target_is_directory=True)
        env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','MCR_FRONTEND_DEPENDENCIES':a.dependencies,
             'PI08_FOUNDATION':a.foundation,'PI08_OUTPUT':str(run/'output'),'PI08_CACHE':str(private/'cache'),
             'PI08_SCRATCH':str(private),'TMPDIR':str(private)}
        entry=REPO/'frontend/src/features/parade-ink/world-hud'/('source.mjs' if a.kind=='source' else 'native.mjs')
        with (run/'job.log').open('wb') as log:
            child=subprocess.Popen([a.node,str(entry)],cwd=REPO/'frontend',env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            while child.poll() is None:
                g.sample()
                if watch.poll() is not None:raise RuntimeError('independent guard stopped')
                time.sleep(.025)
            code=child.wait(timeout=1)
        if code:raise RuntimeError('validation exit '+str(code))
        g.sample();verify_pins(a.pins,a.pins_sha256)
    except BaseException as error:
        failure=str(error)
        if isinstance(error,accounting.AccountingError):write(run/'scan-failure.json',error.receipt())
    finally:
        # Cleanup must survive a concurrent failure signal from the other guard.
        signal.signal(signal.SIGTERM,signal.SIG_IGN);signal.signal(signal.SIGINT,signal.SIG_IGN)
        if child is not None and child.poll() is None:
            os.killpg(child.pid,signal.SIGTERM)
            try:child.wait(timeout=.5)
            except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=1)
        (run/'stop').write_text('stop\n')
        if watch is not None:
            try:watch.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(watch.pid,signal.SIGTERM)
                try:watch.wait(timeout=.5)
                except subprocess.TimeoutExpired:os.killpg(watch.pid,signal.SIGKILL);watch.wait(timeout=1)
            if watch.returncode:failure=failure or 'guardian failed'
        # Remove only this run's regenerable scratch after all guarded processes stop.
        shutil.rmtree(private)
        if link.is_symlink() and os.readlink(link)==str(roots['dependencies']):link.unlink()
        if link.exists() or link.is_symlink():failure=failure or 'dependency link not released'
        remaining=[]
        # Include observed descendants that might have been reparented by Chrome.
        for pid in set(owned(os.getpid())['pids'])|g.observed:
            if pid==os.getpid():continue
            try:os.kill(pid,0);remaining.append(pid)
            except ProcessLookupError:pass
        # A just-terminated child can remain observable while the OS reaps it.
        reaping_deadline=min(time.time()+1,c['fullEpoch'])
        while remaining and time.time()<reaping_deadline:
            time.sleep(.025)
            alive=[]
            for pid in remaining:
                try:os.kill(pid,0);alive.append(pid)
                except ProcessLookupError:pass
            remaining=alive
        if remaining:failure=failure or 'owned children remain'
        if time.time()>c['fullEpoch']:failure=failure or 'FULL deadline'
        released=not remaining and not link.exists() and not link.is_symlink() and not private.exists()
        if not released:
            write(run/'cleanup-detail.json',{'remainingPIDs':remaining,'primary':g.receipt(failure),'privateScratchAbsent':not private.exists(),'dependencyLinkAbsent':not link.exists() and not link.is_symlink()})
            raise RuntimeError('cleanup unverified; retain accepted reservation')
        write(run/'FULL.json',{'offer':a.offer,'status':'FAIL' if failure else 'PASS','failure':failure,
          'actualFullEpoch':time.time(),'workflow':0,'remainingReservation':0,'ownedChildren':remaining,
          'dependencyLinkAbsent':not link.exists() and not link.is_symlink(),'testExit':code,
          'primary':g.receipt(failure),'guardianExit':None if watch is None else watch.returncode,
          'withinFullDeadline':time.time()<=c['fullEpoch'],'privateScratchAbsent':not private.exists(),'scratchRetained':'none; raw evidence retained inside owned checkout'})
        owner_record.update(state='FULL',actualFullEpoch=time.time(),remainingNewUpperBytes=0)
        write(global_census.DIRECTORY/(global_census.OWNER+'.json'),owner_record)
    return 1 if failure else 0


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--guardian');p.add_argument('--run-dir');p.add_argument('--offer')
    p.add_argument('--kind',choices=['source','native']);p.add_argument('--node');p.add_argument('--dependencies');p.add_argument('--foundation')
    p.add_argument('--prior-whole',type=int);p.add_argument('--retained-new',type=int);p.add_argument('--new-cap',type=int);p.add_argument('--pids',type=int)
    p.add_argument('--scratch');p.add_argument('--other',type=int,default=5*1024**3);p.add_argument('--pins');p.add_argument('--pins-sha256');p.add_argument('--whole',type=int,default=8*1024**3);p.add_argument('--rss',type=int,default=4*1024**3);p.add_argument('--work',type=int,default=180);a=p.parse_args()
    if a.guardian:raise SystemExit(guardian(Path(a.guardian)))
    try:result=main(a)
    except Exception as error:
        # An outer exception cannot prove that a started workload was released.
        # Preserve ACCEPT until an explicit verified correction, never synthesize FULL.
        run=Path(a.run_dir)
        if run.is_dir():
            write(run/'cleanup-unverified.json',{'offer':a.offer,'status':'FAIL','failure':repr(error),
              'observedEpoch':time.time(),'phase':'outer exception; release unverified','remainingReservation':'ACCEPT remains binding'})
        raise
    raise SystemExit(result)
