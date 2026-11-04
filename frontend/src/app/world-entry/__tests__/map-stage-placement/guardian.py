"""Independent bounded guardian; metadata authoring is not resource acceptance."""
import datetime,json,os,shutil,signal,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[6]
FRONT=ROOT/'frontend';OUT=ROOT/'docs/mcr/evidence/map-stage-placement'
LEASE=os.environ['MCR_STAGE_LEASE'];END=datetime.datetime.fromisoformat(os.environ['MCR_STAGE_DEADLINE']).timestamp()
TMP=Path(os.environ['MCR_STAGE_TMP']);NODE='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
TARGET='/Users/lhs/Desktop/mcr/frontend/node_modules';LINK=FRONT/'node_modules'
PREFIX='src/app/world-entry/__tests__/map-stage-placement/'
SEEN=set();PEAK=dict(processes=0,rssBytes=0,newBytes=0,tmpBytes=0,wholeBytes=0)
MODE=os.environ.get('MCR_STAGE_MODE','native')
WHOLE_FLOOR=int(os.environ.get('MCR_STAGE_WHOLE_FLOOR','0'))
OTHER_NEW=int(os.environ.get('MCR_STAGE_OTHER_NEW','0'))
assert MODE in ('source','native','qualification')
NEWCAP=(128 if MODE=='source' else 200)*1024**2;RESERVE=32*1024**2
# Own whole retained carry plus new artifacts; shared dependency store is read-only, not ours.
CARRY=[ROOT/'docs/mcr/evidence/map-art05-default-app',FRONT/'src/app/world-entry/__tests__/art05-default-entry',ROOT/'docs/mcr/frontend/map-art05-default-app.md',ROOT/'docs/mcr/evidence/map-production-binding',ROOT/'docs/mcr/evidence/mural-production-binding/placement',OUT,
 ROOT/'docs/mcr/frontend/map-production-binding.md',ROOT/'docs/mcr/frontend/map-stage-placement.md',
 FRONT/'src/app/world-entry/__tests__/map-presentation-v1',FRONT/'src/app/world-entry/__tests__/map-presentation-v2',FRONT/PREFIX,
 FRONT/'src/app/world-entry/map-presentation-v1',FRONT/'src/app/world-entry/map-presentation-v2',
 FRONT/'src/world/reference-scene/map4-service-v1',FRONT/'src/world/reference-scene/map4-service-v2',
 FRONT/'public/world/map-reference-composition-v1',FRONT/'public/world/map-firstview-cozy-v1',
 FRONT/'src/app/world-entry/reference-scene-overview.ts',FRONT/'src/world/reference-scene/overview.ts',FRONT/'src/world/illustrated/overview.ts',
 FRONT/'src/features/places/ui/IllustratedMap.tsx',FRONT/'src/features/places/ui/reference-scene/map-service-links.tsx']
def files(paths):
 result=set()
 for p in paths:
  if p.is_dir():result.update(x for x in p.rglob('*') if x.is_file() and not x.is_symlink())
  elif p.is_file() and not p.is_symlink():result.add(p)
 return result
def size(paths):
 total=0
 for p in files(paths):
  try:total+=p.stat().st_size
  except FileNotFoundError:pass  # Chromium atomically renames transient profile files.
 return total
def record(row):
 with (OUT/'guardian.jsonl').open('a') as f:f.write(json.dumps(row)+'\n')
def table():
 result={}
 for line in subprocess.check_output(['ps','-axo','pid=,ppid=,pgid=,rss=,comm='],text=True).splitlines():
  v=line.strip().split(None,4)
  if len(v)==5:result[int(v[0])]=tuple(int(x) for x in v[1:4])+(v[4],)
 return result
# Entire checkout/dependencies/Git are frozen and precharged before admission.
# Only these owned paths and private TMP can grow during the accepted job.
MUTABLE=[OUT,FRONT/PREFIX,ROOT/'docs/mcr/frontend/map-stage-placement.md',FRONT/'src/features/places/ui/IllustratedMap.tsx',FRONT/'src/world/reference-scene/map4-service-v1/layout.ts']
MUTABLE_BASE=size(MUTABLE)
def measure():
 mutable=size(MUTABLE);tmp=size([TMP])
 extra=RESERVE+mutable+tmp+size([FRONT/'public/world/map-reference-composition-v1'])
 return dict(newBytes=extra,tmpBytes=tmp,wholeBytes=WHOLE_FLOOR+max(0,mutable-MUTABLE_BASE)+tmp)
def guard(fresh=False):
 d=os.statvfs(ROOT);m=measure()
 assert d.f_bavail*d.f_frsize>=(12 if fresh else 8)*1024**3,'disk floor'
 assert time.time()<END-60,'cleanup reserve'
 assert m['newBytes']<NEWCAP and m['tmpBytes']<(96 if MODE=='source' else 200)*1024**2 and m['wholeBytes']<2*1024**3,'NEW/TMP/whole cap'
 # 200MiB own worst-case plus at most 2GiB other frontend reservations leaves >=9GiB on fresh12.
 assert d.f_bavail*d.f_frsize-OTHER_NEW-max(0,NEWCAP-m['newBytes'])>=9*1024**3 if fresh else True,'aggregate reserve'
 for k,v in m.items():PEAK[k]=max(PEAK[k],v)
assert Path.cwd()==ROOT and TMP.as_posix().startswith('/private/tmp/mcr-stage-') and not TMP.exists()
assert not os.path.lexists(LINK),'absent-only dependency link'
assert __import__('hashlib').sha256((FRONT/'pnpm-lock.yaml').read_bytes()).hexdigest()=='47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
guard(True);TMP.mkdir(mode=0o700);LINK.symlink_to(TARGET)
env=dict(os.environ,PATH=str(Path(NODE).parent)+':'+os.environ['PATH'],TMPDIR=str(TMP)+'/',MCR_STAGE_GUARDED='1',MCR_MAP_COMPOSITION_TMP=str(TMP),MCR_MAP_SOURCE_LEASE=LEASE,MCR_MAP_SOURCE_DEADLINE=os.environ['MCR_STAGE_DEADLINE'])
child=None;ids=set();exitcode=1;failure=None;pgids=set()
record(dict(lease=LEASE,startedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),guardianPid=os.getpid(),activeDeadline=END-60,hardDeadline=END,initial=measure()))
try:
 checks=[[NODE,'--check',PREFIX+'native.mjs'],[NODE,'node_modules/oxlint/bin/oxlint',PREFIX+'native.mjs'],[NODE,PREFIX+'native.mjs']]
 if MODE in ('source','qualification'):
  checks=[[NODE,'node_modules/vitest/vitest.mjs','run','--config',PREFIX+'vitest.config.ts','--configLoader','runner'],
   [NODE,'node_modules/typescript/bin/tsc','-p',PREFIX+'tsconfig.json','--tsBuildInfoFile',str(TMP/'stage.tsbuildinfo'),'--pretty','false'],
   [NODE,'node_modules/oxlint/bin/oxlint','--deny-warnings','src/features/places/ui/IllustratedMap.tsx','src/world/reference-scene/map4-service-v1/layout.ts',PREFIX],
   [NODE,'--check',PREFIX+'native.mjs']]
  if MODE=='qualification':checks.append([NODE,PREFIX+'native.mjs'])

 for index,argv in enumerate(checks):
  guard();runout=OUT/os.environ['MCR_STAGE_ROUND'];runout.mkdir(exist_ok=True)
  log=(runout/f'job-{index:02}.log').open('wb');started=datetime.datetime.now(datetime.timezone.utc).isoformat()
  child=subprocess.Popen(argv,cwd=FRONT,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);ids.add(child.pid);pgids.add(child.pid)
  while child.poll() is None:
   t=table();previous=None
   while previous!=ids:
    previous=set(ids);ids.update(pid for pid,v in t.items() if v[0] in ids)
   SEEN.update(ids);live={pid for pid in ids if pid in t};rss=sum(t[pid][2]*1024 for pid in live|{os.getpid()} if pid in t)
   PEAK['processes']=max(PEAK['processes'],len(live)+1);PEAK['rssBytes']=max(PEAK['rssBytes'],rss)
   assert len(live)+1<=(6 if MODE=='source' else 16) and rss<(2.5 if MODE=='source' else 3.5)*1024**3,'process/RSS cap';guard();time.sleep(.1)
  log.close();exitcode=child.returncode
  with (OUT/'attempts.jsonl').open('a') as f:f.write(json.dumps(dict(lease=LEASE,argv=argv,exitCode=exitcode,pid=child.pid,startedAt=started,finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),log=str((runout/f'job-{index:02}.log').relative_to(OUT))))+'\n')
  if exitcode:raise RuntimeError('Allocated stage failed; retain artifacts and fix within original clock')
except BaseException as error:
 failure=str(error);exitcode=1
finally:
 for pgid in pgids:
  try:os.killpg(pgid,signal.SIGTERM)
  except ProcessLookupError:pass
 time.sleep(.5)
 t=table()
 for pid in sorted(ids,reverse=True):
  if pid in t:
   try:os.kill(pid,signal.SIGKILL)
   except ProcessLookupError:pass
 if child is not None:
  try:child.wait(timeout=3)
  except subprocess.TimeoutExpired:pass
 if LINK.is_symlink() and os.readlink(LINK)==TARGET:LINK.unlink()
 shutil.rmtree(TMP,ignore_errors=True);t=table();live=sorted(pid for pid in ids if pid in t)
 release=dict(lease=LEASE,finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),exitCode=exitcode,failure=failure,peak=PEAK,seenPids=sorted(SEEN),livePids=live,ownedLinkAbsent=not os.path.lexists(LINK),privateTmpAbsent=not TMP.exists(),final=measure(),freeBytes=os.statvfs(ROOT).f_bavail*os.statvfs(ROOT).f_frsize)
 with (OUT/'cleanup.jsonl').open('a') as f:f.write(json.dumps(release)+'\n')
 print(json.dumps(release),flush=True)
raise SystemExit(exitcode)
