"""Finite native allocation: one Vite/Chrome workflow, with descendant cleanup."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

p = argparse.ArgumentParser()
for key in ['job', 'print-commit', 'control-commit', 'feed-commit']:
    p.add_argument('--' + key, required=True)
p.add_argument('--deadline-epoch', type=float, required=True)
p.add_argument('--port', type=int, required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--flat-paper-commit')
p.add_argument('--cases')
p.add_argument('--source-checks', action='store_true')
a = p.parse_args()
repo = Path(__file__).resolve().parents[4]
frontend = repo / 'frontend'
node = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
dependencies = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
link = frontend / 'node_modules'
out = a.output.resolve()
assert out.is_relative_to(repo / 'docs/mcr/evidence/reference-chrome-v2') and not out.exists()
assert not link.exists() and not link.is_symlink()
assert 0 < a.deadline_epoch - time.time() <= 300
assert shutil.disk_usage(repo).free >= 16 * 1024**3
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (frontend / name).read_bytes() == (dependencies.parent / name).read_bytes()
assert subprocess.check_output([str(node), '--version'], text=True).strip() == 'v24.22.2'
out.mkdir(parents=True)
temporary = Path(tempfile.mkdtemp(prefix='mcr-reference-chrome-v2-native-', dir='/private/tmp'))
capture = out / 'capture'
env = {**os.environ, 'PATH': str(node.parent) + os.pathsep + os.environ['PATH'], 'TMPDIR': str(temporary),
       'MCR_CHROME_CACHE': str(temporary / 'vitest'),
       'MCR_CHROME_JOB': a.job, 'MCR_CHROME_PORT': str(a.port), 'MCR_CHROME_OUTPUT': str(capture),
       'MCR_CHROME_PRINT_COMMIT': a.print_commit, 'MCR_CHROME_CONTROL_COMMIT': a.control_commit,
       'MCR_CHROME_FEED_COMMIT': a.feed_commit, 'NO_COLOR': '1'}
if a.flat_paper_commit:
    env['MCR_CHROME_FLAT_PAPER_COMMIT'] = a.flat_paper_commit
if a.cases:
    env['MCR_CHROME_CASES'] = a.cases
    env['MCR_CHROME_PRODUCTION_ANONYMOUS'] = '1' if 'landing' in a.cases.split(',') else '0'
roots = [repo, repo.parent / 'fe-mural-editor', repo.parent / 'fe-wall-reference-target', repo.parent / 'be-party-runtime']
report = {'job': a.job, 'status': 'IN_PROGRESS', 'startedEpoch': time.time(), 'workDeadline': a.deadline_epoch,
          'cleanupDeadline': a.deadline_epoch + 60, 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
          'limits': {'workSeconds': 300, 'cleanupSeconds': 60, 'wholeBytes': 2 * 1024**3, 'privateGitReserveBytes': 2 * 1024**2, 'tempBytes': 200 * 1024**2, 'additionalDurableBytes': 16 * 1024**2, 'logsBytes': 2 * 1024**2, 'historicalAndNewPngs': 78, 'pngBytes': 12 * 1024**2, 'processes': 24, 'rssMiB': 4096},
          'commands': [], 'peakProcesses': 0, 'peakRssKiB': 0, 'peakWholeBytes': 0, 'peakTempBytes': 0, 'peakOutputBytes': 0}
child = None
seen = {}

def size(path):
    total = 0
    for f in path.rglob('*'):
        try:
            if f.is_file() and not f.is_symlink():
                total += f.stat().st_size
        except FileNotFoundError:
            # Vite/Vitest atomically replace private cache files during sampling.
            continue
    return total

def temporary_allocated_bytes():
    total = temporary.stat().st_blocks * 512
    for f in temporary.rglob('*'):
        try:
            if not f.is_symlink():
                total += f.stat().st_blocks * 512
        except FileNotFoundError:
            continue
    return total

def processes():
    text = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,command='], text=True)
    rows = {}
    for line in text.splitlines():
        parts = line.split(None, 3)
        if len(parts) == 4:
            pid, parent, rss = map(int, parts[:3])
            rows[pid] = {'pid': pid, 'parent': parent, 'rss': rss, 'commandHash': hashlib.sha256(parts[3].encode()).hexdigest()}
    return rows

def family(rows):
    ids = {os.getpid()}
    while True:
        expanded = ids | {pid for pid, row in rows.items() if row['parent'] in ids}
        if expanded == ids:
            return [row for pid, row in rows.items() if pid in ids]
        ids = expanded

def reap():
    # Chrome may use separate process groups; match retained descendant identity.
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        rows = processes()
        for pid, digest in reversed(list(seen.items())):
            if pid != os.getpid() and pid in rows and rows[pid]['commandHash'] == digest:
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
        if child is not None:
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass

try:
    link.symlink_to(dependencies, target_is_directory=True)
    commands = [
        ('syntax', [str(node), '--check', 'src/app/reference-chrome-v2/native.mjs']),
        ('native', [str(node), 'src/app/reference-chrome-v2/native.mjs']),
    ]
    if a.source_checks:
        commands = [
            ('source-tests', [str(node), str(dependencies / 'vitest/vitest.mjs'), 'run', '--config', 'src/app/reference-chrome-v2/source.vitest.config.ts', '--configLoader', 'runner', '--maxWorkers=1', '--no-file-parallelism', 'src/app/reference-chrome-v2/chrome.test.tsx', 'src/app/__tests__/reference-frame-footer/footer.test.tsx', 'src/app/__tests__/reference-root/root.test.tsx', 'src/features/public/__tests__/reference-target/presentation.test.tsx', 'src/shared/ui/__tests__/primitives.test.tsx']),
            ('source-types', [str(node), str(dependencies / 'typescript/bin/tsc'), '-p', 'tsconfig.json', '--noEmit', '--incremental', 'false']),
            ('source-lint', [str(dependencies / '.bin/oxlint'), 'src/app/WorldShell.tsx', 'src/app/shell-chrome/index.tsx', 'src/app/shell-chrome/parade-footer.tsx', 'src/features/public/ui/PublicFrame.tsx', 'src/shared/ui/concert-window/index.tsx', 'src/app/reference-chrome-v2', 'src/app/__tests__/reference-frame-footer/footer.test.tsx']),
        ] + commands
    report['sourceChecksRequested'] = a.source_checks
    for label, command in commands:
        record = {'label': label, 'command': command, 'startedEpoch': time.time()}
        report['commands'].append(record)
        with (out / (label + '.log')).open('wb') as log:
            child = subprocess.Popen(command, cwd=frontend, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            while child.poll() is None:
                assert time.time() < a.deadline_epoch, 'Original work deadline'
                assert shutil.disk_usage(repo).free >= 8 * 1024**3, '8GiB live floor'
                current = family(processes())
                for row in current:
                    seen[row['pid']] = row['commandHash']
                count, rss = len(current), sum(row['rss'] for row in current)
                whole = sum(int(row.split()[0]) * 1024 for row in subprocess.check_output(['du', '-sk', *map(str, roots)], text=True).splitlines()) + temporary_allocated_bytes() + report['limits']['privateGitReserveBytes']
                temp_bytes, output_bytes = size(temporary), size(repo / 'docs/mcr/evidence/reference-chrome-v2')
                report['peakProcesses'] = max(report['peakProcesses'], count)
                report['peakRssKiB'] = max(report['peakRssKiB'], rss)
                report['peakWholeBytes'] = max(report['peakWholeBytes'], whole)
                report['peakTempBytes'] = max(report['peakTempBytes'], temp_bytes)
                report['peakOutputBytes'] = max(report['peakOutputBytes'], output_bytes)
                assert count <= 24 and rss <= 4096 * 1024, 'Whole descendant process/RSS cap'
                assert whole <= 2 * 1024**3 and temp_bytes <= 200 * 1024**2 and output_bytes <= 16 * 1024**2, 'Whole/temp/output cap'
                assert sum(f.stat().st_size for f in (repo / 'docs/mcr/evidence/reference-chrome-v2').rglob('*.log')) <= 2 * 1024**2, '2MiB complete log cap'
                pngs = list((repo / 'docs/mcr/evidence/reference-chrome-v2').rglob('*.png'))
                assert len(pngs) <= 78 and sum(f.stat().st_size for f in pngs) <= 12 * 1024**2, 'Historical plus new PNG count/bytes cap'
                time.sleep(.5)
            record.update(exitCode=child.returncode, finishedEpoch=time.time())
            assert child.returncode == 0, label + ' failed; retained log/report'
    report['status'] = 'PASS'
except BaseException as error:
    report['error'] = str(error)
    report['status'] = 'FAIL'
    raise
finally:
    reap()
    if link.is_symlink() and link.resolve() == dependencies.resolve():
        link.unlink()
    shutil.rmtree(temporary)
    rows = processes()
    report['remainingOwnDescendants'] = [pid for pid, digest in seen.items() if pid != os.getpid() and pid in rows and rows[pid]['commandHash'] == digest]
    report['finishedEpoch'] = time.time()
    report['cleanup'] = {'linkAbsent': not link.is_symlink(), 'tempAbsent': not temporary.exists(), 'beforeOriginalDeadline': time.time() < a.deadline_epoch + 60}
    if report['remainingOwnDescendants'] or not all(report['cleanup'].values()):
        report['status'] = 'FAIL'
    (out / 'guardian.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'peakProcesses': report['peakProcesses'], 'peakRssKiB': report['peakRssKiB'], 'cleanup': report['cleanup'], 'remainingOwnDescendants': report['remainingOwnDescendants']}))
