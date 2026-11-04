"""Finite FE19 source round; exact declared jobs, own outputs and child processes only."""
import datetime
import hashlib
import json
import os
from pathlib import Path
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
DEADLINE = datetime.datetime.fromisoformat('2026-11-04T10:17:15+00:00').timestamp()
PREFIX = 'src/app/world-entry/__tests__/map-presentation-v1/'
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
PATHS = [ROOT / 'docs/mcr/frontend/map-production-binding.md', OUT.parent,
         FRONT / 'src/app/world-entry/map-presentation-v1', FRONT / PREFIX,
         FRONT / 'src/world/reference-scene/map4-service-v1', FRONT / 'public/world/map-firstview-cozy-v1',
         FRONT / 'src/app/world-entry/reference-scene-overview.ts', FRONT / 'src/world/reference-scene/overview.ts',
         FRONT / 'src/world/illustrated/overview.ts', FRONT / 'src/features/places/ui/IllustratedMap.tsx',
         FRONT / 'src/features/places/ui/reference-scene/map-service-links.tsx']

def size():
    files = set()
    for path in PATHS:
        if path.is_dir(): files.update(p for p in path.rglob('*') if p.is_file() and not p.is_symlink())
        elif path.exists(): files.add(path)
    return sum(p.stat().st_size for p in files)

def guard(fresh=False):
    disk = os.statvfs(ROOT)
    assert disk.f_bavail * disk.f_frsize >= (16 if fresh else 8) * 1024 ** 3, 'disk floor'
    assert size() < 64 * 1024 ** 2, 'whole carry/output cap'
    assert time.time() < DEADLINE - 60, 'cleanup reserve'

assert Path.cwd() == FRONT, 'absolute frontend cwd'
assert hashlib.sha256((FRONT / 'pnpm-lock.yaml').read_bytes()).hexdigest() == LOCK, 'retained lock drift'
guard()
TMP.mkdir(exist_ok=True, parents=True)
link = FRONT / 'node_modules'
if not os.path.lexists(link):
    guard(fresh=True)
    link.symlink_to(TARGET)
else:
    assert link.is_symlink() and os.readlink(link) == TARGET, 'not the owned dependency link'
env = dict(os.environ, PATH=str(Path(NODE).parent) + ':' + os.environ['PATH'], TMPDIR=str(TMP) + '/', MCR_MAP_SOURCE_TMP=str(TMP))
jobs = {
 'runtime': [NODE, '-e', "if(process.version!=='v24.22.2'||process.arch!=='arm64')throw Error('runtime');console.log(JSON.stringify({version:process.version,arch:process.arch,cwd:process.cwd()}))"],
 'pins': [NODE, PREFIX + 'pin-source.mjs'],
 'tests': [NODE, 'node_modules/vitest/vitest.mjs', 'run', '--config', PREFIX + 'vitest.config.ts'],
 'types': [NODE, 'node_modules/typescript/bin/tsc', '-p', PREFIX + 'tsconfig.json', '--tsBuildInfoFile', str(TMP / 'map4.tsbuildinfo'), '--pretty', 'false'],
 'lint': [NODE, 'node_modules/oxlint/bin/oxlint'],
 'native-syntax': [NODE, '--check', PREFIX + 'native.mjs'],
 'pins-syntax': [NODE, '--check', PREFIX + 'pin-source.mjs'],
}
for path in [FRONT / 'src/app/world-entry/map-presentation-v1', FRONT / PREFIX, FRONT / 'src/world/reference-scene/map4-service-v1']:
    jobs['lint'].extend(str(p.relative_to(FRONT)) for p in sorted(path.rglob('*')) if p.suffix in ('.ts', '.tsx'))
jobs['lint'].extend(str(p.relative_to(FRONT)) for p in PATHS if p.suffix in ('.ts', '.tsx') and p.is_file())
names = sys.argv[1:] or list(jobs)
for name in names:
    guard()
    index = len(list(OUT.glob(name + '-*.log'))) + 1
    logpath = OUT / f'{name}-{index:02}.log'
    started = time.time()
    with logpath.open('wb') as log:
        child = subprocess.Popen(jobs[name], cwd=FRONT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            while child.poll() is None:
                guard()
                assert time.time() - started < 180, 'finite child timeout'
                time.sleep(.25)
        except BaseException:
            os.killpg(child.pid, signal.SIGTERM)
            try: child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL); child.wait()
            raise
    receipt = {'job': name, 'argv': jobs[name], 'pid': child.pid, 'exitCode': child.returncode,
               'startedAt': datetime.datetime.fromtimestamp(started, datetime.timezone.utc).isoformat(),
               'finishedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'log': logpath.name,
               'accountedBytes': size()}
    with (OUT / 'attempts.jsonl').open('a') as f: f.write(json.dumps(receipt) + '\n')
    print(json.dumps(receipt), flush=True)
