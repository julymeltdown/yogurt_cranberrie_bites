"""Actual fresh SOURCE01 only: strict scoped TypeScript and original12 cases."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from safety import ROOT,HERE,TEST,DOC,RECORD,NODE,PYTHON,MIB,REPAIR_CAP,PREP_RESERVED,HISTORICAL_NEW,Meter,atomic,pin,readonly_pins

OFFER='ART-PC-STRICT-SOURCE01-0418'
assert sys.argv[1:]==['--head',sys.argv[2]]
head=sys.argv[2];accepted=time.time();work=accepted+180;hard=accepted+240
prior=json.loads(RECORD.read_text());assert prior['state']=='FULL'
full_path=Path(prior['receiptPaths']['full']);actual=prior['actual']
history=prior.get('closedOffers',[])+[dict(ownerThreadId=prior['ownerThreadId'],offerId=prior['offerId'],acceptedAtEpoch=prior['acceptedAtEpoch'],actualFullEpoch=dt.datetime.fromisoformat(actual['at']).timestamp(),fullPath=str(full_path),fullSha256=hashlib.sha256(full_path.read_bytes()).hexdigest(),resourcesReleased=True)]
receipt=HERE/(OFFER+'.jsonl');assert not receipt.exists(),'No closed-round restart'
identity=subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart=,comm='],text=True).split(None,5)
record=dict(ownerThreadId=prior['ownerThreadId'],offerId=OFFER,state='ACCEPTED',phase='ACTUAL_BOOTSTRAP',acceptedAtEpoch=accepted,workDeadlineEpoch=work,fullDeadlineEpoch=hard,kind='source',processCap=4,rssCapBytes=512*MIB,remainingNewUpperBytes=REPAIR_CAP-PREP_RESERVED,controller=dict(pid=os.getpid(),birth=' '.join(identity[:5]),executable=identity[5].strip(),launcherExecutable=sys.executable,script=str(Path(__file__).resolve()),cwd=str(ROOT),outputRoot=str(HERE),role='ART-PC-STRICT scoped TypeScript and twelve hostile-case qualification controller',commandEncoding='argv',argv=[PYTHON,'-B','-u',str(Path(__file__).resolve()),'--head',head]),guardControllers=[],receiptPaths=dict(accept=str(receipt),full=str(receipt)),closedOffers=history)
atomic(RECORD,record)
def emit(event,**fields):
 row=dict(event=event,at=dt.datetime.now(dt.timezone.utc).isoformat(),**fields)
 with receipt.open('a') as f:f.write(json.dumps(row)+'\n')
 print(json.dumps(row),flush=True);return row
emit('ACCEPT',offer=OFFER,acceptedAtEpoch=accepted,head=head,workDeadlineEpoch=work,fullDeadlineEpoch=hard,controller=record['controller'],historicalNewPeak=HISTORICAL_NEW,repairNewCap=REPAIR_CAP,prepReserved=PREP_RESERVED)
temp=None;guardian=None;child=None;meter=None;status='FAIL';failure=None;workers=[];config=None
try:
 assert time.time()<dt.datetime.fromisoformat('2026-11-05T05:00:00+00:00').timestamp()
 assert shutil.disk_usage(ROOT).free>=11*1024**3
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==head
 changed=subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).splitlines()
 assert set(changed)=={'?? docs/mcr/art/evidence/map-roof-silhouette-derived-v4/doc32-booking.json','?? '+str(receipt.relative_to(ROOT))},('Frozen custody mismatch',changed)
 readonly=readonly_pins();source=[pin(TEST),pin(HERE/'scoped-tsconfig.json'),pin(ROOT/'frontend/tsconfig.json'),pin(ROOT/'frontend/src/features/paradise-city-map/art-source/qualify.ts')]
 base=json.loads((ROOT/'frontend/tsconfig.json').read_text())['compilerOptions']
 for name in ('strict','noUncheckedIndexedAccess','exactOptionalPropertyTypes','noEmit','noUnusedLocals','noUnusedParameters','isolatedModules','verbatimModuleSyntax'):assert base[name] is True,name
 scoped=json.loads((HERE/'scoped-tsconfig.json').read_text());assert set(scoped['compilerOptions'])=={'types','typeRoots'}
 temp=Path(tempfile.mkdtemp(prefix='mcr-art-pc-strict-'))
 control=temp/'control.json';state=temp/'state.json';atomic(control,dict(workActive=True,shutdown=False))
 prep=json.loads((HERE/'preparation.json').read_text());assert prep['status']=='FAIL' and prep['allChildrenExited'] and prep['positivePeak']==28423407
 seal=json.loads((HERE/'seal.json').read_text());assert seal['status']=='FAIL' and seal['allChildrenExited'] and seal['positivePeak']==4479502
 config=dict(temp=str(temp),roots=[str(ROOT),'/Users/lhs/Desktop/mcr/.git','/Library/Frameworks/Python.framework/Versions/3.14',str(Path(NODE).parents[1]),'/Users/lhs/Desktop/mcr/frontend/node_modules',str(RECORD),str(temp)],priorWhole=max(4353694762,prep['wholePeak'],seal['wholePeak']),supervisor=os.getpid(),work=work,hard=hard,guardianLog=str(HERE/(OFFER+'-guardian.json')))
 atomic(temp/'config.json',config);meter=Meter(config);initial=meter.check()
 guardian=subprocess.Popen([PYTHON,'-B',str(HERE/'guardian.py'),str(temp/'config.json')],cwd=ROOT,start_new_session=True)
 until=min(work,time.time()+10)
 while not state.exists() and guardian.poll() is None and time.time()<until:time.sleep(.05)
 assert state.exists() and json.loads(state.read_text())['ok'],'Independent initial census'
 complete=meter.check();g=json.loads(state.read_text());assert g['ok'] and guardian.poll() is None
 gid=subprocess.check_output(['ps','-p',str(guardian.pid),'-o','lstart=,comm='],text=True).split(None,5)
 record.update(phase='BOTH_INITIAL_LIVE',guardControllers=[dict(pid=guardian.pid,birth=' '.join(gid[:5]),executable=gid[5].strip(),script=str(HERE/'guardian.py'),cwd=str(ROOT),role='Independent scoped strict-repair resource enforcement')]);atomic(RECORD,record)
 emit('BOTH_INITIAL_LIVE',supervisorInitial=initial,supervisorComplete=complete,independentInitial=g,readonlyPins=readonly,sourcePins=source,effectiveProjectOptions=base,scope='ONE_TEST_ROOT_AND_IMPORTED_QUALIFIER_NOT_WHOLE_APP')
 commands=[('strict-two-file-types',[NODE,'--max-old-space-size=256','/Users/lhs/Desktop/mcr/frontend/node_modules/typescript/bin/tsc','--project',str(HERE/'scoped-tsconfig.json'),'--pretty','false']),('original-twelve-hostile-cases',[NODE,'--max-old-space-size=128',str(TEST)])]
 for index,(name,argv) in enumerate(commands):
  meter.check();before=json.loads(state.read_text());assert before['ok'] and time.time()-before['at']<10
  output=HERE/('command-%02d.log'%index)
  with output.open('w') as f:
   child=subprocess.Popen(argv,cwd=ROOT,env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1',TMPDIR=str(temp)),stdout=f,stderr=subprocess.STDOUT,start_new_session=True);workers.append(child.pid)
   while child.poll() is None:
    meter.check();g=json.loads(state.read_text());assert g['ok'] and guardian.poll() is None and time.time()-g['at']<10,g
    time.sleep(.15)
   child.wait()
  emit('CHILD',name=name,argv=argv,pid=child.pid,exit=child.returncode,output=pin(output));assert child.returncode==0,('Validation failed',name)
 for entry in readonly+source:
  assert pin(Path(entry['path']))==entry,'Frozen source or retained history changed'
 meter.check();status='PASS'
except BaseException as error:failure=repr(error);emit('ERROR',reason=failure)
finally:
 if child is not None and child.poll() is None:
  os.killpg(child.pid,signal.SIGTERM)
  try:child.wait(timeout=2)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=2)
 if guardian is not None:
  atomic(temp/'control.json',dict(workActive=False,shutdown=True))
  try:guardian.wait(timeout=4)
  except subprocess.TimeoutExpired:os.killpg(guardian.pid,signal.SIGTERM);guardian.wait(timeout=2);status='FAIL'
  if guardian.returncode!=0:status='FAIL'
 if meter is not None:
  w,n=meter.inventory.sample();meter.peak['whole']=max(meter.peak['whole'],w);meter.peak['repairNew']=max(meter.peak['repairNew'],PREP_RESERVED+max(0,n-34*MIB))
 if temp is not None:shutil.rmtree(temp)
 if time.time()>=hard:status='FAIL'
 row=emit('FULL',status=status,failure=failure,allChildrenExited=(child is None or child.poll() is not None) and (guardian is None or guardian.poll() is not None),tempAbsent=temp is None or not temp.exists(),workers=workers,peak=meter.peak if meter else None,minimumFree=meter.minimum if meter else shutil.disk_usage(ROOT).free,samples=meter.samples if meter else 0,historicalNewPeak=HISTORICAL_NEW,finalFree=shutil.disk_usage(ROOT).free)
 record.update(state='FULL',remainingNewUpperBytes=0,guardControllers=[],actual=row);atomic(RECORD,record)
sys.exit(0 if status=='PASS' else 1)
