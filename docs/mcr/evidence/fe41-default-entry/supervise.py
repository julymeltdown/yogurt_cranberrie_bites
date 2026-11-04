"""Run only after a named resource21d OFFER/ACCEPT; no defaults extend its lease.
Read-only source accounting includes historical own FE41/Giveaway artifacts.
Adapted from prior Giveaway supervisor; adds prelaunch and continuous hard guards.
"""
from pathlib import Path
import argparse
import json
import os
import shutil
import signal
import subprocess
import time

root = Path(__file__).resolve().parents[4]
out = root / 'docs/mcr/evidence/fe41-default-entry'
node = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
target = '/Users/lhs/Desktop/mcr/frontend/node_modules'
scratch = Path('/private/tmp/mcr-fe41-default-entry-2411')
link = root / 'frontend/node_modules'
parser = argparse.ArgumentParser()
for key in ['offer', 'head', 'job']:
    parser.add_argument('--' + key, required=True)
for key in ['cutoff', 'release', 'whole', 'temp', 'durable', 'profile', 'processes', 'rss']:
    parser.add_argument('--' + key, required=True, type=int)
args = parser.parse_args()
jobs = {
    'unit': [node, 'frontend/node_modules/vitest/vitest.mjs', 'run', '--root', 'frontend', '--configLoader', 'runner', '--no-cache', '--maxWorkers', '1',
        'src/app/errors/__tests__/observer.test.ts', 'src/app/errors/__tests__/connectivity.test.ts',
        'src/app/errors/__tests__/default-entry.test.ts', 'src/app/errors/__tests__/production-reconnect.test.ts', 'src/app/errors/__tests__/default-routes.test.tsx',
        'src/features/connection-status/__tests__/status.test.tsx', 'src/features/connection-status/__tests__/pickup-no-replay.test.ts',
        'src/features/gift-pickup/__tests__/pickup.test.tsx', 'src/world/network/__tests__/replication.test.ts',
        'src/world/composition/__tests__/composition.test.ts', 'src/app/world-entry/__tests__/production.test.tsx'],
    'types': [node, 'frontend/node_modules/typescript/bin/tsc', '--noEmit', '--incremental', 'false', '--project', 'frontend/tsconfig.json'],
    'lint': [node, 'frontend/node_modules/oxlint/bin/oxlint', 'frontend/src/app/errors', 'frontend/src/features/connection-status',
        'frontend/src/main.tsx', 'frontend/src/app/ports.ts', 'frontend/src/app/router.tsx', 'frontend/src/app/WorldShell.tsx', 'frontend/src/app/world-entry/production.ts'],
    'native': [node, 'frontend/src/app/errors/__tests__/default-entry/native.mjs'],
}
assert args.job in jobs and args.cutoff < args.release
assert time.time() < args.cutoff - 15
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == args.head
assert not subprocess.check_output(['git', 'status', '--porcelain', '--', 'frontend/src', 'frontend/e2e', 'frontend/public', 'contracts',
    'frontend/package.json', 'frontend/pnpm-lock.yaml', 'frontend/vite.config.ts', 'frontend/tsconfig.json',
    'docs/mcr/frontend/degraded-states.md', 'docs/mcr/evidence/fe41-default-entry/supervise.py'], cwd=root, text=True).strip()
assert not link.exists() and not link.is_symlink()
assert not scratch.exists()

def size(path):
    if not path.exists() or path.is_symlink():
        return 0
    if path.is_file():
        return path.stat().st_size
    total = 0
    for directory, dirs, files in os.walk(path, followlinks=False):
        dirs[:] = [d for d in dirs if not (Path(directory) / d).is_symlink()]
        for name in files:
            p = Path(directory) / name
            try:
                if not p.is_symlink():
                    total += p.stat().st_size
            except FileNotFoundError:
                pass
    return total

source_paths = ['frontend/src', 'frontend/public', 'frontend/e2e', 'contracts',
    'frontend/package.json', 'frontend/pnpm-lock.yaml', 'frontend/vite.config.ts', 'frontend/tsconfig.json',
    'docs/mcr/frontend/degraded-states.md', 'docs/mcr/frontend/css-parity-giveaway.md', 'docs/mcr/evidence/css-parity-giveaway']
source_bytes = sum(size(root / p) for p in source_paths)
info = dict(offer=args.offer, source=args.head, job=args.job, argv=jobs[args.job], started=time.time(), cutoff=args.cutoff, release=args.release,
    caps=dict(whole=args.whole, temp=args.temp, durable=args.durable, profile=args.profile, processes=args.processes, rss=args.rss), sourceBytes=source_bytes,
    peaks=dict(whole=0, temp=0, durable=0, profile=0, processes=0, rss=0), minFree=2**63-1, exitCode=None)
known = set()
proc = None

def processes():
    rows = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True)
    values = [tuple(map(int, row.split())) for row in rows.splitlines() if row.strip()]
    if proc:
        known.add(proc.pid)
    for _ in range(10):
        before = len(known)
        known.update(pid for pid, parent, group, _ in values if parent in known or (proc and group == proc.pid))
        if len(known) == before:
            break
    info['currentRSS'] = sum(rss * 1024 for pid, _, _, rss in values if pid in known)
    return [pid for pid, _, _, _ in values if pid in known]

def measure():
    temp = size(scratch)
    durable = size(out)
    profile = sum(size(p) for p in scratch.glob('playwright*')) if scratch.exists() else 0
    values = dict(whole=source_bytes + temp + durable, temp=temp, durable=durable, profile=profile, processes=len(processes()), rss=info['currentRSS'])
    free = shutil.disk_usage(root).free
    info['minFree'] = min(free, info['minFree'])
    for key, value in values.items():
        info['peaks'][key] = max(info['peaks'][key], value)
        assert value <= getattr(args, key), f'{key} cap exceeded: {value}'
    # Fresh >=16 GiB is checked at actual lease acceptance, once. Every job
    # inside that same accepted lease keeps its original >=8 GiB run floor.
    assert free >= 8 * 1024**3, f'free disk floor: {free}'
    assert time.time() < args.cutoff, 'absolute active cutoff'

def stop():
    if proc:
        processes()
        try:
            os.killpg(proc.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        for pid in known:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        time.sleep(0.2)
        for pid in known:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        proc.wait(timeout=3)

try:
    measure()
    scratch.mkdir(); link.symlink_to(target, target_is_directory=True)
    env = dict(os.environ, PATH=str(Path(node).parent) + ':' + os.environ['PATH'], TMPDIR=str(scratch), FE41_SCRATCH=str(scratch))
    with (out / (args.job + '.log')).open('a') as log:
        log.write('\nFE41 attempt source=' + args.head + '\n'); log.flush()
        measure()
        proc = subprocess.Popen(jobs[args.job], cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        info['pid'] = proc.pid
        while proc.poll() is None:
            measure(); time.sleep(0.5)
        info['exitCode'] = proc.returncode
        measure()
except BaseException as error:
    info['guardFailure'] = str(error)
    raise
finally:
    stop()
    if link.is_symlink() and os.readlink(link) == target:
        link.unlink()
    if scratch.exists():
        shutil.rmtree(scratch)
    remaining = processes()
    info.update(finished=time.time(), childrenRemaining=remaining, linkAbsent=not link.exists(), scratchAbsent=not scratch.exists(), releasedBeforeDeadline=time.time() < args.release)
    (out / (args.job + '-' + str(time.time_ns()) + '-supervisor.json')).write_text(json.dumps(info, indent=2) + '\n')
    print(json.dumps(info))
    assert not remaining and info['releasedBeforeDeadline']
if info['exitCode']:
    raise SystemExit(info['exitCode'])
