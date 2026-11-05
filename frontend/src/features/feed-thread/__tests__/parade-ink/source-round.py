"""PI02/03 serial source round with two live developer-owned guards."""
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
from guard_support import inventory, processes, sample, violation

parser = argparse.ArgumentParser()
parser.add_argument('--job', required=True)
parser.add_argument('--candidate', required=True)
parser.add_argument('--accepted-epoch', required=True, type=float)
parser.add_argument('--other-new-bytes', required=True, type=int)
parser.add_argument('--output', required=True, type=Path)
a = parser.parse_args()
repo = Path(__file__).resolve().parents[6]
frontend = repo / 'frontend'
node = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
dependencies = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
link = frontend / 'node_modules'
out = a.output.resolve()
common_git = Path(subprocess.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=repo, text=True).strip()).resolve()
roots = [repo, repo.parent / 'fe-mural-editor', repo.parent / 'fe-wall-reference-target',
         repo.parent / 'be-party-runtime', dependencies, node.parent.parent, common_git]
limits = {'workSeconds': 120, 'cleanupSeconds': 60, 'processes': 6, 'rssMiB': 2560,
          'wholeBytes': 5 * 1024**3, 'newGrowthBytes': 128 * 1024**2,
          'tempBytes': 100 * 1024**2, 'durableBytes': 16 * 1024**2, 'logBytes': 2 * 1024**2}
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip()
assert head == a.candidate
assert 0 <= time.time() - a.accepted_epoch < 15
assert out.is_relative_to(repo / 'docs/mcr/evidence/parade-ink-feed-thread') and not out.exists()
assert not link.exists() and not link.is_symlink()
free = shutil.disk_usage(repo).free
assert free >= 11 * 1024**3
assert free - a.other_new_bytes - limits['newGrowthBytes'] >= 9 * 1024**3
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (frontend / name).read_bytes() == (dependencies.parent / name).read_bytes()
initial = inventory(roots)
assert initial['allocatedBytes'] + limits['newGrowthBytes'] <= limits['wholeBytes']
source_paths = [frontend / 'src/features/event-feed/EventFeed.tsx', frontend / 'src/features/event-feed/feed.css',
                frontend / 'src/features/feed-thread/FeedThread.tsx',
                *sorted((frontend / 'src/features/event-feed/parade-ink').glob('*')), *sorted(Path(__file__).parent.glob('*')),
                frontend / 'package.json', frontend / 'pnpm-lock.yaml', node]
report = {'job': a.job, 'status': 'IN_PROGRESS', 'candidate': head, 'acceptedEpoch': a.accepted_epoch,
          'workDeadline': a.accepted_epoch + 120, 'cleanupDeadline': a.accepted_epoch + 180,
          'limits': limits, 'admission': {'freeBytes': free, 'otherNewBytes': a.other_new_bytes,
          'ownNewBytes': limits['newGrowthBytes'], 'residualBytes': free - a.other_new_bytes - limits['newGrowthBytes']},
          'initialInventory': initial, 'priorHistoricalMaxBytes': 2027491328,
          'sourceSha256': {str(path.relative_to(repo)) if path.is_relative_to(repo) else str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_paths if path.is_file()},
          'commands': [], 'peakProcesses': 0, 'peakRssKiB': 0, 'peakWholeBytes': initial['allocatedBytes'],
          'peakTempBytes': 0, 'peakOutputBytes': 0}
out.mkdir(parents=True)
temporary = Path(tempfile.mkdtemp(prefix='mcr-parade-ink-feed-thread-', dir='/private/tmp'))
config = {'repo': str(repo), 'rootPid': os.getpid(), 'roots': [*map(str, roots), str(temporary)],
          'output': str(out), 'temporary': str(temporary), 'limits': limits,
          'initialWholeBytes': initial['allocatedBytes'], 'workDeadline': report['workDeadline'], 'cleanupDeadline': report['cleanupDeadline']}
(temporary / 'config.json').write_text(json.dumps(config))
env = {**os.environ, 'PATH': str(node.parent) + os.pathsep + os.environ['PATH'], 'TMPDIR': str(temporary),
       'MCR_PARADE_FEED_TMP': str(temporary), 'NO_COLOR': '1'}
child = secondary = None
seen = {}

def interrupted(_signal, _frame):
    raise RuntimeError('Independent secondary guard requested fail-closed cleanup')

signal.signal(signal.SIGUSR1, interrupted)
signal.signal(signal.SIGTERM, interrupted)


def observe():
    value = sample(config)
    for row in value['processes']:
        seen[row['pid']] = row['commandHash']
    report['peakProcesses'] = max(report['peakProcesses'], len(value['processes']))
    report['peakRssKiB'] = max(report['peakRssKiB'], value['rssKiB'])
    report['peakWholeBytes'] = max(report['peakWholeBytes'], value['whole']['allocatedBytes'])
    report['peakTempBytes'] = max(report['peakTempBytes'], value['tempBytes'])
    report['peakOutputBytes'] = max(report['peakOutputBytes'], value['durableBytes'])
    problem = violation(value, config)
    assert not problem, problem
    assert time.time() < config['workDeadline'], 'Original work deadline'
    assert secondary is not None and secondary.poll() is None, 'Independent secondary must stay LIVE'

try:
    with (out / 'secondary.log').open('wb') as log:
        secondary = subprocess.Popen([os.sys.executable, '-B', str(Path(__file__).with_name('watchdog.py')), str(temporary / 'config.json')], stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    while not (temporary / 'secondary-ready').exists():
        assert secondary.poll() is None and time.time() < config['workDeadline'], 'Secondary readiness'
        time.sleep(.1)
    observe()
    report['bothGuardsLiveBeforeWorkload'] = True
    link.symlink_to(dependencies, target_is_directory=True)
    commands = [
        ('source-tests', [str(node), str(dependencies / 'vitest/vitest.mjs'), 'run', '--config', 'src/features/feed-thread/__tests__/parade-ink/vitest.config.mjs', '--configLoader', 'runner', '--maxWorkers=1', '--no-file-parallelism']),
        ('source-types', [str(node), str(dependencies / 'typescript/bin/tsc'), '-p', 'tsconfig.json', '--noEmit', '--incremental', 'false', '--allowImportingTsExtensions']),
        ('source-lint', [str(dependencies / '.bin/oxlint'), '--deny-warnings', 'src/features/event-feed/EventFeed.tsx', 'src/features/event-feed/parade-ink', 'src/features/feed-thread/FeedThread.tsx', 'src/features/feed-thread/__tests__/parade-ink']),
    ]
    for label, command in commands:
        observe()
        record = {'label': label, 'command': command, 'startedEpoch': time.time()}
        report['commands'].append(record)
        with (out / (label + '.log')).open('wb') as log:
            child = subprocess.Popen(command, cwd=frontend, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            while child.poll() is None:
                observe()
                time.sleep(.25)
            record.update(exitCode=child.returncode, finishedEpoch=time.time())
            assert child.returncode == 0, label + ' failed; retained log/report'
    report['status'] = 'PASS'
except BaseException as error:
    report['status'] = 'FAIL'
    report['error'] = str(error)
    raise
finally:
    signal.signal(signal.SIGUSR1, signal.SIG_IGN)
    (temporary / 'cleaning').touch()
    if child is not None and child.poll() is None:
        for sig in [signal.SIGTERM, signal.SIGKILL]:
            try:
                os.killpg(child.pid, sig)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass
    (temporary / 'done').touch()
    if secondary is not None:
        try:
            secondary.wait(timeout=min(10, max(.1, config['cleanupDeadline'] - time.time())))
        except subprocess.TimeoutExpired:
            secondary.kill(); secondary.wait(timeout=2)
            report['status'] = 'FAIL'
        secondary_report = out / 'secondary.json'
        if not secondary_report.exists() or any(key in json.loads(secondary_report.read_text()) for key in ['violation', 'error']):
            report['status'] = 'FAIL'
    if link.is_symlink() and link.resolve() == dependencies.resolve():
        link.unlink()
    shutil.rmtree(temporary)
    remaining = [row for row in processes(os.getpid()) if row['pid'] != os.getpid() and row['pid'] in seen and row['commandHash'] == seen[row['pid']]]
    report['remainingOwnDescendants'] = remaining
    report['finishedEpoch'] = time.time()
    report['cleanup'] = {'linkAbsent': not link.is_symlink(), 'tempAbsent': not temporary.exists(),
                         'secondaryExited': secondary is None or secondary.poll() is not None,
                         'beforeOriginalDeadline': time.time() < config['cleanupDeadline']}
    if remaining or not all(report['cleanup'].values()):
        report['status'] = 'FAIL'
    (out / 'guardian.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'peakProcesses': report['peakProcesses'], 'peakRssKiB': report['peakRssKiB'], 'peakWholeBytes': report['peakWholeBytes'], 'cleanup': report['cleanup'], 'remainingOwnDescendants': remaining}))
