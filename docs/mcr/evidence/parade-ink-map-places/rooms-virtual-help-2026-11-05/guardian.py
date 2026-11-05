"""Own finite source/native round. No allocation or deadline is invented here."""
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

ROOT = Path(__file__).resolve().parents[5]
FRONT = ROOT / 'frontend'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPENDENCIES = '/Users/lhs/Desktop/mcr/frontend/node_modules'
LINK = FRONT / 'node_modules'
MODE = os.environ['MCR_PI_MODE']
PHASE = os.environ.get('MCR_PI_PHASE', 'all')
LEASE = os.environ['MCR_PI_LEASE']
OUT = Path(os.environ['MCR_PI_OUTPUT'])
TMP = Path(os.environ['MCR_PI_TMP'])
HARD = datetime.datetime.fromisoformat(os.environ['MCR_PI_DEADLINE'].replace('Z', '+00:00')).timestamp()
WORK = datetime.datetime.fromisoformat(os.environ['MCR_PI_WORK_DEADLINE'].replace('Z', '+00:00')).timestamp()
OTHER = int(os.environ['MCR_PI_OTHER_NEW'])
NEW_CARRY = int(os.environ.get('MCR_PI_NEW_CARRY', '0'))
assert 125112320 <= NEW_CARRY <= int(os.environ['MCR_PI_FAMILY_CAP']) <= 383037440
START_FLOOR = int(os.environ.get('MCR_PI_START_FLOOR', str(12 * 1024**3)))
GIB = 1024**3
MIB = 1024**2
NEW_CAP = int(os.environ['MCR_PI_FAMILY_CAP'])
PROCESS_CAP = 8 if MODE == 'source' else 24
RSS_CAP = int(os.environ.get('MCR_PI_SOURCE_RSS_CAP', str(2 * GIB))) if MODE == 'source' else 3.5 * GIB
assert RSS_CAP in (2 * GIB, 3 * GIB, 3.5 * GIB)
WHOLE_CAP = int(os.environ['MCR_PI_WHOLE_CAP'])

# Publish the actual inner guard before retained-root census.
resource = json.loads(RESOURCE_RECEIPT.read_text())
assert resource['offerId'] == LEASE and resource['state'] == 'ACCEPTED'
resource['guardControllers'] = [identity(os.getpid(), Path(__file__).resolve(), OUT)]
publish_resource(resource)

PRIVATE_GIT = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
COMMON_GIT = Path(subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT, text=True).strip())
CARRY_PATHS = [ROOT, PRIVATE_GIT, COMMON_GIT, Path(DEPENDENCIES), Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64'), Path('/Applications/Google Chrome.app'), Path('/Library/Frameworks/Python.framework/Versions/3.14'), Path('/Users/lhs/.codex/visualizations/2026/11/03/01a1a26e-1de2-7b41-a294-c9a79cfded9b')]
CARRY_PATHS.append(RESOURCE_RECEIPT)
MUTABLE = [ROOT, PRIVATE_GIT, RESOURCE_RECEIPT]
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


def table():
    result = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,comm='], text=True).splitlines():
        parts = line.strip().split(None, 4)
        if len(parts) == 5:
            result[int(parts[0])] = tuple(int(v) for v in parts[1:4]) + (parts[4],)
    return result


def free():
    value = os.statvfs(ROOT)
    return value.f_bavail * value.f_frsize


assert port_free(), 'allocated native port is occupied'
assert MODE in ('source', 'native') and PHASE in ('all', 'remainder', 'adoption', 'final') and Path.cwd() == ROOT
assert OUT.is_relative_to(Path(__file__).resolve().parent) and not OUT.exists()
assert TMP.as_posix().startswith('/private/tmp/mcr-pi11-rooms-') and not TMP.exists()
assert not os.path.lexists(LINK) and HARD - WORK >= 30 and time.time() < WORK
assert OTHER >= 0 and START_FLOOR >= 11 * GIB
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == os.environ['MCR_PI_GUARDIAN_SHA']
readset = json.loads((Path(__file__).resolve().parent / 'qualification-readset.json').read_text())
for row in readset['harness'] + readset['implementation'] + readset['readOnlyCompatibility'] + readset['executionPins'] + readset.get('retainedChangedTests', []):
    path = Path(row['path'])
    if not path.is_absolute():
        path = ROOT / path
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row['sha256'], 'read-set drift: ' + row['path']
assert hashlib.sha256((FRONT / 'pnpm-lock.yaml').read_bytes()).hexdigest() == readset['lockfileSHA256']
fresh_carry = size(CARRY_PATHS) + 34 * MIB  # Shared .git/dependencies are read-only; own Git/proof is reserved.
PRIOR = 6_642_925_568  # Retained density native high-water, never reset downwards.
for path in (Path(__file__).resolve().parent).glob('*/release.json'):
    PRIOR = max(PRIOR, json.loads(path.read_text()).get('peak', {}).get('wholeBytes', 0))
for path in (Path(__file__).resolve().parent).glob('*-controller.jsonl'):
    for line in path.read_text().splitlines():
        PRIOR = max(PRIOR, json.loads(line).get('peak', {}).get('wholeBytes', 0))
CARRY = max(fresh_carry, PRIOR, 1536 * MIB)
BASE = size(MUTABLE)
assert CARRY + NEW_CAP <= WHOLE_CAP, 'whole carry plus worst-case additional growth'
assert free() >= START_FLOOR and free() - OTHER - NEW_CAP >= 9 * GIB, 'fresh aggregate admission'
OUT.mkdir(parents=True)
TMP.mkdir(mode=0o700)
LINK.symlink_to(DEPENDENCIES)
peak = dict(processes=0, rssBytes=0, newBytes=0, tmpBytes=0, wholeBytes=CARRY)
seen, groups, signatures = set(), set(), {}
minimum_free = free()
exit_code, failure, child = 1, None, None


def event(value):
    with (OUT / 'guardian.jsonl').open('a') as stream:
        stream.write(json.dumps(value) + '\n')
    print(json.dumps(value), flush=True)


def measure():
    mutable, temporary = size(MUTABLE), size([TMP])
    return dict(newBytes=NEW_CARRY + int(os.environ.get('MCR_PI_CONTROL_RESERVE', str(34 * MIB))) + max(0, mutable - BASE) + temporary,
                tmpBytes=temporary, wholeBytes=max(CARRY, size(CARRY_PATHS)) + temporary)


def guard():
    global minimum_free
    membership = sample_membership()
    publish_census(OUT / 'guardian-census.json', membership)
    if not hasattr(guard, 'membership') or guard.membership['owners'] != membership['owners']:
        event(dict(type='MEMBERSHIP', sample=membership))
    guard.membership = membership
    available, counters = free(), measure()
    if MODE == 'native':
        pngs = list(OUT.glob('*.png'))
        logs = list(OUT.glob('*.log')) + [OUT / 'guardian.jsonl', Path(__file__).resolve().parent / (OUT.name + '-controller.jsonl')]
        assert len(pngs) <= 24 and sum(path.stat().st_size for path in pngs) <= 24 * MIB, 'native PNG bound'
        assert sum(path.stat().st_size for path in logs if path.exists()) <= 2 * MIB, 'native log bound'
        assert counters['tmpBytes'] <= 48 * MIB, 'native scratch bound'
        assert max(0, size(MUTABLE) - BASE) <= 16 * MIB, 'native durable bound'
    minimum_free = min(minimum_free, available)
    assert available >= 8 * GIB and time.time() < WORK, 'live floor/work deadline'
    assert counters['newBytes'] <= NEW_CAP and counters['tmpBytes'] <= NEW_CAP and counters['wholeBytes'] <= WHOLE_CAP
    for key, value in counters.items():
        peak[key] = max(peak[key], value)


env = dict(os.environ, PATH=str(NODE.parent) + ':' + os.environ['PATH'], TMPDIR=str(TMP) + '/', MCR_PI_GUARDED='1')
control = Path(__file__).resolve().parent
lint_paths = [str(ROOT / row['path']) for row in readset['implementation'] if row['path'].endswith(('.ts', '.tsx'))]
lint_paths += [str(control / name) for name in ['git-inputs.mjs', 'types.mjs', 'native.mjs']]
checks = [[sys.executable, '-B', str(control / 'identity-probe.py')],
          [str(NODE), str(FRONT / 'node_modules/oxlint/bin/oxlint'), '--deny-warnings', *lint_paths],
          [str(NODE), '--max-old-space-size=2304', str(control / 'types.mjs')]]
if MODE == 'source' and PHASE == 'remainder':
    checks = checks[-1:]
    if os.environ.get('MCR_PI_TERMINAL_PROBE') == '1':
        checks.insert(0, [sys.executable, '-B', str(control / 'terminal-probe.py')])
    if os.environ.get('MCR_PI_ORDINARY_PROBE') == '1':
        checks.insert(0, [sys.executable, '-B', str(control / 'ordinary-probe.py')])
if MODE == 'source' and os.environ.get('MCR_PI_FIXTURE_ONLY') == '1':
    checks = [[str(NODE), '--max-old-space-size=1536', str(control / 'fixture-types.mjs')]]
if MODE == 'native':
    checks = [[str(NODE), str(control / 'native.mjs')]]
event(dict(type='ACCEPT', lease=LEASE, mode=MODE, phase=PHASE, at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
           guardianPid=os.getpid(), workDeadline=WORK, hardDeadline=HARD, otherNewBytes=OTHER,
           ownNewCap=NEW_CAP, sharedNewCarryBytes=NEW_CARRY, startFloorBytes=START_FLOOR, freeBytes=free(), carryBytes=CARRY, priorHighWaterBytes=PRIOR))
try:
    for index, argv in enumerate(checks):
        guard()
        with (OUT / f'command-{index:02}.log').open('wb') as log:
            started = datetime.datetime.now(datetime.timezone.utc).isoformat()
            child = subprocess.Popen(argv, cwd=FRONT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            seen.add(child.pid)
            groups.add(child.pid)
            while child.poll() is None:
                rows = table()
                before = None
                while before != seen:
                    before = set(seen)
                    seen.update(pid for pid, row in rows.items() if row[0] in seen)
                live = {pid for pid in seen if pid in rows}
                signatures.update({pid: rows[pid][3] for pid in live})
                rss = sum(rows[pid][2] * 1024 for pid in live | {os.getpid()} if pid in rows)
                peak['processes'] = max(peak['processes'], len(live) + 2)  # Guardian plus exec parent.
                peak['rssBytes'] = max(peak['rssBytes'], rss + 16 * MIB)
                assert len(live) + 2 <= PROCESS_CAP and rss + 16 * MIB <= RSS_CAP, 'process/RSS limit'
                guard()
                time.sleep(.25)
            exit_code = child.returncode
        event(dict(type='COMMAND', index=index, argv=argv, exitCode=exit_code, pid=child.pid,
                   startedAt=started, finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat()))
        if exit_code:
            raise RuntimeError('Command failed; immutable attempt retained, offer closes after cleanup')
    exit_code = 0
except BaseException as error:
    failure, exit_code = repr(error), 1
finally:
    for group in groups:
        try:
            os.killpg(group, signal.SIGTERM)
        except ProcessLookupError:
            pass
    time.sleep(.4)
    rows = table()
    for pid in seen:
        if pid in rows and rows[pid][3] == signatures.get(pid):
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    if child:
        child.wait(timeout=max(.1, min(3, HARD - time.time())))
    if LINK.is_symlink() and os.readlink(LINK) == DEPENDENCIES:
        LINK.unlink()
    shutil.rmtree(TMP)
    remaining = sorted(pid for pid, row in table().items() if pid in signatures and row[3] == signatures[pid])
    # A closed listener can leave TCP TIME_WAIT; wait only inside the original FULL clock.
    while MODE == 'native' and not port_free() and time.time() + 1 < HARD:
        time.sleep(.25)
    port_closed = port_free()
    if not port_closed:
        exit_code = 2
    release = dict(type='FULL', lease=LEASE, mode=MODE, at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   exitCode=exit_code, failure=failure, peak=peak, minimumFreeBytes=minimum_free, finalFreeBytes=free(),
                   seenPids=sorted(seen), livePids=remaining, ownedLinkAbsent=not os.path.lexists(LINK),
                   privateTmpAbsent=not TMP.exists(), ownedPortClosed=port_closed, resources=len(remaining) + (0 if port_closed else 1))
    (OUT / 'release.json').write_text(json.dumps(release, indent=2) + '\n')
    event(release)
raise SystemExit(exit_code)
