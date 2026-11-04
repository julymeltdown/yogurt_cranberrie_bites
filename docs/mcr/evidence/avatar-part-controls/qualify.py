"""FE12 body-palette bounded source checks.
Adapted from MCR source f23344e7 entity reviewed-production/qualify.py;
only the source base/lease paths/argv and whole-retained accounting change.
Requires fresh OFFER+ACCEPT, never authorizes its own allocation.
"""
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
BASE = '654974128a27c948a449fa9e58f466b8e612361d'
EVIDENCE = CHECKOUT / 'docs/mcr/evidence/avatar-part-controls'
NEW_PREFIXES = [
    'frontend/src/features/avatar/part-controls/',
    'frontend/src/features/avatar/wardrobe-options/',
    'docs/mcr/evidence/avatar-part-controls/',
]
EXACT = ['docs/mcr/frontend/avatar-part-controls.md']
CAP = 128 * 1024**2
RSS_CAP = int(2.5 * 1024**3)
PROCESS_CAP = 8
_git_sizes = {}
START_FLOOR = 16 * 1024**3
STOP_FLOOR = 8 * 1024**3

def git(*args):
    return subprocess.check_output(['git', *args], cwd=CHECKOUT, text=True).strip()

def permitted(path):
    return path in EXACT or any(path.startswith(prefix) for prefix in NEW_PREFIXES)

def tree_bytes(root):
    return sum(p.lstat().st_size for p in root.rglob('*') if p.is_file() or p.is_symlink()) if root.exists() else 0

def source_git_bytes():
    head = git('rev-parse', 'HEAD')
    if head not in _git_sizes:
        objects = {row.split()[0] for row in git('rev-list', '--objects', BASE + '..' + head).splitlines()}
        sizes = subprocess.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'],
            input='\n'.join(sorted(objects)) + '\n', cwd=CHECKOUT, text=True) if objects else ''
        _git_sizes[head] = sum(int(row) for row in sizes.splitlines())
    metadata = Path(git('rev-parse', '--absolute-git-dir'))
    return _git_sizes[head] + tree_bytes(metadata)

def charged_bytes(tmp):
    # Charge whole retained own output/source plus historical source-round carry.
    paths = {CHECKOUT / p for p in EXACT}
    retained = NEW_PREFIXES + [
        'frontend/src/entities/avatar/runtime/motion-adoption/',
        'frontend/src/entities/avatar/runtime/__tests__/reviewed-production/',
        'frontend/src/features/avatar/__tests__/reviewed-production/',
        'docs/mcr/evidence/customize-reviewed-production/',
    ]
    for prefix in retained:
        paths.update(p for p in (CHECKOUT / prefix).rglob('*') if p.is_file() or p.is_symlink())
    return source_git_bytes() + tree_bytes(tmp) + sum(p.lstat().st_size for p in paths if p.exists())

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
    if Path.cwd() != CHECKOUT or Path(__file__).resolve() != EVIDENCE / 'qualify.py':
        raise RuntimeError('Absolute exclusive checkout mismatch')
    slot = os.environ.get('MCR_PALETTE_ACCEPTED_SLOT', '')
    expected = os.environ.get('MCR_PALETTE_SOURCE_HEAD', '')
    deadline = dt.datetime.fromisoformat(os.environ['MCR_PALETTE_DEADLINE_UTC']).timestamp()
    if not slot or expected != git('rev-parse', 'HEAD'):
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
    if any(not row[3:].startswith('docs/mcr/evidence/avatar-part-controls/') for row in dirty):
        raise RuntimeError('Source must be committed/clean before validation')
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-fe12-body-palette-', dir='/private/tmp'))
    report = {'status': 'FAIL', 'slot': slot, 'source': expected, 'base': BASE,
              'startedUTC': dt.datetime.now(dt.timezone.utc).isoformat(), 'deadlineUTC': os.environ['MCR_PALETTE_DEADLINE_UTC'],
              'wholeChargedCapBytes': CAP, 'rssCapBytes': RSS_CAP, 'startFloorBytes': START_FLOOR, 'stopFloorBytes': STOP_FLOOR,
              'processCap': PROCESS_CAP, 'sourceGitBytesAtStart': source_git_bytes(), 'peakProcessCount': 0,
              'peakChargedBytes': 0, 'peakAggregateRssBytes': 0, 'checks': [], 'sourcePins': {}}
    env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ.get('PATH', ''),
           'TMPDIR': str(tmp), 'TMP': str(tmp), 'TEMP': str(tmp), 'XDG_CACHE_HOME': str(tmp / 'cache'),
           'MCR_FRONTEND_DEPENDENCIES': str(DEPS), 'MCR_MOTION_CACHE': str(tmp / 'vitest')}
    tests = FRONTEND / 'src/features/avatar/part-controls/__tests__'
    motion_checks = FRONTEND / 'src/world/avatar/__tests__/motion-adoption'
    checks = [
        ('runtime', [str(NODE), '--version'], CHECKOUT),
        ('unit', [str(NODE), str(DEPS / 'node_modules/vitest/vitest.mjs'), 'run', '--configLoader', 'runner', '--root', str(FRONTEND), '--config', str(tests / 'vitest.config.mjs')], FRONTEND),
        ('application-types', [str(NODE), str(motion_checks / 'typecheck.mjs'), '--application'], CHECKOUT),
        ('lint', [str(NODE), str(DEPS / 'node_modules/oxlint/bin/oxlint'), str(FRONTEND / 'src/features/avatar/part-controls'), str(FRONTEND / 'src/features/avatar/wardrobe-options')], CHECKOUT),
        ('whitespace', ['git', 'diff', '--check', BASE], CHECKOUT),
        ('plan', ['python3', str(CHECKOUT / 'scripts/mcr/check-plan.py')], CHECKOUT),
    ]
    child = None
    try:
        for name, argv, cwd in checks:
            if time.time() >= deadline - 60:
                raise RuntimeError('Cleanup reserve reached before next child')
            ids, rss = process_tree(os.getpid())
            if len(ids) > PROCESS_CAP or rss > RSS_CAP or charged_bytes(tmp) > CAP or shutil.disk_usage(CHECKOUT).free < STOP_FLOOR:
                raise RuntimeError('Whole charged carry/free-space guard before child')
            if git('rev-parse', 'HEAD') != expected:
                raise RuntimeError('Source HEAD changed during round')
            with (EVIDENCE / (name + '.log')).open('w') as output:
                child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
                while child.poll() is None:
                    ids, rss = process_tree(os.getpid())
                    report['peakProcessCount'] = max(report['peakProcessCount'], len(ids))
                    charged = charged_bytes(tmp)
                    report['peakAggregateRssBytes'] = max(report['peakAggregateRssBytes'], rss)
                    report['peakChargedBytes'] = max(report['peakChargedBytes'], charged)
                    if len(ids) > PROCESS_CAP or rss > RSS_CAP or charged > CAP or shutil.disk_usage(CHECKOUT).free < STOP_FLOOR or time.time() >= deadline - 60:
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
