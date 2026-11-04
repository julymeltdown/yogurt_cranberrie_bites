"""Start only on an actual fresh named offer; original deadlines never move."""
import argparse
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
from inventory import Inventory,process_snapshot

ROOT = Path(__file__).resolve().parents[5]
PYTHON = '/Library/Frameworks/Python.framework/Versions/3.14/bin/python3'
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
PACK = ROOT/'frontend/public/world/map-facade-derived-v3'
TESTS = ROOT/'frontend/src/world/reference-scene/map-facade-derived-v3'
EVIDENCE = Path(__file__).resolve().parent
DOC = ROOT/'docs/mcr/art/map-facade-derived-v3.md'
BASE = ROOT/'frontend/public/world/map-roof-street-derived-v2'
MIB = 1024**2
WHOLE_CAP = 5*1024**3
NEW_CAP = 64*MIB
START_FREE_FLOOR = 12*1024**3
parser = argparse.ArgumentParser()
parser.add_argument('--offer', required=True)
parser.add_argument('--head', required=True)
parser.add_argument('--work', required=True)
parser.add_argument('--hard', required=True)
parser.add_argument('--other-budget-bytes', required=True, type=int)
parser.add_argument('--foundation-only', action='store_true')
parser.add_argument('--with-foundation', action='store_true')
args = parser.parse_args()
assert not (args.foundation_only and args.with_foundation)
os.chdir(ROOT)
work = dt.datetime.fromisoformat(args.work).timestamp()
hard = dt.datetime.fromisoformat(args.hard).timestamp()
initial_free = shutil.disk_usage(ROOT).free
assert time.time() < work < hard and initial_free >= START_FREE_FLOOR
assert sys.executable == PYTHON and sys.version_info[:3] == (3, 14, 3)
from PIL import __version__ as pillow_version
assert pillow_version == '12.2.0'

def git(*argv):
    return subprocess.check_output(['git', *argv], cwd=ROOT)


assert git('rev-parse', 'HEAD').decode().strip() == args.head
assert not git('status', '--porcelain').strip()
assert not git('diff', '--name-only', 'da33467cd084ecba071244a1939b6217f4b0de02', 'HEAD', '--',
               'frontend/public/world/map-reference-composition-v1', 'frontend/public/world/map-density-derived-v1', 'frontend/public/world/map-roof-street-derived-v2', 'frontend/public/world/reference-scene-v2',
               'frontend/src/entities/avatar', 'frontend/public/world/avatars').strip()


def size(path):
    total=0
    for p in ([path] if path.is_file() else path.rglob('*')):
        try:
            if p.is_file() and not p.is_symlink():total+=p.stat().st_size
        except FileNotFoundError:pass  # Independent guardian atomically replaces its private state.
    return total


def own_size():
    return sum(size(p) for p in (PACK, TESTS, EVIDENCE, DOC))


original = json.loads((BASE/'source-provenance.json').read_text())
input_pins = original['sourceInputs']
for pin in input_pins:
    raw = (ROOT/pin['path']).read_bytes()
    assert len(raw) == pin['bytes'] and 'sha256:'+hashlib.sha256(raw).hexdigest() == pin['sha256']
baseline_pins = []
for p in sorted(BASE.iterdir()):
    raw = p.read_bytes()
    assert raw == git('show', 'da33467cd084ecba071244a1939b6217f4b0de02:'+str(p.relative_to(ROOT)))
    baseline_pins.append(dict(path=str(p.relative_to(ROOT)), bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
source_pins = []
for scope in (PACK, TESTS, EVIDENCE):
    for p in sorted(scope.rglob('*')):
        if p.is_file():
            raw = p.read_bytes()
            source_pins.append(dict(path=str(p.relative_to(ROOT)), bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
source_pins.append(dict(path=str(DOC.relative_to(ROOT)), bytes=DOC.stat().st_size, sha256=hashlib.sha256(DOC.read_bytes()).hexdigest()))

# The ground owner's qualified closure is read-only; source preparation alone
# does not confer qualification or permit fabricated overlay input.
ground_pins={}
def collect_pins(value):
    if isinstance(value,dict):
        if {'path','bytes','sha256'} <= set(value):
            p=ROOT/value['path'];assert p.resolve().is_relative_to(ROOT) and not p.is_symlink()
            raw=p.read_bytes();assert len(raw)==value['bytes'] and hashlib.sha256(raw).hexdigest()==value['sha256'].removeprefix('sha256:')
            ground_pins[value['path']]=value
        else:
            for child_value in value.values():collect_pins(child_value)
    elif isinstance(value,list):
        for child_value in value:collect_pins(child_value)
if not args.foundation_only:
    ground_binding=json.loads((PACK/'ground-input.json').read_text())
    assert ground_binding['path']=='frontend/public/world/map-lawns-derived-v3/interface.json'
    ground_interface=json.loads((ROOT/ground_binding['path']).read_text())
    collect_pins(ground_binding);collect_pins(ground_interface)
    collect_pins(json.loads((ROOT/ground_interface['provenancePin']['path']).read_text()))
    helper=ground_interface['helperPin'];helper_raw=(ROOT/helper['path']).read_bytes()
    assert helper['path']=='frontend/public/world/map-reference-composition-v1/build.py'
    assert hashlib.sha256(helper_raw).hexdigest()==helper['sha256'].removeprefix('sha256:')=='6bf696b7c3edd5adf35e0b5995ca9cd33f915e87898de0e9266f2923cffc0433'
    ground_pins[helper['path']]=dict(path=helper['path'],bytes=len(helper_raw),sha256=helper['sha256'])
    q=ground_interface['qualification'];assert q['status']=='PASS'
    qualified_rows=[json.loads(line) for line in (ROOT/q['receipt']['path']).read_text().splitlines() if line.strip()]
    assert qualified_rows[-1]['event']=='FULL' and qualified_rows[-1]['status']=='PASS'
    assert next(row for row in qualified_rows if row['event']=='ACCEPT')['head']==q['sourceCommit']
source_pins.extend(ground_pins.values())

initial_own = own_size()
# Full retained checkout, shared/private Git and full Python/Node installations.
common_git=Path(git('rev-parse','--git-common-dir').decode().strip()).resolve()
inventory_roots=[str(ROOT),str(common_git),str(Path(PYTHON).resolve().parents[1]),str(Path(NODE).parents[1])]
prior_whole_peak=1634206397
inventory=Inventory(inventory_roots,prior_whole_peak)
fresh_carry=inventory.logical+34*MIB
carry=inventory.carry
assert carry<WHOLE_CAP,('Complete retained carry exceeds whole cap',carry)
assert args.other_budget_bytes >= 0
# Measured free already excludes retained checkout/tools/history. Reserve only
# worst-case additional owned bytes here; enforce the whole-job cap separately.
aggregate_residual = initial_free-args.other_budget_bytes-NEW_CAP
assert aggregate_residual >= 9*1024**3, 'Fresh functional inventory must retain aggregate9GiB margin'
receipt = EVIDENCE/(args.offer+'.jsonl')
guardian_log=EVIDENCE/(args.offer+'-independent-guardian.jsonl')
assert not receipt.exists() and not guardian_log.exists(), 'A retained named offer must never be reused'
temp = Path(tempfile.mkdtemp(prefix='mcr-facade-'))
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
peak = dict(whole=carry, new=32*MIB, tmp=0, rss=0, processes=0)
minimum_free = initial_free
known = {os.getpid()}
samples = 0
status = 'RUNNING'


def emit(event, **fields):
    row = dict(event=event, at=dt.datetime.now(dt.timezone.utc).isoformat(), **fields)
    line = json.dumps(row)
    with receipt.open('a') as f:
        f.write(line+'\n')
    print(line, flush=True)


def sample(deadline):
    global minimum_free,samples
    assert time.time()<deadline,'Original deadline exceeded'
    free=shutil.disk_usage(ROOT).free;minimum_free=min(minimum_free,free)
    assert free>=8*1024**3,'Live free floor'
    whole,new=inventory.sample();tmp=size(temp);durable=own_size()
    assert whole<=WHOLE_CAP and new<=NEW_CAP and tmp<=4*MIB and durable<=10*MIB,'Supervisor storage cap'
    processes=process_snapshot(os.getpid(),temp/'sampler.lock')
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
         qualificationScope='FACADE_FOUNDATION_ONLY' if args.foundation_only else 'COMBINED_FACADE_AND_QUALIFIED_GROUND',
         executable=sys.executable, python=sys.version, pillow=pillow_version, node=NODE,
         initialFree=initial_free, initialOwnBytes=initial_own, inventoryLogical=inventory.logical, inventoryRoots=inventory_roots,
         startFreeFloorBytes=START_FREE_FLOOR,
         otherBudgetBytes=args.other_budget_bytes, aggregateOwnNewReserveBytes=NEW_CAP,
         aggregateResidualBytes=aggregate_residual,
         freshCarry=fresh_carry, priorWholePeak=prior_whole_peak,
         carry=carry, baselinePins=baseline_pins, inputPins=input_pins,
         sourcePins=source_pins, temp=str(temp), wholeCap=WHOLE_CAP, newCap=NEW_CAP,
         durableCap=10*MIB, tempCap=4*MIB, rssCap=512*MIB, processCap=4)
    guardian_config.write_text(json.dumps(dict(root=str(ROOT),temp=str(temp),control=str(control),state=str(guardian_state),
        log=str(guardian_log),scope=[str(p) for p in (PACK,TESTS,EVIDENCE,DOC)],inventoryRoots=inventory.roots,
        priorCarry=max(carry,prior_whole_peak),other=args.other_budget_bytes,wholeCap=WHOLE_CAP,
        supervisor=os.getpid(),work=work,hard=hard)))
    guardian=subprocess.Popen([PYTHON,'-B',str(EVIDENCE/'resource_guardian.py'),str(guardian_config)],
                              cwd=ROOT,env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1'),start_new_session=True)
    known.add(guardian.pid)
    guardian_wait_until=min(work,time.time()+10)
    while not guardian_state.exists() and guardian.poll() is None and time.time()<guardian_wait_until:time.sleep(.05)
    assert guardian_state.exists(),'Independent pre-child guardian did not become ready'
    sample(work)
    emit('INDEPENDENT_GUARDIAN_READY',pid=guardian.pid,config=str(guardian_config),state=json.loads(guardian_state.read_text()))
    commands = [
        ('build', [PYTHON, '-B', str(PACK/'build.py')]),
        ('verify', [PYTHON, '-B', str(PACK/'build.py'), '--verify']),
        ('pure-qualifier-tests', [NODE, '--max-old-space-size=256', str(TESTS/'qualifier.test.ts')]),
        ('source-previews', [PYTHON, '-B', str(EVIDENCE/'previews.py'), str(preview_dir)]),
    ]
    if args.foundation_only or args.with_foundation:
        foundation_commands=[
            ('facade-foundation',[PYTHON,'-B',str(EVIDENCE/'foundation.py'),str(preview_dir)]),
            ('independent-foundation-tests',[NODE,'--max-old-space-size=256',str(TESTS/'foundation.test.ts'),str(preview_dir)]),
        ]
        commands=foundation_commands+([] if args.foundation_only else commands)
    attempt = 0
    while True:
        attempt += 1
        emit('ATTEMPT', attempt=attempt, head=git('rev-parse', 'HEAD').decode().strip(),
             sourcePins=[dict(path=str(p.relative_to(ROOT)), sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                         for p in (PACK/'build.py', TESTS/'qualifier.ts', TESTS/'qualifier.test.ts', TESTS/'png.ts',
                                   TESTS/'foundation.test.ts',EVIDENCE/'foundation.py',EVIDENCE/'previews.py',
                                   EVIDENCE/'resource_guardian.py', EVIDENCE/'inventory.py', Path(__file__).resolve(),
                                   *([] if args.foundation_only else [PACK/'ground-input.json']))])
        failed = None
        update_control()
        for name, argv in commands:
            sample(work)
            started = time.time()
            if name=='source-previews':argv=[*argv,args.offer,str(attempt)]
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
            if name=='independent-foundation-tests' and args.with_foundation:
                foundation=json.loads((preview_dir/'foundation.json').read_text())
                emit('FOUNDATION_CHECKS_COMPLETE',proof=foundation,boundary='Facade mechanics only')
                for p in preview_dir.iterdir():p.unlink()
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
        raw = (ROOT/pin['path']).read_bytes()
        assert len(raw) == pin['bytes'] and hashlib.sha256(raw).hexdigest() == pin['sha256'].removeprefix('sha256:')
    update_control(work_active=False)
    if args.foundation_only:
        emit('FOUNDATION_CHECKS_COMPLETE',proof=json.loads((preview_dir/'foundation.json').read_text()),
             boundary='Facade mechanics only; no lawn/final pack/image assessment/App acceptance')
    else:
        emit('READY_FOR_READONLY_INSPECTION', temp=str(preview_dir), privateRoot=str(temp), lastWorkEnded=dt.datetime.now(dt.timezone.utc).isoformat(),
         packBytes=size(PACK), outputs=[dict(file=p.name, bytes=p.stat().st_size, sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                                      for p in sorted(PACK.iterdir())])
    owner_released = args.foundation_only
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
        whole,new=inventory.sample();peak['whole']=max(peak['whole'],whole);peak['new']=max(peak['new'],new)
    except BaseException as error:
        status='FAIL';whole=peak['whole'];new=peak['new'];emit('ERROR',reason='Final inventory failed: '+repr(error))
    shutil.rmtree(temp)
    if time.time()>=hard or whole>WHOLE_CAP or new>NEW_CAP:status='FAIL'
    emit('FULL', status=status, tempAbsent=not temp.exists(), guardian=os.getpid(), workerPids=workers,
         allChildrenExited=(child is None or child.poll() is not None) and (guardian is None or guardian.poll() is not None),
         resourceGuardianPid=guardian.pid if guardian else None, resourceGuardianExited=guardian is None or guardian.poll() is not None, peak=peak,
         minimumFree=minimum_free, finalFree=shutil.disk_usage(ROOT).free,
         samples=samples, sampledPidCount=len(known))
sys.exit(0 if status == 'PASS' else 1)

