"""Isolated metadata regression preparation; NOT authoritative resource receipts.

Run only inside an allocated outer round after BOTH real guards are live.
Uses actual PS executable/birth/ancestry for this fixture process, isolated JSON
cases, no peer mutations, no target access and no product authority substitutes.
"""
import sys,os,json,time,subprocess,hashlib,importlib.util
from pathlib import Path
sys.dont_write_bytecode=True
BASE=Path('/private/tmp/mcr-gift-public-ready-z8ch5q3t')
spec=importlib.util.spec_from_file_location('gift_guard07_fixture',BASE/'be39_parse_guard_source08.py')
guard=importlib.util.module_from_spec(spec);spec.loader.exec_module(guard)
root=Path(sys.argv[1]);root.mkdir(mode=0o700)
OWNER='01a1a267-37a0-7723-a4e9-a3a6f5b27f52'
OTHER='01a19e5c-e9c6-7973-a53b-a6d19141620f'
birth=subprocess.check_output(['/bin/ps','-p',str(os.getpid()),'-o','lstart='],text=True).strip()
executable=subprocess.check_output(['/bin/ps','-ww','-p',str(os.getpid()),'-o','comm='],text=True).strip()
started=time.time();results=[]
def case(name, observation=False, legacy=False, release=False, wrong_executable=False):
 directory=root/name;directory.mkdir(mode=0o700);receipts=directory/'receipts';receipts.mkdir(mode=0o700)
 # Isolated test metadata uses a fixture-local clock and offer. These files
 # never enter allocator discovery or release any real reservation.
 record={'ownerThreadId':OWNER,'offerId':'ISOLATED_METADATA_FIXTURE_ONLY','state':'ACCEPTED','kind':'source','acceptedAtEpoch':started,'workDeadlineEpoch':started+60,'fullDeadlineEpoch':started+90,'processCap':6,'rssCapBytes':512*1024**2,'remainingNewUpperBytes':0,'controller':{'pid':os.getpid(),'birth':birth,'executable':'/not-actual-runtime' if wrong_executable else executable,'script':str(Path(__file__).resolve()),'cwd':str(Path.cwd()),'outputRoot':str(directory)},'guardControllers':[],'receiptPaths':{'accept':None,'full':None}}
 (receipts/(OWNER+'.json')).write_text(json.dumps(record))
 if observation:
  (receipts/(OTHER+'.json')).write_text(json.dumps({'ordinaryObservation':{'actualpid':0,'birth':'nonlive fixture only','role':'ordinary isolated metadata fixture','booking':{'fixture':True}}}))
 entries=[{'ownerThreadId':OTHER,'offerId':'ISOLATED_OLD_METADATA_ONLY','state':'ACCEPTED','acceptedAtEpoch':started-1,'kind':'source','processCap':6,'rssCapBytes':512*1024**2,'remainingNewUpperBytes':0}] if legacy else []
 membership=directory/'membership.json';membership.write_text(json.dumps({'issuer':'01a1a21d-39ec-7912-84cb-910058e7a110','entries':entries}))
 releases=directory/'releases.json';releases.write_text(json.dumps({'releases':{'ISOLATED_OLD_METADATA_ONLY':{'owner':OTHER,'actual_accept':started-1,'all_owned_released':True,'basis':'ISOLATED_TEST_ONLY_NOT_A_REAL_RELEASE'}} if release else {}}))
 config={'controller_pid':os.getpid(),'owner_thread_id':OWNER,'actual_accept_epoch':started,'receipt_directory':str(receipts),'membership_path':str(membership),'peer_releases_path':str(releases),'peer_releases_sha256':hashlib.sha256(releases.read_bytes()).hexdigest(),'output':str(directory)}
 try:
  records, observations=guard.decode_receipt_files(list(receipts.glob('*.json')))
  assert records[OWNER]['offerId']=='ISOLATED_METADATA_FIXTURE_ONLY'
  if observation:
   assert OTHER not in records and len(observations)==1,'ordinary-only invented a workload'
  guard.resolve_legacy_receipts(records,{'entries':entries},json.loads(releases.read_text())['releases'])
  if wrong_executable:
   raw=subprocess.check_output(['/bin/ps','-ww','-p',str(os.getpid()),'-o','pid=,ppid=,rss=,lstart=,command='],text=True)
   fields=raw.strip().split(maxsplit=8)
   actual={int(fields[0]):(int(fields[1]),int(fields[2])*1024,' '.join(fields[3:8]),fields[8])}
   guard.validate_actual_process_descriptor(os.getpid(),record['controller'],actual,directory/'identity-failure.json')
  assert not wrong_executable and not (legacy and not release),'expected refusal missing' 
  results.append({'case':name,'result':'PASS','actual_ps_sample':wrong_executable,'scope':'metadata helper only; outer guards continuously enforce real global census'})
 except ValueError as error:
  expected='actual PS executable/birth mismatch' if wrong_executable else 'accepted legacy owner lacks actual descriptor'
  assert (wrong_executable or (legacy and not release)) and expected in str(error),str(error)
  results.append({'case':name,'result':'PASS','expected_refusal':str(error)})
case('canonical_without_cohort')
case('ordinary_observation_only_no_workflow',observation=True)
case('observation_only_never_releases_legacy',observation=True,legacy=True)
case('positive_isolated_release_closes_only_old_claim',observation=True,legacy=True,release=True)
case('wrong_actual_executable_refused',wrong_executable=True)
(root/'result.json').write_text(json.dumps({'qualification':'ISOLATED_METADATA_ONLY_OUTER_GUARDS_AUTHORITATIVE','cases':results},indent=2)+'\n')
print(json.dumps(results))
