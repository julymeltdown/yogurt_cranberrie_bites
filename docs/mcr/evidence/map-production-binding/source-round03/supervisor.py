"""Allocated complete-Map foundation and caption source check; no server/browser."""
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

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-runtime-preferences')
FRONT = ROOT / 'frontend'
OUT = Path(__file__).resolve().parent
TMP = OUT / 'tmp'
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
TARGET = '/Users/lhs/Desktop/mcr/frontend/node_modules'
PREFIX = 'src/app/world-entry/__tests__/map-presentation-v2/'
LEASE = os.environ['MCR_MAP_SOURCE_LEASE']
DEADLINE = datetime.datetime.fromisoformat(os.environ['MCR_MAP_SOURCE_DEADLINE']).timestamp()
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
CARRY = [ROOT / 'docs/mcr/frontend/map-production-binding.md', OUT.parent,
         FRONT / PREFIX, FRONT / 'src/app/world-entry/map-presentation-v1',
         FRONT / 'src/world/reference-scene/map4-service-v1', FRONT / 'public/world/map-firstview-cozy-v1',
         FRONT / 'src/app/world-entry/reference-scene-overview.ts', FRONT / 'src/world/reference-scene/overview.ts',
         FRONT / 'src/world/illustrated/overview.ts', FRONT / 'src/features/places/ui/IllustratedMap.tsx',
         FRONT / 'src/features/places/ui/reference-scene/map-service-links.tsx']
CARRY.extend([FRONT / 'src/app/world-entry/__tests__/map-presentation-v1',
              FRONT / 'src/app/world-entry/map-presentation-v2', FRONT / 'src/world/reference-scene/map4-service-v2',
              FRONT / 'public/world/map-reference-composition-v1'])

def bytes_under(path):
    return sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink())

def whole_bytes():
    files = set()
    for path in CARRY:
        if path.is_dir():
            files.update(p for p in path.rglob('*') if p.is_file() and not p.is_symlink())
        elif path.is_file():
            files.add(path)
    return sum(p.stat().st_size for p in files)

def guard(fresh=False):
    disk = os.statvfs(ROOT)
    assert disk.f_bavail * disk.f_frsize >= (16 if fresh else 8) * 1024 ** 3, 'disk floor'
    assert bytes_under(OUT) < 32 * 1024 ** 2, '32MiB additional output/TMP cap'
    assert whole_bytes() < 64 * 1024 ** 2, '64MiB whole carry/output cap'
    assert time.time() < DEADLINE - 60, 'cleanup reserve'

assert Path.cwd() == FRONT
assert hashlib.sha256((FRONT / 'pnpm-lock.yaml').read_bytes()).hexdigest() == LOCK
link = FRONT / 'node_modules'
assert not os.path.lexists(link), 'absent-only owned dependency link'
guard(fresh=True)
TMP.mkdir(parents=True, exist_ok=True)
link.symlink_to(TARGET)
env = dict(os.environ, PATH=str(Path(NODE).parent) + ':' + os.environ['PATH'],
           TMPDIR=str(TMP) + '/', MCR_MAP_COMPOSITION_TMP=str(TMP))
jobs = {
    'tests': ([NODE, 'node_modules/vitest/vitest.mjs', 'run', '--config', PREFIX + 'vitest.config.ts', '--configLoader', 'runner'], 0),
    'types': ([NODE, 'node_modules/typescript/bin/tsc', '-p', PREFIX + 'tsconfig.json', '--tsBuildInfoFile', str(TMP / 'map4.tsbuildinfo'), '--pretty', 'false'], 0),
    'lint': ([NODE, 'node_modules/oxlint/bin/oxlint'], 0),
}
for path in [FRONT / 'src/app/world-entry/map-presentation-v2', FRONT / PREFIX,
             FRONT / 'src/world/reference-scene/map4-service-v2']:
    jobs['lint'][0].extend(str(p.relative_to(FRONT)) for p in sorted(path.rglob('*'))
                           if p.suffix in ('.ts', '.tsx', '.mjs', '.mts'))
jobs['lint'][0].extend(['src/world/reference-scene/overview.ts', 'src/features/places/ui/IllustratedMap.tsx',
                       'src/app/world-entry/reference-scene-overview.ts',
                       'src/features/places/ui/reference-scene/map-service-links.tsx'])

children = []
failures = []
peak = 0
whole_peak = 0
try:
    for name in sys.argv[1:] or list(jobs):
        guard()
        argv, expected = jobs[name]
        index = len(list(OUT.glob(name + '-*.log'))) + 1
        logpath = OUT / f'{name}-{index:02}.log'
        started = datetime.datetime.now(datetime.timezone.utc).isoformat()
        with logpath.open('wb') as log:
            child = subprocess.Popen(argv, cwd=FRONT, env=env, stdout=log,
                                     stderr=subprocess.STDOUT, start_new_session=True)
            children.append(child)
            try:
                start = time.monotonic()
                while child.poll() is None:
                    guard()
                    peak = max(peak, bytes_under(OUT))
                    whole_peak = max(whole_peak, whole_bytes())
                    assert time.monotonic() - start < 180, 'bounded child timeout'
                    time.sleep(.25)
            except BaseException:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
                raise
        row = dict(lease=LEASE, job=name, argv=argv, expectedExit=expected,
                   exitCode=child.returncode, pid=child.pid, log=logpath.name,
                   startedAt=started, finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat())
        with (OUT / 'attempts.jsonl').open('a') as handle:
            handle.write(json.dumps(row) + '\n')
        print(json.dumps(row), flush=True)
        if child.returncode != expected:
            failures.append(name)
finally:
    assert all(child.poll() is not None for child in children)
    assert link.is_symlink() and os.readlink(link) == TARGET
    link.unlink()
    shutil.rmtree(TMP)
    release = dict(lease=LEASE, finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   childPids=[child.pid for child in children], childrenExited=True,
                   ownedLinkAbsent=not os.path.lexists(link), privateTmpAbsent=not TMP.exists(),
                   peakAdditionalBytes=peak, retainedOutputBytes=bytes_under(OUT),
                   peakWholeAccountedBytes=whole_peak, retainedWholeAccountedBytes=whole_bytes(),
                   finalFreeBytes=os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize,
                   failures=failures, servers=0, browsers=0)
    with (OUT / 'cleanup.jsonl').open('a') as handle:
        handle.write(json.dumps(release) + '\n')
    print(json.dumps(release), flush=True)
sys.exit(1 if failures else 0)
