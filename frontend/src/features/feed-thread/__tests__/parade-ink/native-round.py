"""PI02/03 default-App native round; primary/secondary own all children and fixed clocks."""
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
parser.add_argument('--port', required=True, type=int)
parser.add_argument('--foundation-commit', required=True)
a = parser.parse_args()
repo = Path(__file__).resolve().parents[6]
frontend = repo / 'frontend'
node = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
dependencies = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
link = frontend / 'node_modules'
out = a.output.resolve()
common_git = Path(subprocess.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=repo, text=True).strip()).resolve()
roots = [repo, repo.parent / 'fe-mural-editor', repo.parent / 'fe-wall-reference-target',
         repo.parent / 'be-party-runtime', dependencies, node.parent.parent, common_git, Path('/Applications/Google Chrome.app'), Path(os.sys.prefix)]
limits = {'workSeconds': 300, 'cleanupSeconds': 60, 'processes': 24, 'rssMiB': 4096,
          'wholeBytes': 8 * 1024**3, 'newGrowthBytes': 256 * 1024**2,
          'tempBytes': 128 * 1024**2, 'durableBytes': 16 * 1024**2, 'logBytes': 2 * 1024**2}
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip()
assert head == a.candidate
assert 0 <= time.time() - a.accepted_epoch < 15
assert out.is_relative_to(repo / 'docs/mcr/evidence/parade-ink-feed-thread') and not out.exists()
assert not link.exists() and not link.is_symlink()
free = shutil.disk_usage(repo).free
assert free >= 12 * 1024**3
assert free - a.other_new_bytes - limits['newGrowthBytes'] >= 9 * 1024**3
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (frontend / name).read_bytes() == (dependencies.parent / name).read_bytes()
initial = inventory(roots)
prior_maximum = 6115184640  # Immutable native06 peak; never reset historical accounting.
assert max(initial['allocatedBytes'], prior_maximum) + limits['newGrowthBytes'] <= limits['wholeBytes']
source_paths = [frontend / 'src/features/event-feed/EventFeed.tsx', frontend / 'src/features/event-feed/feed.css',
                frontend / 'src/features/feed-thread/FeedThread.tsx',
                *sorted((frontend / 'src/features/event-feed/parade-ink').glob('*')),
                *sorted((frontend / 'src/features/event-feed/__tests__/parade-ink').glob('*')), *sorted(Path(__file__).parent.glob('*')),
                frontend / 'package.json', frontend / 'pnpm-lock.yaml', node, Path(os.sys.executable).resolve()]
report = {'job': a.job, 'status': 'IN_PROGRESS', 'candidate': head, 'acceptedEpoch': a.accepted_epoch,
          'workDeadline': a.accepted_epoch + 300, 'cleanupDeadline': a.accepted_epoch + 360,
          'foundationCommit': a.foundation_commit, 'port': a.port, 'limits': limits, 'admission': {'freeBytes': free, 'otherNewBytes': a.other_new_bytes,
          'ownNewBytes': limits['newGrowthBytes'], 'residualBytes': free - a.other_new_bytes - limits['newGrowthBytes']},
          'initialInventory': initial, 'priorHistoricalMaxBytes': prior_maximum,
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
child = secondary = server = None
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
    readset_bytes = subprocess.check_output(['git', 'show', a.foundation_commit + ':docs/mcr/evidence/parade-ink-foundation/qualification-readset.json'], cwd=repo)
    assert hashlib.sha256(readset_bytes).hexdigest() == '9630cc0b8b487b6a6348612dbadc27be868729f5e5189f61c5a559b31558c2c9', 'Explicit corrected PI01 read-set'
    readset = json.loads(readset_bytes)
    for row in readset['readOnlyCompatibility']:
        assert hashlib.sha256((repo / row['path']).read_bytes()).hexdigest() == row['sha256'], 'PI01 read-only compatibility pin'
    pins = []
    for row in readset['implementation']:
        data = subprocess.check_output(['git', 'show', a.foundation_commit + ':' + row['path']], cwd=repo)
        assert hashlib.sha256(data).hexdigest() == row['sha256'], 'Frozen PI01 postimage'
        path = temporary / 'foundation' / row['path']
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        pins.append({**row, 'file': str(path)})
    manifest = temporary / 'foundation-inputs.json'
    manifest.write_text(json.dumps(pins))
    report['foundationPins'] = pins
    report['foundationReadsetSha256'] = hashlib.sha256(readset_bytes).hexdigest()
    retained_guard = json.loads((repo / 'docs/mcr/evidence/parade-ink-feed-thread/native-06/guardian.json').read_text())
    retained_report = repo / 'docs/mcr/evidence/parade-ink-feed-thread/native-06/capture/report.json'
    for relative in ['frontend/src/features/event-feed/EventFeed.tsx',
                     'frontend/src/features/feed-thread/FeedThread.tsx', 'frontend/src/features/event-feed/parade-ink/AuthorHeading.tsx',
                     'frontend/src/features/event-feed/__tests__/parade-ink/fixture.ts', 'frontend/src/features/event-feed/__tests__/parade-ink/preview.tsx']:
        assert hashlib.sha256((repo / relative).read_bytes()).hexdigest() == retained_guard['sourceSha256'][relative], 'Retained native06 production/fixture pin'
    css_relative = 'frontend/src/features/event-feed/feed.css'
    prior_css = subprocess.check_output(['git', 'show', '3657ad819167fb202e1766048a78e8c38688b8d2:' + css_relative], cwd=repo, text=True)
    current_css = (repo / css_relative).read_text()
    expected_css = prior_css.replace('grid-template-columns: var(--pi-hit) minmax(0, 1fr) var(--pi-hit);',
                                    'grid-template-columns: minmax(var(--pi-hit), 1.2fr) minmax(0, 62%) minmax(var(--pi-hit), 1fr);')
    assert current_css == expected_css and current_css != prior_css, 'Exact footer grid-only production delta'
    native07_report = repo / 'docs/mcr/evidence/parade-ink-feed-thread/native-07/capture/report.json'
    assert json.loads(native07_report.read_text())['status'] == 'PASS'
    report['retainedNative07ReportSha256'] = hashlib.sha256(native07_report.read_bytes()).hexdigest()
    report['presentationDelta'] = {'path': css_relative, 'onlyChange': 'Footer grid columns match primary reference width',
                                   'priorSha256': hashlib.sha256(prior_css.encode()).hexdigest(),
                                   'currentSha256': hashlib.sha256(current_css.encode()).hexdigest()}
    env['MCR_PI_FOOTER_ONLY'] = '1'
    report['retainedNativeReportSha256'] = hashlib.sha256(retained_report.read_bytes()).hexdigest()
    env['MCR_PI_RETAINED_NATIVE_REPORT'] = str(retained_report)
    env.update(MCR_PI_FOUNDATION_INPUTS=str(manifest), MCR_PI_CANDIDATE=head,
               MCR_PI_ORIGIN=f'http://127.0.0.1:{a.port}/', MCR_PI_OUTPUT=str(out / 'capture'), MCR_PI_ALLOCATION=a.job)
    commands = [
        ('fixture-lint', [str(dependencies / '.bin/oxlint'), '--deny-warnings', 'src/features/event-feed/__tests__/parade-ink']),
        ('driver-syntax', [str(node), '--check', 'src/features/event-feed/__tests__/parade-ink/native.mjs']),
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
    observe()
    with (out / 'vite.log').open('wb') as log:
        server = subprocess.Popen([str(node), str(dependencies / 'vite/bin/vite.js'), '--config', 'src/features/event-feed/__tests__/parade-ink/vite.config.ts', '--configLoader', 'runner', '--host', '127.0.0.1', '--port', str(a.port), '--strictPort'], cwd=frontend, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    import urllib.request
    while True:
        assert server.poll() is None, 'Private Vite exited'
        observe()
        try:
            with urllib.request.urlopen(env['MCR_PI_ORIGIN'], timeout=.5) as response:
                if response.status == 200:
                    break
        except Exception:
            time.sleep(.1)
    command = [str(node), 'src/features/event-feed/__tests__/parade-ink/native.mjs']
    record = {'label': 'native', 'command': command, 'startedEpoch': time.time()}
    report['commands'].append(record)
    with (out / 'native.log').open('wb') as log:
        child = subprocess.Popen(command, cwd=frontend, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        while child.poll() is None:
            observe()
            pngs = list(out.rglob('*.png'))
            assert len(pngs) <= 4 and sum(path.stat().st_size for path in pngs) <= 8 * 1024**2, 'PNG count/byte cap'
            time.sleep(.25)
        record.update(exitCode=child.returncode, finishedEpoch=time.time())
        assert child.returncode == 0, 'Native failed; retained report/log'
    report['status'] = 'PASS'
except BaseException as error:
    report['status'] = 'FAIL'
    report['error'] = str(error)
    raise
finally:
    signal.signal(signal.SIGUSR1, signal.SIG_IGN)
    (temporary / 'cleaning').touch()
    for process in [child, server]:
        if process is not None and process.poll() is None:
            for sig in [signal.SIGTERM, signal.SIGKILL]:
                try:
                    os.killpg(process.pid, sig)
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    pass
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        for row in processes(1):
            if row['pid'] in seen and row['pid'] not in [os.getpid(), secondary.pid if secondary else None] and row['commandHash'] == seen[row['pid']]:
                try:
                    os.kill(row['pid'], sig)
                except ProcessLookupError:
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
    remaining = [row for row in processes(1) if row['pid'] != os.getpid() and row['pid'] in seen and row['commandHash'] == seen[row['pid']]]
    report['remainingOwnDescendants'] = remaining
    import socket
    port_absent = False
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind(('127.0.0.1', a.port)); port_absent = True
        except OSError:
            pass
    report['finishedEpoch'] = time.time()
    report['cleanup'] = {'linkAbsent': not link.is_symlink(), 'tempAbsent': not temporary.exists(),
                         'secondaryExited': secondary is None or secondary.poll() is not None, 'portAbsent': port_absent,
                         'beforeOriginalDeadline': time.time() < config['cleanupDeadline']}
    if remaining or not all(report['cleanup'].values()):
        report['status'] = 'FAIL'
    (out / 'guardian.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'peakProcesses': report['peakProcesses'], 'peakRssKiB': report['peakRssKiB'], 'peakWholeBytes': report['peakWholeBytes'], 'cleanup': report['cleanup'], 'remainingOwnDescendants': remaining}))
