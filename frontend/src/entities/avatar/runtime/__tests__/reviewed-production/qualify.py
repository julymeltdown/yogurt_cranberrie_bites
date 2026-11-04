"""Bounded source checks. Requires a fresh named allocation and explicit ACCEPT environment."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

CHECKOUT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-ref-map')
FRONTEND = CHECKOUT / 'frontend'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend')
BASE = '01d604080670c967e1ce194df5c3d1827df11228'
EVIDENCE = CHECKOUT / 'docs/mcr/evidence/customize-reviewed-production'
NEW_PREFIXES = [
    'frontend/src/entities/avatar/runtime/__tests__/reviewed-production/',
    'frontend/src/features/avatar/__tests__/reviewed-production/',
    'docs/mcr/evidence/customize-reviewed-production/',
]
EXACT = [
    'frontend/src/entities/avatar/runtime/motion-adoption/reviewed-production.ts',
    'frontend/src/entities/avatar/runtime/motion-adoption/runtime.ts',
    'frontend/src/entities/avatar/runtime/motion-adoption/reviewed-manifest.ts',
    'frontend/src/features/avatar/motion-adoption/production-persistence.ts',
    'frontend/src/features/avatar/motion-adoption/controller.ts',
    'frontend/src/features/avatar/motion-adoption/MotionEditor.tsx',
    'docs/mcr/frontend/customize-reviewed-production.md',
]
CAP = 128 * 1024**2
RSS_CAP = int(2.5 * 1024**3)
START_FLOOR = 16 * 1024**3
STOP_FLOOR = 8 * 1024**3

def git(*args):
    return subprocess.check_output(['git', *args], cwd=CHECKOUT, text=True).strip()

def permitted(path):
    return path in EXACT or any(path.startswith(prefix) for prefix in NEW_PREFIXES)

def tree_bytes(root):
    return sum(p.lstat().st_size for p in root.rglob('*') if p.is_file() or p.is_symlink()) if root.exists() else 0

def charged_bytes(tmp):
    paths = set(EXACT)
    for prefix in NEW_PREFIXES:
        root = CHECKOUT / prefix
        if root.exists():
            paths.update(str(p.relative_to(CHECKOUT)) for p in root.rglob('*') if p.is_file() or p.is_symlink())
    total = tree_bytes(tmp)
    for path in paths:
        p = CHECKOUT / path
        if not p.exists():
            continue
        baseline = subprocess.run(['git', 'cat-file', '-s', BASE + ':' + path], cwd=CHECKOUT, text=True, capture_output=True)
        total += max(0, p.lstat().st_size - (int(baseline.stdout) if baseline.returncode == 0 else 0))
    return total

def process_tree(root):
    rows = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss='], text=True)
    table = [tuple(map(int, row.split())) for row in rows.splitlines() if row.strip()]
    ids = {root}
    while True:
        found = ids | {pid for pid, parent, _ in table if parent in ids}
        if found == ids:
            break
        ids = found
    return ids, sum(rss * 1024 for pid, _, rss in table if pid in ids)

def stop(child):
    if child.poll() is None:
        os.killpg(child.pid, signal.SIGTERM)
        try:
            child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=3)

def main():
    if Path.cwd() != CHECKOUT or Path(__file__).resolve().parents[7] != CHECKOUT:
        raise RuntimeError('Absolute exclusive checkout mismatch')
    slot = os.environ.get('MCR_AR03_ACCEPTED_SLOT', '')
    expected = os.environ.get('MCR_AR03_SOURCE_HEAD', '')
    deadline = dt.datetime.fromisoformat(os.environ['MCR_AR03_DEADLINE_UTC']).timestamp()
    if not slot.startswith('AR03-') or expected != git('rev-parse', 'HEAD'):
        raise RuntimeError('Missing accepted allocation or frozen source HEAD')
    if deadline - time.time() > 600 or deadline - time.time() < 90:
        raise RuntimeError('Invalid remaining bounded round/cleanup reserve')
    if not NODE.is_file() or not (DEPS / 'node_modules').is_dir():
        raise RuntimeError('Retained absolute runtime/dependencies unavailable')
    if (FRONTEND / 'node_modules').exists() or (FRONTEND / 'node_modules').is_symlink():
        raise RuntimeError('Unexpected writable dependency link')
    if shutil.disk_usage(CHECKOUT).free < START_FLOOR:
        raise RuntimeError('Fresh start requires at least16GiB free')
    changed = git('diff', '--name-only', BASE).splitlines()
    if any(not permitted(path) for path in changed):
        raise RuntimeError('Changed path outside exact source lease')
    dirty = git('status', '--porcelain', '--untracked-files=all').splitlines()
    if any(not row[3:].startswith('docs/mcr/evidence/customize-reviewed-production/') for row in dirty):
        raise RuntimeError('Source must be committed/clean before validation')
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-ar03-production-', dir='/private/tmp'))
    report = {'status': 'FAIL', 'slot': slot, 'source': expected, 'base': BASE,
              'startedUTC': dt.datetime.now(dt.timezone.utc).isoformat(), 'deadlineUTC': os.environ['MCR_AR03_DEADLINE_UTC'],
              'wholeChargedCapBytes': CAP, 'rssCapBytes': RSS_CAP, 'startFloorBytes': START_FLOOR, 'stopFloorBytes': STOP_FLOOR,
              'peakChargedBytes': 0, 'peakAggregateRssBytes': 0, 'checks': [], 'sourcePins': {}}
    env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ.get('PATH', ''),
           'TMPDIR': str(tmp), 'TMP': str(tmp), 'TEMP': str(tmp), 'XDG_CACHE_HOME': str(tmp / 'cache'),
           'MCR_FRONTEND_DEPENDENCIES': str(DEPS), 'MCR_MOTION_CACHE': str(tmp / 'vitest')}
    tests = FRONTEND / 'src/entities/avatar/runtime/__tests__/reviewed-production'
    motion_checks = FRONTEND / 'src/world/avatar/__tests__/motion-adoption'
    checks = [
        ('runtime', [str(NODE), '--version'], CHECKOUT),
        ('unit', [str(NODE), str(DEPS / 'node_modules/vitest/vitest.mjs'), 'run', '--configLoader', 'runner', '--root', str(FRONTEND), '--config', str(tests / 'vitest.config.mjs')], FRONTEND),
        ('application-types', [str(NODE), str(motion_checks / 'typecheck.mjs'), '--application'], CHECKOUT),
        ('lint', [str(NODE), str(DEPS / 'node_modules/oxlint/bin/oxlint'), *[str(CHECKOUT / p) for p in EXACT if p.endswith(('.ts', '.tsx'))], str(tests), str(FRONTEND / 'src/features/avatar/__tests__/reviewed-production')], CHECKOUT),
        ('whitespace', ['git', 'diff', '--check', BASE], CHECKOUT),
        ('plan', ['python3', str(CHECKOUT / 'scripts/mcr/check-plan.py')], CHECKOUT),
    ]
    child = None
    try:
        for name, argv, cwd in checks:
            if time.time() >= deadline - 60:
                raise RuntimeError('Cleanup reserve reached before next child')
            if charged_bytes(tmp) > CAP or shutil.disk_usage(CHECKOUT).free < STOP_FLOOR:
                raise RuntimeError('Whole charged carry/free-space guard before child')
            if git('rev-parse', 'HEAD') != expected:
                raise RuntimeError('Source HEAD changed during round')
            with (EVIDENCE / (name + '.log')).open('w') as output:
                child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
                while child.poll() is None:
                    _, rss = process_tree(os.getpid())
                    charged = charged_bytes(tmp)
                    report['peakAggregateRssBytes'] = max(report['peakAggregateRssBytes'], rss)
                    report['peakChargedBytes'] = max(report['peakChargedBytes'], charged)
                    if rss > RSS_CAP or charged > CAP or shutil.disk_usage(CHECKOUT).free < STOP_FLOOR or time.time() >= deadline - 60:
                        stop(child)
                        raise RuntimeError('Fail-closed RSS/whole-output/free-space/deadline guard')
                    time.sleep(0.5)
                code = child.returncode
            report['checks'].append({'name': name, 'argv': argv, 'cwd': str(cwd), 'exit': code})
            print(json.dumps(report['checks'][-1]), flush=True)
            if code != 0:
                raise RuntimeError(name + ' failed')
            if name == 'runtime' and (EVIDENCE / 'runtime.log').read_text().strip() != 'v24.22.2':
                raise RuntimeError('Retained Node patch mismatch; no validation child admitted')
        for path in changed:
            p = CHECKOUT / path
            if p.is_file():
                report['sourcePins'][path] = hashlib.sha256(p.read_bytes()).hexdigest()
        report['status'] = 'PASS'
    except Exception as error:
        report['failure'] = str(error)
    finally:
        if child:
            stop(child)
        report['peakChargedBytes'] = max(report['peakChargedBytes'], charged_bytes(tmp))
        shutil.rmtree(tmp)
        report['ownedTemporaryRemoved'] = not tmp.exists()
        report['localNodeModulesPresent'] = (FRONTEND / 'node_modules').exists() or (FRONTEND / 'node_modules').is_symlink()
        report['finishedUTC'] = dt.datetime.now(dt.timezone.utc).isoformat()
        (EVIDENCE / ('checks-' + dt.datetime.now(dt.timezone.utc).strftime('%H%M%S') + '.json')).write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['status'] == 'PASS' else 1

if __name__ == '__main__':
    sys.exit(main())
