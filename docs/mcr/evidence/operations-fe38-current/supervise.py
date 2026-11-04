"""Owner-local guard. Starts before the browser child; never controls peer work."""
from pathlib import Path
import subprocess, os, json, time, signal, shutil, sys
repo=Path(__file__).resolve().parents[4]
out=Path(__file__).resolve().parent
node=Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
link=repo/'frontend/node_modules'
target='/Users/lhs/Desktop/mcr/frontend/node_modules'
job=os.environ['MCR_FE38_JOB']; deadline=float(os.environ['MCR_FE38_DEADLINE']); cleanup_deadline=deadline+60
round_name=sys.argv[1]; kind=os.environ['MCR_FE38_KIND']; assert kind in ['source','native']; temp=Path('/private/tmp/mcr-fe38-'+round_name)
assert round_name.replace('-','').isalnum()
assert time.time()<deadline and deadline-time.time()<=600
assert not link.exists() and not link.is_symlink() and not temp.exists()
assert shutil.disk_usage(repo).free>=12*1024**3
outstanding=int(os.environ['MCR_FE38_OUTSTANDING'])
assert shutil.disk_usage(repo).free-outstanding-128*1024**2>=9*1024**3
def du(path):return int(subprocess.check_output(['du','-sk',str(path)],text=True).split()[0])*1024
gitdir=Path(subprocess.check_output(['git','rev-parse','--git-dir'],cwd=repo,text=True).strip()).resolve()
checkout_start=du(repo);deps_start=du(target);gitmeta_start=du(gitdir)
ids=[line.split()[0] for line in subprocess.check_output(['git','rev-list','--objects','0624d581325946c593b5bbe00c4cfde0213ad538..HEAD'],cwd=repo,text=True).splitlines()]
objrows=subprocess.check_output(['git','cat-file','--batch-check=%(objectname) %(objectsize:disk)'],input='\n'.join(ids)+'\n',cwd=repo,text=True).splitlines()
gitobjects=sum(int(row.split()[1]) for row in objrows)
assert checkout_start+deps_start+gitmeta_start+gitobjects+128*1024**2<2*1024**3
proc=None; info={'job':job,'round':round_name,'started':time.time(),'deadline':deadline,'cleanupDeadline':cleanup_deadline,'ownedGitReservation':8*1024**2,'peaks':{},'floor':8*1024**3,'outstandingGrowthReservation':outstanding,'retainedCheckoutStart':checkout_start,'retainedReadOnlyDepsStart':deps_start,'ownGitMetadataStart':gitmeta_start,'ownGitObjectsStart':gitobjects}
def bytes_at(path):
 total=0
 for root,dirs,files in os.walk(path,followlinks=False):
  for name in files:
   try: total+=(Path(root)/name).lstat().st_size
   except FileNotFoundError: pass
 return total
def measure():
 free=shutil.disk_usage(repo).free
 info['minFree']=min(info.get('minFree',free),free)
 checkout=du(repo);deps=du(target);gitmeta=du(gitdir)
 values={'durable':bytes_at(out),'temp':bytes_at(temp)}
 values['new']=max(0,checkout-checkout_start)+max(0,deps-deps_start)+max(0,gitmeta-gitmeta_start)+values['temp']+info['ownedGitReservation']
 values['whole']=checkout+deps+gitmeta+gitobjects+values['temp']+info['ownedGitReservation']
 caps={'durable':24*1024**2,'temp':96*1024**2,'new':128*1024**2,'whole':2*1024**3}
 for key,value in values.items():
  info['peaks'][key]=max(info['peaks'].get(key,0),value)
  if value>=caps[key]: return key+' byte cap'
 if free<8*1024**3: return '8GiB running floor'
 if time.time()>=deadline-3: return 'absolute work deadline'
 if proc:
  rows=[line.split() for line in subprocess.check_output(['ps','-axo','pid,ppid,rss'],text=True).splitlines()[1:]]
  owned={proc.pid}
  for _ in range(12):
   descendants={int(row[0]) for row in rows if len(row)==3 and int(row[1]) in owned}
   if descendants.issubset(owned):break
   owned.update(descendants)
  group=[row for row in rows if len(row)==3 and int(row[0]) in owned]
  info['lastDescendants']=sorted(owned)
  count=len(group);rss=sum(int(row[2])*1024 for row in group)
  info['peakPids']=max(info.get('peakPids',0),count);info['peakRss']=max(info.get('peakRss',0),rss)
  if count>(16 if kind=='source' else 24) or rss>4*1024**3:return 'PID/RSS cap'
 return None
try:
 link.symlink_to(target,target_is_directory=True);temp.mkdir()
 env=dict(os.environ,PATH=str(node.parent)+':'+os.environ['PATH'],TMPDIR=str(temp),MCR_FE38_TEMP=str(temp),MCR_FE38_ROUND=round_name)
 with (out/(round_name+'.log')).open('w') as log:
  proc=subprocess.Popen([str(node),str(out/(kind+'.mjs'))],cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);info['pid']=proc.pid
  while proc.poll() is None:
   reason=measure()
   if reason:
    info['stopReason']=reason
    for owned_pid in reversed(info.get('lastDescendants',[])):
     try:os.kill(owned_pid,signal.SIGTERM)
     except ProcessLookupError:pass
    os.killpg(proc.pid,signal.SIGTERM)
    try:proc.wait(timeout=1)
    except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL)
    break
   time.sleep(.5)
  info['exitCode']=proc.wait()
finally:
 if proc:
  try:os.killpg(proc.pid,signal.SIGTERM)
  except ProcessLookupError:pass
 if link.is_symlink() and os.readlink(link)==target:link.unlink()
 if temp.exists():shutil.rmtree(temp)
 info.update(finished=time.time(),ownLinkAbsent=not link.exists(),ownTempAbsent=not temp.exists(),durableBytes=bytes_at(out),cleanupWithinDeadline=time.time()<cleanup_deadline,free=shutil.disk_usage(repo).free)
 (out/('supervisor-'+round_name+'.json')).write_text(json.dumps(info,indent=2)+'\n');print(json.dumps(info))
sys.exit(info.get('exitCode',1))
