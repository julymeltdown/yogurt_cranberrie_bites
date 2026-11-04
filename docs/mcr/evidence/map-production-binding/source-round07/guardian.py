"""Independent bounded source07 guardian; run only under a fresh explicit accepted lease."""
import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import time

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-runtime-preferences')
FRONT = ROOT / 'frontend'
OUT = Path(__file__).resolve().parent
LEASE = os.environ['MCR_MAP_SOURCE_LEASE']
END = datetime.datetime.fromisoformat(os.environ['MCR_MAP_SOURCE_DEADLINE']).timestamp()
assert Path.cwd() == ROOT and LEASE and time.time() < END - 60
LOG = OUT / 'guardian.jsonl'
SEEN = set()
PEAK = dict(processes=0, rssBytes=0, additionalBytes=0)

def record(row):
    with LOG.open('a') as handle:
        handle.write(json.dumps(row) + '\n')

def table():
    result = {}
    raw = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,comm='], text=True)
    for line in raw.splitlines():
        value = line.strip().split(None, 4)
        if len(value) == 5:
            result[int(value[0])] = tuple(int(v) for v in value[1:4]) + (value[4],)
    return result

def run(argv, cwd):
    child = subprocess.Popen(argv, cwd=cwd, start_new_session=True)
    ids = {child.pid}
    record(dict(startedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(), argv=argv, pid=child.pid))
    try:
        while child.poll() is None:
            processes = table()
            previous = set()
            while previous != ids:
                previous = set(ids)
                ids.update(pid for pid, value in processes.items() if value[0] in ids)
            new = ids - SEEN
            if new:
                record(dict(observedDescendants={pid: processes.get(pid) for pid in sorted(new)}))
                SEEN.update(new)
            live = {pid for pid in ids if pid in processes}
            rss = sum(processes[pid][2] * 1024 for pid in live | {os.getpid()} if pid in processes)
            extra = 24 * 1024 ** 2 + sum(p.stat().st_size for base in [OUT, FRONT / 'src/app/world-entry/map-presentation-v2',
                FRONT / 'src/app/world-entry/__tests__/map-presentation-v2', FRONT / 'src/world/reference-scene/map4-service-v2']
                for p in base.rglob('*') if p.is_file() and not p.is_symlink())
            PEAK['processes'] = max(PEAK['processes'], len(live) + 1)
            PEAK['rssBytes'] = max(PEAK['rssBytes'], rss)
            PEAK['additionalBytes'] = max(PEAK['additionalBytes'], extra)
            disk = os.statvfs(ROOT)
            assert time.time() < END - 60, 'independent active deadline'
            assert disk.f_bavail * disk.f_frsize >= 8 * 1024 ** 3, 'independent disk floor'
            assert len(live) + 1 <= 4, 'independent process cap'
            assert rss < 2.5 * 1024 ** 3, 'independent RSS cap'
            assert extra < 32 * 1024 ** 2, 'independent additional cap'
            time.sleep(.3)
    except BaseException:
        try:
            os.kill(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            for pid in sorted(ids, reverse=True):
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            child.wait()
        raise
    record(dict(finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(), pid=child.pid, exitCode=child.returncode))
    assert child.returncode == 0, 'allocated stage failed; retain logs and repair within original clock'

record(dict(lease=LEASE, guardianPid=os.getpid(), startedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
            activeDeadline=datetime.datetime.fromtimestamp(END - 60, datetime.timezone.utc).isoformat(),
            hardDeadline=os.environ['MCR_MAP_SOURCE_DEADLINE']))
try:
    run(['python3', str(OUT / 'supervisor.py'), 'registration'], FRONT)
    run(['git', 'add', '--', 'frontend/src/world/reference-scene/map4-service-v2/registration.ts',
         'frontend/src/world/reference-scene/map4-service-v2/current.ts'], ROOT)
    run(['git', 'commit', '-m', 'feat(map): atomically register ART05 architectural composition'], ROOT)
    run(['python3', str(OUT / 'supervisor.py'), 'tests', 'types', 'lint', 'registration-check'], FRONT)
finally:
    record(dict(guardianFinishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                allObservedDescendants=sorted(SEEN), peak=PEAK))
