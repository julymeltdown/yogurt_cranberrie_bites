"""Two independent finite source controls, with exact kernel identity domains."""
import ctypes, fcntl, hashlib, json, os, select, signal, subprocess, sys, time
from pathlib import Path

HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
OWNER='01a1a231-0dc4-7b50-92bd-c0f0f56bd970'
BASE=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d')
INDEX=BASE/'owner-resource-receipts'/(OWNER+'.json')
OFFER='ART231-PHOTO2-SOURCE03-0647';GIB=1024**3;MIB=1024**2
PYTHON='/Library/Frameworks/Python.framework/Versions/3.14/bin/python3'
NODE='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
PROJECTION=BASE/'resource-future-projection-20261105T064814.json'
PROJECTION_SHA='f3a02c7b5e3f440a9290ea2932a7a53482ea6673ef8ff953fac8a43661c51e5e'
lib=ctypes.CDLL(None,use_errno=True);proc=ctypes.CDLL('/usr/lib/libproc.dylib',use_errno=True)
class BSD(ctypes.Structure):
 _fields_=[(k,ctypes.c_uint32) for k in ('flags','status','xstatus','pid','ppid','uid','gid','ruid','rgid','svuid','svgid','rfu')]+[('comm',ctypes.c_char*16),('name',ctypes.c_char*32)]+[(k,ctypes.c_uint32) for k in ('nfiles','pgid','pjobc','tdev','tpgid','nice')]+[('sec',ctypes.c_uint64),('usec',ctypes.c_uint64)]
def sha(b):return hashlib.sha256(b).hexdigest()
def atomic(p,j):
 b=(json.dumps(j,indent=2)+'\n').encode()
 if p==INDEX:assert len(b)<=48*1024
 pending=p.with_suffix('.pending');pending.write_bytes(b);pending.replace(p)
def pin(p):
 b=p.read_bytes();return dict(path=str(p),bytes=len(b),sha256=sha(b))
def kernel(pid):
 b=BSD();n=proc.proc_pidinfo(pid,3,0,ctypes.byref(b),ctypes.sizeof(b))
 if n==0:return None
 assert n==136
 image=ctypes.create_string_buffer(4096)
 if proc.proc_pidpath(pid,image,4096)<=0:return dict(pid=pid,parentPid=b.ppid,processGroup=b.pgid,birthEpochSeconds=b.sec,birthMicroseconds=b.usec,zombie=True)
 mib=(ctypes.c_int*3)(1,49,pid);size=ctypes.c_size_t()
 assert lib.sysctl(mib,3,None,ctypes.byref(size),None,0)==0
 buf=ctypes.create_string_buffer(size.value);assert lib.sysctl(mib,3,buf,ctypes.byref(size),None,0)==0
 raw=buf.raw[:size.value];argc=int.from_bytes(raw[:4],sys.byteorder,signed=True);assert 0<argc<4096
 at=raw.index(b'\0',4)+1
 while raw[at]==0:at+=1
 argv=[]
 for _ in range(argc):
  end=raw.index(b'\0',at);argv.append(raw[at:end].decode());at=end+1
 return dict(pid=pid,parentPid=b.ppid,processGroup=b.pgid,birthEpochSeconds=b.sec,birthMicroseconds=b.usec,executable=image.value.decode(),argv=argv,kernelArgv=argv,kernelArgvEncoding='KERN_PROCARGS2-argc-bounded-utf8-json-vector-v1',commandEncoding='argv')
def identity(role):
 result=kernel(os.getpid());assert result and not result.get('zombie')
 entry=Path(sys.argv[0]).resolve()
 result.update(script=str(entry),scriptSHA256=sha(entry.read_bytes()),cwd=os.getcwd(),workingRoot=str(ROOT),role=role,launcherExecutable=sys.executable,pythonSysArgv=sys.argv,pythonOrigArgv=sys.orig_argv)
 return result
def raw_ps(identity):
 birth=subprocess.check_output(['/bin/ps','-p',str(identity['pid']),'-o','lstart='],text=True).removesuffix('\n')
 identity.update(birth=birth,birthRawPS=birth,birthSource='literal /bin/ps -p OWN_PID -o lstart= after numeric BSD registration')
 raw=subprocess.check_output(['/bin/ps','-ww','-p',str(identity['pid']),'-o','command='],text=True).removesuffix('\n')
 identity.update(psCommandRaw=raw,psCommandSHA256=sha(raw.encode()),commandDigest=sha(raw.encode()),commandDigestEncoding='sha256:ps-ww-command-no-terminal-lf-utf8-v1')
 return identity
def scan(roots):
 found={}
 for root in roots:
  pending=[Path(root)]
  while pending:
   p=pending.pop()
   try:s=p.lstat()
   except FileNotFoundError:continue
   if p.is_dir() and not p.is_symlink():
    try:pending.extend(Path(e.path) for e in os.scandir(p))
    except FileNotFoundError:pass
   else:found[(s.st_dev,s.st_ino,int(s.st_birthtime*1000000))]=s.st_size
 return found
def children(table,roots):
 ids=set(roots)&set(table)
 while True:
  extra={p for p,r in table.items() if r['parent'] in ids}
  if extra<=ids:return ids
  ids|=extra
def current_projection():
 allocation=json.loads((BASE/'resource-allocation-current.json').read_bytes())
 path=Path(allocation['currentFutureReconciliation']['record']);projection=json.loads(path.read_bytes())
 assert projection['ordinaryCombinedFutureUpperBytes']==2684354560
 assert projection['sourceWindow']['futureTargetBytes']==0
 assert time.time()<1793863800,'Source window through07:30UTC'
 own=projection['phaseOwners']['ART231'];assert own['owner']==OWNER and OFFER in own['offers'] and own['includedPhaseUpperBytes']==64*MIB
 return projection,pin(path)
def bind(declared,actual):
 assert actual['birthEpochSeconds']==declared['birthEpochSeconds'] and actual['birthMicroseconds']==declared['birthMicroseconds'],('birth',declared)
 if actual.get('zombie'):return
 assert actual['executable']==declared['executable'],('image',declared['pid'])
 assert actual['argv']==declared['argv'],('actual argc-bounded kernel argv',declared['pid'])
 assert actual['parentPid']==declared['parentPid'],('parent',declared['pid'])
 if 'processGroup' in declared:assert actual['processGroup']==declared['processGroup']
 if declared.get('script'):assert sha(Path(declared['script']).read_bytes())==declared['scriptSHA256']
 if declared.get('cwd'):
  raw=subprocess.check_output(['/usr/sbin/lsof','-a','-p',str(declared['pid']),'-d','cwd','-Fn'],text=True)
  paths=[line[1:] for line in raw.splitlines() if line.startswith('n/')];assert paths==[declared['cwd']],('cwd',declared['pid'],paths)
def census(config):
 lock=Path(config['temp'])/'sampler.lock'
 with lock.open('a') as handle:
  fcntl.flock(handle,fcntl.LOCK_EX)
  raw=subprocess.check_output(['/bin/ps','-ww','-axo','pid=,ppid=,rss=,pgid=,lstart=,state=,command='],text=True)
  table={}
  for line in raw.splitlines():
   p=line.split(None,10)
   if len(p)==11:table[int(p[0])]=dict(parent=int(p[1]),rss=int(p[2])*1024,group=int(p[3]),state=p[9],command=p[10])
  membership=json.loads((BASE/'resource-membership-current.json').read_bytes());records={r.get('ownerThreadId',r.get('owner')):r for r in membership['entries']}
  for path in (BASE/'owner-resource-receipts').glob('*.json'):
   raw=path.read_bytes();assert len(raw)<=65536,('oversized owner record',str(path),len(raw))
   row=json.loads(raw);old=records.get(row['ownerThreadId'],{})
   if float(row.get('acceptedAtEpoch') or 0)>=float(old.get('acceptedAtEpoch',old.get('actualAccept')) or 0):records[row['ownerThreadId']]=row
  active=[r for r in records.values() if r.get('state')=='ACCEPTED' and r.get('kind')!='ordinary'];kinds=[r['kind'] for r in active]
  assert 'target' not in kinds,'Compiler/target window withdrawn'
  assert len(active)<=4 and kinds.count('source')<=3 and kinds.count('browser')+kinds.count('copied-native')<=2
  assert sum(r['processCap'] for r in active)<=96 and sum(r['rssCapBytes'] for r in active)<=18*GIB
  classified=set();owners=[]
  for row in records.values():
   roots=[]
   for declared in [row.get('controller'),row.get('ordinaryObservation'),*row.get('guardControllers',[])]:
    if not declared or declared.get('pid') not in table:continue
    actual=kernel(declared['pid'])
    if not actual:continue
    if (actual['birthEpochSeconds'],actual['birthMicroseconds'])!=(declared.get('birthEpochSeconds'),declared.get('birthMicroseconds')):
     if row.get('state')=='ACCEPTED':raise AssertionError(('accepted root birth unavailable/mismatch',row['ownerThreadId']))
     continue
    bind(declared,actual);roots.append(declared['pid'])
   owned=children(table,roots);classified|=owned
   rss=sum(table[p]['rss'] for p in owned)
   if owned:assert len(owned)<=row['processCap'] and rss<=row['rssCapBytes'],('live actual cap, including FULL roots',row['ownerThreadId'])
   owners.append(dict(owner=row['ownerThreadId'],state=row.get('state'),kind=row.get('kind'),pids=sorted(owned),rss=rss))
  unknown=[]
  for pid,r in table.items():
   exe=r['command'].split()[0] if r['command'] else ''
   if pid not in classified and any(t in exe.lower() for t in ('python','node','cargo','rustc','chrome','postgres')) and any(t in r['command'].lower() for t in ('/mcr/','/mcr-','mcr-worktrees','mcr-evidence')):unknown.append((pid,r['command']))
  assert not unknown,('unknown actual MCR roots',unknown)
  owned=children(table,[config['supervisor'],config.get('guardian',0)])
  # The completed ps helper is counted conservatively even after communicate/reap.
  count=len(owned)+1;rss=sum(table[p]['rss'] for p in owned)+16*MIB
  assert count<=8 and rss<=GIB,('own aggregate',count,rss)
  assert len(classified)<=96 and sum(table[p]['rss'] for p in classified)<=18*GIB
  projection,projection_pin=current_projection();future=projection['conservativeApplicableFutureUpperBytes']
  by_owner={p['owner']:p for p in projection['phaseOwners'].values()}
  for row in active:
   phase=by_owner.get(row['ownerThreadId']);remaining=row['remainingNewUpperBytes']
   assert isinstance(remaining,int) and remaining>=0
   if phase:
    assert row['offerId'] in phase['offers'],('Accepted phase missing current mapping',row['offerId'])
    future+=max(0,remaining-phase['includedPhaseUpperBytes'])
   else:future+=remaining
  return dict(processes=count,rss=rss,pids=sorted(owned),owners=owners,unknown=unknown,source=kinds.count('source'),workflows=len(active),applicableFutureBytes=future,currentProjection=projection_pin)
class Meter:
 def __init__(self,config):
  self.config=config;self.baseline=scan(config['roots']);self.seen={};self.whole=max(4357560906,sum(self.baseline.values()));self.peak=dict(whole=self.whole,positive=0,rss=0,processes=0);self.minimumFree=2**63;self.samples=0
 def check(self,work=True):
  c=self.config;assert time.time()<(c['work'] if work else c['hard'])
  free=os.statvfs(ROOT).f_bavail*os.statvfs(ROOT).f_frsize;self.minimumFree=min(self.minimumFree,free)
  assert free>=8*GIB,'Live floor'
  current=scan(c['roots'])
  for key,size in current.items():
   gain=max(0,size-self.baseline.get(key,0))
   if gain:self.seen[key]=max(self.seen.get(key,0),gain)
  positive=sum(self.seen.values())+1048576;whole=self.whole+positive
  assert positive<=64*MIB and whole<=8*GIB
  # Every entire ordinary book plus historical repair remains charged once.
  assert 221435639+37817629+1048576+16777216+67108864+34526193+16777216+positive<=536870912
  result=census(c)
  assert free>=result['applicableFutureBytes']+9*GIB,'After all current phases and higher actual accepted reservations'
  for key,value in dict(whole=whole,positive=positive,rss=result['rss'],processes=result['processes']).items():self.peak[key]=max(self.peak[key],value)
  self.samples+=1;return dict(at=time.time(),free=free,peak=self.peak,census=result,samples=self.samples)

def guardian(config):
 me=identity('Independent complete ART source resource guard')
 record=json.loads(INDEX.read_bytes());record['guardControllers']=[me];atomic(INDEX,record)
 raw_ps(me);record['guardControllers']=[me];atomic(INDEX,record)
 meter=Meter(config);failure=None
 try:
  while True:
   control=json.loads((Path(config['temp'])/'control.json').read_bytes())
   result=meter.check(control['work']);atomic(Path(config['temp'])/'guard.json',dict(ok=True,**result))
   if control.get('shutdown'):break
   time.sleep(.15)
 except BaseException as e:
  failure=repr(e);atomic(Path(config['temp'])/'guard.json',dict(ok=False,at=time.time(),failure=failure))
 finally:atomic(HERE/'source03-independent-full.json',dict(at=time.time(),failure=failure,peak=meter.peak,samples=meter.samples,minimumFree=meter.minimumFree))
 return 1 if failure else 0

def main():
 if sys.argv[1:2]==['--guardian']:return guardian(json.loads(Path(sys.argv[2]).read_bytes()))
 raise SystemExit('Controller body is deliberately supplied by the frozen source controller entry.')
if __name__=='__main__':sys.exit(main())
