"""Complete allocation and actual process guards; import starts no workload."""
import json, os, time, subprocess, hashlib, gzip, traceback, datetime
from pathlib import Path
import accounting
import posixpath
from receipt_proof import validate_full, zero, unique_pairs
_PROOF_CACHE={}

def write(out,name,value):
 p=Path(out)/name;t=p.with_suffix(p.suffix+'.tmp');t.write_text(json.dumps(value,indent=2)+'\n');os.replace(t,p)

def digest(path):
 h=hashlib.sha256()
 with Path(path).open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()

def census():
 raw=subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,pgid=,rss=,lstart=,command='],text=True,timeout=3)
 return {int(v[0]):(int(v[1]),int(v[2]),int(v[3])*1024,' '.join(v[4:9]),v[9]) for line in raw.splitlines() if len(v:=line.split(maxsplit=9))==10}

def identity(pid,script,out):
 raw=subprocess.check_output(['/bin/ps','-p',str(pid),'-o','pid=,ppid=,pgid=,rss=,lstart=,command='],text=True,timeout=3)
 v=raw.strip().split(maxsplit=9);assert len(v)==10 and int(v[0])==pid,'actual local PID identity'
 r=(int(v[1]),int(v[2]),int(v[3])*1024,' '.join(v[4:9]),v[9]);assert str(script) in r[4],'actual script identity'
 return {'pid':pid,'ppid':r[0],'birth':r[3],'pgid':r[1],'executable':r[4].split()[0],'script':str(script),'outputRoot':str(out)}


def retain_owner_index(out, raw):
 sha=hashlib.sha256(raw).hexdigest();directory=Path(out)/'owner-index-snapshots';directory.mkdir(exist_ok=True)
 final=directory/(sha+'.json')
 if final.exists():assert digest(final)==sha,'retained owner-index snapshot changed'
 else:
  temp=directory/(sha+'.'+str(os.getpid())+'.'+str(time.time_ns())+'.tmp')
  with temp.open('xb') as stream:stream.write(raw)
  os.replace(temp,final)
 return {'ownerIndexSHA256':sha,'ownerIndexSnapshotPath':str(final)}

def relative_primary_path(argv,cwd,declared_cwd,script):
 assert isinstance(cwd,str) and posixpath.isabs(cwd) and cwd==declared_cwd,'actual relative-program cwd mismatch'
 assert isinstance(script,str) and posixpath.isabs(script),'absolute expected script required'
 assert isinstance(argv,list) and len(argv)>=2 and all(isinstance(v,str) for v in argv),'decoded actual argv required'
 index=1
 while index<len(argv) and argv[index] in ['-B','-u','-I','-E','-s','-S','-O','-OO','-q','-P']:index+=1
 if index<len(argv) and argv[index]=='--':index+=1
 assert index<len(argv) and argv[index] and not argv[index].startswith('-') and not posixpath.isabs(argv[index]),'relative primary Python program required'
 primary=posixpath.normpath(posixpath.join(cwd,argv[index]))
 assert primary==posixpath.normpath(script),'actual primary script mismatch'
 return primary

def stdin_primary(argv,cwd,declared_cwd):
 assert isinstance(cwd,str) and posixpath.isabs(cwd) and cwd==declared_cwd,'actual stdin cwd mismatch'
 assert isinstance(argv,list) and len(argv)>=2 and all(isinstance(v,str) for v in argv),'actual stdin argv required'
 index=1
 while index<len(argv) and argv[index] in ['-B','-u','-I','-E','-s','-S','-O','-OO','-q','-P']:index+=1
 if index<len(argv) and argv[index]=='--':index+=1
 assert index<len(argv) and argv[index]=='-','actual primary Python stdin required'
 return '-'

def kernel_argv(pid):
 # KERN_PROCARGS2=49 is read from the installed macOS sysctl.h. Decode the
 # kernel argv vector; a displayed command substring is not argv evidence.
 import ctypes
 libc=ctypes.CDLL('/usr/lib/libSystem.B.dylib',use_errno=True)
 # XNU sysctl_procargsx rejects buflen-sizeof(int)>ARG_MAX with EINVAL.
 # Read KERN_ARGMAX=8 instead of supplying the old invalid fixed2MiB.
 limit=ctypes.c_int();limit_size=ctypes.c_size_t(ctypes.sizeof(limit))
 limit_mib=(ctypes.c_int*2)(1,8)
 assert libc.sysctl(limit_mib,2,ctypes.byref(limit),ctypes.byref(limit_size),None,0)==0,'actual KERN_ARGMAX unavailable'
 assert limit_size.value==ctypes.sizeof(limit) and 4096<=limit.value<=16*1024**2,'actual KERN_ARGMAX invalid'
 mib=(ctypes.c_int*3)(1,49,pid);size=ctypes.c_size_t(limit.value);buf=ctypes.create_string_buffer(size.value)
 assert libc.sysctl(mib,3,buf,ctypes.byref(size),None,0)==0,'actual argv unavailable pid='+str(pid)+' errno='+str(ctypes.get_errno())+' argumentLimit='+str(limit.value)
 assert 4<size.value<=len(buf),'actual argv result size invalid'
 raw=buf.raw[:size.value];assert len(raw)>=5
 argc=int.from_bytes(raw[:4],sys_byteorder(),signed=True);assert 1<=argc<=16384
 cursor=raw.index(b'\0',4)+1
 while cursor<len(raw) and raw[cursor]==0:cursor+=1
 argv=[]
 for _ in range(argc):
  end=raw.index(b'\0',cursor);argv.append(os.fsdecode(raw[cursor:end]));cursor=end+1
 return {'argv':argv,'argumentLimit':limit.value,'bufferBytes':len(buf),'returnedBytes':size.value}

def relative_identity(pid,expected,row,stdin=False):
 import ctypes
 decoded=kernel_argv(pid);argv=decoded['argv']
 libc=ctypes.CDLL('/usr/lib/libSystem.B.dylib',use_errno=True)
 imagebuf=ctypes.create_string_buffer(4096)
 assert libc.proc_pidpath(pid,imagebuf,len(imagebuf))>0,'actual OS image unavailable'
 image=os.fsdecode(imagebuf.value)
 assert str(Path(image).resolve())==str(Path(row[4].split()[0]).resolve()),'actual OS image changed'
 output=subprocess.check_output(['/usr/sbin/lsof','-a','-p',str(pid),'-d','cwd','-Fn'],text=True,timeout=3)
 names=[v[1:] for v in output.splitlines() if v.startswith('n')];assert len(names)==1,'actual cwd unavailable'
 primary=stdin_primary(argv,names[0],expected.get('cwd')) if stdin else relative_primary_path(argv,names[0],expected.get('cwd'),expected.get('script'))
 # Bracket the factual collection with the same OS birth and parent, failing
 # closed on an exit/reuse/race rather than caching an unverifiable identity.
 again=subprocess.check_output(['/bin/ps','-p',str(pid),'-o','ppid=,lstart='],text=True,timeout=3).split()
 assert again and int(again[0])==row[0] and ' '.join(again[1:])==' '.join(row[3].split()),'relative identity birth/parent changed'
 return {'pid':pid,'birth':row[3],'parent':row[0],'actual_cwd':names[0],'actual_image':image,'primary_script':primary,'argvSHA256':hashlib.sha256(json.dumps(argv).encode()).hexdigest(),'argumentLimit':decoded['argumentLimit'],'argvBufferBytes':decoded['bufferBytes']}

def sys_byteorder():
 import sys
 return sys.byteorder

def processes(a):
 rows=census()
 snapshot={'stage':'CENSUS_BEFORE_ASSERT','epoch':time.time(),'sampler_pid':os.getpid(),'relevant_actual_processes':[{'pid':pid,'ppid':r[0],'pgid':r[1],'rss':r[2],'birth':r[3],'command':r[4]} for pid,r in rows.items() if any(x in r[4] for x in ['/tmp/mcr-','/T/mcr-','/mcr-worktrees/'])]}
 write(a['output_directory'],'global-sample-before-assert-'+str(os.getpid())+'.json',snapshot)
 def descendants(roots):
  own=set(roots)
  while True:
   new={pid for pid,r in rows.items() if r[0] in own}-own
   if not new:break
   own.update(new)
  return own & rows.keys()
 own=descendants([a['controller_pid']])
 groups=Path(a['output_directory'])/'owned-groups.json'
 if groups.exists():
  ids=set(json.loads(groups.read_text()));own |= {pid for pid,r in rows.items() if r[1] in ids}
 roster=json.loads(Path(a['global_roster_path']).read_text())
 assert roster['issuer']=='Resource21d','literal roster authority'
 membership=json.loads(Path(roster['management_membership_path']).read_text())
 receipt_entries=[]
 receipt_directory=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')
 receipt_files=list(receipt_directory.glob('*.json'));assert len(receipt_files)<=256,'receipt directory bound'
 for path in receipt_files:
  assert path.stat().st_size<=262144,'owner receipt bound'
  raw_index=path.read_bytes();rec=json.loads(raw_index,object_pairs_hook=unique_pairs)
  observation_only=False
  if set(rec)=={'ordinaryObservation'}:
   observed=rec['ordinaryObservation'];booking=observed['booking']
   assert observed.get('ordinaryCommandsComplete') is True and observed.get('commandExit')==0 and observed.get('completedAt'),'ordinary observation not positively completed'
   assert observed['role'].startswith('ordinary ') and 'Git' in observed['role'],'unknown observed role'
   assert isinstance(booking['additionalUpperBytes'],int) and booking['additionalUpperBytes']>=0,'ordinary observed booking missing'
   epoch=datetime.datetime.fromisoformat(observed['observedAt'].replace('Z','+00:00')).timestamp()
   stop=datetime.datetime.fromisoformat(booking['expiresAt'].replace('Z','+00:00')).timestamp()
   # This legacy observation is NOT literal FULL. Retain its entire ordinary
   # booking until its owner publishes canonical FULL; never erase a claim.
   c={'pid':observed['actualpid'],'birth':observed['birth'],'executable':observed['executable'],'script':None,'outputRoot':observed['cwd'],'cwd':observed['cwd']}
   rec={'ownerThreadId':path.stem,'offerId':booking['id'],'state':'ACCEPTED','acceptedAtEpoch':epoch,'workDeadlineEpoch':stop,'fullDeadlineEpoch':stop,'kind':'ordinary','processCap':None,'rssCapBytes':None,'remainingNewUpperBytes':booking['additionalUpperBytes'],'controller':c,'guardControllers':[],'receiptPaths':{'accept':str(path),'full':None}}
   observation_only=True
   snapshot.setdefault('retained_ordinary_observations',[]).append({'path':str(path),'sha256':digest(path),'booking':booking['id'],'retained_upper_bytes':booking['additionalUpperBytes'],'literal_full':False})
  assert path.stem==rec['ownerThreadId'],'owner receipt filename'
  observed=rec.get('ordinaryObservation') if not observation_only else None
  if observed is not None:
   assert isinstance(observed,dict),'ordinary observation object required'
   observed_pid=observed.get('actualpid',observed.get('pid'))
   ended=observed.get('ended') is True or observed.get('endedAt') is not None or observed.get('endedAtEpoch') is not None or observed.get('state')=='COMPLETED' or observed.get('completedAt') is not None
   current=rows.get(observed_pid)
   same_birth=current is not None and isinstance(observed.get('birth'),str) and ' '.join(current[3].split())==' '.join(observed['birth'].split())
   if ended or not same_birth:
    snapshot.setdefault('ordinary_observations_not_current_identity',[]).append({'owner':rec['ownerThreadId'],'pid':observed_pid,'birth':observed.get('birth'),'explicitEnded':ended,'matchesCurrentBirth':same_birth,'oldExecutionReleased':False,'futureOrdinaryEnvelopeRetained':True,'ownerIndexSHA256':hashlib.sha256(raw_index).hexdigest()})
   else:
    booking=observed.get('booking');upper=None
    if isinstance(booking,dict):
     upper=booking.get('additionalUpperBytes');assert type(upper) is int and upper>=0,'ordinary observation future bytes missing'
     assert isinstance(booking.get('id'),str) and booking['id'],'ordinary observation booking ID missing'
    else:
     # The interface permits factual metadata finishers without a per-owner
     # numeric booking. Do not invent zero growth: the source guard charges the
     # allocator's entire combined2.5GiB ordinary envelope once for these too.
     assert booking is None or isinstance(booking,str),'ordinary observation booking schema'
    epoch=observed.get('observedAt',observed.get('at',snapshot['epoch']))
    if isinstance(epoch,str):epoch=datetime.datetime.fromisoformat(epoch.replace('Z','+00:00')).timestamp()
    assert type(epoch) in [int,float] and 0<epoch<=time.time(),'ordinary observation epoch invalid'
    assert type(observed_pid) is int and observed_pid>0 and isinstance(observed.get('birth'),str) and observed['birth'],'ordinary observation PID/birth missing'
    assert isinstance(observed.get('cwd'),str) and posixpath.isabs(observed['cwd']),'ordinary observation cwd missing'
    assert isinstance(observed.get('executable'),str) and posixpath.isabs(observed['executable']),'ordinary observation OS image missing'
    role=observed.get('role')
    # The explicit ordinaryObservation field supplies the type. Its factual
    # purpose is evidence, not an English-prefix-based classification rule.
    assert isinstance(role,str) and role.strip(),'ordinary observation purpose missing: '+rec['ownerThreadId']
    script=observed.get('script')
    if script in [None,'-','stdin','stdin via /bin/sh login:false directexec absolutePython']:script=None
    else:assert isinstance(script,str) and posixpath.isabs(script),'ordinary observation absolute script or explicit stdin required'
    c={'pid':observed_pid,'birth':observed['birth'],'executable':observed['executable'],'script':script,'cwd':observed['cwd'],'outputRoot':observed['cwd']}
    key='ordinary-observation:'+rec['ownerThreadId']+':'+str(epoch)
    proof=retain_owner_index(a['output_directory'],raw_index)
    receipt_entries.append({'cohort':key,'owner':rec['ownerThreadId'],'kind':'ordinary','state':'ACTUAL_ACCEPT_NO_FULL_RECEIPT_YET','knownRootMatchers':[c['script'] or c['cwd']],'pidRecords':[c],'processCap':0,'rssCapGiB':0,'remainingNewConservativeUpperBytes':upper if upper is not None else 0,'unknownOrdinaryUpperCoveredByEnvelope':upper is None,'acceptedAtEpoch':epoch,'observationOnly':True,'publishedOwnerReceiptPath':str(path),'ordinaryObservationProof':proof})
    snapshot.setdefault('live_nested_ordinary_observations',[]).append({'owner':rec['ownerThreadId'],'pid':c['pid'],'birth':c['birth'],'futureBytes':upper,**proof})
  if rec.get('offerId') is None and rec.get('kind')=='ordinary' and rec.get('state') not in ['FULL','DECLINED']:
   # Explicit activity observation is not an execution offer. Keep its full
   # booking under an owner/epoch accounting key without inventing a grant.
   assert rec.get('executionWorkflow') is False and isinstance(rec.get('authorizationReference'),str) and rec['authorizationReference'],'unbound ordinary observation'
   assert type(rec.get('ordinaryBookingUpperBytes')) is int and rec['ordinaryBookingUpperBytes']==rec.get('remainingNewUpperBytes'),'ordinary observation booking mismatch'
   assert type(rec.get('acceptedAtEpoch')) in [int,float] and rec['acceptedAtEpoch']>0,'ordinary observation epoch'
   assert rec.get('controller',{}).get('pid') and rec['controller'].get('birth') and rec['controller'].get('script'),'ordinary observation identity missing'
   snapshot.setdefault('ordinary_activity_keys',[]).append({'owner':rec['ownerThreadId'],'raw_offerId':None,'acceptedAtEpoch':rec['acceptedAtEpoch'],'authorizationReference':rec['authorizationReference'],'remainingNewUpperBytes':rec['remainingNewUpperBytes']})
   rec['offerId']='ordinary-observation:'+rec['ownerThreadId']+':'+str(rec['acceptedAtEpoch'])
  history=[]
  for field in ['closedHistory','closedOffers','closedReceipts']:
   values=rec.get(field,[]);assert len(values)<=256,'bounded owner closure history'
   if isinstance(values,dict):values=[dict(value,offerId=value.get('offerId',key)) for key,value in values.items() if isinstance(value,dict)]
   assert isinstance(values,list),'owner closure history must be list or map'
   history.extend(values)
  for closure in history:
   if not isinstance(closure,dict):continue
   if closure.get('state')!='FULL':continue
   matching=[e for e in membership['entries'] if e.get('owner',e.get('ownerThreadId'))==rec['ownerThreadId'] and closure.get('ownerThreadId',rec['ownerThreadId'])==rec['ownerThreadId'] and e.get('offerId',e.get('cohort'))==closure.get('offerId') and closure.get('acceptedAtEpoch') is not None and e.get('actualAccept',e.get('acceptedAtEpoch'))==closure['acceptedAtEpoch'] and e.get('state') not in ['FULL','DECLINED','LITERAL_FULL_RECEIVED_RESOURCES_RELEASED']]
   if not matching:continue
   proof=validate_full(closure,rec['ownerThreadId'],closure['offerId'],closure['acceptedAtEpoch'],lambda p:Path(p).read_text(),time.time())
   proof.update(retain_owner_index(a['output_directory'],raw_index))
   key=(proof['owner'],proof['offer'],proof['acceptedAtEpoch'])
   assert _PROOF_CACHE.setdefault(key,proof['sha256'])==proof['sha256'],'immutable historical FULL changed'
   for entry in matching:entry['state']='LITERAL_FULL_RECEIVED_RESOURCES_RELEASED'
   snapshot.setdefault('matched_literal_history_full',[]).append(proof)
  state=rec['state']
  if state=='UNREADY_YIELD':
   # A never-accepted offer is not a release of any earlier execution claim.
   assert isinstance(rec.get('offerId'),str) and rec['offerId'],'yield offer missing'
   assert rec.get('acceptedAtEpoch') is None and rec.get('controller') is None and rec.get('guardControllers')==[],'yield contains actual acceptance/identity'
   assert type(rec.get('remainingNewUpperBytes')) is int and rec['remainingNewUpperBytes']==0,'yield retains unknown growth'
   assert rec.get('resourcesHeld') is False or type(rec.get('workflow')) is int and rec['workflow']==0,'yield resource disposition missing'
   assert type(rec.get('yieldAtEpoch')) in [int,float] and 0<rec['yieldAtEpoch']<=time.time(),'yield clock missing/future'
   snapshot.setdefault('never_accepted_yields',[]).append({'owner':rec['ownerThreadId'],'offer':rec['offerId'],**retain_owner_index(a['output_directory'],raw_index)})
   continue
  if state in ['ACCEPTED_RUNNING','ACCEPTED_BOOTSTRAP','ACCEPTED_GUARDS_STARTING','ACTUAL_ACCEPT','ACCEPTED_INNER_CENSUS','ACTUAL_ACCEPT_NO_FULL']:
   # Known positive legacy spelling: retain all caps, remaining growth and
   # actual identity; normalization must never infer a release.
   snapshot.setdefault('retained_accepted_state_aliases',[]).append({'path':str(path),'raw_state':state,'normalized_state':'ACCEPTED','offer':rec.get('offerId')})
   state='ACCEPTED'
  assert state in ['ACCEPTED','FULL','DECLINED'],'owner receipt state '+str(path)+' '+repr(state)+' offer='+str(rec.get('offerId'))
  if state=='FULL':assert rec.get('remainingNewUpperBytes')==0,'canonical FULL remaining NEW nonzero/unknown'
  if state!='ACCEPTED':
   # FULL files are factual closures, never erase a different legacy accepted claim.
   # An exact legacy offer closure is applied below only after its literal FULL is read.
   if state=='FULL':
    for e in membership['entries']:
     if e.get('owner',e.get('ownerThreadId'))==rec['ownerThreadId'] and e.get('offerId',e.get('cohort'))==rec.get('offerId') and rec.get('acceptedAtEpoch') is not None and e.get('actualAccept',e.get('acceptedAtEpoch'))==rec['acceptedAtEpoch']:
      if e.get('state') not in ['FULL','DECLINED','LITERAL_FULL_RECEIVED_RESOURCES_RELEASED']:
       fp=Path(rec['receiptPaths']['full']);assert fp.stat().st_size<=4*1024*1024
       raw=fp.read_text()
       try:packets=[json.loads(raw)]
       except json.JSONDecodeError:packets=[json.loads(line) for line in raw.splitlines() if line.strip()]
       epoch=rec.get('actualFullEpoch',rec.get('fullAtEpoch'))
       attested=rec.get('resourcesReleased') is True or any(isinstance(rec.get(k),dict) and bool(rec[k]) and zero(rec[k]) for k in ['resources','remainingResources'])
       for packet in packets:
        if not isinstance(packet,dict):continue
        if epoch is None:epoch=packet.get('actualFullEpoch',packet.get('fullAtEpoch',packet.get('actual_full_epoch')))
        attested=attested or packet.get('resourcesReleased') is True or all(k in packet and zero(packet[k]) for k in ['workflows','children','guards'])
       assert epoch is not None and attested,'literal FULL exact clock/resourcezero proof missing'
       closure={'state':'FULL','ownerThreadId':rec['ownerThreadId'],'offerId':rec['offerId'],'acceptedAtEpoch':rec['acceptedAtEpoch'],'actualFullEpoch':epoch,'resourcesReleased':True,'fullPath':str(fp),'sha256':digest(fp)}
       proof=validate_full(closure,rec['ownerThreadId'],rec['offerId'],rec['acceptedAtEpoch'],lambda p:Path(p).read_text(),time.time())
       proof.update(retain_owner_index(a['output_directory'],raw_index))
       key=(proof['owner'],proof['offer'],proof['acceptedAtEpoch'])
       assert _PROOF_CACHE.setdefault(key,proof['sha256'])==proof['sha256'],'immutable current FULL changed'
       snapshot.setdefault('matched_literal_current_full',[]).append(proof)
      e['state']='LITERAL_FULL_RECEIVED_RESOURCES_RELEASED'
   continue
  assert isinstance(rec.get('offerId'),str) and rec['offerId'],'accepted offer missing'
  epoch=rec.get('acceptedAtEpoch')
  assert type(epoch) in [int,float] and 0<epoch<=time.time(),'accepted epoch invalid/future'
  assert rec.get('kind') in ['source','browser','target','copied-native','ordinary'],'accepted execution class missing'
  remaining=rec.get('remainingNewUpperBytes')
  assert type(remaining) is int and remaining>=0,'accepted remaining NEW invalid'
  c=rec['controller'] or {};pins=[]
  for actual in [c,*rec.get('guardControllers',[])]:
   if actual.get('pid'):pins.append(actual)
  # Ordinary source/Git preparation has no execution slot and can explicitly
  # publish null reserved PID/RSS caps. Its actual tree still counts below.
  ordinary_record=rec['kind']=='ordinary'
  process_cap=rec['processCap'];rss_cap=rec['rssCapBytes']
  if ordinary_record:
   assert process_cap is None or isinstance(process_cap,int) and process_cap>=0
   assert rss_cap is None or isinstance(rss_cap,int) and rss_cap>=0
   process_cap=process_cap or 0;rss_cap=rss_cap or 0
  else:assert type(process_cap) is int and process_cap>0 and type(rss_cap) is int and rss_cap>0,'accepted execution caps missing'
  matchers=[item.get(key) for item in [c,*rec.get('guardControllers',[])] for key in ['outputRoot','script'] if item.get(key)]
  receipt_entries.append({'cohort':rec['offerId'],'owner':rec['ownerThreadId'],'kind':'warmRust' if rec['kind']=='target' else rec['kind'],'state':'ACTUAL_ACCEPT_NO_FULL_RECEIPT_YET','knownRootMatchers':matchers,'pidRecords':pins,'processCap':process_cap,'rssCapGiB':rss_cap/1024**3,'remainingNewConservativeUpperBytes':rec['remainingNewUpperBytes'],'acceptedAtEpoch':rec['acceptedAtEpoch'],'acceptReceiptPath':rec.get('receiptPaths',{}).get('accept'),'observationOnly':observation_only,'publishedOwnerReceiptPath':str(path)})
  # Only the exact owner/offer/ACCEPT tuple may deduplicate a legacy claim.
  for e in membership['entries']:
   if e.get('owner',e.get('ownerThreadId'))==rec['ownerThreadId'] and e.get('offerId',e.get('cohort'))==rec['offerId'] and e.get('actualAccept',e.get('acceptedAtEpoch'))==rec['acceptedAtEpoch']:e['sameOwnerReceiptCohort']=rec['offerId']
 own_cap=a.get('process_cap',32);own_rss_cap=a.get('rss_cap_bytes',8*1024**3);own_kind=a.get('kind','warmRust')
 active=[];foreign=set();ordinary=set();unresolved_pids=0;unresolved_rss=0;covered=set();records=[];declared_pids=own_cap;declared_rss=own_rss_cap;remaining_new=0;ordinary_remaining_new=0
 for cohort in [*receipt_entries,*roster['cohorts'],*membership['entries']]:
  if cohort.get('sameOwnerReceiptCohort') in covered:continue
  key=cohort.get('cohort') or cohort.get('offerId') or cohort.get('ownerThreadId')
  assert key,'cohort identity missing'
  if cohort.get('owner',cohort.get('ownerThreadId'))=='01a1a21a-a088-7c31-83e3-19de119ba873':continue
  if key in covered:continue
  covered.add(key)
  close=roster['closed_cohort_receipts'].get(key)
  if close:
   assert digest(close['path'])==close['sha256'],'literal release receipt changed'
   continue
  state=cohort.get('state','')
  if state in ['FULL','DECLINED'] or 'FULL_RECEIVED_RESOURCES_RELEASED' in state:continue
  if not ('ACCEPT' in state and 'NOT_ACCEPTED' not in state):continue
  controller=cohort.get('controller') or {}
  matchers=cohort.get('knownRootMatchers') or [v for v in [controller.get('script'),controller.get('outputRoot')] if v]
  if not matchers:
   # Positive owner ACCEPT with no process identity is still fully reserved.
   # It cannot identify any actual live process; unknown actual roots still fail.
   assert cohort.get('publishedOwnerReceiptPath') and not cohort.get('pidRecords'),'accepted workflow missing root identity'
  hits=set()
  if not cohort.get('observationOnly'):
   for pid,r in rows.items():
    executable=r[4].split()[0] if r[4].split() else ''
    if pid not in own and any(x in executable for x in ['python','node','cargo','rustc']) and any(m in r[4] for m in matchers):hits.add(pid)
  for expected in cohort.get('pidRecords',[controller,*cohort.get('guardControllers',[])]):
   if not expected or not expected.get('pid'):continue
   if expected['pid'] in rows:
    row=rows[expected['pid']]
    if cohort.get('observationOnly') and ' '.join(row[3].split())!=' '.join(expected['birth'].split()):continue
    assert expected.get('birth') and (expected.get('pgid') is None or row[1]==expected['pgid']) and ' '.join(row[3].split())==' '.join(expected['birth'].split()),'PID birth/group missing or reused'
    relative=None
    if not any(m in row[4] for m in matchers) and expected.get('script'):
     relative=relative_identity(expected['pid'],expected,row)
     snapshot.setdefault('actual_relative_primary_programs',[]).append(relative)
    if not any(m in row[4] for m in matchers) and relative is None:
     if cohort.get('observationOnly'):
      assert expected.get('cwd') and expected.get('script') is None,'ordinary observed process identity changed'
      proof=relative_identity(expected['pid'],expected,row,stdin=True)
      snapshot.setdefault('ordinary_observed_actual_cwds',[]).append(proof)
     else:
     # The published ordinary interface also covers inline stdin preparation.
     # Resolve only its exact live PID/birth/image and matching owner booking;
     # never generalize this to target/source/browser execution workloads.
      assert cohort.get('kind')=='ordinary' and expected.get('script') is None and row[4].split()[-1]=='-','PID root no longer matches '+str(expected['pid'])+' '+row[4]
      booking=Path(cohort.get('acceptReceiptPath') or cohort['receiptPaths']['accept'])
      output_root=Path(expected.get('outputRoot') or controller['outputRoot'])
      assert booking.resolve().is_relative_to(output_root.resolve()) and booking.stat().st_size<=262144,'ordinary booking/root mismatch'
      booking_data=json.loads(booking.read_text())
      assert booking_data.get('offerId',booking_data.get('offer'))==key and booking_data.get('acceptedAtEpoch')==cohort.get('acceptedAtEpoch',cohort.get('actualAccept')),'ordinary owner booking identity mismatch'
      snapshot.setdefault('ordinary_stdin_identities',[]).append({'pid':expected['pid'],'birth':row[3],'actual_command':row[4],'declared_output_root':str(output_root),'matching_owner_booking':str(booking),'booking_sha256':digest(booking)})
    if expected.get('executable'):
     advertised=str(Path(expected['executable']).resolve());image=str(Path(row[4].split()[0]).resolve())
     if advertised!=image:
      pins=json.loads((Path(a['output_directory'])/'python-launcher-image.json').read_text())
      assert (advertised,image)==(pins['sys_executable'],pins['actual_ps_executable']),'unknown actual executable change'
      for path,info in pins['files'].items():assert digest(path)==info['sha256'],'pinned Python launcher/image changed'
      snapshot.setdefault('observed_framework_reexec',[]).append({'pid':expected['pid'],'birth':row[3],'advertised_launcher':advertised,'actual_image':image})
      write(a['output_directory'],'global-sample-before-assert-'+str(os.getpid())+'.json',snapshot)
    hits.add(expected['pid'])
  kind=cohort.get('kind')
  if kind=='target':kind='warmRust'
  if kind is None:
   kind='warmRust' if any(x in key.lower() for x in ['compile','rust','mains']) else 'source' if 'source' in key.lower() else 'browser' if 'native' in key.lower() else None
  assert kind in ['source','browser','warmRust','copied-native','ordinary'],'unclassified actual accepted cohort '+key
  if kind=='ordinary':
   booked=cohort.get('remainingNewUpperBytes',cohort.get('remainingNewConservativeUpperBytes',0))
   ordinary|=descendants(hits);remaining_new+=booked;ordinary_remaining_new+=booked;continue
  active.append((key,kind));foreign |= descendants(hits)
  rss_cap=cohort.get('rssCapBytes',int(cohort.get('rssCapGiB',0)*1024**3))
  assert rss_cap>0,'accepted resource RSS cap missing'
  declared_pids+=cohort['processCap'];declared_rss+=rss_cap
  remaining_new+=cohort.get('remainingNewUpperBytes',cohort.get('remainingNewConservativeUpperBytes',0))
  records.extend({'cohort':key,'pid':pid,'pgid':rows[pid][1],'birth':rows[pid][3],'root_command':rows[pid][4]} for pid in hits)
  if not hits:
   # An accepted missing process is retained conservatively until literal FULL.
   unresolved_pids+=cohort['processCap'];unresolved_rss+=rss_cap
 # Do not silently admit a newly discovered MCR controller outside the receipt roster.
 unknown=[]
 for pid,r in rows.items():
  executable=r[4].split()[0] if r[4].split() else ''
  relevant=any(x in executable for x in ['python','node','cargo','rustc']) or executable.rsplit('/',1)[-1] in ['sh','zsh','bash','dash','fish','env']
  if pid not in own|foreign|ordinary and relevant and any(x in r[4] for x in ['/tmp/mcr-','/T/mcr-','/mcr-worktrees/']):unknown.append({'pid':pid,'ppid':r[0],'pgid':r[1],'rss':r[2],'birth':r[3],'command':r[4]})
 allids=own|foreign|ordinary
 result={'own_processes':len(own),'own_rss':sum(rows[p][2] for p in own),'global_processes':len(allids)+unresolved_pids,'global_rss':sum(rows[p][2] for p in allids)+unresolved_rss,'global_workflows':1+len(active),'global_source':int(own_kind=='source')+sum(k=='source' for _,k in active),'global_browser':int(own_kind=='browser')+sum(k=='browser' for _,k in active),'global_warm_rust':int(own_kind=='warmRust')+sum(k=='warmRust' for _,k in active),'global_cohorts':active,'global_pid_birth_group_root_records':records,'own_pids':sorted(own),'unresolved_accepted_reserved_pids':unresolved_pids,'declared_global_pids':declared_pids,'declared_global_rss':declared_rss}
 result.update(unknown=unknown,other_accepted_remaining_new_bytes=remaining_new,other_accepted_ordinary_remaining_new_bytes=ordinary_remaining_new,unquantified_ordinary_envelope_bytes=int(2.5*1024**3) if any(c.get('unknownOrdinaryUpperCoveredByEnvelope') for c in receipt_entries) else 0)
 write(a['output_directory'],'global-sample-before-assert-'+str(os.getpid())+'.json',{**snapshot,'stage':'COMPLETE_CENSUS_BEFORE_ASSERT','sample':result})
 assert not unknown,'unreconciled actual MCR controller '+str(unknown[0])
 assert result['own_processes']<=own_cap,'actual own PID cap'
 assert result['own_rss']<=own_rss_cap,'actual own RSS cap'
 assert declared_pids<=96 and declared_rss<=18*1024**3,'96PID/18GiB accepted declared global caps'
 assert result['global_processes']<=96,'96 actual/reserved global PIDs'
 assert result['global_rss']<=18*1024**3,'18GiB actual/reserved global RSS'
 assert result['global_workflows']<=4 and result['global_source']<=3 and result['global_browser']<=2 and result['global_warm_rust']<=1,'GLOBAL4/SOURCE3/browser2/warmRust1'
 for pid in own:
  name=rows[pid][4].split()[0]
  assert not any(x in name for x in ['postgres','pg_ctl','initdb','Chromium','playwright','node']),'excluded process '+name
 return result

class Guard:
 def __init__(self,a):
  self.a=a;self.peak={};self.identities={k:accounting.RootIdentity.capture(v) for k,v in a['roots'].items()};self.lastfast=0;self.fastvalue={};self.frozen=False
  with gzip.open(Path(a['output_directory'])/'complete-baseline.json.gz','rt') as f:baseline=json.load(f)
  self.high={(dev,ino):(scope,size) for scope,dev,ino,size in baseline['inodes']};self.positive={k:0 for k in a['roots']}
  self.positive.update({k:0 for k in a['file_baselines']})
 def fast(self,cleanup=False):
  now=time.time();assert now < (self.a['full_stop']-10 if cleanup else self.a['work_stop']-10),'fixed phase clock'
  free=os.statvfs(self.a['output_directory']);free=free.f_bavail*free.f_frsize
  assert free>=8*1024**3,'8GiB live free floor'
  if now-self.lastfast>.25:
   try:self.fastvalue=processes(self.a)
   except BaseException as error:
    detail={'epoch':time.time(),'sampler_pid':os.getpid(),'failure':type(error).__name__+': '+str(error),'traceback':traceback.format_exc()[-32768:]}
    if hasattr(error,'receipt'):detail['accounting_receipt']=error.receipt()
    write(self.a['output_directory'],'global-failure-'+str(os.getpid())+'.json',detail)
    raise
   self.lastfast=now
   for k in ['own_processes','own_rss','global_processes','global_rss','global_workflows']:self.peak[k]=max(self.peak.get(k,0),self.fastvalue[k])
  self.peak['minimum_free']=min(self.peak.get('minimum_free',free),free)
  return {**self.fastvalue,'free':free,'epoch':now}
 def sample(self,cleanup=False):
  self.fast(cleanup)
  roots={k:v for k,v in self.identities.items() if not(self.frozen and k=='target')}
  deadline=min(time.monotonic()+35,time.monotonic()+(self.a['full_stop']-15 if cleanup else self.a['work_stop']-15)-time.time())
  receipt=accounting.collect(roots,deadline=deadline,checkpoint=lambda:self.fast(cleanup),attempts=3)
  assert receipt['complete'],'incomplete root sample'
  observed=receipt['inodes']
  for path in self.a['file_baselines']:
   stat=os.lstat(path);observed.append((path,stat.st_dev,stat.st_ino,stat.st_blocks*512))
  for scope,dev,ino,size in observed:
   oldscope,old=self.high.get((dev,ino),(scope,0))
   if size>old:
    self.positive[oldscope]=self.positive.get(oldscope,0)+size-old;self.high[(dev,ino)]=(oldscope,size)
  for scope,growth in self.positive.items():self.peak[scope]=growth
  target=self.peak.get('target',0);outside=sum(self.peak.get(k,0) for k in self.a['roots'] if k!='target')+sum(self.peak.get(k,0) for k in self.a['file_baselines']);new=target+outside;whole=self.a['carry_bytes']+new
  self.peak.update(new_bytes=max(self.peak.get('new_bytes',0),new),whole_bytes=max(self.peak.get('whole_bytes',0),whole))
  assert target<=1536*1024**2,'1.5GiB target positive growth'
  assert outside<=512*1024**2,'512MiB complete outside positive growth'
  assert new<=2*1024**3,'2GiB total NEW'
  assert whole<=48*1024**3,'48GiB nonshrinking WHOLE'
  return {**self.fast(cleanup),'peak':self.peak,'complete_allocation':True,'target_frozen':self.frozen}
