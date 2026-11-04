"""Run only the admitted, bounded chrome source cohort; no install/build/browser."""
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

parser = argparse.ArgumentParser()
parser.add_argument('--job', required=True)
parser.add_argument('--dependencies', type=Path, required=True)
parser.add_argument('--node', type=Path, required=True)
parser.add_argument('--deadline-epoch', type=float, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[4]
frontend = repo / 'frontend'
output = args.output.resolve()
assert output.is_relative_to(repo / 'docs/mcr/evidence/reference-chrome-v2')
assert not output.exists()
dependencies = args.dependencies.resolve()
link = frontend / 'node_modules'
assert not link.exists() and not link.is_symlink(), 'Absent-only dependency link'
assert dependencies.is_dir() and args.node.is_file()
assert 0 < args.deadline_epoch - time.time() <= 120, 'Fresh <=120-second admitted work deadline'
cleanup_deadline = args.deadline_epoch + 60
assert shutil.disk_usage(repo).free >= 16 * 1024**3, '16GiB start floor'
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (frontend / name).read_bytes() == (args.dependencies.parent / name).read_bytes(), name
node_version = subprocess.check_output([str(args.node), '--version'], text=True).strip()
assert node_version == 'v24.22.2', 'Use the retained pinned runtime; do not install or silently substitute'
tests = [
    'src/app/reference-chrome-v2/chrome.test.tsx',
    'src/app/__tests__/reference-frame-footer/footer.test.tsx',
    'src/app/__tests__/reference-root/root.test.tsx',
    'src/features/public/__tests__/reference-target/presentation.test.tsx',
    'src/shared/ui/__tests__/primitives.test.tsx',
]
lint = [
    'src/app/WorldShell.tsx', 'src/app/shell-chrome/index.tsx',
    'src/app/shell-chrome/parade-footer.tsx', 'src/features/public/ui/PublicFrame.tsx',
    'src/shared/ui/concert-window/index.tsx', 'src/app/reference-chrome-v2',
    'src/app/__tests__/reference-frame-footer/footer.test.tsx',
]
commands = [
    ('tests', [str(args.node), str(dependencies / 'vitest/vitest.mjs'), 'run', '--config', 'src/app/reference-chrome-v2/source.vitest.config.ts', '--configLoader', 'runner', '--maxWorkers=1', '--no-file-parallelism', *tests]),
    ('types', [str(args.node), str(dependencies / 'typescript/bin/tsc'), '-p', 'tsconfig.json', '--noEmit', '--incremental', 'false']),
    ('lint', [str(dependencies / '.bin/oxlint'), *lint]),
    ('native-syntax', [str(args.node), '--check', 'src/app/reference-chrome-v2/native.mjs']),
]
carry_roots = [repo, repo.parent / 'fe-mural-editor', repo.parent / 'fe-wall-reference-target', repo.parent / 'be-party-runtime']
report = {'job': args.job, 'status': 'IN_PROGRESS', 'deadlineEpoch': args.deadline_epoch, 'cleanupDeadlineEpoch': cleanup_deadline,
          'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
          'limits': {'workSeconds': 120, 'cleanupSeconds': 60, 'processes': 12, 'rssMiB': 3072, 'wholeBytes': 2 * 1024**3, 'outputBytes': 2 * 1024**2, 'tempBytes': 200 * 1024**2, 'runtimeFreeGiB': 8},
          'dependencyMode': 'Absent-only temporary link; shared dependency directory is read-only',
          'node': str(args.node), 'nodeVersion': node_version, 'commands': [], 'peakProcesses': 0, 'peakRssKiB': 0}
output.mkdir(parents=True)
temporary = Path(tempfile.mkdtemp(prefix='mcr-reference-chrome-v2-', dir='/private/tmp'))
environment = {**os.environ, 'TMPDIR': str(temporary), 'MCR_CHROME_CACHE': str(temporary / 'vite'),
               'PATH': str(args.node.parent) + os.pathsep + os.environ['PATH'], 'CI': '1', 'NO_COLOR': '1'}
process = None

def bytes_in(path):
    return sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink())

def whole_bytes():
    rows = subprocess.check_output(['du', '-sk', *map(str, carry_roots), str(temporary)], text=True)
    return sum(int(row.split()[0]) * 1024 for row in rows.splitlines())

def terminate():
    if process is not None and process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=2)

try:
    report['carryRoots'] = list(map(str, carry_roots))
    report['startWholeBytes'] = whole_bytes()
    assert report['startWholeBytes'] <= 2 * 1024**3, '2GiB whole carry cap'
    link.symlink_to(args.dependencies, target_is_directory=True)
    for label, command in commands:
        started = time.time()
        entry = {'label': label, 'command': command, 'startedEpoch': started}
        report['commands'].append(entry)
        with (output / (label + '.log')).open('wb') as log:
            process = subprocess.Popen(command, cwd=frontend, env=environment, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            while process.poll() is None:
                assert time.time() < args.deadline_epoch, 'Admitted deadline expired'
                assert shutil.disk_usage(repo).free >= 8 * 1024**3, '8GiB runtime floor'
                assert bytes_in(output) <= 2 * 1024**2, '2MiB source output cap'
                assert bytes_in(temporary) <= 200 * 1024**2, '200MiB private temp cap'
                assert whole_bytes() <= 2 * 1024**3, '2GiB whole carry cap'
                rows = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True)
                group = [list(map(int, row.split())) for row in rows.splitlines() if len(row.split()) == 4 and (int(row.split()[2]) == process.pid or int(row.split()[0]) == os.getpid() or int(row.split()[1]) == os.getpid())]
                count, rss = len(group), sum(row[3] for row in group)
                report['peakProcesses'] = max(report['peakProcesses'], count)
                report['peakRssKiB'] = max(report['peakRssKiB'], rss)
                assert count <= 12 and rss <= 3072 * 1024, 'Process/RSS ceiling'
                time.sleep(.5)
            entry.update(exitCode=process.returncode, finishedEpoch=time.time())
            assert process.returncode == 0, label + ' failed; see retained log'
    report['status'] = 'PASS'
except BaseException as error:
    report['status'] = 'FAIL'
    report['error'] = str(error)
    terminate()
    raise
finally:
    terminate()
    if link.is_symlink() and link.resolve() == dependencies:
        link.unlink()
    shutil.rmtree(temporary)
    report['finishedEpoch'] = time.time()
    report['cleanupWithinDeadline'] = report['finishedEpoch'] < cleanup_deadline
    if not report['cleanupWithinDeadline']:
        report['status'] = 'FAIL'
        report['error'] = 'Cleanup deadline exceeded'
    report['cleanup'] = {'ownDependencyLinkRemoved': not link.is_symlink(), 'privateTempRemoved': not temporary.exists(), 'childStopped': process is None or process.poll() is not None}
    report['logs'] = {p.name: {'bytes': p.stat().st_size, 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in output.glob('*.log')}
    (output / 'source-report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'peakProcesses': report['peakProcesses'], 'peakRssKiB': report['peakRssKiB'], 'cleanup': report['cleanup']}))
