"""Start only on an actual fresh named offer; original deadlines never move."""
import argparse
import atexit
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from inventory import Inventory,process_snapshot,qualification_deadlines

ROOT = Path(__file__).resolve().parents[5]
PYTHON = '/Library/Frameworks/Python.framework/Versions/3.14/bin/python3'
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
PACK = ROOT/'frontend/public/venue/paradise-city/v1'
TESTS = ROOT/'frontend/src/features/paradise-city-map/art-source'
EVIDENCE = Path(__file__).resolve().parent
DOC = ROOT/'docs/mcr/art/paradise-city-siteplan-v1.md'
SOURCE_BASE='8ac5dc2e2b9b336885da3ec85e82641a9f8cebe1'
MIB = 1024**2
WHOLE_CAP = 5*1024**3
NEW_CAP = 256*MIB
NEW_CARRY = 221400720  # previous cumulative carry plus actual closed QUALIFY04 growth
FUTURE_NEW = NEW_CAP-NEW_CARRY
OUTPUT_CAP = 4*MIB
START_FREE_FLOOR = 12*1024**3
parser = argparse.ArgumentParser()
parser.add_argument('--offer', required=True)
parser.add_argument('--head', required=True)
parser.add_argument('--work', required=True)
parser.add_argument('--hard', required=True)
parser.add_argument('--other-budget-bytes', required=True, type=int)
parser.add_argument('--resource-record', required=True)
parser.add_argument('--reference-root', required=True)
args = parser.parse_args()
resource_record=Path(args.resource_record)
assert resource_record.is_absolute() and resource_record.name=='01a1a231-0dc4-7b50-92bd-c0f0f56bd970.json' and resource_record.parent.name=='owner-resource-receipts'
assert resource_record.parent.is_dir() and not resource_record.is_symlink()
previous_resource_record=json.loads(resource_record.read_text()) if resource_record.exists() else None
os.chdir(ROOT)
work = dt.datetime.fromisoformat(args.work).timestamp()
hard = dt.datetime.fromisoformat(args.hard).timestamp()
accepted_epoch=time.time()
work,hard=qualification_deadlines(accepted_epoch,work,hard)
args.work=dt.datetime.fromtimestamp(work,dt.timezone.utc).isoformat()
args.hard=dt.datetime.fromtimestamp(hard,dt.timezone.utc).isoformat()
receipt = EVIDENCE/(args.offer+'.jsonl')
assert not receipt.exists(),'A closed offer must never be reused'
birth_line=subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart=,comm='],text=True).split(None,5)
bootstrap_identity=dict(pid=os.getpid(),birth=' '.join(birth_line[:5]),executable=birth_line[5].strip(),launcherExecutable=sys.executable,script=str(Path(__file__).resolve()),cwd=str(ROOT),outputRoot=str(EVIDENCE))
closed_offers=previous_resource_record.get('closedOffers',[])
if previous_resource_record.get('state')=='FULL':
    old_path=Path(previous_resource_record['receiptPaths']['full'])
    actual=previous_resource_record['actual']
    old_epoch=dt.datetime.fromisoformat(actual.get('fullAt',actual.get('at'))).timestamp()
    closed_offers=closed_offers+[dict(ownerThreadId=previous_resource_record['ownerThreadId'],offerId=previous_resource_record['offerId'],acceptedAtEpoch=previous_resource_record['acceptedAtEpoch'],actualFullEpoch=old_epoch,fullPath=str(old_path),fullSha256=hashlib.sha256(old_path.read_bytes()).hexdigest(),resourcesReleased=True)]
bootstrap=dict(ownerThreadId='01a1a231-0dc4-7b50-92bd-c0f0f56bd970',offerId=args.offer,state='ACCEPTED',acceptedAtEpoch=accepted_epoch,workDeadlineEpoch=work,fullDeadlineEpoch=hard,kind='source',processCap=4,rssCapBytes=512*MIB,remainingNewUpperBytes=FUTURE_NEW,controller=bootstrap_identity,guardControllers=[],receiptPaths=dict(accept=str(receipt),full=str(receipt)),closedOffers=closed_offers,phase='ACTUAL_BOOTSTRAP_BEFORE_HEAVY_CENSUS')
staging=resource_record.with_suffix('.pending');staging.write_text(json.dumps(bootstrap,indent=2)+'\n');staging.replace(resource_record)
with receipt.open('a') as handle:handle.write(json.dumps(dict(event='BOOTSTRAP_ACCEPT',at=dt.datetime.now(dt.timezone.utc).isoformat(),acceptedAtEpoch=accepted_epoch,controller=bootstrap_identity,work=args.work,hard=args.hard))+'\n')
def close_failed_bootstrap():
    current=json.loads(resource_record.read_text())
    if current.get('offerId')==args.offer and current.get('state')=='ACCEPTED':
        if 'temp' in globals():shutil.rmtree(temp,ignore_errors=True)
        row=dict(event='FULL',at=dt.datetime.now(dt.timezone.utc).isoformat(),status='FAIL',phase='BOOTSTRAP_OR_PREFLIGHT',allChildrenExited=True,tempAbsent='temp' not in globals() or not temp.exists())
        with receipt.open('a') as handle:handle.write(json.dumps(row)+'\n')
        current.update(state='FULL',remainingNewUpperBytes=0,actual=row)
        pending=resource_record.with_suffix('.pending');pending.write_text(json.dumps(current,indent=2)+'\n');pending.replace(resource_record)
        print(json.dumps(row),flush=True)
atexit.register(close_failed_bootstrap)
initial_free = shutil.disk_usage(ROOT).free
assert time.time() < work < hard and initial_free >= START_FREE_FLOOR
assert sys.executable == PYTHON and sys.version_info[:3] == (3, 14, 3)
from PIL import __version__ as pillow_version
assert pillow_version == '12.2.0'

def git(*argv):
    return subprocess.check_output(['git', *argv], cwd=ROOT)


assert git('rev-parse', 'HEAD').decode().strip() == args.head
# One pre-existing untracked raw receipt is explicitly preserved by Root.
assert set(git('status','--porcelain').decode().splitlines())=={
    '?? docs/mcr/art/evidence/map-roof-silhouette-derived-v4/doc32-booking.json',
    '?? '+str(receipt.relative_to(ROOT)),
},'Only preserved old raw receipt and this actual bootstrap receipt may be untracked'
SCOPES=('frontend/public/venue/paradise-city/v1/','frontend/src/features/paradise-city-map/art-source/',
        'docs/mcr/art/paradise-city-siteplan-v1.md','docs/mcr/art/evidence/paradise-city-siteplan-v1/')
assert all(any(name==prefix or name.startswith(prefix) for prefix in SCOPES) for name in git('diff','--name-only',SOURCE_BASE,'HEAD').decode().splitlines()),'New four scopes only'


def size(path):
    total=0
    for p in ([path] if path.is_file() else path.rglob('*')):
        try:
            if p.is_file() and not p.is_symlink():total+=p.stat().st_size
        except FileNotFoundError:pass  # Independent guardian atomically replaces its private state.
    return total


def own_size():
    return sum(size(p) for p in (PACK, TESTS, EVIDENCE, DOC))


custody=json.loads((EVIDENCE/'preserved-custody.json').read_text())
baseline_pins=custody['files']
reference_root=Path(args.reference_root)
input_pins=[dict(path=str(reference_root/'5-Photo-5.jpg'),bytes=328720,sha256='d0713633289f87fc98603927f7cbc02df2054e989f6360332a93476d6c9da4e0'),
            dict(path=str(reference_root/'10-Photo-10.jpg'),bytes=320653,sha256='1f0e133be3a4bf999fb7871188cce67086d09106a095c266b74e648a67777f63')]
for p in baseline_pins+input_pins:
    path=Path(p['path']) if Path(p['path']).is_absolute() else ROOT/p['path'];raw=path.read_bytes()
    assert len(raw)==p['bytes'] and hashlib.sha256(raw).hexdigest()==p['sha256']
source_pins=[]
for scope in (PACK,TESTS,EVIDENCE):
    for p in sorted(scope.rglob('*')):
        if p.is_file():source_pins.append(dict(path=str(p.relative_to(ROOT)),bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
source_pins.append(dict(path=str(DOC.relative_to(ROOT)),bytes=DOC.stat().st_size,sha256=hashlib.sha256(DOC.read_bytes()).hexdigest()))
ground_pins={}
initial_own = own_size()
# Full retained checkout, shared/private Git and full Python/Node installations.
common_git=Path(git('rev-parse','--git-common-dir').decode().strip()).resolve()
inventory_roots=[str(ROOT),str(common_git),str(Path(PYTHON).resolve().parents[1]),str(Path(NODE).parents[1]),str(resource_record),*[p['path'] for p in input_pins]]
prior_whole_peak=max(4181242520,json.loads((EVIDENCE/'preparation-booking.json').read_text())['wholePeak'])
inventory=Inventory(inventory_roots,prior_whole_peak)
fresh_carry=inventory.logical+34*MIB
carry=inventory.carry
assert carry<WHOLE_CAP,('Complete retained carry exceeds whole cap',carry)
assert args.other_budget_bytes >= 0
# Measured free already excludes retained checkout/tools/history. Reserve only
# worst-case additional owned bytes here; enforce the whole-job cap separately.
aggregate_residual = initial_free-args.other_budget_bytes-FUTURE_NEW
assert aggregate_residual >= 9*1024**3, 'Fresh functional inventory must retain aggregate9GiB margin'
guardian_log=EVIDENCE/(args.offer+'-independent-guardian.jsonl')
assert not guardian_log.exists(), 'A retained named offer must never be reused'
temp = Path(tempfile.mkdtemp(prefix='mcr-paradise-art-'))
preview_dir=temp/'previews';preview_dir.mkdir()
inventory.roots.append(str(temp))
control=temp/'control.json';guardian_state=temp/'guardian-state.json';guardian_config=temp/'guardian-config.json'
guardian=None
child = None
workers = []
def update_control(work_active=True,shutdown=False):
    staging=control.with_suffix('.pending')
    staging.write_text(json.dumps(dict(workActive=work_active,shutdown=shutdown,worker=child.pid if child is not None and child.poll() is None else None)))
    staging.replace(control)
update_control()
peak = dict(whole=carry, new=NEW_CARRY, tmp=0, rss=0, processes=0)
minimum_free = initial_free
known = {os.getpid()}
samples = 0
status = 'RUNNING'
birth_line=subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart=,comm='],text=True).split(None,5)
last_identity=dict(birth=' '.join(birth_line[:5]),executable=birth_line[5].strip(),controllerIdentities={})


def emit(event, **fields):
    row = dict(event=event, at=dt.datetime.now(dt.timezone.utc).isoformat(), **fields)
    line = json.dumps(row)
    with receipt.open('a') as f:
        f.write(line+'\n')
    print(line, flush=True)
    if event in ('ACCEPT','INDEPENDENT_GUARDIAN_READY','FULL'):
        identity=last_identity
        record=dict(ownerThreadId='01a1a231-0dc4-7b50-92bd-c0f0f56bd970',offerId=args.offer,
            state='FULL' if event=='FULL' else 'ACCEPTED',acceptedAtEpoch=accepted_epoch,
            workDeadlineEpoch=work,fullDeadlineEpoch=hard,kind='source',processCap=4,rssCapBytes=512*MIB,
            remainingNewUpperBytes=0 if event=='FULL' else max(0,NEW_CAP-peak['new']),
            controller=dict(pid=os.getpid(),birth=identity['birth'],executable=identity['executable'],script=str(Path(__file__).resolve()),cwd=str(ROOT),outputRoot=str(temp)),
            guardControllers=([dict(**identity['controllerIdentities'][str(guardian.pid)],script=str(EVIDENCE/'resource_guardian.py'))] if guardian is not None and guardian.poll() is None else []),
            receiptPaths=dict(accept=str(receipt),full=str(receipt)),closedOffers=closed_offers,actual=row)
        staging=resource_record.with_suffix('.pending');staging.write_text(json.dumps(record,indent=2)+'\n');staging.replace(resource_record)


def sample(deadline):
    global minimum_free,samples,last_identity
    assert time.time()<deadline,'Original deadline exceeded'
    free=shutil.disk_usage(ROOT).free;minimum_free=min(minimum_free,free)
    assert free>=8*1024**3,'Live free floor'
    whole,new=inventory.sample();new=NEW_CARRY+max(0,new-34*MIB);tmp=size(temp);durable=own_size()
    assert max(0,durable-initial_own)<=OUTPUT_CAP,'Generated evidence/output cap'
    assert whole<=WHOLE_CAP and new<=NEW_CAP and tmp<=4*MIB and durable<=8*MIB,'Supervisor storage cap'
    processes=process_snapshot(os.getpid(),temp/'sampler.lock')
    last_identity=processes
    assert free-max(args.other_budget_bytes,processes['globalCensus']['otherRemainingUpper'])-max(0,NEW_CAP-new)>=9*1024**3,'Actual aggregate growth envelope'
    known.update(processes['pids'])
    assert processes['rss']<=512*MIB and processes['processes']<=4,'Supervisor process/RSS cap'
    if guardian is not None:
        state=json.loads(guardian_state.read_text())
        known.update(state.get('pids',[]))
        assert state['ok'] and guardian.poll() is None and time.time()-state['at']<10,('Independent guardian unavailable',state)
        for key,value in state['peak'].items():peak[key]=max(peak[key],value)
        minimum_free=min(minimum_free,state['minimumFree'])
    for key,value in dict(whole=whole,new=new,tmp=tmp,rss=processes['rss'],processes=processes['processes']).items():
        peak[key]=max(peak[key],value)
    samples+=1
    assert time.time()<deadline,'Original deadline exceeded during sampling'

def stop():
    if child is not None and child.poll() is None:
        try:os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:pass
        try:
            child.wait(timeout=1)
        except subprocess.TimeoutExpired:
            try:os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:pass
            child.wait(timeout=1)


try:
    emit('ACCEPT', offer=args.offer, head=args.head, guardian=os.getpid(), work=args.work, hard=args.hard,
         qualificationScope='EXACT_PHOTO5_AND_SOURCE_PRESENTATION_ANCHORS_ONLY',
         executable=sys.executable, python=sys.version, pillow=pillow_version, node=NODE,
         initialFree=initial_free, initialOwnBytes=initial_own, inventoryLogical=inventory.logical, inventoryRoots=inventory_roots,
         startFreeFloorBytes=START_FREE_FLOOR,
         otherBudgetBytes=args.other_budget_bytes, aggregateOwnNewReserveBytes=FUTURE_NEW, cumulativeNewCap=NEW_CAP, retainedNewCarry=NEW_CARRY,
         aggregateResidualBytes=aggregate_residual,
         freshCarry=fresh_carry, priorWholePeak=prior_whole_peak,
         carry=carry, baselinePins=baseline_pins, inputPins=input_pins,
         sourcePins=source_pins, temp=str(temp), wholeCap=WHOLE_CAP, newCap=NEW_CAP,
         durableCap=8*MIB, tempCap=4*MIB, rssCap=512*MIB, processCap=4)
    guardian_config.write_text(json.dumps(dict(root=str(ROOT),temp=str(temp),control=str(control),state=str(guardian_state),
        log=str(guardian_log),scope=[str(p) for p in (PACK,TESTS,EVIDENCE,DOC)],inventoryRoots=inventory.roots,
        priorCarry=max(carry,prior_whole_peak),newCarry=NEW_CARRY,newCap=NEW_CAP,initialOwn=initial_own,outputCap=OUTPUT_CAP,other=args.other_budget_bytes,wholeCap=WHOLE_CAP,
        supervisor=os.getpid(),work=work,hard=hard)))
    guardian=subprocess.Popen([PYTHON,'-B',str(EVIDENCE/'resource_guardian.py'),str(guardian_config)],
                              cwd=ROOT,env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1'),start_new_session=True)
    known.add(guardian.pid)
    guardian_wait_until=min(work,time.time()+10)
    while not guardian_state.exists() and guardian.poll() is None and time.time()<guardian_wait_until:time.sleep(.05)
    assert guardian_state.exists(),'Independent pre-child guardian did not become ready'
    sample(work)
    emit('INDEPENDENT_GUARDIAN_READY',pid=guardian.pid,config=str(guardian_config),state=json.loads(guardian_state.read_text()),supervisorCensus=last_identity)
    commands = [
        ('verify-source-package', [PYTHON,'-B',str(TESTS/'package.py'),'--verify']),
        ('bind-unchanged-source-checks-and-measured-comparison',[PYTHON,'-B',str(EVIDENCE/'bind_positive_evidence.py'),args.offer,args.head]),
    ]
    attempt = 0
    while True:
        attempt += 1
        emit('ATTEMPT', attempt=attempt, head=git('rev-parse', 'HEAD').decode().strip(),
             sourcePins=[dict(path=str(p.relative_to(ROOT)), sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                         for p in (TESTS/'package.py',TESTS/'source-anchors.json',TESTS/'qualify.ts',TESTS/'qualifier.test.ts',EVIDENCE/'compare.py',EVIDENCE/'resource_guardian.py',EVIDENCE/'inventory.py',EVIDENCE/'guard_regressions.py',EVIDENCE/'identity_regressions.py',EVIDENCE/'bind_positive_evidence.py',Path(__file__).resolve())])
        failed = None
        update_control()
        for name, argv in commands:
            sample(work)
            started = time.time()
            if name=='measured-reference-asset-comparison':argv=[*argv,'--attempt',str(attempt)]
            child = subprocess.Popen(argv, cwd=ROOT, env=dict(os.environ, PYTHONDONTWRITEBYTECODE='1', TMPDIR=str(temp)),
                                     start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            update_control()
            known.add(child.pid)
            workers.append(child.pid)
            while child.poll() is None:
                sample(work)
                time.sleep(0.15)
            stdout, stderr = child.communicate()
            emit('CHILD', attempt=attempt, name=name, pid=child.pid, argv=argv, exit=child.returncode,
                 seconds=round(time.time()-started, 3), stdout=stdout, stderr=stderr)
            sample(work)
            if child.returncode != 0:
                failed = name
                break
        if failed is None:
            break
        emit('WAIT_FOR_BOUNDED_REPAIR', attempt=attempt, failed=failed, work=args.work,
             instruction='RETRY restarts all serial steps within the unchanged work clock; ABORT releases FAIL')
        while True:
            sample(work)
            if select.select([sys.stdin], [], [], 0.2)[0]:
                value = sys.stdin.readline()
                assert value, 'Repair input closed'
                if value.strip() == 'RETRY':
                    break
                assert value.strip() != 'ABORT', 'Owner aborted failed attempt'
    for pin in baseline_pins+input_pins+list(ground_pins.values()):
        raw = (Path(pin['path']) if Path(pin['path']).is_absolute() else ROOT/pin['path']).read_bytes()
        assert len(raw) == pin['bytes'] and hashlib.sha256(raw).hexdigest() == pin['sha256'].removeprefix('sha256:')
    update_control(work_active=False)
    emit('READY_FOR_READONLY_INSPECTION', temp=str(preview_dir), comparisonPrefix=str(EVIDENCE/'PARADISECITY-ART-QUALIFY01-0125-attempt-1'), reusedMeasuredComparison=True, privateRoot=str(temp), lastWorkEnded=dt.datetime.now(dt.timezone.utc).isoformat(),
         packBytes=size(PACK), outputs=[dict(file=p.name, bytes=p.stat().st_size, sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted(PACK.iterdir())])
    owner_released = False
    input_closed = False
    while not owner_released and time.time() < hard-2:
        sample(hard)
        if select.select([] if input_closed else [sys.stdin], [], [], 0.2)[0]:
            value = sys.stdin.readline()
            if value.strip() == 'RELEASE':
                owner_released = True
                break
            if not value:
                input_closed = True
                emit('INSPECTION_INPUT_CLOSED', reason='EOF is not owner assessment or release; original hard deadline remains')
    assert owner_released, 'Owner inspection was not explicitly released before the original hard deadline'
    status = 'PASS'
except BaseException as error:
    status = 'FAIL'
    emit('ERROR', reason=repr(error))
finally:
    stop()
    if guardian is not None:
        update_control(work_active=False,shutdown=True)
        try:guardian.wait(timeout=4)
        except subprocess.TimeoutExpired:
            os.killpg(guardian.pid,signal.SIGTERM);guardian.wait(timeout=1);status='FAIL'
        if guardian.returncode!=0:status='FAIL'
        if guardian_state.exists():
            state=json.loads(guardian_state.read_text())
            for key,value in state.get('peak',{}).items():peak[key]=max(peak[key],value)
    try:
        whole,new=inventory.sample();new=NEW_CARRY+max(0,new-34*MIB);peak['whole']=max(peak['whole'],whole);peak['new']=max(peak['new'],new)
    except BaseException as error:
        status='FAIL';whole=peak['whole'];new=peak['new'];emit('ERROR',reason='Final inventory failed: '+repr(error))
    shutil.rmtree(temp)
    if time.time()>=hard or whole>WHOLE_CAP or new>NEW_CAP:status='FAIL'
    emit('FULL', status=status, tempAbsent=not temp.exists(), guardian=os.getpid(), workerPids=workers,
         allChildrenExited=(child is None or child.poll() is not None) and (guardian is None or guardian.poll() is not None),
         resourceGuardianPid=guardian.pid if guardian else None, resourceGuardianExited=guardian is None or guardian.poll() is not None, peak=peak,
         minimumFree=minimum_free, finalFree=shutil.disk_usage(ROOT).free,
         samples=samples, sampledPidCount=len(known), lastLiveCensus=last_identity)
sys.exit(0 if status == 'PASS' else 1)
