"""Primary finite source supervisor; frozen browser-guard.py is the separate guardian process."""
import datetime, hashlib, json, os, signal, subprocess, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
E=ROOT/'docs/mcr/evidence/signup-consent-admission'
r=json.loads((E/'world-source-ready.json').read_text())
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==r['head']
assert hashlib.sha256((ROOT/r['guard']).read_bytes()).hexdigest()==r['guardSha256']
assert not (ROOT/'frontend/node_modules').exists() and not (ROOT/'frontend/node_modules').is_symlink()
assert not Path(r['newScratchOnly']).exists() and not (ROOT/r['retainedOutput']).exists()
roots=[Path(p).resolve() for p in r['custodyRoots']]
def census():
 size,entries=208,4
 for root in roots:
  for base,dirs,files in os.walk(root,followlinks=False):
   for name in dirs+files:
    p=Path(base)/name
    try:
     size+=p.lstat().st_size;entries+=1
     if p.is_symlink():assert any(p.resolve().is_relative_to(v) for v in roots),str(p)
    except FileNotFoundError:pass
 return size,entries
size,entries=census()
assert size<=r['maxWholeBytes'] and entries<=r['maxWholeEntries']
free=lambda:os.statvfs(ROOT).f_bavail*os.statvfs(ROOT).f_frsize
other=4*1024**3
assert free()>=11*1024**3 and free()-other-r['maxBytes']>=9*1024**3
now=datetime.datetime.now(datetime.timezone.utc)
assert now<datetime.datetime.fromisoformat('2026-11-04T21:00:00+00:00')
lease={**r,'id':'FE10-WORLD-OBSERVER-SOURCE-20261104T2035Z','accepted':True,'acceptedAt':now.isoformat(),
 'workDeadline':(now+datetime.timedelta(seconds=60)).isoformat(),'hardDeadline':(now+datetime.timedelta(seconds=120)).isoformat(),
 'scratch':r['newScratchOnly'],'maxRepairRounds':1,'measuredFreeBytes':free(),'otherReservationsBytes':other,
 'remainingAfterReservationsBytes':free()-other-r['maxBytes'],'measuredWholeBytes':max(size,r['measuredWholeBytes']),
 'measuredWholeEntries':max(entries,r['measuredWholeEntries']),'supervisorSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
out=ROOT/r['retainedOutput'];out.mkdir();(out/'offer.json').write_text(json.dumps(lease,indent=2)+'\n')
print(json.dumps({k:lease[k] for k in ['id','acceptedAt','workDeadline','hardDeadline','remainingAfterReservationsBytes','measuredWholeBytes','measuredWholeEntries']}),flush=True)
stop=time.monotonic()+60; hard=stop+60
seen=set(); peak={'bytes':0,'rss':0,'processes':0}; whole=[lease['measuredWholeBytes'],lease['measuredWholeEntries']]
last_census=0; guardian=None; failure=None

def table():
 result={}
 for line in subprocess.check_output(['ps','-axo','pid=,ppid=,pgid=,rss='],text=True).splitlines():
  v=line.split()
  if len(v)==4:
   pid,parent,group,rss=map(int,v);result[pid]=(parent,group,rss*1024)
 return result

def sample():
 global last_census
 rows=table();ids={os.getpid()}
 while True:
  more={pid for pid,row in rows.items() if row[0] in ids}
  if more<=ids:break
  ids|=more
 seen.update(ids)
 rss=sum(rows[p][2] for p in ids if p in rows)
 paths=set()
 for name in lease['taskCarryRoots']+[lease['scratch']]:
  p=Path(name)
  if p.exists():
   paths.add(p)
   if p.is_dir() and not p.is_symlink():paths.update(p.rglob('*'))
 total=lease['gitCarryBytes']+sum(p.lstat().st_size for p in paths if p.exists() or p.is_symlink())
 assert total<=lease['maxBytes'] and rss<=lease['maxRssBytes'] and len(ids)<=lease['maxProcesses']
 assert free()>=8*1024**3
 if time.monotonic()-last_census>=2:
  a,b=census();whole[0]=max(whole[0],a);whole[1]=max(whole[1],b);last_census=time.monotonic()
 assert whole[0]<=lease['maxWholeBytes'] and whole[1]<=lease['maxWholeEntries']
 peak['bytes']=max(peak['bytes'],total);peak['rss']=max(peak['rss'],rss);peak['processes']=max(peak['processes'],len(ids))
 with (out/'primary-supervisor.jsonl').open('a') as f:f.write(json.dumps({'epoch':time.time(),'pids':sorted(ids),'newBytes':total,'rss':rss,'freeBytes':free(),'whole':whole})+'\n')
try:
 sample() # Independent primary admission sample before guardian/command children.
 env={**os.environ,'MCR_FE10_ACCEPTED':lease['id']}
 guardian=subprocess.Popen(['python3',str(ROOT/r['guard']),str(out/'offer.json')],cwd=ROOT,env=env,start_new_session=True)
 while guardian.poll() is None:
  sample();assert time.monotonic()<stop,'Original WORK deadline';time.sleep(.1)
 sample();assert guardian.returncode==0,('Guardian failed',guardian.returncode)
except BaseException as error:
 failure=repr(error)
 if guardian and guardian.poll() is None:
  guardian.send_signal(signal.SIGINT)
  try:guardian.wait(timeout=10)
  except subprocess.TimeoutExpired:os.killpg(guardian.pid,signal.SIGKILL);guardian.wait()
finally:
 remaining=sorted((seen-{os.getpid()})&table().keys())
 link=ROOT/'frontend/node_modules';scratch=Path(lease['scratch'])
 released=not remaining and not link.exists() and not link.is_symlink() and not scratch.exists()
 receipt={'state':'FULL' if released else 'NOT_FULL','actualFullAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'epoch':time.time(),
  'allocation':lease['id'],'head':lease['head'],'commands':4,'guardianExit':guardian.returncode if guardian else None,'failure':failure,
  'ownedProcessesRemaining':remaining,'ownedGuardianRemaining':guardian is not None and guardian.poll() is None,'dependencyLinkAbsent':not link.exists() and not link.is_symlink(),
  'scratchCacheAbsent':not scratch.exists(),'peakNew':peak,'peakWhole':{'bytes':whole[0],'entries':whole[1]},'priorUnconfirmedExternalCarry':'208 bytes/four entries untouched; no external Chrome started',
  'workDeadline':lease['workDeadline'],'hardDeadline':lease['hardDeadline'],'noBrowserDatabaseBuildTarget':True}
 (out/'release.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt),flush=True)
 if not released or failure:raise SystemExit(1)
