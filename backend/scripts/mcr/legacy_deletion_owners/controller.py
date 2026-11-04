#!/usr/bin/env python3
"""Finite accepted four-owner qualification. Never grants a resource allocation.
Execution/ownership guards adapted from the qualified BE33 c9f8 controller;
no old offers, full source archives, cold target or Notification workload reuse.
Run guardian.py OUTPUT build|qualify; target release is recorded separately.
"""
import datetime,hashlib,json,os,pathlib,shutil,signal,subprocess,sys,time,stat
OUT=pathlib.Path(sys.argv[1]).resolve()
cfg=json.loads((OUT/'accepted-offer.json').read_text())
assert cfg['explicit_accept'] is True
ROOT=pathlib.Path(cfg['wt'])
TARGET=pathlib.Path(cfg['target']); CARRY=cfg['carry']; CAP=cfg['cap']; FLOOR=8*1024**3
COMPILE=cfg['compile_stop']; QUALIFY=cfg['qualify_stop']; STOP=cfg['full_stop']; LEASE=cfg['lease']
PG=pathlib.Path('/opt/homebrew/opt/postgresql@16/bin')
stage=sys.argv[2]; proc=None;server_proc=None;cluster=None;cleaning=False;owned_seen={};servers=[];redis=None;copy_started=None
report={'lease':LEASE,'stage':stage,'started':time.time(),'status':'FAIL','phases':[],'minimum_free':shutil.disk_usage(OUT).free,'peak_whole':CARRY,'peak_processes':0,'peak_rustc':0}
def cmd(args,cwd=ROOT):return subprocess.check_output(args,cwd=cwd,text=True).strip()
def sha(path):
 h=hashlib.sha256()
 with open(path,'rb') as f:
  for b in iter(lambda:f.read(1024**2),b''):h.update(b)
 return h.hexdigest()
def size(path):return sum(allocations(path).values())
def write(name,v):
 p=OUT/name;tmp=OUT/(name+'.tmp');tmp.write_text(json.dumps(v,indent=2)+'\n');tmp.chmod(0o600);tmp.replace(p)
def terminate(p):
 if p is None:return
 try:os.killpg(p.pid,signal.SIGTERM)
 except ProcessLookupError:pass
 try:p.wait(timeout=3)
 except subprocess.TimeoutExpired:
  os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=3)
def processes():
 rows=[]
 for line in cmd(['ps','-axo','pid=,ppid=,pgid=,rss=,args=']).splitlines():
  cols=line.strip().split(None,4)
  if len(cols)==5:rows.append((int(cols[0]),int(cols[1]),int(cols[2]),cols[4],int(cols[3])*1024))
 return rows
def allocations(root):
 result={}
 for d,ds,fs in os.walk(root):
  for n in fs:
   p=pathlib.Path(d)/n
   try:
    st=p.lstat()
    if stat.S_ISREG(st.st_mode): result[str(p)]=max(st.st_size,st.st_blocks*512)
   except FileNotFoundError: pass
 return result
def guard(deadline):
 assert time.time()<min(deadline,STOP),'absolute cutoff'
 free=shutil.disk_usage(OUT).free;report['minimum_free']=min(report['minimum_free'],free);
 if not cleaning:assert free>=FLOOR,'free floor'
 if copy_started is not None:assert time.time()<copy_started+30,'copy freeze cutoff'
 growth=json.loads((OUT/'target-release.json').read_text())['target_growth_bytes'] if (OUT/'target-release.json').exists() else sum(max(v-offer['target_files'].get(k,0),0) for k,v in allocations(TARGET).items())
 source_growth=max(0,size(ROOT)-cfg['wt_baseline'])+max(0,size(pathlib.Path(cfg['git_dir']))-cfg['git_baseline'])
 outside=size(OUT)
 if not cleaning:
  assert growth<=cfg['target_growth_cap'],'new target growth cap'
  assert outside<=cfg['outside_cap'],'new outside cap'
 whole=CARRY+source_growth+growth+outside;report['peak_whole']=max(report['peak_whole'],whole);
 if not cleaning:
  assert whole<=CAP,'whole cap'
  assert growth+outside<=cfg['new_total_cap'],'single ALLNEW growth including target/copies/PG/Redis/logs'
 rows=processes();roots={os.getpid()}
 roots.add(os.getppid())
 if proc is not None:roots.add(proc.pid)
 if cluster is not None:
  for row in rows:
   if '/postgres ' in row[3] and (' -D '+str(cluster.data)) in row[3]:roots.add(row[0])
  if cluster.started and (cluster.data/'postmaster.pid').exists():roots.add(cluster.assert_postmaster())
 while True:
  added={r[0] for r in rows if r[1] in roots};expanded=roots|added
  if expanded==roots:break
  roots=expanded
 selected=[r for r in rows if r[0] in roots]
 for r in selected:owned_seen[r[0]]=r[3]
 n=len(selected);r=sum('/rustc ' in x[3] or x[3].endswith('/rustc') for x in selected)
 report['peak_processes']=max(report['peak_processes'],n);report['peak_rustc']=max(report['peak_rustc'],r)
 if not cleaning:
  assert n<=(cfg['compile_process_cap'] if stage=='build' else cfg['native_process_cap']),'process cap'
  assert r<=2,'rustc cap'
  rss=sum(x[4] for x in selected);report['peak_rss_bytes']=max(report.get('peak_rss_bytes',0),rss)
  assert rss<=(cfg['build_rss_cap'] if stage=='build' else cfg['native_rss_cap']),'RSS cap'
  if stage=='qualify':assert not any('/rustc ' in x[3] or '/cargo ' in x[3] for x in selected),'native compilation refused'

 return whole
def run(name,args,env=None,deadline=None,cwd=ROOT/'backend'):
 global proc
 if stage=='qualify':name='attempt-'+str(int(report['started']))+'-'+name
 deadline=deadline or (STOP if cleaning else COMPILE if stage=='build' else QUALIFY);guard(deadline)
 write('live-state.json',{'controller_pid':os.getpid(),'server_pgids':[p.pid for p in servers]+([redis.pid] if redis else []),'pg_root':str(cluster.root) if cluster else None,'pg_token':cluster.token if cluster else None,'pg_identity':list(cluster.identity) if cluster else None})
 phase={'name':name,'argv':list(map(str,args)),'exit_code':None,'started':time.time()};report['phases'].append(phase)
 with (OUT/(name+'.stdout')).open('wb') as so,(OUT/(name+'.stderr')).open('wb') as se:
  proc=subprocess.Popen(args,cwd=cwd,env=env,stdout=so,stderr=se,start_new_session=True)
  write('live-state.json',{'controller_pid':os.getpid(),'child_pgid':proc.pid,'server_pgids':[p.pid for p in servers]+([redis.pid] if redis else []),'pg_root':str(cluster.root) if cluster else None,'pg_token':cluster.token if cluster else None,'pg_identity':list(cluster.identity) if cluster else None})
  try:
   while proc.poll() is None:guard(deadline);time.sleep(.25 if stage=='qualify' else 1)
  finally:
   if proc.poll() is None:terminate(proc)
  phase['exit_code']=proc.returncode;phase['finished']=time.time()
 assert proc.returncode==0,name+' exit '+str(proc.returncode)
 proc=None;guard(deadline)
 return (OUT/(name+'.stdout')).read_text(errors='replace')
# The guardian must be the actual parent before any workload child exists.
assert 'guardian.py' in cmd(['ps','-p',str(os.getppid()),'-o','args='])
assert OUT.stat().st_uid==os.getuid() and OUT.stat().st_mode & 0o077==0
assert cmd(['git','rev-parse','HEAD'])==cfg['source']
assert cmd(['git','status','--porcelain'])==''
if stage=='build':
 assert shutil.disk_usage(OUT).free>=16*1024**3
 offer={'lease':LEASE,'accepted':cfg['accepted'],'target_baseline':size(TARGET),'target_files':allocations(TARGET)}
 write('offer.json',offer)
else:
 offer=json.loads((OUT/'offer.json').read_text())
 assert shutil.disk_usage(OUT).free>=12*1024**3

def state():
 write('live-state.json',{'controller_pid':os.getpid(),'child_pgid':proc.pid if proc else None,
  'server_pgids':[p.pid for p in servers]+([redis.pid] if redis else []),
  'pg_root':str(cluster.root) if cluster else None,'pg_token':cluster.token if cluster else None,
  'pg_identity':list(cluster.identity) if cluster else None,'copy_started':copy_started})
def interrupted(*_):raise KeyboardInterrupt()
signal.signal(signal.SIGTERM,interrupted)
signal.signal(signal.SIGINT,interrupted)
try:
 if stage=='build':
  names=cmd(['git','ls-files','-z','backend']).split('\0')
  source={n:sha(ROOT/n) for n in names if n and (ROOT/n).is_file()}
  report.update(source_commit=cfg['source'],source_sha256=source,controller_sha256=sha(__file__))
  paths=[cmd(['rustup','which','--toolchain','1.88.0',n]) for n in ['cargo','rustc']]+[str(PG/n) for n in ['initdb','pg_ctl','psql','createdb','postgres']]+['/usr/bin/openssl','/opt/homebrew/bin/redis-server',sys.executable]
  report['runtime_sha256']={p:sha(p) for p in paths}
  env={**os.environ,'CARGO_TARGET_DIR':str(TARGET),'CARGO_BUILD_JOBS':'2','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_INCREMENTAL':'0'}
  for n in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(n,None)
  report['artifacts']={};report['automatic_executables']=[]
  for owner in ['profile','friend','post','auth']:
   name='legacy_deletion_receipts' if owner=='auth' else 'committed_deletion_rpc'
   argv=['cargo','+1.88.0','test','--locked','--offline','-p',owner+'-service','--test',name,'--no-run','--message-format=json','--config','profile.dev.package.contracts-build.opt-level=2']
   data=run('cargo-'+owner,argv,env);events=[]
   for line in data.splitlines():
    try:events.append(json.loads(line))
    except ValueError:pass
   choices={e['executable']:e for e in events if e.get('reason')=='compiler-artifact' and e.get('executable') and e['target']['name']==name and e['profile']['test']}
   assert len(choices)==1,'one exact requested EXE'
   path,event=next(iter(choices.items()));path=pathlib.Path(path).resolve(strict=True)
   assert path.is_relative_to(TARGET.resolve()) and path.stat().st_size<=128*1024**2
   before=time.time();copy_started=before;state();dest=OUT/(owner+'-'+name);shutil.copy2(path,dest)
   assert time.time()-before<=30 and sha(dest)==sha(path),'bounded byte-identical copy'
   copy_started=None;state();guard(COMPILE)
   report['artifacts'][owner]={'path':str(dest),'sha256':sha(dest),'bytes':dest.stat().st_size,'original':str(path),'source_commit':cfg['source']}
   report['automatic_executables'] += [e['executable'] for e in events if e.get('reason')=='compiler-artifact' and e.get('executable')]
  assert sum(v['bytes'] for v in report['artifacts'].values())<=512*1024**2
  assert all(sha(ROOT/n)==v for n,v in source.items())
  report['status']='PASS';write('artifact-receipt.json',report)
 elif stage=='qualify':
  release=json.loads((OUT/'target-release.json').read_text());assert release['unconditional_release'] and release['target_handles']==[]
  B=json.loads((OUT/'artifact-receipt.json').read_text());assert B['status']=='PASS'
  assert all(sha(ROOT/n)==v for n,v in B['source_sha256'].items()) and all(sha(p)==v for p,v in B['runtime_sha256'].items())
  artifacts={k:pathlib.Path(a['path']) for k,a in B['artifacts'].items()}
  assert all(sha(artifacts[k])==a['sha256'] for k,a in B['artifacts'].items())
  sys.path.insert(0,str(ROOT/'backend/scripts/mcr'));import migration_smoke as migration
  def guarded_execute(argv,*,env,phase,timeout=60,allowed=(0,),cwd=None):
   data=run('helper-'+phase+'-'+str(len(report['phases'])),list(map(str,argv)),env,min(STOP if cleaning else QUALIFY,time.time()+timeout),cwd)
   return subprocess.CompletedProcess(argv,0,data,'')
  migration.execute=guarded_execute
  cluster=migration.Cluster({n:migration.executable(PG/n) for n in migration.PG_TOOLS},OUT/'pg')
  state();pki=cluster.root/'pki';pki.mkdir(mode=0o700)
  ca=pki/'ca.conf';ca.write_text('[req]\ndistinguished_name=dn\n[dn]\n[v3_ca]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n')
  cluster.env['OPENSSL_CONF']=str(ca)
  def openssl(*args):run('pki-'+str(len(report['phases'])),['/usr/bin/openssl',*map(str,args)],cluster.env,QUALIFY,pki)
  openssl('req','-x509','-newkey','rsa:2048','-nodes','-keyout','ca.key','-out','ca.crt','-days','2','-subj','/CN=MCR legacy deletion fixture','-config',ca,'-extensions','v3_ca')
  for role in ['auth','profile','friend','post','gateway']:
   ext=pki/(role+'.ext');ext.write_text('basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:'+role+'.svc.internal,URI:spiffe://internal/'+role+'\n')
   openssl('req','-new','-newkey','rsa:2048','-nodes','-keyout',role+'.key','-out',role+'.csr','-subj','/CN='+role+'.svc.internal')
   openssl('x509','-req','-in',role+'.csr','-CA','ca.crt','-CAkey','ca.key','-CAcreateserial','-out',role+'.crt','-days','2','-extfile',ext)
  for key in pki.glob('*.key'):key.chmod(0o600)
  cluster.init()
  with (cluster.data/'postgresql.conf').open('a') as f:f.write('\nmax_connections=20\nmax_worker_processes=2\nmax_parallel_workers=0\nautovacuum=off\n')
  guard(QUALIFY);cluster.start();state()
  assert int(cluster.sql('postgres','show server_version_num'))//10000==16
  for owner in ['profile','friend','post','auth']:cluster.sql('postgres','create database mcr_legacy_'+owner+' template template0')
  # Private bounded Redis, TCP only on a kernel-assigned loopback port, no data persistence.
  import socket
  with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
  redis_log=(OUT/'redis.log').open('wb')
  redis=subprocess.Popen(['/opt/homebrew/bin/redis-server','--bind','127.0.0.1','--port',str(port),'--save','','--appendonly','no','--maxmemory','32mb','--maxmemory-policy','allkeys-lru','--dir',str(cluster.root)],stdout=redis_log,stderr=redis_log,start_new_session=True);state()
  env={**cluster.env,'TOKIO_WORKER_THREADS':'2','RUST_BACKTRACE':'0','LEGACY_TEST_OWNER_ROOT':str(cluster.root),'LEGACY_TEST_OWNER_TOKEN':cluster.token,'LEGACY_TEST_OWNER_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}','POST_REDIS_URL':f'redis://127.0.0.1:{port}/','POST_EXTERNAL_PUBLISHER':'disabled','POST_FRIEND_GRPC_ENDPOINT':'https://127.0.0.1:9','POST_PROFILE_GRPC_ENDPOINT':'https://127.0.0.1:9','POST_NOTIFICATION_GRPC_ENDPOINT':'https://127.0.0.1:9','POST_GATEWAY_GRPC_ENDPOINT':'https://127.0.0.1:9','FRIEND_PROFILE_GRPC_ENDPOINT':'https://127.0.0.1:9'}
  for owner in ['profile','friend','post','auth']:
   env[owner.upper()+'_DATABASE_URL']=cluster.url('mcr_legacy_'+owner)
   env[owner.upper()+'_RUNTIME_MODE']='parity'
   env[owner.upper()+'_SEED_ENABLED']='false'
   env[owner.upper()+'_DB_POOL_MAX_CONNECTIONS']='3'
  def case(owner,name,ignored=True):
   args=[str(artifacts[owner])]+(['--ignored'] if ignored else [])+['--exact',name,'--test-threads=1','--nocapture']
   data=run(owner+'-'+name,args,env,QUALIFY)
   assert '1 passed; 0 failed' in data,'requested test did not execute'
  for owner in ['profile','friend','post']:case('auth',owner+'_binding_rejects_boolean_partial_owner_and_every_rebound',False)
  for owner in ['profile','friend','post']:case(owner,'populated_actual_rpc_atomic_replay_rebound_fence_and_original_clock')
  def start_servers(label):
   for owner in ['profile','friend','post']:
    for name in [owner+'-endpoint',owner+'-stop']:
     p=cluster.root/name
     if p.exists():p.unlink()
    so=(OUT/(label+'-'+owner+'.stdout')).open('wb');se=(OUT/(label+'-'+owner+'.stderr')).open('wb')
    guard(QUALIFY)
    child=subprocess.Popen([str(artifacts[owner]),'--ignored','--exact','serve_actual_owner_for_auth_worker','--test-threads=1','--nocapture'],cwd=ROOT/'backend',env=env,stdout=so,stderr=se,start_new_session=True)
    servers.append(child);state();deadline=min(QUALIFY,time.time()+20)
    while not (cluster.root/(owner+'-endpoint')).exists():
     guard(deadline);assert child.poll() is None,'actual RPC fixture exited';time.sleep(.05)
  def stop_servers():
   for owner in ['profile','friend','post']:(cluster.root/(owner+'-stop')).touch()
   for child in servers:
    while child.poll() is None:guard(QUALIFY);time.sleep(.05)
    assert child.returncode==0,'actual RPC shutdown'
   servers.clear();state()
  start_servers('before-restart')
  case('auth','actual_eight_step_worker_commits_only_three_real_owners_and_recovers_rollback')
  stop_servers();before=cluster.assert_postmaster();cluster.stop();cluster.start();after=cluster.assert_postmaster()
  assert before!=after;report['restart']={'actual':True,'before_pid':before,'after_pid':after}
  start_servers('after-restart');case('auth','actual_restart_preserves_all_three_receipts_and_five_incomplete_steps');stop_servers()
  report['ledgers']={}
  for owner in ['profile','friend','post','auth']:
   manifests=migration.migration_manifest(ROOT/'backend',owner+'-service')
   report['ledgers'][owner]={'count':migration.check_ledger(cluster,'mcr_legacy_'+owner,manifests),'migration_sha384':manifests}
  report['prepared_after']=int(cluster.sql('postgres','select count(*) from pg_prepared_xacts'));assert report['prepared_after']==0
  assert all(sha(ROOT/n)==v for n,v in B['source_sha256'].items());report['status']='PASS'
 else:raise AssertionError('stage')
except BaseException as error:report['failure']=str(error)
finally:
 cleaning=True;terminate(proc);proc=None
 for p in servers:terminate(p)
 servers.clear();terminate(redis);redis=None
 if stage=='build':
  try:
   # Release on PASS or FAIL; preserve the warm target and all unique evidence.
   free=shutil.disk_usage(OUT).free
   handlelog=OUT/'target-handles.txt'
   with handlelog.open('wb') as f:
    proc=subprocess.Popen(['/usr/sbin/lsof','-nP','+D',str(TARGET)],stdout=f,stderr=subprocess.STDOUT,start_new_session=True)
    while proc.poll() is None:guard(STOP);time.sleep(.1)
   assert proc.returncode in (0,1),'target handle inspection failed'
   lines=handlelog.read_text().splitlines();assert not lines,'target still open'
   proc=None
   growth=sum(max(v-offer['target_files'].get(k,0),0) for k,v in allocations(TARGET).items())
   write('target-release.json',{'lease':LEASE,'unconditional_release':True,'target_handles':[],'at':time.time(),'target_growth_bytes':growth,'target_preserved':True,'free':free})
  except BaseException as error:report['target_release_failure']=str(error)
 if cluster is not None:
  try:
   if (cluster.root/'postgres.log').exists():shutil.copy2(cluster.root/'postgres.log',OUT/('postgres-'+str(int(report['started']))+'.log'))
   cluster.cleanup();report['owned_pg_removed']=not cluster.root.exists()
  except BaseException as error:report['cleanup_failure']=str(error)
 report['finished']=time.time();report['elapsed']=report['finished']-report['started'];report['output_bytes']=size(OUT);report['owned_seen']=owned_seen
 write('live-state.json',{'controller_pid':os.getpid(),'finished':True})
 write(stage+'-result-'+str(int(report['started']))+'.json',report)
 print(json.dumps({k:v for k,v in report.items() if k not in ['source_sha256','runtime_sha256','phases','automatic_executables','owned_seen']}))
sys.exit(0 if report['status']=='PASS' and 'cleanup_failure' not in report and 'target_release_failure' not in report else 1)
