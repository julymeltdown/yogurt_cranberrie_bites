"""One explicitly allocated source workflow; no HTTP, server or browser."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish')
FRONT = ROOT / 'frontend'
OUT = Path(__file__).resolve().parent
TMP = Path('/private/tmp') / os.environ['MCR_SOCIAL_WIRE_LEASE'].lower()
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
DEPS = '/Users/lhs/Desktop/mcr/frontend/node_modules'
PIN = os.environ['MCR_SOCIAL_WIRE_PIN']
LEASE = os.environ['MCR_SOCIAL_WIRE_LEASE']
DEADLINE = datetime.datetime.fromisoformat(os.environ['MCR_SOCIAL_WIRE_DEADLINE']).timestamp()
CARRY = 53 * 1024 ** 2
link = FRONT / 'node_modules'
started = time.time()
rows = []
child = None
failure = None
owned_link = False
minimum = 10**30
peaks = dict(temp=0, durable=0, whole=0)


def size(path):
    return sum(max(p.stat().st_size, p.stat().st_blocks * 512) for p in path.rglob('*')
               if p.is_file() and not p.is_symlink()) if path.exists() else 0


def guard(fresh=False):
    global minimum
    free = shutil.disk_usage(ROOT).free
    minimum = min(minimum, free)
    assert free >= (16 if fresh else 8) * 1024 ** 3, 'disk floor'
    values = dict(temp=size(TMP), durable=size(OUT))
    values['whole'] = values['temp'] + values['durable'] + CARRY
    for key, value in values.items():
        peaks[key] = max(peaks[key], value)
    assert values['temp'] < 32 * 1024 ** 2 and values['durable'] < 8 * 1024 ** 2
    assert values['whole'] < 96 * 1024 ** 2, 'whole cap'
    assert time.time() < DEADLINE - 60 and time.time() - started < 120, 'cleanup minute'


prefix = 'src/world/network/'
jobs = [
    ('generate', [NODE, prefix + '__tests__/generate-types.mjs']),
    ('drift', [NODE, prefix + '__tests__/generate-types.mjs', '--check']),
    ('tests', [NODE, 'node_modules/vitest/vitest.mjs', 'run', '--config', prefix + '__tests__/social-policy-wire.config.ts', '--configLoader', 'runner']),
    ('types', [NODE, 'node_modules/typescript/bin/tsc', '-p', prefix + '__tests__/social-policy-wire.tsconfig.json', '--tsBuildInfoFile', str(TMP / 'types.tsbuildinfo'), '--pretty', 'false']),
    ('lint', [NODE, 'node_modules/oxlint/bin/oxlint', prefix + 'transport/engine.ts', prefix + 'transport/wire-types.ts', prefix + '__tests__/social-policy-wire.test.ts', prefix + '__tests__/social-policy-wire.config.ts']),
]
try:
    assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == PIN
    assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT)
    assert not os.path.lexists(link) and not TMP.exists()
    assert hashlib.sha256((FRONT / 'pnpm-lock.yaml').read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
    guard(fresh=True)
    TMP.mkdir(mode=0o700)
    link.symlink_to(DEPS, target_is_directory=True)
    owned_link = True
    env = {**os.environ, 'PATH': str(Path(NODE).parent) + ':' + os.environ['PATH'],
           'TMPDIR': str(TMP), 'TMP': str(TMP), 'TEMP': str(TMP),
           'XDG_CACHE_HOME': str(TMP / 'cache'), 'MCR_SOCIAL_WIRE_TMP': str(TMP), 'CI': 'true'}
    for name, argv in jobs:
        guard()
        at = datetime.datetime.now(datetime.timezone.utc).isoformat()
        with (OUT / (name + '.log')).open('wb') as log:
            child = subprocess.Popen(argv, cwd=FRONT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            while child.poll() is None:
                guard()
                time.sleep(.25)
            guard()
        row = dict(job=name, argv=argv, startedAt=at, exitCode=child.returncode,
                   finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat())
        rows.append(row)
        print(json.dumps(row), flush=True)
        assert child.returncode == 0, name + ' failed'
except BaseException as error:
    failure = str(error)
finally:
    if child is not None:
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=5)
    if owned_link and link.is_symlink() and os.readlink(link) == DEPS:
        link.unlink()
    if owned_link:
        shutil.rmtree(TMP, ignore_errors=True)
    result = dict(lease=LEASE, source=PIN, commands=rows, failure=failure,
                  startedAt=datetime.datetime.fromtimestamp(started, datetime.timezone.utc).isoformat(),
                  finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(), elapsedSeconds=time.time()-started,
                  peaks=peaks, minimumFreeBytes=minimum, ownLinkAbsent=not os.path.lexists(link),
                  privateTmpAbsent=not TMP.exists(), browsers=0, servers=0)
    (OUT / 'release.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result), flush=True)
raise SystemExit(1 if failure else 0)
