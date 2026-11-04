#!/usr/bin/env python3
"""Independent deadline/disk/all-descendant guard; preserves explicit offer authority."""
import pathlib,subprocess,sys,os,time,json,shutil,signal,stat
OUT=pathlib.Path(sys.argv[1]).resolve();C=json.loads((OUT/'accepted-offer.json').read_text());stage=sys.argv[2];assert C['explicit_accept']
stop=C['compile_stop'] if stage=='build' else C['qualify_stop'];full=C['full_stop'];start=time.time();p=None;failure=None;peak=0;peakrss=0;maxprocesses=0;minfree=shutil.disk_usage(OUT).free

def size(path):return sum(allocations(path).values())
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
def check():
 global peak,minfree,peakrss,maxprocesses
 now=time.time();assert now<stop,'independent phase deadline'
 state=json.loads((OUT/'live-state.json').read_text()) if (OUT/'live-state.json').exists() else {}
 if state.get('copy_started'):assert now<state['copy_started']+30,'independent copy freeze deadline'
 free=shutil.disk_usage(OUT).free;minfree=min(minfree,free);assert free>=8*1024**3,'independent 8GiB floor'
 o=json.loads((OUT/'offer.json').read_text()) if (OUT/'offer.json').exists() else None
 growth=sum(max(v-o['target_files'].get(k,0),0) for k,v in allocations(C['target']).items()) if stage=='build' and o else (json.loads((OUT/'target-release.json').read_text())['target_growth_bytes'] if stage=='qualify' else 0)
 outside=size(OUT);source=max(0,size(C['wt'])-C['wt_baseline'])+max(0,size(C['git_dir'])-C['git_baseline'])
 total=C['carry']+growth+outside+source;peak=max(peak,total)
 assert total<=C['cap'] and growth<=C['target_growth_cap'] and outside<=C['outside_cap'] and growth+outside<=C['new_total_cap'],'independent whole/growth/outside cap'
 rows=[]
 for line in subprocess.check_output(['ps','-axo','pid=,ppid=,rss=,args='],text=True).splitlines():
  parts=line.strip().split(None,3)
  if len(parts)==4:rows.append((int(parts[0]),int(parts[1]),parts[3],int(parts[2])*1024))
 roots={os.getpid()}
 if (OUT/'live-state.json').exists():
  state=json.loads((OUT/'live-state.json').read_text());root=state.get('pg_root')
  if root:
   for row in rows:
    if '/postgres ' in row[2] and (' -D '+str(pathlib.Path(root)/'data')) in row[2]:roots.add(row[0])
 while True:
  new=roots|{r[0] for r in rows if r[1] in roots}
  if new==roots:break
  roots=new
 owned=[r for r in rows if r[0] in roots];maxprocesses=max(maxprocesses,len(owned));peakrss=max(peakrss,sum(r[3] for r in owned));assert peakrss<=(C['build_rss_cap'] if stage=='build' else C['native_rss_cap']),'independent RSS cap'
 assert len(owned)<=(C['compile_process_cap'] if stage=='build' else C['native_process_cap']),'independent process cap'
 assert sum('/rustc ' in r[2] for r in owned)<=2,'independent rustc cap'

try:
 check()
 p=subprocess.Popen([sys.executable,str(pathlib.Path(__file__).with_name('controller.py')),str(OUT),stage],cwd=C['wt'],start_new_session=True)
 while p.poll() is None:check();time.sleep(.5)
except BaseException as e:
 failure=str(e)
 if p and p.poll() is None:
  p.send_signal(signal.SIGTERM)
  try:p.wait(timeout=min(8,max(1,full-time.time())))
  except subprocess.TimeoutExpired:p.kill();p.wait()
 # Emergency cleanup only of the private root whose token/inode marker matches
 # the controller's owned Cluster. Never attach, stop or remove a foreign root.
 state=json.loads((OUT/'live-state.json').read_text()) if (OUT/'live-state.json').exists() else {}
 for pgid in [state.get('child_pgid'),*state.get('server_pgids',[])]:
  if pgid:
   try:os.killpg(pgid,signal.SIGTERM)
   except ProcessLookupError:pass
 root=pathlib.Path(state['pg_root']) if state.get('pg_root') else None
 if root and root.exists():
  st=root.lstat();marker=json.loads((root/'mcr-owner.json').read_text())
  assert not root.is_symlink() and [st.st_dev,st.st_ino]==state['pg_identity'] and st.st_uid==os.getuid() and marker=={'token':state['pg_token'],'uid':os.getuid()}
  pf=root/'data/postmaster.pid'
  if pf.exists():
   lines=pf.read_text().splitlines();assert lines[1]==str(root/'data') and lines[4]==str(root/'socket')
   subprocess.run(['/opt/homebrew/opt/postgresql@16/bin/pg_ctl','-D',str(root/'data'),'-w','-t','10','-m','immediate','stop'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=min(15,max(1,full-time.time())),check=True)
  assert not pf.exists();shutil.rmtree(root)
finally:
 result={'stage':stage,'independent':True,'started':start,'finished':time.time(),'controller_exit':p.returncode if p else None,'failure':failure,'peak_whole':peak,'minimum_free':minfree,'peak_rss_bytes':peakrss,'maximum_processes':maxprocesses}
 (OUT/('watchdog-'+stage+'-'+str(int(start))+'.json')).write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result))
sys.exit(0 if p and p.returncode==0 and failure is None else 1)
