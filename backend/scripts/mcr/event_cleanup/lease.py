#!/usr/bin/env python3
"""Finite accepted allocation controller with an independent disk/time/process guardian.

Usage: lease.py /absolute/evidence-root build|release|run. acceptance.json is
a fresh, explicitly accepted resource allocation; this tool never grants one.
Adapted from the previously qualified Social owner controller, without touching
its frozen source or artifacts.
"""
import os, sys, json, time, datetime, hashlib, shutil, signal, stat, subprocess, tempfile, re
from pathlib import Path
OUT=Path(sys.argv[1]).resolve()
A=json.loads((OUT/'acceptance.json').read_text())
WT=Path(A['worktree']); TARGET=Path(A['target']); MODE=sys.argv[2]
PG=Path(A['pg_bin']); STATE=OUT/'state.json'
def stamp(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def write(name,v):
 p=OUT/name; q=p.with_suffix(p.suffix+'.tmp');q.write_text(json.dumps(v,indent=2)+'\n');q.replace(p)
def allocations(root):
 root=Path(root); result={}
 if root.is_file():
  s=root.lstat();return {str(root):max(s.st_size,s.st_blocks*512)}
 for d,ds,fs in os.walk(root):
  for f in fs:
   p=Path(d)/f
   try:
    s=p.lstat()
    if stat.S_ISREG(s.st_mode):result[str(p)]=max(s.st_size,s.st_blocks*512)
   except FileNotFoundError:pass
 return result
def process_rows():
 result=[]
 for line in subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,comm=,args='],text=True).splitlines():
  p=line.strip().split(maxsplit=3)
  if len(p)==4:result.append((int(p[0]),int(p[1]),p[2],p[3]))
 return result
def family(rows,roots):
 found=set(roots)
 while True:
  nxt=found|{p for p,parent,comm,args in rows if parent in found}
  if nxt==found:return found
  found=nxt
def cleanup_pg():
 p=OUT/'frozen/native/cluster.json'
 if not p.exists():return
 v=json.loads(p.read_text());root=Path(v['root']);data=root/'data'
 if not root.exists():return
 s=root.stat();assert [s.st_dev,s.st_ino,s.st_uid]==v['identity'] and (root/'owner').read_text()==v['token']
 if (data/'postmaster.pid').exists():
  lines=(data/'postmaster.pid').read_text().splitlines();assert Path(lines[1]).resolve()==data
  r=subprocess.run([str(PG/'pg_ctl'),'-D',str(data),'-m','fast','-w','stop'],capture_output=True,text=True,timeout=50)
  (OUT/'pg-final-stop.stdout').write_text(r.stdout);(OUT/'pg-final-stop.stderr').write_text(r.stderr)
  assert r.returncode==0
 if (root/'postgres.log').exists():shutil.copy2(root/'postgres.log',OUT/'postgres.log')
 assert not (data/'postmaster.pid').exists()
 shutil.rmtree(root)
 write('pg-cleanup.json',{'at':stamp(),'root':str(root),'removed':not root.exists(),'token_sha256':hashlib.sha256(v['token'].encode()).hexdigest()})

if MODE=='guard':
 mode=sys.argv[3]; parent=int(sys.argv[4]); baseline=json.loads((OUT/'target-baseline.json').read_text()) if mode in ('build','release') else None
 growth=0 if mode=='build' else json.loads((OUT/'build-result.json').read_text())['peak_positive_target_growth_bytes']
 peak=0;low=shutil.disk_usage(OUT).free;failure=None;maxprocess=0
 try:
  while True:
   if (OUT/('guard-'+mode+'.stop')).exists():break
   s=json.loads(STATE.read_text()); now=time.time()
   assert now<A['full_release_epoch'],'full release cutoff'
   cutoff=A['all_work_epoch']
   if mode=='build':cutoff=min(cutoff,A['compile_freeze_epoch'])
   if s.get('copy_started'):cutoff=min(cutoff,s['copy_started']+30)
   if mode=='run':cutoff=min(cutoff,s['native_started']+240,A['target_released_epoch']+360)
   assert now<cutoff,'work cutoff; independent cleanup reserve retained'
   free=shutil.disk_usage(OUT).free;low=min(low,free);assert free>=A['continuous_floor_bytes'],'free floor'
   if baseline is not None and s.get('target_active'):
    cur=allocations(TARGET);growth=max(growth,sum(max(v-baseline.get(k,0),0) for k,v in cur.items()))
   assert growth<=A['target_growth_cap_bytes'],'positive target growth cap'
   carry=sum(sum(allocations(p['path']).values()) for p in A['carry'])
   output=sum(allocations(OUT).values());pg=s.get('pg_root')
   cluster=OUT/'frozen/native/cluster.json'
   if cluster.exists():pg=json.loads(cluster.read_text())['root']
   if pg:output+=sum(allocations(pg).values())
   assert output<=A['new_output_cap_bytes'],'new output cap including PG'
   whole=carry+growth+output;peak=max(peak,whole);assert whole<=A['whole_cap_bytes'],'whole carry/WT/output/growth cap'
   rows=process_rows();roots={parent}
   if pg and (Path(pg)/'data/postmaster.pid').exists():roots.add(int((Path(pg)/'data/postmaster.pid').read_text().splitlines()[0]))
   count=len(family(rows,roots));maxprocess=max(maxprocess,count)
   assert count<=(20 if mode in ('build','release') else 12),'process ceiling'
   assert any(pid==parent for pid,pp,co,ar in rows),'controller disappeared'
   summary={'at':stamp(),'peak_whole_bytes':peak,'peak_positive_target_growth_bytes':growth,'minimum_free_bytes':low,'maximum_processes':maxprocess,'last_carry_bytes':carry,'last_new_output_bytes':output}
   write('guard-'+mode+'.json',summary)
   (OUT/('guard-'+mode+'.ready')).touch()
   time.sleep(.6)
 except BaseException as e:
  failure=repr(e);write('guard-'+mode+'-failure.json',{'at':stamp(),'failure':failure})
  try:os.kill(parent,signal.SIGUSR1)
  except ProcessLookupError:pass
  time.sleep(2)
  try:
   s=json.loads(STATE.read_text());child=s.get('child')
   if child:
    try:os.killpg(child,signal.SIGTERM)
    except ProcessLookupError:pass
   cleanup_pg()
  except BaseException as ce:write('guard-cleanup-failure.json',{'failure':repr(ce)})
 sys.exit(1 if failure else 0)

state={'controller':os.getpid(),'mode':MODE,'target_active':MODE in ('build','release'),'child':None}
if MODE=='run':state['native_started']=time.time()
if MODE=='release':state['target_active']=False
if MODE=='build':
 assert shutil.disk_usage(OUT).free>=A['start_floor_bytes']
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=WT,text=True).strip()==A['head']
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=WT,text=True)
 write('target-baseline.json',allocations(TARGET))
write('state.json',state)
guardian=subprocess.Popen([sys.executable,__file__,str(OUT),'guard',MODE,str(os.getpid())],start_new_session=True)
write('guardian-'+MODE+'-pid.json',{'pid':guardian.pid,'started_at':stamp(),'controller':os.getpid()})
def interrupted(signum,frame): raise RuntimeError('independent guardian interrupted work')
signal.signal(signal.SIGUSR1,interrupted);signal.signal(signal.SIGTERM,interrupted)
started=time.monotonic();checks=[]
def execute(name,args,env=None,cwd=None,expected=0):
 assert guardian.poll() is None,'guardian is required before child'
 assert time.time()<A['all_work_epoch']
 with (OUT/(name+'.stdout')).open('wb') as so,(OUT/(name+'.stderr')).open('wb') as se:
  p=subprocess.Popen([str(x) for x in args],cwd=cwd,env=env,stdout=so,stderr=se,start_new_session=True)
  state['child']=p.pid;write('state.json',state)
  try:
   while p.poll() is None:
    assert guardian.poll() is None,'guardian stopped';time.sleep(.15)
  finally:
   if p.poll() is None:
    os.killpg(p.pid,signal.SIGTERM)
    try:p.wait(3)
    except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(3)
   state['child']=None;write('state.json',state)
  code=p.returncode
 checks.append({'name':name,'exit_code':code,'argv':[str(x) for x in args]})
 if expected is not None:assert code==expected,name+' failed'
 return '' if name=='source-archive' else (OUT/(name+'.stdout')).read_text()
def snapshot():
 paths=subprocess.check_output(['git','ls-files','backend','contracts','AGENTS.md','docs/mcr/backend/event-cleanup-runtime.md'],cwd=WT,text=True).splitlines()
 return {p:sha(WT/p) for p in paths if (WT/p).is_file()}
status='FAIL'
try:
 while not (OUT/('guard-'+MODE+'.ready')).exists():
  assert guardian.poll() is None;time.sleep(.05)
 if MODE=='build':
  handles=execute('preflight-target-handles',['/usr/sbin/lsof','+D',TARGET],expected=None)
  assert checks[-1]['exit_code']==1 and not handles.strip() and not (OUT/'preflight-target-handles.stderr').read_text().strip()
  assert not any(str(TARGET) in args and Path(comm).name in ('cargo','rustc','rustdoc') for pid,pp,comm,args in process_rows())
  source=snapshot();write('source-pins.json',{'head':A['head'],'files':source})
  execute('source-archive',['git','archive','--format=tar',A['head'],'backend','contracts','AGENTS.md','docs/mcr/backend/event-cleanup-runtime.md'],cwd=WT)
  (OUT/'source-archive.stdout').rename(OUT/'source.tar')
  script=WT/'backend/scripts/mcr/event_cleanup/freeze.py'
  execute('source-snapshot',[sys.executable,script,'--snapshot',OUT/'before.json'],cwd=WT)
  env={**os.environ,**A['env']}
  for k in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER'):env.pop(k,None)
  for i,argv in enumerate(A['argv']):execute('cargo-'+str(i),argv,env=env,cwd=WT/'backend')
  assert snapshot()==source,'source changed during compile'
  state['copy_started']=time.time();write('state.json',state)
  execute('freeze',[sys.executable,script,'--before',OUT/'before.json','--cargo-json',OUT/'cargo-0.stdout','--cargo-json',OUT/'cargo-1.stdout','--out',OUT/'frozen'],env=env,cwd=WT)
  f=json.loads((OUT/'frozen/freeze.json').read_text())
  assert sum(v['size'] for v in f['binaries'].values())<=A['copied_exe_cap_bytes']
  state.pop('copy_started');write('state.json',state)
 elif MODE=='release':
  f=json.loads((OUT/'frozen/freeze.json').read_text())
  for name,v in f['binaries'].items():assert sha(OUT/'frozen/bin'/name)==v['sha256']
  rows=process_rows();assert not any(str(TARGET) in args and Path(comm).name in ('cargo','rustc','rustdoc') for pid,pp,comm,args in rows)
  handles=execute('release-target-handles',['/usr/sbin/lsof','+D',TARGET],expected=None)
  assert checks[-1]['exit_code']==1 and not handles.strip() and not (OUT/'release-target-handles.stderr').read_text().strip()
  A['target_released_epoch']=time.time();write('acceptance.json',A)
  write('target-release.json',{'lease':A['lease'],'at':stamp(),'target':str(TARGET),'zero_handles':True,'compiler_children':0,'unconditional_target_release':True,'copied_tests_started':False,'no_target_reaccess':True})
 elif MODE=='run':
  r=json.loads((OUT/'target-release.json').read_text());assert r['unconditional_target_release'] and r['zero_handles']
  assert snapshot()==json.loads((OUT/'source-pins.json').read_text())['files']
  env={k:v for k,v in os.environ.items() if not k.startswith(('PG','DATABASE','CONCERT_DATABASE','MCR_','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'))}
  env.update(PATH='/usr/bin:/bin',TMPDIR=str(OUT/'tmp'),MCR_EVENT_CLEANUP_PG_BIN=str(PG))
  owned=[args for pid,pp,comm,args in process_rows() if '/bin/postgres ' in args and '-D /private/tmp/' in args and 'python' not in comm]
  assert len(owned)<3,'native aggregate limit before own cluster'
  execute('native',[sys.executable,WT/'backend/scripts/mcr/event_cleanup/run.py','--frozen',OUT/'frozen'],env=env,cwd=WT)
  n=json.loads((OUT/'frozen/native.json').read_text());assert n['status']=='PASS' and n['cleanup']
 else:raise ValueError(MODE)
 status='PASS'
except BaseException as e:
 write(MODE+'-failure.json',{'at':stamp(),'failure':repr(e),'checks':checks})
 raise
finally:
 if MODE=='run':
  try:cleanup_pg()
  except BaseException as e:write('cleanup-failure.json',{'failure':repr(e)});status='FAIL'
 (OUT/('guard-'+MODE+'.stop')).touch()
 try:guardian.wait(15)
 except subprocess.TimeoutExpired:guardian.terminate();guardian.wait(3)
 summary=json.loads((OUT/('guard-'+MODE+'.json')).read_text()) if (OUT/('guard-'+MODE+'.json')).exists() else {}
 write(MODE+'-result.json',{'status':status,'at':stamp(),'elapsed_seconds':time.monotonic()-started,'checks':checks,**summary})
 print(json.dumps({'mode':MODE,'status':status,'checks':len(checks),'guardian_exit':guardian.returncode,**summary}),flush=True)
