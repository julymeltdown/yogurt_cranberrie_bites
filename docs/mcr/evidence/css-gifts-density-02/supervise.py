"""Owner-local guard. Starts before the browser child; never controls peer work."""
from pathlib import Path
import subprocess, os, json, time, signal, shutil, sys
repo=Path(__file__).resolve().parents[4]
out=Path(__file__).resolve().parent
node=Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
link=repo/'frontend/node_modules'
target='/Users/lhs/Desktop/mcr/frontend/node_modules'
job=os.environ['MCR_GIFTS_JOB']; deadline=float(os.environ['MCR_GIFTS_DEADLINE']); cleanup_deadline=deadline+60
round_name=sys.argv[1]; temp=Path('/private/tmp/mcr-css-gifts-'+round_name)
assert round_name.replace('-','').isalnum()
assert time.time()<deadline and deadline-time.time()<=600
assert not link.exists() and not link.is_symlink() and not temp.exists()
assert shutil.disk_usage(repo).free>=12*1024**3
assert shutil.disk_usage(repo).free-144*1024**2>=9*1024**3
proc=None; info={'job':job,'round':round_name,'started':time.time(),'deadline':deadline,'cleanupDeadline':cleanup_deadline,'ownedGitReservation':1024**2,'peaks':{},'floor':8*1024**3}
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
 values={'durable':bytes_at(out),'temp':bytes_at(temp)};values['whole']=sum(values.values())+info['ownedGitReservation']+(link.lstat().st_size if link.is_symlink() else 0)
 caps={'durable':24*1024**2,'temp':120*1024**2,'whole':144*1024**2}
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
  if count>24 or rss>4*1024**3:return 'PID/RSS cap'
 return None
try:
 link.symlink_to(target,target_is_directory=True);temp.mkdir()
 env=dict(os.environ,PATH=str(node.parent)+':'+os.environ['PATH'],TMPDIR=str(temp),MCR_GIFTS_TEMP=str(temp),MCR_GIFTS_ROUND=round_name)
 with (out/(round_name+'.log')).open('w') as log:
  proc=subprocess.Popen([str(node),str(out/'native.mjs')],cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);info['pid']=proc.pid
  while proc.poll() is None:
   reason=measure()
   if reason:
    info['stopReason']=reason;os.killpg(proc.pid,signal.SIGTERM)
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
