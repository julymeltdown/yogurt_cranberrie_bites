# Derived finite guards; exact provenance: guard-provenance.json.
"""Own finite source/native round. No allocation or deadline is invented here."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[4]
FRONT = ROOT / 'frontend'
PREFIX = '../docs/mcr/evidence/map-viewport-fit/'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPENDENCIES = '/Users/lhs/Desktop/mcr/frontend/node_modules'
LINK = FRONT / 'node_modules'
MODE = os.environ['MCR_MAP_FIT_MODE']
PHASE = os.environ.get('MCR_MAP_FIT_PHASE', 'all')
LEASE = os.environ['MCR_MAP_FIT_LEASE']
OUT = Path(os.environ['MCR_MAP_FIT_OUTPUT'])
TMP = Path(os.environ['MCR_MAP_FIT_TMP'])
HARD = datetime.datetime.fromisoformat(os.environ['MCR_MAP_FIT_DEADLINE'].replace('Z', '+00:00')).timestamp()
WORK = datetime.datetime.fromisoformat(os.environ['MCR_MAP_FIT_WORK_DEADLINE'].replace('Z', '+00:00')).timestamp()
OTHER = int(os.environ['MCR_MAP_FIT_OTHER_NEW'])
START_FLOOR = int(os.environ.get('MCR_MAP_FIT_START_FLOOR', str(12 * 1024**3)))
GIB = 1024**3
MIB = 1024**2
NEW_CAP = (128 if MODE == 'source' else 200) * MIB
PROCESS_CAP = 6 if MODE == 'source' else 24
RSS_CAP = (3 if MODE == 'source' else 3.5) * GIB
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


def retained_matches():
    expected = json.loads((ROOT / 'docs/mcr/evidence/map-viewport-fit/retained-dependencies.json').read_text())
    actual = []
    for path in sorted(LINK.rglob('*')):
        relative = path.relative_to(LINK).as_posix()
        if path.is_symlink():
            actual.append(dict(path=relative, link=os.readlink(path)))
        elif path.is_file():
            actual.append(dict(path=relative, bytes=path.stat().st_size,
                               sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    return actual == expected


assert MODE in ('source', 'native') and PHASE in ('all', 'remaining') and Path.cwd() == ROOT
assert OUT.is_relative_to(ROOT / 'docs/mcr/evidence/map-viewport-fit') and not OUT.exists()
assert TMP.as_posix().startswith('/private/tmp/mcr-map-viewport-fit-') and not TMP.exists()
assert LINK.is_dir() and not LINK.is_symlink() and HARD - WORK >= 30 and time.time() < WORK
assert OTHER >= 0 and START_FLOOR >= 11 * GIB
assert hashlib.sha256((FRONT / 'pnpm-lock.yaml').read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
for filename in ('package.json', 'pnpm-lock.yaml'):
    assert (FRONT / filename).read_bytes() == (Path(DEPENDENCIES).parent / filename).read_bytes()
assert (LINK / '.pnpm/lock.yaml').read_bytes() == (FRONT / 'pnpm-lock.yaml').read_bytes()
for name in ('vitest', 'typescript', 'oxlint', 'vite', '@playwright/test'):
    assert (LINK / name).resolve().is_relative_to(LINK) and (LINK / name).exists()
for row in json.loads((ROOT / 'docs/mcr/evidence/map-viewport-fit/dependency-tool-pins.json').read_text()):
    for base in (LINK, Path(DEPENDENCIES)):
        assert hashlib.sha256((base / row['path']).read_bytes()).hexdigest() == row['sha256']
assert retained_matches(), 'retained dependency snapshot'
for row in json.loads((ROOT / 'docs/mcr/evidence/map-viewport-fit/readset.json').read_text())['pins']:
    assert hashlib.sha256((ROOT / row['path']).read_bytes()).hexdigest() == row['sha256'], row['path']
fresh_carry = max(INVENTORY['allocatedWholeBytes'], INVENTORY['logicalWholeBytes'], size(WHOLE_PATHS)) + 34 * MIB
PRIOR = 1_675_186_956  # Retained density native high-water, never reset downwards.
for path in (ROOT / 'docs/mcr/evidence/map-viewport-fit').glob('*/release.json'):
    PRIOR = max(PRIOR, json.loads(path.read_text()).get('peak', {}).get('wholeBytes', 0))
for path in (ROOT / 'docs/mcr/evidence/map-viewport-fit').glob('*-controller.jsonl'):
    for line in path.read_text().splitlines():
        PRIOR = max(PRIOR, json.loads(line).get('peak', {}).get('wholeBytes', 0))
CARRY = max(fresh_carry, PRIOR, 1536 * MIB)
BASE = size(MUTABLE)
assert CARRY + NEW_CAP <= WHOLE_CAP, 'whole carry plus worst-case additional growth'
assert free() >= START_FLOOR and free() - OTHER - NEW_CAP >= 9 * GIB, 'fresh aggregate admission'
OUT.mkdir(parents=True)
TMP.mkdir(mode=0o700)
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
    return dict(newBytes=34 * MIB + mutable + temporary,
                tmpBytes=temporary, wholeBytes=max(CARRY, size(WHOLE_PATHS) + 34 * MIB) + temporary)


def guard():
    global minimum_free
    available, counters = free(), measure()
    minimum_free = min(minimum_free, available)
    assert available >= 8 * GIB and time.time() < WORK, 'live floor/work deadline'
    assert counters['newBytes'] <= NEW_CAP and counters['tmpBytes'] <= NEW_CAP and counters['wholeBytes'] <= WHOLE_CAP
    for key, value in counters.items():
        peak[key] = max(peak[key], value)


env = dict(os.environ, PATH=str(NODE.parent) + ':' + os.environ['PATH'], TMPDIR=str(TMP) + '/',
           MCR_MAP_COMPOSITION_TMP=str(TMP), MCR_MAP_FIT_GUARDED='1')
checks = [[str(NODE), PREFIX + 'native.mjs']]
if MODE == 'source':
    checks = [[str(NODE), 'node_modules/vitest/vitest.mjs', 'run', '--config',
               'src/features/places/__tests__/map-viewport-fit/vitest.config.ts', '--configLoader', 'runner'],
              [str(NODE), 'node_modules/typescript/bin/tsc', '-p', 'tsconfig.json', '--incremental', 'false', '--pretty', 'false'],
              [str(NODE), 'node_modules/oxlint/bin/oxlint', '--deny-warnings',
               'src/features/places/ui/IllustratedMap.tsx', 'src/features/places/ui/map-viewport-fit',
               'src/features/places/__tests__/map-viewport-fit']]
event(dict(type='ACCEPT', lease=LEASE, mode=MODE, phase=PHASE, at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
           guardianPid=os.getpid(), workDeadline=WORK, hardDeadline=HARD, otherNewBytes=OTHER,
           ownNewCap=NEW_CAP, startFloorBytes=START_FLOOR, freeBytes=free(), carryBytes=CARRY, priorHighWaterBytes=PRIOR))
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
    shutil.rmtree(TMP)
    remaining = sorted(pid for pid, row in table().items() if pid in signatures and row[3] == signatures[pid])
    preserved = retained_matches()
    if not preserved:
        failure, exit_code = 'retained dependency tree changed', 1
    release = dict(type='FULL', lease=LEASE, mode=MODE, at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   exitCode=exit_code, failure=failure, peak=peak, minimumFreeBytes=minimum_free, finalFreeBytes=free(),
                   seenPids=sorted(seen), livePids=remaining, retainedDependenciesPreserved=preserved,
                   privateTmpAbsent=not TMP.exists(), resources=0 if not remaining else len(remaining))
    (OUT / 'release.json').write_text(json.dumps(release, indent=2) + '\n')
    event(release)
raise SystemExit(exit_code)
