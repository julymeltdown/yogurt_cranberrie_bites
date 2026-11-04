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

ROOT = Path(__file__).resolve().parents[5]
PYTHON = '/Library/Frameworks/Python.framework/Versions/3.14/bin/python3'
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
PACK = ROOT/'frontend/public/world/map-roof-street-derived-v2'
TESTS = ROOT/'frontend/src/world/reference-scene/map-roof-street-derived-v2'
EVIDENCE = Path(__file__).resolve().parent
DOC = ROOT/'docs/mcr/art/map-roof-street-derived-v2.md'
BASE = ROOT/'frontend/public/world/map-density-derived-v1'
MIB = 1024**2
WHOLE_CAP = 2*1024**3
NEW_CAP = 64*MIB
START_FREE_FLOOR = 11*1024**3
parser = argparse.ArgumentParser()
parser.add_argument('--offer', required=True)
parser.add_argument('--head', required=True)
parser.add_argument('--work', required=True)
parser.add_argument('--hard', required=True)
parser.add_argument('--other-budget-bytes', required=True, type=int)
parser.add_argument('--assessment-of', help='Retained qualified offer whose eight outputs are inspected without rebuilding')
parser.add_argument('--metadata-of', help='Retained qualified offer whose seven art/layout outputs must remain byte-exact')
args = parser.parse_args()
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
assert not git('diff', '--name-only', '7ba6de456949136c33a068bdeee8bebb8e7822fd', 'HEAD', '--',
               'frontend/public/world/map-reference-composition-v1', 'frontend/public/world/map-density-derived-v1', 'frontend/public/world/reference-scene-v2',
               'frontend/src/entities/avatar', 'frontend/public/world/avatars').strip()


def size(path):
    if path.is_file():
        return path.stat().st_size
    return sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink())


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
    assert raw == git('show', '7acfac3aed91f1eb06fa48eee63ce3da1431fadc:'+str(p.relative_to(ROOT)))
    baseline_pins.append(dict(path=str(p.relative_to(ROOT)), bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
source_pins = []
for scope in (PACK, TESTS, EVIDENCE):
    for p in sorted(scope.rglob('*')):
        if p.is_file():
            raw = p.read_bytes()
            source_pins.append(dict(path=str(p.relative_to(ROOT)), bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
source_pins.append(dict(path=str(DOC.relative_to(ROOT)), bytes=DOC.stat().st_size, sha256=hashlib.sha256(DOC.read_bytes()).hexdigest()))
initial_own = own_size()
# Full checkout and tools are carried, plus all six immutable art packs, duplicate
# original inputs,32MiB nonshrinking owned Git reserve and2MiB receipt/failure reserve.
checkout_carry = size(ROOT)
archives = ('4618026884c1b9a846fcd2a46567ae26340bb75f', '8fcd1287496aedbde12ef0d2e426855f8f8ec4d6',
            '1f2f9ff6bac13467d76c430ff67a26e51b421727', '94c47a8b3219efc76a4be9646014cfeaad2f1957',
            '97d9d3f40e0d17ab536ac696bdb2140093225f6a', '77a4d405f65f82dc14edda98d2447d5f0ed302b7')
historical = sum(sum(int(line.split()[3]) for line in git('ls-tree', '-rl', ref, '--',
                  'frontend/public/world/map-reference-composition-v1').decode().splitlines()) for ref in archives)
carry = checkout_carry+historical+Path(NODE).stat().st_size+2*sum(p['bytes'] for p in input_pins)+34*MIB
fresh_carry = carry
prior_whole_peak = 1634192269
carry = max(fresh_carry, prior_whole_peak)
assert carry < WHOLE_CAP, ('Initial carry exceeds whole cap', carry)
assert args.other_budget_bytes >= 0
# Measured free already excludes retained checkout/tools/history. Reserve only
# worst-case additional owned bytes here; enforce the whole-job cap separately.
aggregate_residual = initial_free-args.other_budget_bytes-NEW_CAP
assert aggregate_residual >= 9*1024**3, 'Fresh functional inventory must retain aggregate9GiB margin'
assert not (args.assessment_of and args.metadata_of)
retained_offer = args.assessment_of or args.metadata_of
if retained_offer:
    assert Path(retained_offer).name == retained_offer
    qualified = [json.loads(line) for line in (EVIDENCE/(retained_offer+'.jsonl')).read_text().splitlines()]
    assert qualified[-1]['event'] == 'FULL' and qualified[-1]['status'] == 'PASS'
    prior_whole_peak = max(prior_whole_peak, qualified[-1]['peak']['whole'])
    carry = max(fresh_carry, prior_whole_peak)
    assert carry < WHOLE_CAP, ('Retained prior peak exceeds whole cap', carry)
    ready = next(row for row in qualified if row['event'] == 'READY_FOR_READONLY_INSPECTION')
    outputs = [pin for pin in ready['outputs'] if pin['file'] not in ('README.md', 'build.py')]
    assert {pin['file'] for pin in outputs} == {'layout.json', 'source-provenance.json',
        *(era+'-scene.'+ext for era in ('revenge', 'parade', 'danger') for ext in ('png', 'svg'))}
    for pin in outputs:
        raw = (PACK/pin['file']).read_bytes()
        assert len(raw) == pin['bytes'] and hashlib.sha256(raw).hexdigest() == pin['sha256']
receipt = EVIDENCE/(args.offer+'.jsonl')
assert not receipt.exists(), 'A retained named offer must never be reused'
metadata_before = json.loads((PACK/'source-provenance.json').read_text()) if args.metadata_of else None
temp = Path(tempfile.mkdtemp(prefix='mcr-roof-street-'))
child = None
workers = []
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
    global minimum_free, samples
    assert time.time() < deadline, 'Original deadline exceeded'
    free = shutil.disk_usage(ROOT).free
    minimum_free = min(minimum_free, free)
    assert free >= 8*1024**3, 'Live free floor'
    tmp = size(temp)
    growth = max(0, own_size()-initial_own)
    new, whole = 32*MIB+growth+tmp+2*MIB, carry+growth+tmp
    durable = size(PACK)
    assert tmp <= 4*MIB and durable <= 10*MIB and new <= NEW_CAP and whole <= WHOLE_CAP
    sampler = subprocess.Popen(['ps', '-axo', 'pid=,rss='], stdout=subprocess.PIPE, text=True)
    known.add(sampler.pid)
    out = sampler.communicate()[0]
    active = {os.getpid(), sampler.pid}
    if child is not None and child.poll() is None:
        active.add(child.pid)
    rss = sum(int(parts[1])*1024 for line in out.splitlines() if len(parts := line.split()) == 2 and int(parts[0]) in active)+16*MIB
    count = len(active)+1  # optional exec shell reserved, even when elided.
    assert rss <= 512*MIB and count <= 4
    for key, value in dict(whole=whole, new=new, tmp=tmp, rss=rss, processes=count).items():
        peak[key] = max(peak[key], value)
    samples += 1


def stop():
    if child is not None and child.poll() is None:
        os.killpg(child.pid, signal.SIGTERM)
        try:
            child.wait(timeout=1)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=1)


try:
    emit('ACCEPT', offer=args.offer, head=args.head, guardian=os.getpid(), work=args.work, hard=args.hard,
         executable=sys.executable, python=sys.version, pillow=pillow_version, node=NODE,
         initialFree=initial_free, initialOwnBytes=initial_own, checkoutCarry=checkout_carry,
         startFreeFloorBytes=START_FREE_FLOOR,
         assessmentOf=args.assessment_of,
         metadataOf=args.metadata_of,
         otherBudgetBytes=args.other_budget_bytes, aggregateOwnNewReserveBytes=NEW_CAP,
         aggregateResidualBytes=aggregate_residual,
         historicalCarry=historical, freshCarry=fresh_carry, priorWholePeak=prior_whole_peak,
         carry=carry, baselinePins=baseline_pins, inputPins=input_pins,
         sourcePins=source_pins, temp=str(temp), wholeCap=WHOLE_CAP, newCap=NEW_CAP,
         durableCap=10*MIB, tempCap=4*MIB, rssCap=512*MIB, processCap=4)
    commands = [
        ('build', [PYTHON, '-B', str(PACK/'build.py')]),
        ('verify', [PYTHON, '-B', str(PACK/'build.py'), '--verify']),
        ('pure-qualifier-tests', [NODE, str(TESTS/'qualifier.test.ts')]),
        ('source-previews', [PYTHON, '-B', str(EVIDENCE/'previews.py'), str(temp)]),
    ]
    if args.assessment_of:
        commands = commands[-1:]
    elif args.metadata_of:
        commands = commands[:3]
    attempt = 0
    while True:
        attempt += 1
        emit('ATTEMPT', attempt=attempt, head=git('rev-parse', 'HEAD').decode().strip(),
             sourcePins=[dict(path=str(p.relative_to(ROOT)), sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                         for p in (PACK/'build.py', TESTS/'qualifier.ts', TESTS/'qualifier.test.ts', EVIDENCE/'previews.py', Path(__file__).resolve())])
        failed = None
        for name, argv in commands:
            sample(work)
            started = time.time()
            child = subprocess.Popen(argv, cwd=ROOT, env=dict(os.environ, PYTHONDONTWRITEBYTECODE='1', TMPDIR=str(temp)),
                                     start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
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
    for pin in baseline_pins+input_pins:
        raw = (ROOT/pin['path']).read_bytes()
        assert len(raw) == pin['bytes'] and hashlib.sha256(raw).hexdigest() == pin['sha256'].removeprefix('sha256:')
    if args.metadata_of:
        for pin in outputs:
            if pin['file'] != 'source-provenance.json':
                raw = (PACK/pin['file']).read_bytes()
                assert len(raw) == pin['bytes'] and hashlib.sha256(raw).hexdigest() == pin['sha256'], pin['file']
        metadata_after = json.loads((PACK/'source-provenance.json').read_text())
        expected_calls = ['surface', 'overhead_roof for complete incumbent structural underlays; protected-adjacent roofs remain unchanged']
        assert metadata_after['helperReuse']['called'] == expected_calls
        metadata_before['recipe'] = metadata_after['recipe']
        metadata_before['helperReuse']['called'] = expected_calls
        assert metadata_after == metadata_before, 'Unexpected provenance change beyond recipe pin and helper-call description'
        emit('METADATA_ONLY_ART_BYTES_EXACT', retainedOffer=args.metadata_of, exactFiles=7,
             inheritedAssessment='Three full rasters plus nine scaled previews observed under retained offer')
    emit('READY_FOR_READONLY_INSPECTION', temp=str(temp), lastWorkEnded=dt.datetime.now(dt.timezone.utc).isoformat(),
         packBytes=size(PACK), outputs=[dict(file=p.name, bytes=p.stat().st_size, sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                                      for p in sorted(PACK.iterdir())])
    owner_released = False
    input_closed = False
    while time.time() < hard-2:
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
    shutil.rmtree(temp)
    emit('FULL', status=status, tempAbsent=not temp.exists(), guardian=os.getpid(), workerPids=workers,
         allChildrenExited=child is None or child.poll() is not None, peak=peak,
         minimumFree=minimum_free, finalFree=shutil.disk_usage(ROOT).free,
         samples=samples, sampledPidCount=len(known))
sys.exit(0 if status == 'PASS' else 1)
