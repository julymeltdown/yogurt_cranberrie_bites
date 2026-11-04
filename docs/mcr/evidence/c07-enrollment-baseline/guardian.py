"""Finite independent admission guardian; requires an explicit Social offer."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
OUT = Path(os.environ['MCR_C07_OUTPUT'])
TMP = Path(os.environ['TMPDIR'])
LEASE = os.environ['MCR_C07_LEASE']
DEADLINE = float(os.environ['MCR_C07_DEADLINE'])
ACTIVE_END = DEADLINE - 60
OTHER_NEW = int(os.environ['MCR_C07_OTHER_NEW'])
START_FLOOR = int(os.environ.get('MCR_C07_START_FLOOR_GIB', '12'))
assert START_FLOOR in (11, 12), 'fresh offer must state an authorized floor'
GIB = 1024 ** 3
MIB = 1024 ** 2
assert Path.cwd() == ROOT and OUT.parent == HERE and not OUT.exists()
assert TMP.parent == Path('/private/tmp') and TMP.name.startswith('mcr-c07-enrollment-') and not TMP.exists()
assert LEASE and time.time() < ACTIVE_END
assert subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT) == b''
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == os.environ['MCR_C07_SOURCE']
READSET = json.loads((HERE / 'readset.json').read_text())
assert all(hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == value for path, value in READSET.items())
GIT = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
def allocated(path):
    return int(subprocess.check_output(['du', '-sk', str(path)], text=True).split()[0]) * 1024
BASE = allocated(ROOT) + allocated(GIT)
RETAINED = allocated(Path('/Users/lhs/Desktop/mcr/frontend/node_modules')) + allocated(Path('/Users/lhs/Desktop/mcr-tools/contracts-validation-venv')) + Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node').stat().st_size
free = shutil.disk_usage(ROOT).free
assert free >= START_FLOOR * GIB and free - OTHER_NEW - 128 * MIB >= 9 * GIB
assert BASE + RETAINED + 128 * MIB <= 2 * GIB
OUT.mkdir(); TMP.mkdir()
def record(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')
record('acceptance.json', dict(lease=LEASE, source=os.environ['MCR_C07_SOURCE'], canonical=os.environ['MCR_C07_CANONICAL'],
    readsetSha256=hashlib.sha256((HERE/'readset.json').read_bytes()).hexdigest(),
    guardianSha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    acceptedAt=time.time(), activeEnd=ACTIVE_END, deadline=DEADLINE, freeBytes=free, otherWorstNewBytes=OTHER_NEW,
    startFloorGiB=START_FLOOR, liveFloorGiB=8, reserveMarginGiB=9,
    ownWorstNewBytes=128*MIB, remainingMarginBytes=free-OTHER_NEW-128*MIB, wholeFloorBytes=BASE+RETAINED,
    caps=dict(processes=24, rssBytes=4*GIB, wholeBytes=2*GIB, newBytes=128*MIB, tmpBytes=96*MIB, durableBytes=24*MIB),
    privateTmp=str(TMP), portsCreated=[]))
def table():
    result = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,comm='], text=True).splitlines():
        row = line.split(None, 4)
        if len(row) == 5:
            result[int(row[0])] = tuple(map(int, row[1:4])) + (row[4],)
    return result
seen = {os.getpid()}
peak = dict(processes=0, rssBytes=0, wholeBytes=BASE+RETAINED, newBytes=0, tmpBytes=0, durableBytes=0)
child = None
status = 'FAIL'
error = None
last_disk = 0
try:
    # Guardian is running before the first allocating child. All subprocesses
    # inherit one owned session; samples also track descendants across parents.
    log = (OUT / 'owner.log').open('w')
    child = subprocess.Popen([sys.executable, '-B', str(HERE / 'qualify.py')], cwd=ROOT,
                             stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    seen.add(child.pid)
    while True:
        processes = table()
        previous = set()
        while previous != seen:
            previous = set(seen)
            seen.update(pid for pid, row in processes.items() if row[0] in seen or row[1] == child.pid)
        live = set(processes) & seen
        rss = sum(processes[pid][2] * 1024 for pid in live)
        peak['processes'] = max(peak['processes'], len(live))
        peak['rssBytes'] = max(peak['rssBytes'], rss)
        now = time.time()
        assert now < ACTIVE_END, 'finite active deadline'
        assert len(live) <= 24 and rss <= 4*GIB, 'process/RSS cap'
        assert shutil.disk_usage(ROOT).free >= 8*GIB, 'live free-space floor'
        if now-last_disk >= 1:
            storage = allocated(ROOT) + allocated(GIT)
            temporary = allocated(TMP)
            durable = allocated(OUT)
            growth = max(0, storage-BASE) + temporary
            whole = max(BASE, storage) + RETAINED + temporary
            for key, value in dict(wholeBytes=whole, newBytes=growth, tmpBytes=temporary, durableBytes=durable).items():
                peak[key] = max(peak[key], value)
            assert whole <= 2*GIB and growth <= 128*MIB and temporary <= 96*MIB and durable <= 24*MIB, 'disk allocation cap'
            with (OUT / 'guardian.jsonl').open('a') as handle:
                handle.write(json.dumps(dict(at=now, live=sorted(live), rssBytes=rss, wholeBytes=whole, newBytes=growth))+'\n')
            last_disk = now
        if child.poll() is not None:
            assert child.returncode == 0, 'qualification child failed; original logs retained'
            status = 'PASS'
            break
        time.sleep(.1)
except BaseException as failure:
    error = repr(failure)
finally:
    if child is not None:
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        until = min(DEADLINE-5, time.time()+4)
        while child.poll() is None and time.time() < until:
            time.sleep(.1)
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait()
    # Remove only private scratch and C07's own abandoned compile scratch.
    shutil.rmtree(TMP)
    for path in (OUT / 'checks').glob('compile-*'):
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
    remaining = sorted((set(table()) & seen) - {os.getpid()})
    assert not remaining, 'owned descendants must drain before FULL'
    # Capture terminal disk growth even when a child fails between 1s samples.
    storage = allocated(ROOT) + allocated(GIT)
    terminal = dict(wholeBytes=max(BASE, storage)+RETAINED, newBytes=max(0, storage-BASE),
                    tmpBytes=0, durableBytes=allocated(OUT))
    for key, value in terminal.items():
        peak[key] = max(peak[key], value)
    if peak['wholeBytes'] > 2*GIB or peak['newBytes'] > 128*MIB or peak['durableBytes'] > 24*MIB:
        status = 'FAIL'; error = 'terminal disk allocation cap'
    mutable = {'contracts/generated/mcr-domain.openapi.json', 'backend/docs/api/mcr-domain.openapi.json',
               'scripts/mcr/contracts/qa01-coverage/pins.json',
               *('contracts/fixtures/c07/' + name + '.json' for name in ('responses', 'compatibility', 'index'))}
    readonly = {path: value for path, value in READSET.items() if path not in mutable and not path.startswith(
        ('contracts/generated/api-client/', 'frontend/src/shared/api/generated/'))}
    readonly_unchanged = all(hashlib.sha256((ROOT/path).read_bytes()).hexdigest() == value for path, value in readonly.items())
    if not readonly_unchanged:
        status = 'FAIL'; error = 'frozen read-only input changed'
    record('full-release.json', dict(lease=LEASE, status=status, error=error, actualFullAt=time.time(),
        source=os.environ['MCR_C07_SOURCE'], canonical=os.environ['MCR_C07_CANONICAL'], peak=peak,
        allObservedPids=sorted(seen), liveOwnedPids=remaining, privateTmpAbsent=not TMP.exists(),
        compileScratchAbsent=not list((OUT/'checks').glob('compile-*')), readonlyInputsUnchanged=readonly_unchanged,
        portsCreated=[], freeBytes=shutil.disk_usage(ROOT).free))
print(json.dumps(dict(status=status, error=error, output=str(OUT))))
raise SystemExit(0 if status == 'PASS' else 1)
