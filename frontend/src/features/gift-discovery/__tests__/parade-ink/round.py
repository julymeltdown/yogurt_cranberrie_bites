"""Finite PI04 execution. Requires sole21d fresh offer, actual ACCEPT and both live guards."""
import hashlib, json, os, shutil, signal, socket, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode=True
from membership import sample as sample_membership, publish_census
root=Path(__file__).resolve().parents[6]; frontend=root/'frontend'; GiB=1024**3
mode=sys.argv[1] if len(sys.argv)==2 else '';assert mode in ['checks','unit-source02','unit-source03','native','native02','native03','native04','native05','native06','source07','source08','native07','native08','native09','native10','native11','native12','native13']
is_native=mode.startswith('native')
booking=os.environ['MCR_PI04_BOOKING'];temporary=Path(os.environ['MCR_PI04_TEMP']).resolve();borrowed=Path(os.environ['MCR_PI04_BORROWED']).resolve()
assert temporary.parent==Path('/private/tmp') and temporary.name.startswith('mcr-pi04-') and not temporary.exists()
assert borrowed.name=='frontend' and (borrowed/'node_modules').is_dir()
cap=int(os.environ['MCR_PI04_CAP_MIB'])*1024**2;assert 0<cap<=(200 if mode in ['native04','native05','native06'] else 384 if mode in ['native12','native13'] else 352 if mode=='native11' else 320 if mode in ['native07','native08','native09','native10'] else 128)*1024**2
start_floor=int(os.environ['MCR_PI04_START_GIB']);assert start_floor in [11,12]
seconds=int(os.environ['MCR_PI04_SECONDS']);assert 0<seconds<=1200
full_seconds=int(os.environ.get('MCR_PI04_FULL_SECONDS','720'));assert seconds<full_seconds<=1380
carried=int(os.environ.get('MCR_PI04_CARRIED_MIB','0'))*1024**2;assert 0<=carried<cap
all_limit=int(os.environ.get('MCR_PI04_ALL','12'));rss_limit=int(os.environ.get('MCR_PI04_RSS_MIB','3072'))*1024**2
assert (all_limit<=24 and rss_limit<=3584*1024**2) if is_native else (all_limit==12 and rss_limit==3*GiB)
accepted=float(os.environ['MCR_PI04_ACCEPTED_EPOCH']);assert accepted<=time.time()<accepted+seconds
other=int(os.environ['MCR_PI04_OTHER_MIB'])*1024**2
free=lambda:os.statvfs(root).f_bavail*os.statvfs(root).f_frsize
assert free()>=start_floor*GiB and free()-other-cap>=9*GiB,'Fresh admission/reserved residual floor failed; do not retry this offer'
link=frontend/'node_modules';assert not link.exists() and not link.is_symlink()
git=Path(subprocess.check_output(['git','rev-parse','--git-common-dir'],cwd=root,text=True).strip()).resolve()
def blocks(path):
 total=0
 for base,dirs,files in os.walk(path,followlinks=False):
  dirs[:]=[d for d in dirs if not (Path(base)/d).is_symlink()]
  for name in files:
   p=Path(base)/name
   if not p.is_symlink():
    try:total+=p.stat().st_blocks*512
    except FileNotFoundError:pass
 return total
receipt_path=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a5d9-6955-7820-bcd4-8e0a21629aa2.json') if mode in ['native04','native05','native06','source07','source08','native07','native08','native09','native10','native11','native12','native13'] else None
assert receipt_path is None or receipt_path.parent.is_dir() and receipt_path.parent.stat().st_dev==root.stat().st_dev
def external_bytes():
 try:return receipt_path.stat().st_blocks*512 if receipt_path else 0
 except FileNotFoundError:return 0
bootstrap={'pid':os.getpid(),'birth':' '.join(subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart='],text=True).split()),'executable':subprocess.check_output(['ps','-p',str(os.getpid()),'-o','comm='],text=True).strip(),'script':str(Path(__file__).resolve()),'cwd':str(root),'acceptedEpoch':accepted};print('BOOTSTRAP '+json.dumps(bootstrap),flush=True)
before_local=before=0;started=time.monotonic();output=root/'docs/mcr/evidence/parade-ink-giveaway'/mode
whole_roots=[r['path'] for r in json.loads((root/'docs/mcr/evidence/parade-ink-giveaway/native-whole-census.json').read_text())['roots']] if is_native or mode in ['source07','source08'] else []
whole_limit=int(os.environ.get('MCR_PI04_WHOLE_GIB','5'))*GiB
def whole():return sum(int(line.split()[0])*1024 for line in subprocess.check_output(['du','-sk',*whole_roots],text=True).splitlines())+external_bytes() if whole_roots else 0
def new_used():return carried+max(0,blocks(root)+blocks(git)+external_bytes()-before)+blocks(temporary)
assert not output.exists(),'Each finite round requires a fresh evidence directory'
record={'booking':booking,'mode':mode,'acceptedEpoch':accepted,'workDeadline':accepted+seconds,'fullDeadline':accepted+full_seconds,'carriedNewBytes':carried,'startAvailable':free(),'status':'RUNNING','commands':[],'temporaryCleaned':False,'dependencyLinkRemoved':False}
child=guardian=None;new_high=carried;last_index=0
def process_metadata(pid,script):
 return {'pid':pid,'birth':' '.join(subprocess.check_output(['ps','-p',str(pid),'-o','lstart='],text=True).split()),'executable':subprocess.check_output(['ps','-p',str(pid),'-o','comm='],text=True).strip(),'script':str(Path(script).resolve()),'cwd':str(root),'outputRoot':str(output)}
def publish_index(state):
 global last_index
 if receipt_path is None:return
 body={'ownerThreadId':'01a1a5d9-6955-7820-bcd4-8e0a21629aa2','offerId':booking,'state':state,'kind':'browser' if is_native else 'source','acceptedAtEpoch':accepted,'workDeadlineEpoch':accepted+seconds,'fullDeadlineEpoch':accepted+full_seconds,'processCap':all_limit,'rssCapBytes':rss_limit,'newCapBytes':cap,'carriedNewBytes':carried,'remainingNewUpperBytes':0 if state=='FULL' else max(0,cap-new_high),'controller':record['controllerMetadata'],'guardControllers':[record['guardianMetadata']] if 'guardianMetadata' in record else [],'receiptPaths':{'accept':str(output/'accepted.json'),'full':str(output/'round.json')},'publishedAtEpoch':time.time(),'resources':0 if state=='FULL' else 1}
 body['closedOffers']=[]
 for full_path in sorted((root/'docs/mcr/evidence/parade-ink-giveaway').glob('*/round.json')):
  raw=full_path.read_bytes();closed=json.loads(raw)
  if 'actualFullEpoch' not in closed:continue
  body['closedOffers'].append({'ownerThreadId':body['ownerThreadId'],'offerId':closed['booking'],'acceptedAtEpoch':closed['acceptedEpoch'],'actualFullEpoch':closed['actualFullEpoch'],'immutableFULLpath':str(full_path),'immutableFULLsha256':hashlib.sha256(raw).hexdigest(),'resources':0})
 if state=='FULL':body['actualFullEpoch']=record['actualFullEpoch']
 else:
  if not (output/'accepted.json').exists():(output/'accepted.json').write_text(json.dumps(body,indent=2)+'\n')
 stage=output/'.owner-receipt-next.json'
 with stage.open('w') as file:json.dump(body,file,indent=2);file.flush();os.fsync(file.fileno())
 os.replace(stage,receipt_path);last_index=time.time();record['resourceIndex']=str(receipt_path)

def interrupted(_signal,_frame):raise RuntimeError('Independent guard or external termination')
signal.signal(signal.SIGTERM,interrupted)
try:
 output.mkdir(parents=True,exist_ok=True);temporary.mkdir();link.symlink_to(borrowed/'node_modules',target_is_directory=True)
 env={**os.environ,'MCR_PI04_OUTPUT':str(output),'TMPDIR':str(temporary),'UV_THREADPOOL_SIZE':'1','RAYON_NUM_THREADS':'1','TOKIO_WORKER_THREADS':'1','NODE_OPTIONS':'--v8-pool-size=1 --max-old-space-size=1536'}
 claim={'owner':'01a1a5d9-6955-7820-bcd4-8e0a21629aa2','booking':booking,'kind':'browser' if is_native else 'source','root':str(root),'controller':str(Path(__file__).resolve()),'pid':os.getpid(),'pgid':os.getpgrp(),'birth':subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart='],text=True).strip(),'at':time.time(),'acceptedEpoch':accepted,'workDeadline':accepted+seconds,'fullDeadline':accepted+full_seconds,'processCap':all_limit,'rssCapBytes':rss_limit,'newCapBytes':cap,'carriedNewBytes':carried,'wholeCapBytes':whole_limit,'wholeRoots':whole_roots,'privateTemporaryRoot':str(temporary)}
 record['controllerMetadata']=process_metadata(os.getpid(),__file__)
 publish_index('ACCEPTED')
 guardian=subprocess.Popen([sys.executable,str(Path(__file__).with_name('guardian.py')),str(temporary/'guard-config.json')])
 bootstrap_handshake=time.monotonic()+5
 while not (temporary/'guardian-bootstrap.json').exists():
  assert guardian.poll() is None and time.monotonic()<bootstrap_handshake,'Guardian bootstrap failed before census'
  time.sleep(.05)
 record['guardianMetadata']=process_metadata(guardian.pid,Path(__file__).with_name('guardian.py'));publish_index('ACCEPTED')
 before_local=blocks(root)+blocks(git);before=before_local+external_bytes()
 assert not whole_roots or whole()+cap<=whole_limit,'Complete WHOLE plus all NEW does not fit'
 (temporary/'primary-live.json').write_text(json.dumps(claim));record['ownerClaim']=claim
 config={'output':str(output),'root':str(root),'git':str(git),'scratch':str(temporary),'primaryPid':os.getpid(),'all':all_limit,'rss':rss_limit,'new':cap,'carriedNew':carried,'baselineBytes':before+blocks(temporary),'wholeRoots':whole_roots+[str(temporary)] if whole_roots else [],'externalFiles':[str(receipt_path)] if receipt_path else [],'wholeLimit':whole_limit if whole_roots else 0,'fullDeadline':accepted+full_seconds}
 (temporary/'guard-config.json').write_text(json.dumps(config))
 (temporary/'guardian-metadata-published').touch()
 handshake=time.monotonic()+5
 while not (temporary/'guardian-live.json').exists():
  assert guardian.poll() is None and time.monotonic()<handshake,'Independent guardian failed before workload'
  (temporary/'primary-live.json').touch();time.sleep(.05)
 assert free()-other-cap>=9*GiB
 record['bothLiveBeforeWorkload']={'primary':os.getpid(),'guardian':guardian.pid,'at':time.time(),'initial':json.loads((temporary/'guardian-live.json').read_text())}
 record['guardianMetadata']=process_metadata(guardian.pid,Path(__file__).with_name('guardian.py'));publish_index('ACCEPTED')
 record['initialMembership']=sample_membership();publish_census(output/'controller-census.json',record['initialMembership'])
 print('BOTH_LIVE: primary and independent guardian sampled before workload',flush=True)
 commands=[
  [str(link/'.bin/tsc'),'--project','src/features/gift-discovery/__tests__/parade-ink/tsconfig.json','--pretty','false'],
  [str(link/'.bin/oxlint'),'src/features/gift-discovery/ui','src/features/gift-discovery/__tests__/parade-ink','src/features/my-gifts/MyGifts.tsx','src/features/giveaway-authoring/ui','src/features/giveaway-authoring/__tests__/parade-ink','src/features/gift-pickup/ui','src/features/gift-pickup/__tests__/parade-ink','src/app/community-entry/routes.tsx','src/app/community-entry/pickup-media.tsx','src/app/community-entry/__tests__/parade-ink-pickup-media'],
  [str(link/'.bin/vitest'),'run','--config','src/features/gift-discovery/__tests__/parade-ink/vitest.config.mjs'],
 ] if mode=='checks' else [[str(link/'.bin/vitest'),'run','--config','src/features/gift-discovery/__tests__/parade-ink/vitest.config.mjs']] if mode.startswith('unit-source') else [['node','src/features/gift-discovery/__tests__/parade-ink/native.mjs']]
 labels=['types','lint','unit'] if mode=='checks' else ['unit'] if mode.startswith('unit-source') else ['native']
 if mode=='unit-source03':
  commands=[
   [str(link/'.bin/tsc'),'--project','src/features/gift-discovery/__tests__/parade-ink/tsconfig.json','--pretty','false'],
   [str(link/'.bin/oxlint'),'src/features/gift-pickup/__tests__/harness.ts','src/features/giveaway-authoring/__tests__/harness.ts','src/features/gift-discovery/__tests__/fixtures/data.ts','src/app/community-entry/__tests__/fixture.ts','src/app/community-entry/__tests__/parade-ink-pickup-media','src/features/gift-pickup/__tests__/parade-ink','src/features/gift-discovery/__tests__/parade-ink'],
   commands[0],
  ];labels=['fixture-types','fixture-lint','unit']
 if mode in ['source07','source08']:
  commands=[
   [str(link/'.bin/tsc'),'--project','src/features/gift-discovery/__tests__/parade-ink/tsconfig.json','--pretty','false'],
   [str(link/'.bin/oxlint'),'src/features/gift-discovery/ui','src/features/gift-pickup/ui','src/features/giveaway-authoring/ui','src/features/gift-discovery/__tests__/parade-ink','src/features/gift-pickup/__tests__/parade-ink'],
   [str(link/'.bin/vitest'),'run','--config','src/features/gift-discovery/__tests__/parade-ink/vitest.config.mjs','src/features/gift-discovery/__tests__/parade-ink/presentation.test.tsx','src/features/gift-pickup/__tests__/parade-ink/presentation.test.tsx','src/features/gift-pickup/__tests__/pickup.test.tsx','src/features/giveaway-authoring/__tests__/parade-ink/presentation.test.tsx'],
   ['node','--check','src/features/gift-discovery/__tests__/parade-ink/native.mjs'],
   [sys.executable,'-B','src/features/gift-discovery/__tests__/parade-ink/membership_test.py'],
  ];labels=['affected-types','affected-lint','affected-unit','driver-syntax','resource-cases']
 if is_native:
  commands=[
   [str(link/'.bin/tsc'),'--project','src/features/gift-discovery/__tests__/parade-ink/tsconfig.native.json','--pretty','false'],
   [str(link/'.bin/oxlint'),'src/features/gift-discovery/__tests__/parade-ink/preview.tsx','src/features/gift-discovery/__tests__/parade-ink/state-preview.tsx','src/features/gift-discovery/__tests__/parade-ink/native.mjs','src/features/gift-discovery/__tests__/parade-ink/reference-comparison.mjs'],
   commands[0],
  ];labels=['native-types','native-lint','native']
  if mode in ['native02','native03','native04','native05','native06','native07','native08','native09','native10','native11','native12','native13']:commands=commands[1:];labels=labels[1:]
 if mode in ['native10','native11','native12','native13']:
  commands.insert(0,[sys.executable,'-B','src/features/gift-discovery/__tests__/parade-ink/membership_test.py']);labels.insert(0,'resource-cases')
 if mode in ['native12','native13']:
  commands[0]=[sys.executable,'-B','src/features/gift-discovery/__tests__/parade-ink/membership_test.py','MembershipCases.test_completed_state_observation_cannot_identify_new_payload','MembershipCases.test_incomplete_live_observation_rejects_with_owner']
 for label,command in zip(labels,commands):
  assert time.time()<accepted+seconds and free()>=9*GiB
  with (output/(label+'.log')).open('w') as log:
   child=subprocess.Popen(command,cwd=frontend,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
   (temporary/'workload-pgid').write_text(str(child.pid));record['commands'].append({'label':label,'pid':child.pid})
   while child.poll() is None:
    (temporary/'primary-live.json').touch()
    publish_census(output/'controller-census.json',sample_membership())
    assert guardian.poll() is None and time.time()-(temporary/'guardian-live.json').stat().st_mtime<5,'Independent guardian lost'
    new_high=max(new_high,new_used())
    assert free()>=8*GiB and new_high<=cap and time.time()<accepted+seconds,'Primary finite storage/growth/time guard failed'
    assert not whole_roots or whole()+blocks(temporary)<=whole_limit,'Primary complete WHOLE cap failed'
    if time.time()-last_index>=3:publish_index('ACCEPTED')
    time.sleep(.3)
   code=child.returncode;record['commands'][-1]['exitCode']=code;print(f'{label}: exit {code}',flush=True)
   if code:raise RuntimeError(f'{label} failed; round closed, no automatic restart')
   child=None
 record['status']='PASS'
except BaseException as error:
 record['status']='FAIL';record['failure']=str(error);raise
finally:
 signal.signal(signal.SIGTERM,signal.SIG_IGN)
 if child and child.poll() is None:
  os.killpg(child.pid,signal.SIGTERM)
  try:child.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
 if is_native:
  # Chromium may use a detached process group. Match only this private profile.
  def owned_chrome():
   return [int(line.strip().split(None,1)[0]) for line in subprocess.check_output(['ps','-axo','pid=,command='],text=True).splitlines() if '/Applications/Google Chrome.app/' in line and str(temporary) in line]
  for kill_signal in [signal.SIGTERM,signal.SIGKILL]:
   for pid in owned_chrome():
    try:os.kill(pid,kill_signal)
    except ProcessLookupError:pass
   time.sleep(.2)
  record['remainingOwnedChrome']=owned_chrome()
  with socket.socket() as probe:
   probe.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
   try:probe.bind(('127.0.0.1',int(os.environ['MCR_PI04_NATIVE_PORT'])));record['nativePortReleased']=True
   except OSError:record['nativePortReleased']=False
 if guardian:
  (temporary/'guardian-stop').touch()
  try:guardian.wait(timeout=5)
  except subprocess.TimeoutExpired:guardian.terminate();guardian.wait(timeout=5)
  if (temporary/'guardian-final.json').exists():record['guardian']=json.loads((temporary/'guardian-final.json').read_text())
 if link.is_symlink() and link.resolve()==(borrowed/'node_modules').resolve():link.unlink();record['dependencyLinkRemoved']=True
 if temporary.exists():shutil.rmtree(temporary)
 record['temporaryCleaned']=not temporary.exists()
 record.update({'endAvailable':free(),'elapsedSeconds':round(time.monotonic()-started,2),'checkoutPositiveBytes':max(0,blocks(root)+blocks(git)-before_local),'outsideMetadataBytes':external_bytes(),'remainingManagedParent':0 if child is None or child.poll() is not None else 1,'actualFullEpoch':time.time()})
 if output.exists():
  (output/'round.json').write_text(json.dumps(record,indent=2)+'\n')
  if 'controllerMetadata' in record:publish_index('FULL')
 print(json.dumps(record),flush=True)
