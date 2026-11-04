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

ROOT = Path(__file__).resolve().parents[6]
FRONT = ROOT / 'frontend'
PREFIX = 'src/app/world-entry/__tests__/map-roof-street-uptake/'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPENDENCIES = '/Users/lhs/Desktop/mcr/frontend/node_modules'
LINK = FRONT / 'node_modules'
MODE = os.environ['MCR_ROOF_MODE']
PHASE = os.environ.get('MCR_ROOF_PHASE', 'all')
LEASE = os.environ['MCR_ROOF_LEASE']
OUT = Path(os.environ['MCR_ROOF_OUTPUT'])
TMP = Path(os.environ['MCR_ROOF_TMP'])
HARD = datetime.datetime.fromisoformat(os.environ['MCR_ROOF_DEADLINE'].replace('Z', '+00:00')).timestamp()
WORK = datetime.datetime.fromisoformat(os.environ['MCR_ROOF_WORK_DEADLINE'].replace('Z', '+00:00')).timestamp()
OTHER = int(os.environ['MCR_ROOF_OTHER_NEW'])
START_FLOOR = int(os.environ.get('MCR_ROOF_START_FLOOR', str(12 * 1024**3)))
GIB = 1024**3
MIB = 1024**2
NEW_CAP = (128 if MODE == 'source' else 200) * MIB
PROCESS_CAP = 8 if MODE == 'source' else 24
RSS_CAP = (2 if MODE == 'source' else 3.5) * GIB
MUTABLE = [ROOT / 'docs/mcr/evidence/map-roof-street-uptake', ROOT / 'docs/mcr/frontend/map-roof-street-uptake.md',
           FRONT / PREFIX, FRONT / 'src/world/reference-scene/map-roof-street-uptake',
           FRONT / 'src/world/reference-scene/map4-service-v2',
           FRONT / 'src/app/world-entry/__tests__/map-presentation-v2/fixture.ts',
           FRONT / 'src/app/world-entry/__tests__/map-presentation-v2/loader.test.ts',
           FRONT / 'src/app/world-entry/__tests__/map-presentation-v2/registration.test.ts']


def size(paths):
    total = 0
    for path in paths:
        if path.is_symlink():
            continue
        entries = path.rglob('*') if path.is_dir() else [path]
        for entry in entries:
            try:
                if entry.is_file() and not entry.is_symlink():
                    total += entry.stat().st_size
            except FileNotFoundError:
                pass  # Chromium renames its own private temporary profile files.
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


assert MODE in ('source', 'native') and PHASE in ('all', 'remaining', 'compatibility') and Path.cwd() == ROOT
assert OUT.is_relative_to(ROOT / 'docs/mcr/evidence/map-roof-street-uptake') and not OUT.exists()
assert TMP.as_posix().startswith('/private/tmp/mcr-roof-street-uptake-') and not TMP.exists()
assert not os.path.lexists(LINK) and HARD - WORK >= 30 and time.time() < WORK
assert OTHER >= 0 and START_FLOOR >= 11 * GIB
assert hashlib.sha256((FRONT / 'pnpm-lock.yaml').read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
fresh_carry = size([ROOT, NODE]) + 34 * MIB  # Shared .git/dependencies are read-only; own Git/proof is reserved.
PRIOR = 1_675_186_956  # Retained density native high-water, never reset downwards.
for path in (ROOT / 'docs/mcr/evidence/map-roof-street-uptake').glob('*/release.json'):
    PRIOR = max(PRIOR, json.loads(path.read_text()).get('peak', {}).get('wholeBytes', 0))
for path in (ROOT / 'docs/mcr/evidence/map-roof-street-uptake').glob('*-controller.jsonl'):
    for line in path.read_text().splitlines():
        PRIOR = max(PRIOR, json.loads(line).get('peak', {}).get('wholeBytes', 0))
CARRY = max(fresh_carry, PRIOR, 1536 * MIB)
BASE = size(MUTABLE)
assert CARRY + NEW_CAP <= 2 * GIB, 'whole carry plus worst-case additional growth'
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
    return dict(newBytes=34 * MIB + mutable + temporary,
                tmpBytes=temporary, wholeBytes=CARRY + max(0, mutable - BASE) + temporary)


def guard():
    global minimum_free
    available, counters = free(), measure()
    minimum_free = min(minimum_free, available)
    assert available >= 8 * GIB and time.time() < WORK, 'live floor/work deadline'
    assert counters['newBytes'] <= NEW_CAP and counters['tmpBytes'] <= NEW_CAP and counters['wholeBytes'] <= 2 * GIB
    for key, value in counters.items():
        peak[key] = max(peak[key], value)


env = dict(os.environ, PATH=str(NODE.parent) + ':' + os.environ['PATH'], TMPDIR=str(TMP) + '/',
           MCR_MAP_COMPOSITION_TMP=str(TMP), MCR_ROOF_GUARDED='1')
checks = [[str(NODE), PREFIX + 'native.mjs']]
if MODE == 'source':
    checks = [[str(NODE), PREFIX + 'register.mjs'],
              [str(NODE), 'node_modules/vitest/vitest.mjs', 'run', '--config', PREFIX + 'vitest.config.ts', '--configLoader', 'runner'],
              [str(NODE), 'node_modules/typescript/bin/tsc', '-p', 'tsconfig.json', '--pretty', 'false'],
              [str(NODE), 'node_modules/oxlint/bin/oxlint', '--deny-warnings', 'src/world/reference-scene/map4-service-v2',
               'src/world/reference-scene/map-roof-street-uptake', PREFIX,
               'src/app/world-entry/__tests__/map-presentation-v2/fixture.ts',
               'src/app/world-entry/__tests__/map-presentation-v2/loader.test.ts',
               'src/app/world-entry/__tests__/map-presentation-v2/registration.test.ts'],
              [str(NODE), '--check', PREFIX + 'native.mjs']]
if MODE == 'source' and PHASE == 'compatibility':
    checks = [[str(NODE), 'node_modules/vitest/vitest.mjs', 'run', '--config', PREFIX + 'compatibility-vitest.config.ts', '--configLoader', 'runner'],
              [str(NODE), 'node_modules/oxlint/bin/oxlint', '--deny-warnings', 'src/world/reference-scene/map-density-uptake/adoption.test.ts', PREFIX + 'compatibility-vitest.config.ts']]
if MODE == 'source' and PHASE == 'remaining':
    prior = (ROOT / 'docs/mcr/evidence/map-roof-street-uptake/source01/guardian.jsonl').read_text().splitlines()
    passed = [json.loads(line) for line in prior if json.loads(line).get('type') == 'COMMAND']
    assert len(passed) >= 2 and all(row['exitCode'] == 0 for row in passed[:2]), 'retained registration and unit proof required'
    checks = checks[2:]
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
    if LINK.is_symlink() and os.readlink(LINK) == DEPENDENCIES:
        LINK.unlink()
    shutil.rmtree(TMP)
    remaining = sorted(pid for pid, row in table().items() if pid in signatures and row[3] == signatures[pid])
    release = dict(type='FULL', lease=LEASE, mode=MODE, at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   exitCode=exit_code, failure=failure, peak=peak, minimumFreeBytes=minimum_free, finalFreeBytes=free(),
                   seenPids=sorted(seen), livePids=remaining, ownedLinkAbsent=not os.path.lexists(LINK),
                   privateTmpAbsent=not TMP.exists(), resources=0 if not remaining else len(remaining))
    (OUT / 'release.json').write_text(json.dumps(release, indent=2) + '\n')
    event(release)
raise SystemExit(exit_code)
