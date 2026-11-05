"""Independent outer finite-job guard; observes the separately pinned guardian."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import time
sys.dont_write_bytecode = True
from membership import OWN as RESOURCE_RECEIPT, identity, publish as publish_resource, publish_census, sample as sample_membership

ROOT = Path(__file__).resolve().parents[6]
PREFIX = ROOT / 'frontend/src/features/paradise-city-map/__tests__/spatial'
GUARDIAN = PREFIX / 'guardian.py'
EVIDENCE = ROOT / 'docs/mcr/evidence/paradise-city-spatial-model'
OUT = Path(os.environ['MCR_PI_OUTPUT'])
TMP = Path(os.environ['MCR_PI_TMP'])
LINK = ROOT / 'frontend/node_modules'
DEPENDENCIES = '/Users/lhs/Desktop/mcr/frontend/node_modules'
MODE = os.environ['MCR_PI_MODE']
GIB, MIB = 1024**3, 1024**2
CAP = (128 if MODE == 'source' else 200) * MIB
WORK = datetime.datetime.fromisoformat(os.environ['MCR_PI_WORK_DEADLINE'].replace('Z', '+00:00')).timestamp()
HARD = datetime.datetime.fromisoformat(os.environ['MCR_PI_DEADLINE'].replace('Z', '+00:00')).timestamp()
OTHER = int(os.environ['MCR_PI_OTHER_NEW'])
NEW_CARRY = int(os.environ.get('MCR_PI_NEW_CARRY', '0'))
assert 0 <= NEW_CARRY <= 200 * 1024**2
PRIVATE_GIT = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
COMMON_GIT = Path(subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT, text=True).strip())
CARRY_PATHS = [ROOT, PRIVATE_GIT, COMMON_GIT, Path(DEPENDENCIES), Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64'), Path('/Applications/Google Chrome.app'), Path('/Library/Frameworks/Python.framework/Versions/3.14'), Path('/Users/lhs/.codex/visualizations/2026/11/03/01a1a26e-1de2-7b41-a294-c9a79cfded9b')]
CARRY_PATHS.append(RESOURCE_RECEIPT)
MUTABLE = [ROOT, PRIVATE_GIT, RESOURCE_RECEIPT]
WHOLE_CAP = int(os.environ['MCR_PI_WHOLE_CAP'])
PORT = int(os.environ.get('MCR_PI_PORT', '0'))
def port_free():
    if MODE != 'native':
        return True
    assert 1024 < PORT < 65536
    with socket.socket() as probe:
        try:
            probe.bind(('127.0.0.1', PORT))
            return True
        except OSError:
            return False




def size(paths):
    # Charge complete checkout, private/common Git, retained dependencies and Node.
    # Deduplicate hardlinks across every root; do not follow peer/dependency links.
    seen, total = set(), 0
    for path in paths:
        if path in CARRY_PATHS:
            assert path.exists(), 'missing required retained root: ' + str(path)
        entries = path.rglob('*') if path.is_dir() and not path.is_symlink() else [path]
        for entry in entries:
            try:
                if entry.is_symlink() or not entry.is_file():
                    continue
                stat = entry.stat()
                key = (stat.st_dev, stat.st_ino)
                if key in seen:
                    continue
                seen.add(key)
                total += max(stat.st_size, stat.st_blocks * 512)
            except FileNotFoundError:
                pass
    return total


def free():
    v = os.statvfs(ROOT)
    return v.f_bavail * v.f_frsize


def table():
    rows = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,comm='], text=True).splitlines():
        fields = line.strip().split(None, 4)
        if len(fields) == 5:
            rows[int(fields[0])] = tuple(int(v) for v in fields[1:4]) + (fields[4],)
    return rows


log = EVIDENCE / (OUT.name + '-controller.jsonl')
accepted = float(os.environ.get('MCR_PI_ACCEPT_EPOCH', str(time.time())))
resource = dict(ownerThreadId='01a1a26e-1de2-7b41-a294-c9a79cfded9b', offerId=os.environ['MCR_PI_LEASE'],
                state='ACCEPTED', acceptedAtEpoch=accepted, workDeadlineEpoch=WORK, fullDeadlineEpoch=HARD,
                kind='source' if MODE == 'source' else 'browser', processCap=8 if MODE == 'source' else 24,
                rssCapBytes=int((2 if MODE == 'source' else 3.5) * GIB), remainingNewUpperBytes=CAP - NEW_CARRY,
                controller=identity(os.getpid(), Path(__file__).resolve(), OUT), guardControllers=[],
                receiptPaths=dict(accept=str(log), full=str(log), primaryFull=str(OUT / 'release.json')))
publish_resource(resource)

assert port_free(), 'allocated native port is occupied'
assert MODE in ('source', 'native') and Path.cwd() == ROOT
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == os.environ['MCR_PI_CONTROLLER_SHA']
assert hashlib.sha256(GUARDIAN.read_bytes()).hexdigest() == os.environ['MCR_PI_GUARDIAN_SHA']
assert hashlib.sha256((EVIDENCE / 'qualification-readset.json').read_bytes()).hexdigest() == os.environ['MCR_PI_READSET_SHA']
assert OUT.is_relative_to(EVIDENCE) and not OUT.exists()
assert TMP.as_posix().startswith('/private/tmp/mcr-paradise-spatial-') and not TMP.exists()
assert not os.path.lexists(LINK) and HARD - WORK >= 30 and time.time() < WORK
assert int(os.environ['MCR_PI_START_FLOOR']) >= 11 * GIB and OTHER >= 0
assert free() >= int(os.environ['MCR_PI_START_FLOOR']) and free() - OTHER - CAP >= 9 * GIB
prior = max([5_560_971_264] + [json.loads(p.read_text()).get('peak', {}).get('wholeBytes', 0)
                            for p in EVIDENCE.glob('*/release.json')])
for path in EVIDENCE.glob('*-controller.jsonl'):
    for line in path.read_text().splitlines():
        prior = max(prior, json.loads(line).get('peak', {}).get('wholeBytes', 0))
carry = max(prior, size(CARRY_PATHS) + 34 * MIB)
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
    membership = sample_membership()
    publish_census(EVIDENCE / (OUT.name + '-controller-census.json'), membership)
    if not hasattr(check, 'membership') or check.membership['owners'] != membership['owners']:
        emit(dict(type='MEMBERSHIP', sample=membership))
    check.membership = membership
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
                    newBytes=NEW_CARRY + 34 * MIB + max(0, mutable - baseline) + temporary, tmpBytes=temporary,
                    wholeBytes=max(carry, size(CARRY_PATHS)) + temporary)
    for key, value in counters.items():
        peak[key] = max(peak[key], value)
    minimum = min(minimum, free())
    assert free() >= 8 * GIB and time.time() < WORK, 'outer live floor/work stop'
    assert counters['processes'] <= (8 if MODE == 'source' else 24), 'outer all-process cap'
    assert counters['rssBytes'] <= (2 if MODE == 'source' else 3.5) * GIB, 'outer RSS cap'
    assert counters['newBytes'] <= CAP and counters['tmpBytes'] <= CAP and counters['wholeBytes'] <= WHOLE_CAP


emit(dict(type='ACCEPT', lease=os.environ['MCR_PI_LEASE'], at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
          controllerPid=os.getpid(), workDeadline=WORK, hardDeadline=HARD, freeBytes=free(),
          otherNewBytes=OTHER, ownNewCap=CAP, sharedNewCarryBytes=NEW_CARRY, carryBytes=carry, priorHighWaterBytes=prior))
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
    if LINK.is_symlink() and os.readlink(LINK) == DEPENDENCIES:
        LINK.unlink()
    if TMP.exists():
        shutil.rmtree(TMP)
    live = [pid for pid, row in table().items() if pid in signatures and row[3] == signatures[pid]]
    port_closed = port_free()
    if not port_closed:
        code = 2
    emit(dict(type='FULL', lease=os.environ['MCR_PI_LEASE'], at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
              exitCode=code, failure=failure, peak=peak, minimumFreeBytes=minimum, finalFreeBytes=free(),
              seenPids=sorted(seen), livePids=live, ownedLinkAbsent=not os.path.lexists(LINK),
              privateTmpAbsent=not TMP.exists(), ownedPortClosed=port_closed, resources=len(live) + (0 if port_closed else 1)))
    resource = json.loads(RESOURCE_RECEIPT.read_text())
    resource.update(state='FULL', actualFullEpoch=time.time(), remainingNewUpperBytes=0,
                    resources=len(live) + (0 if port_closed else 1))
    publish_resource(resource)
raise SystemExit(code)
