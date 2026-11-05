"""One frozen actual Photo2 source generation and asset qualification round."""
import importlib.util, json, os, shutil, signal, subprocess, sys, time
from pathlib import Path
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('photo2_control',HERE/'source-control.py');control=importlib.util.module_from_spec(spec);spec.loader.exec_module(control)
ROOT=control.ROOT;INDEX=control.INDEX;OFFER=control.OFFER
started=time.time();work=started+240;hard=started+300
me=control.identity('ART Photo2 original raster generation, owned source qualifier and one producer decode probe')
raw_prior=INDEX.read_bytes();prior=json.loads(raw_prior);assert prior['state']=='FULL'
archive=HERE/'owner-before-source03.json';assert not archive.exists();archive.write_bytes(raw_prior)
raw_path=HERE/'source03-full.json';assert not raw_path.exists(),'No closed-round restart'
record=dict(ownerThreadId=control.OWNER,offerId=OFFER,state='ADMISSION',kind='source',phase='INITIAL_ADMISSION',acceptedAtEpoch=started,workDeadlineEpoch=work,fullDeadlineEpoch=hard,processCap=8,rssCapBytes=control.GIB,remainingNewUpperBytes=64*control.MIB,controller=me,guardControllers=[],previousClosedRecordRef=control.pin(archive),historicalWholeFloorBytes=4357560906,historicalNewRetained=221435639,separateStrictRepairRetained=37817629,entirePreparationBooksChargedOnce=[1048576,16777216,67108864,33554432,16777216],previousPreparation32ActualHigherBytes=34526193,previousClose16MeasuredPositiveBytes=16503519,familyCapBytes=536870912,executionWorkflow=True)
control.atomic(INDEX,record)
temp=Path('/private/tmp/mcr-art231-photo2-source03');guardian=None;child=None;meter=None;failure=None;status='FAIL';commands=[];accepted=None
def emit(event,**fields):
 row=dict(event=event,at=time.time(),**fields)
 with (HERE/'source03-events.jsonl').open('a') as f:f.write(json.dumps(row)+'\n')
 print(json.dumps(row),flush=True)
def reload_record():return json.loads(INDEX.read_bytes())
try:
 assert started<1793862900,'Accept by07:15UTC'
 assert control.sha(control.PROJECTION.read_bytes())==control.PROJECTION_SHA
 free=os.statvfs(ROOT).f_bavail*os.statvfs(ROOT).f_frsize;assert free>=11*control.GIB and free>=3355443200+9*control.GIB
 # Kernel identity exists before the first helper and any Git subprocess.
 me=control.raw_ps(me);record['controller']=me;control.atomic(INDEX,record)
 frozen=HERE/'frozen-source03-readset.json'
 if not frozen.exists():
  # Readiness freeze within this granted bootstrap, before finite ACCEPT or any asset command.
  paths=[HERE/n for n in ['author.py','controller.py','source-control.py','memory-probe.py','scoped-tsconfig.json']]+[ROOT/'frontend/src/features/parade-ink/world-hud/__tests__/photo2-art-source-v1/qualifier.test.ts',ROOT/'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade/README.md',ROOT/'frontend/tsconfig.json']
  actual_head=subprocess.check_output(['/usr/bin/git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
  control.atomic(frozen,dict(head=actual_head,files=[control.pin(p) for p in paths],meaning='Actual source frozen during authorized readiness bootstrap; final controller bootstrap fix is pinned by exact SHA and committed with delivery'))
 readset=json.loads(frozen.read_bytes())
 for pin in readset['files']:assert control.pin(Path(pin['path']))==pin,('Frozen source drift',pin['path'])
 assert subprocess.check_output(['/usr/bin/git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==readset['head']
 assert not temp.exists();temp.mkdir()
 config=dict(supervisor=os.getpid(),temp=str(temp),work=work,hard=hard,roots=[str(ROOT),'/Users/lhs/Desktop/mcr/.git','/Library/Frameworks/Python.framework/Versions/3.14','/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64','/Users/lhs/Desktop/mcr/frontend/node_modules',str(INDEX),str(temp)])
 control.atomic(temp/'control.json',dict(work=True,shutdown=False));control.atomic(temp/'config.json',config)
 meter=control.Meter(config)
 record.update(state='ACCEPTED',phase='INITIAL_DUAL_GUARD');control.atomic(INDEX,record)
 accepted=started;projection,current_projection_pin=control.current_projection();emit('ACCEPT',offer=OFFER,acceptedAtEpoch=started,identity=me,projection=current_projection_pin,free=free)
 guardian=subprocess.Popen([control.PYTHON,'-B','-u',str(HERE/'source-control.py'),'--guardian',str(temp/'config.json')],cwd=ROOT,start_new_session=True)
 config['guardian']=guardian.pid
 deadline=min(work,time.time()+35)
 while not (temp/'guard.json').exists() and guardian.poll() is None and time.time()<deadline:time.sleep(.1)
 assert (temp/'guard.json').exists(),'Independent initial snapshot missing'
 independent=json.loads((temp/'guard.json').read_bytes());assert independent['ok'] and guardian.poll() is None,independent
 own=meter.check();record=reload_record();record['phase']='BOTH_INITIAL_COMPLETE_LIVE';control.atomic(INDEX,record)
 emit('BOTH_INITIAL_COMPLETE_LIVE',own=own,independent=independent)
 jobs=[('original-vector-raster',[control.PYTHON,'-B','-u',str(HERE/'author.py'),'--build']),('scoped-strict-types',[control.NODE,'--max-old-space-size=256','/Users/lhs/Desktop/mcr/frontend/node_modules/typescript/bin/tsc','--project',str(HERE/'scoped-tsconfig.json'),'--pretty','false']),('owned-hostile-frame-support-qualifier',[control.NODE,'--max-old-space-size=256',str(ROOT/'frontend/src/features/parade-ink/world-hud/__tests__/photo2-art-source-v1/qualifier.test.ts')]),('single-producer-decode-probe',[control.PYTHON,'-B','-u',str(HERE/'memory-probe.py')])]
 for i,(name,argv) in enumerate(jobs):
  meter.check();state=json.loads((temp/'guard.json').read_bytes());assert state['ok'] and guardian.poll() is None and time.time()-state['at']<35,state
  log=HERE/f'source03-command-{i:02}.log'
  with log.open('w') as out:
   child=subprocess.Popen(argv,cwd=ROOT,env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1',TMPDIR=str(temp)),stdout=out,stderr=subprocess.STDOUT,start_new_session=True)
   while child.poll() is None:
    meter.check();state=json.loads((temp/'guard.json').read_bytes());assert state['ok'] and guardian.poll() is None and time.time()-state['at']<35,state;time.sleep(.1)
   child.wait()
  commands.append(dict(name=name,argv=argv,pid=child.pid,exit=child.returncode,log=control.pin(log)));emit('CHILD',**commands[-1]);assert child.returncode==0,('Actual source command failed',name)
 for pin in readset['files']:assert control.pin(Path(pin['path']))==pin,('Frozen source drift',pin['path'])
 status='PASS';emit('OUTPUTS_READY_FOR_PERSONAL_INSPECTION',sourceOnly=True)
 # Stop owned execution promptly; personal view_image reads follow actual FULL.
except BaseException as e:failure=repr(e);emit('ERROR',reason=failure)
finally:
 if child is not None and child.poll() is None:
  os.killpg(child.pid,signal.SIGTERM)
  try:child.wait(timeout=2)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=2)
 if guardian is not None:
  control.atomic(temp/'control.json',dict(work=False,shutdown=True))
  try:guardian.wait(timeout=12)
  except subprocess.TimeoutExpired:os.killpg(guardian.pid,signal.SIGTERM);guardian.wait(timeout=2);status='FAIL'
  if guardian.returncode:status='FAIL'
 if meter is not None:
  try:final=meter.check(False)
  except BaseException as e:status='FAIL';failure=(failure or '')+'; final:'+repr(e)
 if temp.exists():shutil.rmtree(temp)
 ended=time.time()
 if ended>=hard:status='FAIL';failure=(failure or '')+'; FULLdeadline'
 report=dict(offerId=OFFER,status=status,failure=failure,acceptedAtEpoch=accepted,bootstrapAtEpoch=started,actualFullEpoch=ended,commands=commands,peak=meter.peak if meter else None,minimumFree=meter.minimumFree if meter else None,allChildrenReaped=(child is None or child.poll() is not None) and (guardian is None or guardian.poll() is not None),tempAbsent=not temp.exists(),resources=dict(processes=0,appSlots=0),sourceArtOnly=True,personalInspection='PENDING',worldRuntimeAcceptance=False)
 control.atomic(raw_path,report);record=reload_record();record.update(state='FULL',phase='FULL',remainingNewUpperBytes=0,actualFullEpoch=ended,guardControllers=[],literalReceiptPath=str(raw_path),literalReceiptSHA256=control.sha(raw_path.read_bytes()),resources=dict(processes=0,appSlots=0));control.atomic(INDEX,record);emit('FULL',**report,sha256=control.sha(raw_path.read_bytes()))
sys.exit(0 if status=='PASS' else 1)
