import os,sys,time,json,hashlib,subprocess,signal,fcntl
from pathlib import Path
import membership as m
P=Path(__file__).resolve().parent;ROOT=P.parents[4];OUT=P/'source14';INNER=len(sys.argv)>1 and sys.argv[1]=='inner';T=float(os.environ['PI10_ACCEPT']);WORK=T+90;HARD=T+120;ROLE='inner' if INNER else 'outer';LOG=OUT/(ROLE+'.jsonl');GIB=1024**3
BASE=json.loads((OUT/'baseline.json').read_text());OLD=304807936;CAP=305070080;RSSLIMIT=536870912;WHOLEMAX=8589934592;PEAK={'processes':0,'rssBytes':0,'newBytes':OLD,'wholeBytes':6739521536};SEEN=set();CHILD=None;FAIL=None;CODE=1
IDENTITY=m.identity(os.getpid(),Path(__file__).resolve());IDENTITY['scriptSHA256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();D=json.loads(m.OWN.read_text())
if INNER:D['guardControllers']=[IDENTITY]
else:D.update(kind='source',controller=IDENTITY,phase='GUARDED_CHANGED_SOURCE_VALIDATION',remainingNewUpperBytes=262144)
m.publish(D)
def emit(x):
 with LOG.open('a') as stream:stream.write(json.dumps(x)+'\n')
 print(json.dumps(x),flush=True)
def filecharge(q):
 try:
  if q.is_symlink() or not q.is_file():return 0
  s=q.stat();return max(s.st_size,s.st_blocks*512)
 except FileNotFoundError:return 0
with (OUT/'sampling.lock').open('a') as lock:
 fcntl.flock(lock,fcntl.LOCK_EX)
 git=Path(subprocess.check_output(['git','rev-parse','--absolute-git-dir'],text=True,cwd=ROOT).strip());common=Path(subprocess.check_output(['git','rev-parse','--path-format=absolute','--git-common-dir'],text=True,cwd=ROOT).strip())
ROOTS=[ROOT,git,common,Path('/Users/lhs/Desktop/mcr/frontend/node_modules'),Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64'),Path('/Applications/Google Chrome.app'),Path('/Library/Frameworks/Python.framework/Versions/3.14'),Path('/Users/lhs/.codex/visualizations/2026/11/03/01a1a26e-1de2-7b41-a294-c9a79cfded9b'),m.OWN]
def whole():
 seen=set();total=0
 for root in ROOTS:
  assert root.exists()
  for q in root.rglob('*') if root.is_dir() and not root.is_symlink() else [root]:
   try:
    if q.is_symlink() or not q.is_file():continue
    s=q.stat();key=(s.st_dev,s.st_ino)
    if key not in seen:seen.add(key);total+=max(s.st_size,s.st_blocks*512)
   except FileNotFoundError:pass
 return max(6739521536,total)
def check(initial=False):
 with (OUT/'sampling.lock').open('a') as lock:
  fcntl.flock(lock,fcntl.LOCK_EX)
  readset=P/'qualification-readset.json';assert hashlib.sha256(readset.read_bytes()).hexdigest()==os.environ['PI11_READSET_SHA'],'readset hash changed'
  frozen=json.loads(readset.read_text())
  for row in frozen['harness']+frozen['implementation']+frozen['readOnlyCompatibility']+frozen['executionPins']+frozen.get('retainedChangedTests',[]):
   assert hashlib.sha256((ROOT/row['path']).read_bytes()).hexdigest()==row['sha256'],'pinned input changed: '+row['path']
  allocation=json.loads(Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/resource-allocation-current.json').read_text());other=allocation['conservativeApplicableFutureUpperBytes']-allocation['sourceBrowserRemainingConservativeUpperBytes']['PI11'];os.environ['MCR_PI_OTHER_NEW']=str(other)
  sample=m.sample();rows=m.processes();roots={IDENTITY['pid']}
  d=json.loads(m.OWN.read_text());roots.update(c['pid'] for c in [d.get('controller')]+d.get('guardControllers',[]) if c)
  previous=None
  while previous!=roots:previous=set(roots);roots.update(pid for pid,row in rows.items() if row['parent'] in roots)
  live=roots.intersection(rows);SEEN.update(live);rss=sum(rows[pid]['rss'] for pid in live)
  files=[q for q in P.rglob('*') if q.is_file()]+[m.OWN,ROOT/'docs/mcr/frontend/parade-ink-map-places.md'];new=OLD+16384+sum(max(0,filecharge(q)-BASE.get(str(q),0)) for q in files)
  v=os.statvfs(ROOT);free=v.f_bavail*v.f_frsize;counters={'processes':len(live),'rssBytes':rss,'newBytes':new,'wholeBytes':whole()}
  for k,v in counters.items():PEAK[k]=max(PEAK[k],v)
  assert time.time()<WORK,'work deadline';assert len(live)<=4 and rss<=RSSLIMIT,'actual source PID/RSS cap';assert new<=CAP and counters['wholeBytes']<=WHOLEMAX,'actual source NEW/WHOLE cap';assert free>=8*GIB and free-other-CAP>=9*GIB,'live/all-future floor'
  if initial:assert free>=11*GIB
  emit(dict(type='LIVE' if initial else 'SAMPLE',atEpoch=time.time(),counters=counters,freeBytes=free,otherFutureBytes=other,membership=sample if initial else None))
  return counters
try:
 check(True)
 if INNER:
  (OUT/'inner-live.json').write_text(json.dumps({'pid':os.getpid(),'atEpoch':time.time()}))
  while not (OUT/'outer-live.json').exists():check();time.sleep(.1)
  node='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
  commands=[[sys.executable,'-B',str(P/'source14-case.py')],[node,'/Users/lhs/Desktop/mcr/frontend/node_modules/oxlint/bin/oxlint','--deny-warnings',str(P/'native.mjs')]]
  for i,argv in enumerate(commands):
   with (OUT/('command-%02d.log'%i)).open('w') as log:
    CHILD=subprocess.Popen(argv,cwd=ROOT/'frontend',stdout=log,stderr=subprocess.STDOUT)
    while CHILD.poll() is None:check();time.sleep(.2)
    emit(dict(type='COMMAND',index=i,argv=argv,exitCode=CHILD.returncode,finishedAtEpoch=time.time()));assert CHILD.returncode==0,'changed source check failed'
  CODE=0
 else:
  CHILD=subprocess.Popen([sys.executable,'-B',str(Path(__file__).resolve()),'inner'],cwd=ROOT,start_new_session=True)
  while not (OUT/'inner-live.json').exists():
   assert CHILD.poll() is None,'inner guard stopped';check();time.sleep(.1)
  check(True);(OUT/'outer-live.json').write_text(json.dumps({'pid':os.getpid(),'atEpoch':time.time()}))
  while CHILD.poll() is None:check();time.sleep(.2)
  CODE=CHILD.returncode;check()
except BaseException as error:FAIL=repr(error);CODE=1
finally:
 for owner,raw in m.CONSUMED_RAW.items():
  (OUT/'consumed-12cc-raw.json').write_bytes(raw)
  (OUT/'consumed-12cc-binding.json').write_text(json.dumps(dict(owner=owner,sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw),meaning='Exact bytes actually decoded by latest completed or failed census; not historical Source13 bytes')))
 if CHILD and CHILD.poll() is None:
  if INNER:CHILD.terminate()
  else:os.killpg(CHILD.pid,signal.SIGTERM)
  try:CHILD.wait(timeout=max(.1,min(3,HARD-time.time())))
  except subprocess.TimeoutExpired:
   if INNER:CHILD.kill()
   else:os.killpg(CHILD.pid,signal.SIGKILL)
   CHILD.wait(timeout=2)
 result=dict(type='FULL',state='FULL',offerId='PI11-DRIVER-LINT-SOURCE14-0720',acceptedAtEpoch=T,actualFullEpoch=time.time(),exitCode=CODE,failure=FAIL,peak=PEAK,resources=0,livePids=[])
 (OUT/(ROLE+'-full.json')).write_text(json.dumps(result,indent=2)+'\n');emit(result)
 if not INNER:
  d=json.loads(m.OWN.read_text());d.update(state='FULL',actualFullEpoch=result['actualFullEpoch'],remainingNewUpperBytes=0,resources=0,receiptPaths={'full':str(OUT/'outer-full.json')},retainedNewCarryBytes=max(CAP,PEAK['newBytes']));m.publish(d)
 assert time.time()<HARD
sys.exit(CODE)
