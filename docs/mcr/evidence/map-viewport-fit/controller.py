# Derived finite guards; exact provenance: guard-provenance.json.
"""Independent outer finite-job guard; observes the separately pinned guardian."""
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
PREFIX = ROOT / 'docs/mcr/evidence/map-viewport-fit'
GUARDIAN = PREFIX / 'guardian.py'
EVIDENCE = ROOT / 'docs/mcr/evidence/map-viewport-fit'
OUT = Path(os.environ['MCR_MAP_FIT_OUTPUT'])
TMP = Path(os.environ['MCR_MAP_FIT_TMP'])
LINK = ROOT / 'frontend/node_modules'
DEPENDENCIES = '/Users/lhs/Desktop/mcr/frontend/node_modules'
RETAINED = LINK.is_dir() and not LINK.is_symlink()
MODE = os.environ['MCR_MAP_FIT_MODE']
GIB, MIB = 1024**3, 1024**2
CAP = (128 if MODE == 'source' else 200) * MIB
WORK = datetime.datetime.fromisoformat(os.environ['MCR_MAP_FIT_WORK_DEADLINE'].replace('Z', '+00:00')).timestamp()
HARD = datetime.datetime.fromisoformat(os.environ['MCR_MAP_FIT_DEADLINE'].replace('Z', '+00:00')).timestamp()
OTHER = int(os.environ['MCR_MAP_FIT_OTHER_NEW'])
MUTABLE = [ROOT / 'docs/mcr/evidence/map-viewport-fit', ROOT / 'docs/mcr/frontend/map-viewport-fit.md',
           ROOT / 'frontend/src/features/places/ui/map-viewport-fit',
           ROOT / 'frontend/src/features/places/__tests__/map-viewport-fit',
           ROOT / 'frontend/src/features/places/ui/IllustratedMap.tsx',
           ROOT / 'frontend/src/features/places/ui/places.css']

INVENTORY = json.loads((ROOT / 'docs/mcr/evidence/map-viewport-fit/inventory.json').read_text())
WHOLE_PATHS = [Path(row['path']) for row in INVENTORY['rows']]
WHOLE_CAP = int(os.environ['MCR_MAP_FIT_WHOLE_CAP'])
assert WHOLE_CAP == 8 * GIB


def size(paths):
    total, visited = 0, set()
    for path in paths:
        if path.is_symlink():
            continue
        for entry in path.rglob('*') if path.is_dir() else [path]:
            try:
                if entry.is_file() and not entry.is_symlink():
                    st = entry.stat()
                    key = (st.st_dev, st.st_ino)
                    if key not in visited:
                        visited.add(key)
                        total += max(st.st_size, getattr(st, 'st_blocks', 0) * 512)
            except FileNotFoundError:
                pass
    return total


def free():
    v = os.statvfs(ROOT)
    return v.f_bavail * v.f_frsize


def retained_matches():
    expected = json.loads((EVIDENCE / 'retained-dependencies.json').read_text())
    actual = []
    for path in sorted(LINK.rglob('*')):
        relative = path.relative_to(LINK).as_posix()
        if path.is_symlink():
            actual.append(dict(path=relative, link=os.readlink(path)))
        elif path.is_file():
            actual.append(dict(path=relative, bytes=path.stat().st_size,
                               sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    return actual == expected


def table():
    rows = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,comm='], text=True).splitlines():
        fields = line.strip().split(None, 4)
        if len(fields) == 5:
            rows[int(fields[0])] = tuple(int(v) for v in fields[1:4]) + (fields[4],)
    return rows


assert MODE in ('source', 'native') and Path.cwd() == ROOT
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == os.environ['MCR_MAP_FIT_CONTROLLER_SHA']
assert hashlib.sha256(GUARDIAN.read_bytes()).hexdigest() == os.environ['MCR_MAP_FIT_GUARDIAN_SHA']
assert hashlib.sha256((EVIDENCE / 'readset.json').read_bytes()).hexdigest() == os.environ['MCR_MAP_FIT_READSET_SHA']
assert OUT.is_relative_to(EVIDENCE) and not OUT.exists()
assert TMP.as_posix().startswith('/private/tmp/mcr-map-viewport-fit-') and not TMP.exists()
assert RETAINED and HARD - WORK >= 30 and time.time() < WORK
assert (LINK / '.pnpm/lock.yaml').read_bytes() == (ROOT / 'frontend/pnpm-lock.yaml').read_bytes()
assert retained_matches(), 'retained dependency snapshot'
assert int(os.environ['MCR_MAP_FIT_START_FLOOR']) >= 11 * GIB and OTHER >= 0
assert free() >= int(os.environ['MCR_MAP_FIT_START_FLOOR']) and free() - OTHER - CAP >= 9 * GIB
prior = max([1_675_186_956] + [json.loads(p.read_text()).get('peak', {}).get('wholeBytes', 0)
                            for p in EVIDENCE.glob('*/release.json')])
for path in EVIDENCE.glob('*-controller.jsonl'):
    for line in path.read_text().splitlines():
        prior = max(prior, json.loads(line).get('peak', {}).get('wholeBytes', 0))
carry = max(prior, INVENTORY['allocatedWholeBytes'], INVENTORY['logicalWholeBytes'], size(WHOLE_PATHS)) + 34 * MIB
assert carry + CAP <= WHOLE_CAP
baseline = size(MUTABLE)
log = EVIDENCE / (OUT.name + '-controller.jsonl')
assert not log.exists()
seen, signatures, groups = set(), {}, set()
peak = dict(processes=0, rssBytes=0, newBytes=0, tmpBytes=0, wholeBytes=carry)
minimum = free()
child, failure, code = None, None, 1


def emit(value):
    with log.open('a') as stream:
        stream.write(json.dumps(value) + '\n')
    print(json.dumps(value), flush=True)


def check():
    global minimum
    rows = table()
    previous = None
    while previous != seen:
        previous = set(seen)
        seen.update(pid for pid, row in rows.items() if row[0] in seen)
    live = seen.intersection(rows)
    signatures.update({pid: rows[pid][3] for pid in live})
    groups.update(rows[pid][1] for pid in live)
    mutable, temporary = size(MUTABLE), size([TMP])
    counters = dict(processes=len(live) + 3,  # Outer, exec wrapper and this ps probe.
                    rssBytes=sum(rows[pid][2] * 1024 for pid in live | {os.getpid()} if pid in rows) + 24 * MIB,  # Wrapper16 + monitor8 reserve.
                    newBytes=34 * MIB + mutable + temporary, tmpBytes=temporary,
                    wholeBytes=max(carry, size(WHOLE_PATHS) + 34 * MIB) + temporary)
    for key, value in counters.items():
        peak[key] = max(peak[key], value)
    minimum = min(minimum, free())
    assert free() >= 8 * GIB and time.time() < WORK, 'outer live floor/work stop'
    assert counters['processes'] <= (6 if MODE == 'source' else 24), 'outer all-process cap'
    assert counters['rssBytes'] <= (3 if MODE == 'source' else 3.5) * GIB, 'outer RSS cap'
    assert counters['newBytes'] <= CAP and counters['tmpBytes'] <= CAP and counters['wholeBytes'] <= WHOLE_CAP


emit(dict(type='ACCEPT', lease=os.environ['MCR_MAP_FIT_LEASE'], at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
          controllerPid=os.getpid(), workDeadline=WORK, hardDeadline=HARD, freeBytes=free(),
          otherNewBytes=OTHER, ownNewCap=CAP, carryBytes=carry, priorHighWaterBytes=prior))
try:
    check()
    child = subprocess.Popen([sys.executable, '-B', str(GUARDIAN)], cwd=ROOT, start_new_session=True)
    seen.add(child.pid)
    while child.poll() is None:
        check()
        time.sleep(.25)
    code = child.returncode
    check()
except BaseException as error:
    failure, code = repr(error), 1
finally:
    for group in groups:
        try:
            os.killpg(group, signal.SIGTERM)
        except ProcessLookupError:
            pass
    time.sleep(.3)
    for pid, row in table().items():
        if pid in signatures and row[3] == signatures[pid]:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    if child:
        child.wait(timeout=max(.1, min(3, HARD - time.time())))
    if TMP.exists():
        shutil.rmtree(TMP)
    live = [pid for pid, row in table().items() if pid in signatures and row[3] == signatures[pid]]
    preserved = retained_matches()
    if not preserved:
        failure, code = 'retained dependency tree changed', 1
    emit(dict(type='FULL', lease=os.environ['MCR_MAP_FIT_LEASE'], at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
              exitCode=code, failure=failure, peak=peak, minimumFreeBytes=minimum, finalFreeBytes=free(),
              seenPids=sorted(seen), livePids=live, retainedDependenciesPreserved=preserved,
              privateTmpAbsent=not TMP.exists(), resources=len(live)))
raise SystemExit(code)
