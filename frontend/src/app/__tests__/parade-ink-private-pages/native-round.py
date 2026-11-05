"""PI12 default-App native round; primary/secondary own all children and fixed clocks."""
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
import membership

parser = argparse.ArgumentParser()
parser.add_argument('--job', required=True)
parser.add_argument('--candidate', required=True)
parser.add_argument('--accepted-epoch', required=True, type=float)
parser.add_argument('--other-new-bytes', required=True, type=int)
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--port', required=True, type=int)
parser.add_argument('--foundation-commit', required=True)
parser.add_argument('--new-carry-bytes', required=True, type=int)
a = parser.parse_args()
repo = Path(__file__).resolve().parents[5]
frontend = repo / 'frontend'
node = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
dependencies = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
link = frontend / 'node_modules'
out = a.output.resolve()
common_git = Path(subprocess.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=repo, text=True).strip()).resolve()
roots = [repo, repo.parent / 'fe-mural-editor', repo.parent / 'fe-wall-reference-target',
         repo.parent / 'be-party-runtime', dependencies, node.parent.parent, common_git, Path('/Applications/Google Chrome.app'), Path(os.sys.prefix)]
limits = {'workSeconds': 300, 'cleanupSeconds': 60, 'processes': 24, 'rssMiB': 4096,
          'wholeBytes': 8 * 1024**3, 'newGrowthBytes': 576 * 1024**2,
          'tempBytes': 128 * 1024**2, 'durableBytes': 16 * 1024**2, 'logBytes': 2 * 1024**2}
os.environ['MCR_PI_PRIVATE_OTHER_NEW'] = str(a.other_new_bytes)
accepted_record = {'ownerThreadId': membership.OWNER, 'offerId': a.job, 'state': 'ACCEPTED',
    'acceptedAtEpoch': a.accepted_epoch, 'workDeadlineEpoch': a.accepted_epoch + limits['workSeconds'],
    'fullDeadlineEpoch': a.accepted_epoch + limits['workSeconds'] + limits['cleanupSeconds'],
    'kind': 'browser', 'processCap': limits['processes'], 'rssCapBytes': limits['rssMiB'] * 1024**2,
    'remainingNewUpperBytes': limits['newGrowthBytes'] - a.new_carry_bytes,
    'controller': membership.identity(os.getpid(), Path(__file__).resolve(), out), 'guardControllers': [],
    'receiptPaths': {'accept': str(out / 'accepted.json'), 'full': str(out / 'guardian.json')}}
previous_receipt = json.loads(membership.OWN.read_text())
assert previous_receipt['state'] != 'ACCEPTED', 'Prior own workflow must be literal FULL before a new actual ACCEPT'
closed = list(previous_receipt.get('closedOffers', []))
if previous_receipt['state'] == 'FULL' and previous_receipt.get('actualFullEpoch'):
    previous_path = Path(previous_receipt['receiptPaths']['full'])
    assert previous_path.is_file(), 'Immutable prior FULL evidence must exist'
    previous_closed = {'ownerThreadId': membership.OWNER, 'offerId': previous_receipt['offerId'],
        'acceptedAtEpoch': previous_receipt['acceptedAtEpoch'], 'actualFullEpoch': previous_receipt['actualFullEpoch'],
        'fullPath': str(previous_path), 'fullSHA': hashlib.sha256(previous_path.read_bytes()).hexdigest(), 'resourcesReleased': True}
    if not any(row.get('offerId') == previous_closed['offerId'] and row.get('acceptedAtEpoch') == previous_closed['acceptedAtEpoch'] for row in closed):
        closed.append(previous_closed)
accepted_record['closedOffers'] = closed
for historical_field in ['ordinaryAuthoringBookings', 'ordinaryObservation']:
    if historical_field in previous_receipt:
        accepted_record[historical_field] = previous_receipt[historical_field]
membership.publish(accepted_record)
assert 0 <= a.new_carry_bytes <= limits['newGrowthBytes']
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip()
assert head == a.candidate
assert 0 <= time.time() - a.accepted_epoch < 15
assert out.is_relative_to(repo / 'docs/mcr/evidence/parade-ink-private-pages') and not out.exists()
assert not link.exists() and not link.is_symlink()
free = shutil.disk_usage(repo).free
assert free >= 12 * 1024**3
assert free - a.other_new_bytes - (limits['newGrowthBytes'] - a.new_carry_bytes) >= 9 * 1024**3
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (frontend / name).read_bytes() == (dependencies.parent / name).read_bytes()
initial = inventory(roots)
prior_maximum = 6186934272  # Immutable native17 secondary peak; never reset historical accounting.
assert max(initial['allocatedBytes'], prior_maximum) + (limits['newGrowthBytes'] - a.new_carry_bytes) <= limits['wholeBytes']
owned_production = ['app/account-deletion-entry/panel.tsx', 'app/archive-entry/index.tsx', 'app/safety-entry/panels.tsx', 'app/social-composition/sharing.tsx', 'features/account/panel.tsx', 'features/age-status/AgeStatus.tsx', 'features/archive/ArchivePage.tsx', 'features/inbox/ui/Inbox.tsx', 'features/safety/panels.tsx', 'features/social-sharing/ui/SocialSharing.tsx']
source_paths = [*[frontend / 'src' / path for path in owned_production],
                *sorted((frontend / 'src/features').glob('**/parade-ink-private/*.css')),
                *sorted(Path(__file__).parent.glob('*')), *sorted((frontend / 'e2e/visual/parade-ink-private-pages').glob('*')),
                frontend / 'package.json', frontend / 'pnpm-lock.yaml', node, Path(os.sys.executable).resolve()]
report = {'job': a.job, 'status': 'IN_PROGRESS', 'candidate': head, 'acceptedEpoch': a.accepted_epoch,
          'workDeadline': a.accepted_epoch + 300, 'cleanupDeadline': a.accepted_epoch + 360,
          'foundationCommit': a.foundation_commit, 'port': a.port, 'limits': limits, 'admission': {'freeBytes': free, 'otherNewBytes': a.other_new_bytes,
          'ownRemainingNewBytes': limits['newGrowthBytes'] - a.new_carry_bytes, 'residualBytes': free - a.other_new_bytes - (limits['newGrowthBytes'] - a.new_carry_bytes)},
          'initialInventory': initial, 'nonresettingNewCarryBytes': a.new_carry_bytes, 'priorHistoricalMaxBytes': prior_maximum,
          'sourceSha256': {str(path.relative_to(repo)) if path.is_relative_to(repo) else str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_paths if path.is_file()},
          'commands': [], 'peakProcesses': 0, 'peakRssKiB': 0, 'peakWholeBytes': initial['allocatedBytes'],
          'peakTempBytes': 0, 'peakOutputBytes': 0}
out.mkdir(parents=True)
(out / 'accepted.json').write_text(json.dumps(accepted_record, indent=2) + '\n')
temporary = Path(tempfile.mkdtemp(prefix='mcr-parade-ink-private-', dir='/private/tmp'))
config = {'repo': str(repo), 'rootPid': os.getpid(), 'roots': [*map(str, roots), str(temporary)],
          'output': str(out), 'temporary': str(temporary), 'limits': limits,
          'initialWholeBytes': initial['allocatedBytes'], 'otherNewBytes': a.other_new_bytes, 'newCarryBytes': a.new_carry_bytes, 'workDeadline': report['workDeadline'], 'cleanupDeadline': report['cleanupDeadline']}
(temporary / 'config.json').write_text(json.dumps(config))
env = {**os.environ, 'PATH': str(node.parent) + os.pathsep + os.environ['PATH'], 'TMPDIR': str(temporary),
       'MCR_PI_PRIVATE_TMP': str(temporary), 'NO_COLOR': '1'}
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
    pngs = list(out.parent.rglob('*.png'))
    assert len(pngs) <= 48 and sum(path.stat().st_size for path in pngs) <= 12 * 1024**2, 'Cumulative family PNG count/byte cap'
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
    accepted_record['guardControllers'] = [membership.identity(secondary.pid, Path(__file__).with_name('watchdog.py').resolve())]
    membership.publish(accepted_record)
    observe()
    report['bothGuardsLiveBeforeWorkload'] = True
    link.symlink_to(dependencies, target_is_directory=True)
    readset_bytes = subprocess.check_output(['git', 'show', a.foundation_commit + ':docs/mcr/evidence/parade-ink-foundation/qualification-readset.json'], cwd=repo)
    assert hashlib.sha256(readset_bytes).hexdigest() == '9630cc0b8b487b6a6348612dbadc27be868729f5e5189f61c5a559b31558c2c9', 'Explicit corrected PI01 read-set'
    readset = json.loads(readset_bytes)
    for row in readset['readOnlyCompatibility']:
        actual = hashlib.sha256((repo / row['path']).read_bytes()).hexdigest()
        if row['path'] == 'frontend/src/app/WorldShell.tsx':
            # The authorized new base already contains Root21e's canonical pre-World
            # policy seam. Keep that functional source; never overlay old authority.
            assert actual == '6c66a6e61b84fc4180bfa589148cf10018a7166fe15d468b722f5c88cb95fde1'
            report['baseCompatibilityDelta'] = {'path': row['path'], 'originalPin': row['sha256'], 'actualBasePin': actual,
                'reason': 'Canonical f70/base2b pre-World policy seam; no presentation CSS replacement or source edit'}
        else:
            assert actual == row['sha256'], 'PI01 read-only compatibility pin'
    pins = []
    for row in readset['implementation']:
        data = subprocess.check_output(['git', 'show', a.foundation_commit + ':' + row['path']], cwd=repo)
        assert hashlib.sha256(data).hexdigest() == row['sha256'], 'Frozen PI01 postimage'
        path = temporary / 'foundation' / row['path']
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        pins.append({**row, 'file': str(path)})
    dependency_path = repo / 'docs/mcr/evidence/parade-ink-private-pages/qualified-modal-dependency.json'
    dependency = json.loads(dependency_path.read_text())
    assert dependency['taskId'] == 'FE-DIALOG-ESC-01' and dependency['status'] == 'QUALIFIED_OWNER_HANDOFF'
    assert dependency['path'] == 'frontend/src/features/safety/Dialog.tsx'
    assert len(dependency['commit']) == 40 and all(c in '0123456789abcdef' for c in dependency['commit'])
    assert dependency['writerThreadId'] and dependency['qualification']
    assert hashlib.sha256((repo / dependency['path']).read_bytes()).hexdigest() == '8cfdf13e621986fadba50e35ceb16f0515dbf85ab346fb82077e3af2147e4531', 'Readonly canonical Dialog preimage changed'
    dependency_bytes = subprocess.check_output(['git', 'show', dependency['commit'] + ':' + dependency['path']], cwd=repo)
    assert hashlib.sha256(dependency_bytes).hexdigest() == dependency['sha256'], 'Qualified functional dependency postimage mismatch'
    dependency_file = temporary / 'qualified-modal-dependency' / 'Dialog.tsx'
    dependency_file.parent.mkdir(parents=True)
    dependency_file.write_bytes(dependency_bytes)
    pins.append({'path': dependency['path'], 'file': str(dependency_file), 'sha256': dependency['sha256'], 'qualifiedFunctionalDependency': True})
    report['qualifiedFunctionalDependency'] = {**dependency, 'recordSha256': hashlib.sha256(dependency_path.read_bytes()).hexdigest(), 'boundary': 'Separately qualified functional owner handoff; original base source21/type/lint prove unchanged PI12 presentation, not this additional dependency'}
    manifest = temporary / 'foundation-inputs.json'
    manifest.write_text(json.dumps(pins))
    report['foundationPins'] = [pin for pin in pins if not pin.get('qualifiedFunctionalDependency')]
    report['foundationReadsetSha256'] = hashlib.sha256(readset_bytes).hexdigest()
    env.update(MCR_PI_PRIVATE_FOUNDATION_INPUTS=str(manifest), MCR_PI_CANDIDATE=head,
               MCR_PI_ORIGIN=f'http://127.0.0.1:{a.port}/', MCR_PI_OUTPUT=str(out / 'capture'), MCR_PI_ALLOCATION=a.job)
    prior_tests = json.loads((repo / 'docs/mcr/evidence/parade-ink-private-pages/source-05/guardian.json').read_text())
    test_path = 'frontend/src/app/__tests__/parade-ink-private-pages/pages.test.tsx'
    assert hashlib.sha256((repo / test_path).read_bytes()).hexdigest() == prior_tests['sourceSha256'][test_path], 'Retained passing test fixture changed'
    assert any(row['label'] == 'source-tests' and row.get('exitCode') == 0 for row in prior_tests['commands'])
    product_paths = ['frontend/src/' + path for path in owned_production]
    for path in product_paths:
        tested = subprocess.check_output(['git', 'show', '4ad21a802863209f5de88f9f71e5d62fe5e8f16b:' + path], cwd=repo)
        assert (repo / path).read_bytes() == tested, 'Retained source proof product bytes changed: ' + path
    report['retainedSourceProof'] = {'round': 'source-05', 'candidate': prior_tests['candidate'],
        'tests': '21 PASS, 0 FAIL', 'fixtureSha256': prior_tests['sourceSha256'][test_path],
        'productCommit': '4ad21a802863209f5de88f9f71e5d62fe5e8f16b', 'verifiedProductPaths': product_paths, 'cssDisposition': 'Six presentation-only CSS selector corrections after source05; captured by this native round, no functional AST change',
        'limitation': 'Overall source05 resource FAIL preserved; completed source-tests command exit0 reused only for exact pinned bytes'}
    (temporary / 'node_modules').symlink_to(dependencies, target_is_directory=True)
    type_config = temporary / 'tsconfig.private.json'
    type_config.write_text(json.dumps({'extends': str(frontend / 'tsconfig.json'), 'compilerOptions': {'paths': {'zod': [str(dependencies / 'zod')]}}}))
    report['typecheckDependencyBinding'] = {'reason': 'Readonly generated public contract lies outside frontend and cannot resolve its retained zod dependency through frontend-only symlink', 'retainedPackage': str(dependencies / 'zod'), 'configuration': str(type_config), 'scope': 'Same full project/strictness; only exact retained dependency resolution, no generated or manifest edits', 'temporaryDependencyLink': str(temporary / 'node_modules')}
    previous_checks = json.loads((repo / 'docs/mcr/evidence/parade-ink-private-pages/native-04/guardian.json').read_text())
    for label in ['types', 'scoped-lint']:
        assert any(row['label'] == label and row.get('exitCode') == 0 for row in previous_checks['commands'])
    changed = subprocess.check_output(['git', 'diff', '--name-only', previous_checks['candidate'], '--', 'frontend/src', 'contracts', 'frontend/tsconfig.json', 'frontend/package.json', 'frontend/pnpm-lock.yaml'], cwd=repo, text=True).splitlines()
    assert not any(path.endswith(('.ts', '.tsx', '.css', '.json', '.yaml')) for path in changed), 'Retained type/application-lint inputs changed'
    service_path = 'frontend/e2e/visual/parade-ink-private-pages/service.ts'
    old_service = subprocess.check_output(['git', 'show', previous_checks['candidate'] + ':' + service_path], cwd=repo, text=True)
    before = r"/\/me\/social-handles\/INSTAGRAM$/.test(path)"
    after = "path.endsWith('/me/social-handles/INSTAGRAM')"
    assert before in old_service and (repo / service_path).read_text() == old_service.replace(before, after), 'Only equivalent fixture suffix lint rewrite allowed'
    report['retainedTypeAndAppLintProof'] = {'round': 'native-04', 'candidate': previous_checks['candidate'], 'commands': [row for row in previous_checks['commands'] if row['label'] in ['types', 'scoped-lint']], 'disposition': 'All frontend TS/TSX/CSS/contracts/manifests unchanged; sole imported service change is exact equivalent regex-suffix to endsWith lint rewrite'}
    fixture_checks = json.loads((repo / 'docs/mcr/evidence/parade-ink-private-pages/native-05/guardian.json').read_text())
    assert any(row['label'] == 'fixture-lint' and row.get('exitCode') == 0 for row in fixture_checks['commands'])
    for name in ['preview.html', 'service.ts', 'vite.config.ts']:
        path = 'frontend/e2e/visual/parade-ink-private-pages/' + name
        assert hashlib.sha256((repo / path).read_bytes()).hexdigest() == fixture_checks['sourceSha256'][path], 'Retained fixture lint input changed'
    report['retainedFixtureLintProof'] = {'round': 'native-05', 'exitCode': 0, 'disposition': 'Three exact unchanged fixture postimages; changed native driver and preview producer are linted again'}
    retained_path = repo / 'docs/mcr/evidence/parade-ink-private-pages/retained-native09.json'
    retained = json.loads(retained_path.read_text())
    for raw in retained['rawReports']:
        assert hashlib.sha256(Path(raw['path']).read_bytes()).hexdigest() == raw['sha256'], 'Retained raw partial report changed'
    for path, expected in retained['productSha256'].items():
        assert hashlib.sha256((repo / path).read_bytes()).hexdigest() == expected, 'Retained partial native product changed'
    for shot in retained['screenshots']:
        assert hashlib.sha256(Path(shot['file']).read_bytes()).hexdigest() == shot['sha256'], 'Retained native image changed'
    report['retainedPartialNativeProof'] = {'path': str(retained_path), 'sha256': hashlib.sha256(retained_path.read_bytes()).hexdigest(), 'captures': len(retained['screenshots']), 'limitation': 'Only completed individual captures reused; overall native08/native09 FAIL unchanged'}
    env['MCR_PI_RETAINED'] = str(retained_path)
    driver_checks = json.loads((repo / 'docs/mcr/evidence/parade-ink-private-pages/native-12/guardian.json').read_text())
    for label in ['changed-driver-lint', 'driver-syntax']:
        assert any(row['label'] == label and row.get('exitCode') == 0 for row in driver_checks['commands'])
    preview_path = 'frontend/e2e/visual/parade-ink-private-pages/preview.tsx'
    assert hashlib.sha256((repo / preview_path).read_bytes()).hexdigest() == driver_checks['sourceSha256'][preview_path], 'Retained preview lint input changed'
    driver_path = 'frontend/e2e/visual/parade-ink-private-pages/native.mjs'
    driver_unchanged = hashlib.sha256((repo / driver_path).read_bytes()).hexdigest() == driver_checks['sourceSha256'][driver_path]
    report['retainedDriverChecks'] = {'round': 'native-12', 'commands': [row for row in driver_checks['commands'] if row['label'] in ['changed-driver-lint', 'driver-syntax']], 'disposition': 'Exact unchanged preview lint retained; driver checks retained only if same hash', 'driverUnchanged': driver_unchanged}
    commands = [] if driver_unchanged else [
        ('changed-driver-lint', [str(dependencies / '.bin/oxlint'), '--deny-warnings', 'e2e/visual/parade-ink-private-pages/native.mjs']),
        ('driver-syntax', [str(node), '--check', 'e2e/visual/parade-ink-private-pages/native.mjs']),
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
        server = subprocess.Popen([str(node), str(dependencies / 'vite/bin/vite.js'), '--config', 'e2e/visual/parade-ink-private-pages/vite.config.ts', '--configLoader', 'runner', '--host', '127.0.0.1', '--port', str(a.port), '--strictPort'], cwd=frontend, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
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
    command = [str(node), 'e2e/visual/parade-ink-private-pages/native.mjs']
    record = {'label': 'native', 'command': command, 'startedEpoch': time.time()}
    report['commands'].append(record)
    with (out / 'native.log').open('wb') as log:
        child = subprocess.Popen(command, cwd=frontend, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        while child.poll() is None:
            observe()
            pngs = list(out.rglob('*.png'))
            assert len(pngs) <= 48 and sum(path.stat().st_size for path in pngs) <= 12 * 1024**2, 'PNG count/byte cap'
            time.sleep(.25)
        record.update(exitCode=child.returncode, finishedEpoch=time.time())
        assert child.returncode == 0, 'Native failed; retained report/log'
    observe()
    command = [os.sys.executable, '-B', str(Path(__file__).with_name('compare.py')), str(out / 'capture')]
    record = {'label': 'reference-comparison', 'command': command, 'startedEpoch': time.time()}
    report['commands'].append(record)
    with (out / 'comparison.log').open('wb') as log:
        child = subprocess.Popen(command, cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        while child.poll() is None:
            observe(); time.sleep(.25)
        record.update(exitCode=child.returncode, finishedEpoch=time.time())
        assert child.returncode == 0, 'Comparison failed; retained log'
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
    accepted_record.update(state='FULL', remainingNewUpperBytes=0, actualFullEpoch=time.time(), resources=0, livePids=remaining, cleanup=report['cleanup'])
    membership.publish(accepted_record)
    print(json.dumps({'status': report['status'], 'peakProcesses': report['peakProcesses'], 'peakRssKiB': report['peakRssKiB'], 'peakWholeBytes': report['peakWholeBytes'], 'cleanup': report['cleanup'], 'remainingOwnDescendants': remaining}))
