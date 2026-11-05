"""PI06 bounded native round with two live developer-owned guards."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import tempfile
import time
from guard_support import inventory, processes, sample, violation, publish_receipt

parser = argparse.ArgumentParser()
parser.add_argument('--job', required=True)
parser.add_argument('--candidate', required=True)
parser.add_argument('--accepted-epoch', required=True, type=float)
parser.add_argument('--other-new-bytes', required=True, type=int)
parser.add_argument('--output', required=True, type=Path)
a = parser.parse_args()
repo = Path(__file__).resolve().parents[5]
frontend = repo / 'frontend'
node = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
dependencies = frontend / 'node_modules'
link = frontend / 'node_modules'
out = a.output.resolve()
common_git = Path(subprocess.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=repo, text=True).strip()).resolve()
roots = [repo, repo.parent / 'fe-ref-public', repo.parent / 'fe-social-feed',
         Path('/Users/lhs/Desktop/mcr/frontend/node_modules'), node.parent.parent, common_git,
         Path('/Library/Frameworks/Python.framework'), Path('/Applications/Google Chrome.app')]
limits = {'workSeconds': 240, 'cleanupSeconds': 60, 'processes': 20, 'rssMiB': 4096,
          'wholeBytes': 8 * 1024**3, 'newGrowthBytes': 256 * 1024**2,
          'tempBytes': 192 * 1024**2, 'durableBytes': 24 * 1024**2, 'logBytes': 2 * 1024**2}
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip()
assert head == a.candidate
assert 0 <= time.time() - a.accepted_epoch < 15
assert out.is_relative_to(repo / 'docs/mcr/evidence/parade-ink-profile') and not out.exists()
assert link.is_dir() and not link.is_symlink()
assert (link / '.pnpm/lock.yaml').read_bytes() == (frontend / 'pnpm-lock.yaml').read_bytes()
free = shutil.disk_usage(repo).free
assert free >= 12 * 1024**3
assert free - a.other_new_bytes - limits['newGrowthBytes'] >= 9 * 1024**3
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (frontend / name).read_bytes() == (dependencies.parent / name).read_bytes()
initial = inventory(roots)
prior_whole_max = 6109081600
charged_initial = max(initial['allocatedBytes'], prior_whole_max)
assert charged_initial + limits['newGrowthBytes'] <= limits['wholeBytes']
source_paths = [frontend / p for p in ['src/features/me/ui/MePages.tsx', 'src/features/me/ui/me.css',
    'src/features/me/ui/reference-profile/ProfileActivity.tsx', 'src/features/profile/PublicProfile.tsx',
    'src/features/profile/profile.css', 'src/features/profile/reference-presentation/activity.css',
    'src/app/social-composition/panels.tsx', 'src/features/settings/SettingsPage.tsx', 'src/features/settings/parade-ink-settings.css']]
source_paths += [*sorted(Path(__file__).parent.glob('*')), frontend / 'package.json', frontend / 'pnpm-lock.yaml', node, Path(os.sys.executable).resolve(),
                 Path('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome')]
initial_ledger = initial.pop('ledger')
report = {'job': a.job, 'status': 'IN_PROGRESS', 'candidate': head, 'acceptedEpoch': a.accepted_epoch,
          'workDeadline': a.accepted_epoch + 240, 'cleanupDeadline': a.accepted_epoch + 300,
          'limits': limits, 'controller': {'pid': os.getpid(), 'parentPid': os.getppid(), 'processGroup': os.getpgrp(),
          'birth': subprocess.check_output(['ps', '-p', str(os.getpid()), '-o', 'lstart='], text=True).strip(),
          'path': str(Path(__file__).resolve()), 'outputRoot': str(out)}, 'admission': {'freeBytes': free, 'otherNewBytes': a.other_new_bytes,
          'ownNewBytes': limits['newGrowthBytes'], 'residualBytes': free - a.other_new_bytes - limits['newGrowthBytes']},
          'initialInventory': initial, 'priorHistoricalMaxBytes': prior_whole_max, 'chargedInitialWholeBytes': charged_initial,
          'sourceSha256': {str(path.relative_to(repo)) if path.is_relative_to(repo) else str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_paths if path.is_file()},
          'commands': [], 'peakProcesses': 0, 'peakRssKiB': 0, 'peakWholeBytes': charged_initial,
          'peakTempBytes': 0, 'peakOutputBytes': 0, 'peakPositiveGrowthBytes': 0}
out.mkdir(parents=True)
(out / 'accepted.json').write_text(json.dumps({key: report[key] for key in ['job','status','candidate','acceptedEpoch','workDeadline','cleanupDeadline','limits','controller','admission']}, indent=2) + '\n')
publish_receipt(report, 'ACCEPTED')
temporary = Path(tempfile.mkdtemp(prefix='mcr-pi06-native-', dir='/private/tmp'))
config = {'repo': str(repo), 'rootPid': os.getpid(), 'roots': [*map(str, roots), str(temporary)],
          'output': str(out), 'temporary': str(temporary), 'limits': limits,
          'initialWholeBytes': charged_initial, 'priorHistoricalMaxBytes': prior_whole_max, 'initialLedger': initial_ledger, 'workDeadline': report['workDeadline'], 'cleanupDeadline': report['cleanupDeadline']}
(temporary / 'config.json').write_text(json.dumps(config))
env = {**os.environ, 'PATH': str(node.parent) + os.pathsep + os.environ['PATH'], 'TMPDIR': str(temporary),
       'MCR_PI06_TMP': str(temporary), 'NO_COLOR': '1',
       'MCR_PI06_OUTPUT': str(out / 'screens'), 'MCR_PI06_SOURCE': a.candidate, 'MCR_PI06_JOB': a.job, 'MCR_PI06_PORT': '4196'}
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
    report['peakWholeBytes'] = max(report['peakWholeBytes'], value['chargedWholeBytes'])
    report['peakPositiveGrowthBytes'] = max(report['peakPositiveGrowthBytes'], value['positiveGrowthBytes'])
    report['peakTempBytes'] = max(report['peakTempBytes'], value['tempBytes'])
    report['peakOutputBytes'] = max(report['peakOutputBytes'], value['durableBytes'])
    problem = violation(value, config)
    assert not problem, problem
    assert time.time() < config['workDeadline'], 'Original work deadline'
    assert secondary is not None and secondary.poll() is None, 'Independent secondary must stay LIVE'

try:
    with (out / 'secondary.log').open('wb') as log:
        secondary = subprocess.Popen([os.sys.executable, '-B', str(Path(__file__).with_name('watchdog.py')), str(temporary / 'config.json')], stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    publish_receipt(report, 'ACCEPTED', secondary)
    while not (temporary / 'secondary-ready').exists():
        assert secondary.poll() is None and time.time() < config['workDeadline'], 'Secondary readiness'
        time.sleep(.1)
    observe()
    report['bothGuardsLiveBeforeWorkload'] = True
    report['initialGlobalCensus'] = sample(config)['global']
    commands = [('driver-syntax', [str(node), '--check', str(Path(__file__).with_name('native.mjs'))]),
                ('frozen-source-syntax', [str(node), '--check', str(Path(__file__).with_name('frozen-source.mjs'))]),
                ('changed-driver-lint', [str(dependencies / '.bin/oxlint'), '--deny-warnings', str(Path(__file__).with_name('native.mjs')), str(Path(__file__).with_name('frozen-source.mjs'))]),
                ('native', [str(node), str(Path(__file__).with_name('native.mjs'))])]
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
    shutil.rmtree(temporary)
    def recorded_descendants():
        return [row for row in processes(None) if row['pid'] != os.getpid() and row['pid'] in seen and row['commandHash'] == seen[row['pid']]]
    detached = recorded_descendants()
    report['recordedChildrenBeforeFinalCleanup'] = detached
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        for row in recorded_descendants():
            try:
                os.kill(row['pid'], sig)
            except ProcessLookupError:
                pass
        if not recorded_descendants():
            break
        time.sleep(.25)
    remaining = recorded_descendants()
    report['remainingOwnDescendants'] = remaining
    report['finishedEpoch'] = time.time()
    port_absent = False
    try:
        with socket.socket() as probe:
            probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            probe.bind(('127.0.0.1', 4196))
            port_absent = True
    except OSError:
        pass
    report['cleanup'] = {'strictPort4196Absent': port_absent, ** {'retainedDependenciesUnchanged': link.is_dir() and not link.is_symlink() and (link / '.pnpm/lock.yaml').read_bytes() == (frontend / 'pnpm-lock.yaml').read_bytes(), 'tempAbsent': not temporary.exists(),
                         'secondaryExited': secondary is None or secondary.poll() is not None,
                         'beforeOriginalDeadline': time.time() < config['cleanupDeadline']}}
    if remaining or not all(report['cleanup'].values()):
        report['status'] = 'FAIL'
    (out / 'guardian.json').write_text(json.dumps(report, indent=2) + '\n')
    (out / 'full.json').write_text(json.dumps({'job': a.job, 'status': 'FULL' if not remaining and all(report['cleanup'].values()) else 'RESIDUAL', 'resourcesReleased': not remaining and all(report['cleanup'].values()), 'qualification': report['status'], 'acceptedEpoch': a.accepted_epoch, 'finishedEpoch': report['finishedEpoch'], 'controller': report['controller'], 'remainingOwnDescendants': remaining, 'cleanup': report['cleanup']}, indent=2) + '\n')
    if not remaining and all(report['cleanup'].values()):
        publish_receipt(report, 'FULL')
    print(json.dumps({'status': report['status'], 'peakProcesses': report['peakProcesses'], 'peakRssKiB': report['peakRssKiB'], 'peakWholeBytes': report['peakWholeBytes'], 'cleanup': report['cleanup'], 'remainingOwnDescendants': remaining}))
