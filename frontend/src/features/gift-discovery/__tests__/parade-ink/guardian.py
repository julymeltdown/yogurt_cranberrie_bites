"""Independent PI04 guardian: separate process, independent observations, fail-closed kill domain."""
import json, os, signal, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode=True
from membership import sample as sample_membership, publish_census
configuration=Path(sys.argv[1]);scratch=configuration.parent
bootstrap={'pid':os.getpid(),'birth':' '.join(subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart='],text=True).split()),'executable':subprocess.check_output(['ps','-p',str(os.getpid()),'-o','comm='],text=True).strip(),'script':str(Path(__file__).resolve()),'cwd':os.getcwd()}
(scratch/'guardian-bootstrap.json').write_text(json.dumps(bootstrap))
bootstrap_deadline=time.monotonic()+15
while not (scratch/'guardian-metadata-published').exists():
 assert time.monotonic()<bootstrap_deadline,'Guardian identity was not published before census'
 time.sleep(.05)
cfg=json.loads(configuration.read_text());root=Path(cfg['root']);scratch=Path(cfg['scratch']);owner=cfg['primaryPid']
def external_bytes():
 total=0
 for value in cfg.get('externalFiles',[]):
  try:total+=Path(value).stat().st_blocks*512
  except FileNotFoundError:pass
 return total
def footprint():
 return sum(int(line.split()[0])*1024 for line in subprocess.check_output(['du','-sk',str(root),cfg['git'],str(scratch)],text=True).splitlines())+external_bytes()
def sample():
 table={}
 owned_chrome=set()
 for line in subprocess.check_output(['ps','-axo','pid=,ppid=,rss=,command='],text=True).splitlines():
  pid,parent,rss,command=line.strip().split(None,3);p=int(pid);table[p]=(int(parent),int(rss))
  if '/Applications/Google Chrome.app/' in command and str(scratch) in command:owned_chrome.add(p)
 members={owner}|owned_chrome;changed=True
 while changed:
  additions={p for p,(parent,_) in table.items() if parent in members}-members;changed=bool(additions);members|=additions
 # The ps helper is itself present in this ancestry snapshot. Threads share
 # process RSS and must not be counted again as processes or memory.
 whole=sum(int(line.split()[0])*1024 for line in subprocess.check_output(['du','-sk',*cfg['wholeRoots']],text=True).splitlines())+external_bytes() if cfg.get('wholeRoots') else 0
 return {'free':os.statvfs(root).f_bavail*os.statvfs(root).f_frsize,'all':len(members),'rss':sum(table.get(p,(0,0))[1] for p in members)*1024,'new':cfg.get('carriedNew',0)+max(0,footprint()-cfg['baselineBytes']),'whole':whole,'pids':sorted(members)}
ready=scratch/'guardian-live.json'; stop=scratch/'guardian-stop'; result=scratch/'guardian-final.json';last=None;maximum={'all':0,'rss':0,'new':0,'whole':0}
try:
 while not stop.exists():
  publish_census(Path(cfg['output'])/'guardian-census.json',sample_membership())
  os.kill(owner,0);last=sample()
  for key in maximum:maximum[key]=max(maximum[key],last[key])
  assert last['free']>=8*1024**3,'guardian live free floor'
  assert last['all']<=cfg['all'] and last['rss']<=cfg['rss'],'guardian ALL/RSS cap'
  assert last['new']<=cfg['new'],'guardian total NEW cap'
  assert last['whole']<=cfg.get('wholeLimit',0),'guardian complete WHOLE cap'
  assert time.time()<cfg['fullDeadline'],'guardian finite FULL deadline'
  assert time.time()-(scratch/'primary-live.json').stat().st_mtime<5,'primary heartbeat lost'
  ready.write_text(json.dumps({'pid':os.getpid(),'at':time.time(),'sample':last}))
  time.sleep(.3)
 result.write_text(json.dumps({'status':'STOPPED','pid':os.getpid(),'last':last,'maximum':maximum}))
except BaseException as error:
 result.write_text(json.dumps({'status':'FAILED','pid':os.getpid(),'failure':str(error),'last':last,'maximum':maximum}))
 if (scratch/'workload-pgid').exists():
  try:os.killpg(int((scratch/'workload-pgid').read_text()),signal.SIGTERM)
  except ProcessLookupError:pass
 try:os.kill(owner,signal.SIGTERM)
 except ProcessLookupError:pass
 raise
