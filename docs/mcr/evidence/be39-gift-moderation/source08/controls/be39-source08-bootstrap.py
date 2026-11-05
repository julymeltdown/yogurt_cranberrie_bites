import os,sys,json,time,hashlib,subprocess,shutil,tempfile
from pathlib import Path
B=Path('/private/tmp/mcr-gift-public-ready-z8ch5q3t')
ROOT=Path('/Users/lhs/Desktop/mcr-worktrees/be-gifts-live')
OWNER='01a1a267-37a0-7723-a4e9-a3a6f5b27f52'
DIRECTORY=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')
GIB=1024**3
assert Path.cwd()==ROOT
r=json.loads((B/'be39-source08-readset.json').read_text())
for path,pin in r['private_control_paths'].items(): assert hashlib.sha256(Path(path).read_bytes()).hexdigest()==pin
assert hashlib.sha256((B/'be39-source08-peer-releases.json').read_bytes()).hexdigest()==r['peer_releases_sha256']
actual=subprocess.check_output(['/bin/ps','-ww','-p',str(os.getpid()),'-o','comm='],text=True,timeout=2).strip()
birth=subprocess.check_output(['/bin/ps','-p',str(os.getpid()),'-o','lstart='],text=True,timeout=2).strip()
assert actual==r['actual_python_executable'] and hashlib.sha256(Path(actual).read_bytes()).hexdigest()==r['actual_python_executable_sha256']
free=shutil.disk_usage(B).free
assert free>=11*GIB and free-5*GIB-16*1024**2>=9*GIB
accepted=time.time()
assert accepted<1793845200
output=Path(tempfile.mkdtemp(prefix='mcr-gift-be39-source08-',dir='/private/tmp'));output.chmod(0o700)
a={'allocation_id':'GIFT-SOURCE08-0159','actual_accept_epoch':accepted,'work_deadline_epoch':accepted+120,'full_deadline_epoch':accepted+180,'output_dir':str(output),'allocation_output_root':str(output),'source_revision':r['source_revision'],'private_packet_commit':r['private_packet_commit'],'retained_carry_bytes':r['retained_carry_bytes'],'whole_cap_bytes':48*GIB,'new_limit_bytes':16*1024**2,'process_cap':6,'rss_cap_bytes':512*1024**2,'other_growth_bytes':5*GIB,'free_before_accept_bytes':free,'readset_sha256':hashlib.sha256((B/'be39-source08-readset.json').read_bytes()).hexdigest(),'private_control_pins':r['private_control_paths'],'target_access':'NONE; saved baseline only','global_controls':{'owner_thread_id':OWNER,'receipt_directory':str(DIRECTORY),'membership_path':'/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/resource-membership-current.json','peer_releases_path':str(B/'be39-source08-peer-releases.json'),'peer_releases_sha256':hashlib.sha256((B/'be39-source08-peer-releases.json').read_bytes()).hexdigest()}}
record={'ownerThreadId':OWNER,'offerId':a['allocation_id'],'cohort':'GiftBE39Source08','state':'ACCEPTED','acceptedAtEpoch':accepted,'workDeadlineEpoch':accepted+120,'fullDeadlineEpoch':accepted+180,'kind':'source','processCap':6,'rssCapBytes':512*1024**2,'remainingNewUpperBytes':16*1024**2,'controller':{'pid':os.getpid(),'birth':birth,'executable':actual,'script':str(Path(__file__).resolve()),'cwd':str(Path.cwd()),'outputRoot':str(output)},'guardControllers':[],'receiptPaths':{'accept':str(B/'be39-source08-actual-accept.json'),'full':str(output/'FULL.json')},'phase':'ACTUAL_BOOTSTRAP_BEFORE_CENSUS'}
def atomic(path,value):
 temp=path.with_suffix('.267.tmp')
 with os.fdopen(os.open(temp,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'w') as stream:json.dump(value,stream,indent=2)
 os.replace(temp,path)
atomic(DIRECTORY/(OWNER+'.json'),record)
atomic(B/'be39-source08-actual-accept.json',a)
print(json.dumps(record),flush=True)
try:
 old=json.loads((B/'be39-source08-outside-census.json').read_text());paths=[Path(x['root']) for x in old['rows']]
 for extra in (Path('/Library/Developer/CommandLineTools'),Path('/usr/bin/git'),Path('/bin/ps')):
  if extra.exists() and extra not in paths:paths.append(extra)
 rows=[]
 for path in paths:
  assert time.time()<accepted+120 and shutil.disk_usage(output).free>=8*GIB
  info=path.lstat();size=int(subprocess.check_output(['/usr/bin/du','-sk',str(path)],text=True,timeout=8).split()[0])*1024
  rows.append({'root':str(path),'allocated_bytes':size,'dev':info.st_dev,'ino':info.st_ino})
 conservative=sum(x['allocated_bytes'] for x in rows)+19550818304+old['retained_outside_max']+2*GIB
 carry=max(43203280896+2*GIB,conservative,r['retained_carry_bytes'])
 assert carry+16*1024**2<=48*GIB
 peers=[json.loads(path.read_text()) for path in DIRECTORY.glob('*.json')]
 actual_future=sum(p.get('remainingNewUpperBytes',0) for p in peers if p.get('state')=='ACCEPTED' and p.get('ownerThreadId')!=OWNER)
 other=max(5*GIB,int(2.5*GIB)+2*GIB+actual_future)
 carry=max(carry,43203280896+2*GIB+actual_future)
 assert carry+16*1024**2<=48*GIB
 free=shutil.disk_usage(output).free
 assert free>=11*GIB and free-other-16*1024**2>=9*GIB
 census={'observed_epoch':time.time(),'rows':rows,'fresh_conservative_union':conservative,'retained_carry_bytes':carry,'post_observed_whole_lower_bound':43203280896,'future_Root_bytes':2*GIB,'ordinary_future_bytes':int(2.5*GIB),'actual_peer_future_bytes':actual_future,'other_growth_bytes':other,'saved_target_bytes_NO_CURRENT_READ':19550818304,'target_access':'NONE','no_history_reset':True}
 atomic(output/'outside-census.json',census)
 a.update(retained_carry_bytes=carry,other_growth_bytes=other,outside_census_sha256=hashlib.sha256((output/'outside-census.json').read_bytes()).hexdigest())
 atomic(B/'be39-source08-actual-accept.json',a)
 # Truthful transition: the receipt names this bootstrap until exec occurs.
 # The runner publishes its own observed identity before its expensive census.
 os.execve(sys.executable,[sys.executable,'-B',str(B/'be39-source08-runner.py')],dict(os.environ,PYTHONDONTWRITEBYTECODE='1'))
except BaseException as error:
 failure={'allocation_id':a['allocation_id'],'actual_accept_epoch':accepted,'actual_full_epoch':time.time(),'result':'BOOTSTRAP_FAIL_NO_WORKLOAD','error':type(error).__name__+': '+str(error),'all_owned_released':False,'controller_exit_pending':True,'remaining_owned':[os.getpid()],'target_access':'NONE'}
 atomic(output/'bootstrap-failure.json',failure)
 raise
