#!/usr/bin/env python3
"""Bounded isolated-part preview source/native guardian; execute only under a fresh direct named offer."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import subprocess
import tempfile
import time
sys.dont_write_bytecode = True
from git_carry import inventory, foundation_inventory
from membership import sample as membership_sample, publish_census

ROOT = Path(__file__).resolve().parents[4]
BASE = ROOT / 'docs/mcr/evidence/parade-ink-part-preview'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
CHROME = Path('/Applications/Google Chrome.app')
GIB = 1024 ** 3


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def deadline(name):
    return datetime.datetime.fromisoformat(os.environ[name].replace('Z', '+00:00')).timestamp()


def free():
    v = os.statvfs(ROOT)
    return v.f_bavail * v.f_frsize


def size(path):
    if not path.exists(): return 0
    if path.is_file(): return path.stat().st_size
    total = 0
    for p in path.rglob('*'):
        try:
            if p.is_file() and not p.is_symlink(): total += p.stat().st_size
        except FileNotFoundError: pass
    return total


def allocated(path):
    # Chrome atomically replaces preference files during a census. Retry only
    # literal vanished-file races, retain every partial upper, reject all else.
    highest = 0
    for _ in range(3):
        result = subprocess.run(['/usr/bin/du', '-sk', str(path)], text=True, capture_output=True)
        if result.stdout.strip(): highest = max(highest, int(result.stdout.split()[0]) * 1024)
        if result.returncode == 0: return highest
        lines = result.stderr.strip().splitlines()
        assert result.returncode == 1 and lines and all(line.endswith('No such file or directory') for line in lines), result.stderr
    raise RuntimeError('No complete allocated-byte census after three strict ENOENT retries: ' + str(path))


def processes(groups):
    rows = [list(map(int, line.split())) for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines()]
    selected = {os.getpid()}
    while True:
        found = selected | {pid for pid, parent, group, _ in rows if parent in selected or group in groups}
        if found == selected: break
        selected = found
    live = []
    for pid, _, group, rss in rows:
        if pid not in selected: continue
        try: os.kill(pid, 0)
        except ProcessLookupError: continue
        live.append((pid, group, rss * 1024))
    return live


def publish_discovery(resource, output, controller_pid, outer_pid, actual_full=None):
    discovery = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a230-6acc-7e82-9e4b-92506decc745.json')
    def process(pid, script):
        return {'pid': pid, 'birth': resource['pidBirth'][str(pid)],
            'executable': resource['executables'][str(pid)], 'script': str((BASE / script).resolve()),
            'cwd': str(ROOT), 'outputRoot': str(output)}
    document = {'ownerThreadId': '01a1a230-6acc-7e82-9e4b-92506decc745', 'offerId': resource['allocation'],
        'state': 'FULL' if actual_full is not None else 'ACCEPTED',
        'acceptedAtEpoch': datetime.datetime.fromisoformat(resource['acceptedUtc'].replace('Z', '+00:00')).timestamp(),
        'workDeadlineEpoch': datetime.datetime.fromisoformat(resource['workUtc'].replace('Z', '+00:00')).timestamp(),
        'fullDeadlineEpoch': datetime.datetime.fromisoformat(resource['hardUtc'].replace('Z', '+00:00')).timestamp(),
        'kind': 'browser' if resource['class'] == 'native' else 'source', 'processCap': resource['caps']['processes'],
        'rssCapBytes': resource['caps']['rssBytes'], 'remainingNewUpperBytes': 0 if actual_full is not None else resource['caps']['newBytes'] - int(os.environ.get('MCR_PART_NEW_PRIOR_BYTES', '0')),
        'controller': process(controller_pid, 'run-allocated.py'), 'guardControllers': [process(outer_pid, 'supervise-allocated.py')],
        'receiptPaths': {'accept': str(output / 'accepted.json'), 'full': str(output / 'release.json')},
        'publishedAtEpoch': time.time()}
    if actual_full is not None: document.update(actualFullEpoch=actual_full, resources=0)
    discovery.parent.mkdir(parents=True, exist_ok=True)
    temporary = discovery.with_name('.' + discovery.name + '.' + str(os.getpid()) + '.tmp')
    temporary.write_text(json.dumps(document, indent=2) + '\n'); os.replace(temporary, discovery)


def main():
    mode = os.environ['MCR_PART_CLASS']; assert mode in ['source', 'native']
    source_plan = os.environ.get('MCR_PART_SOURCE_PLAN', 'full')
    assert source_plan in ['full', 'types-lint', 'lifetime-delta']
    run = os.environ['MCR_PART_RUN']; assert run.startswith(mode + '-') and all(c.islower() or c.isdigit() or c == '-' for c in run)
    work, hard = deadline('MCR_PART_WORK_UTC'), deadline('MCR_PART_HARD_UTC')
    accepted = deadline('MCR_PART_ACCEPTED_UTC')
    assert accepted <= time.time() < work < hard and hard - work >= 20
    caps = {'wholeBytes': 8 * GIB, 'newBytes': (128 if mode == 'source' else 200) * 1024 ** 2,
            'temporaryBytes': (96 if mode == 'source' else 160) * 1024 ** 2,
            'durableBytes': (32 if mode == 'source' else 40) * 1024 ** 2,
            'rssBytes': (3 if mode == 'source' else 4) * GIB, 'processes': 8 if mode == 'source' else 32}
    admitted = int(os.environ['MCR_PART_ACCEPT_FREE_BYTES']); reserved = int(os.environ['MCR_PART_OTHER_RESERVE_BYTES'])
    assert admitted >= (11 if mode == 'source' else 12) * GIB and reserved >= 0 and admitted - reserved - caps['newBytes'] >= 9 * GIB
    assert free() >= 8 * GIB
    outer_pid = int(os.environ['MCR_PART_OUTER_PID'])
    outer_tmp = Path(os.environ['MCR_PART_OUTER_TMP'])
    outer_source = BASE / 'supervise-allocated.py'
    assert hashlib.sha256(outer_source.read_bytes()).hexdigest() == os.environ['MCR_PART_OUTER_SHA256']
    os.kill(outer_pid, 0)
    assert outer_pid == os.getppid() and (outer_tmp / 'standing.json').is_file()
    assert json.loads((outer_tmp / 'standing.json').read_text())['initialSampleBeforePrimary']['freeBytes'] >= 8 * GIB
    # The outer sealed this sole new factual receipt before publishing ACCEPT.
    # Every other tracked or untracked mutation still fails frozen admission.
    status = subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=all'], cwd=ROOT, text=True).splitlines()
    expected_bootstrap = str((BASE / 'runs' / run / 'bootstrap-accepted.json').relative_to(ROOT))
    assert status == ['?? ' + expected_bootstrap], status
    output = BASE / 'runs' / run; output.mkdir(parents=True, exist_ok=True)
    # Publish both real Python identities before dependency/Git census work.
    discovery = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a230-6acc-7e82-9e4b-92506decc745.json')
    bootstrap = json.loads(discovery.read_text())
    assert bootstrap['offerId'] == os.environ['MCR_PART_ALLOCATION'] and bootstrap['state'] == 'ACCEPTED'
    def actual_control(pid, script):
        return {'pid': pid, 'birth': subprocess.check_output(['ps', '-p', str(pid), '-o', 'lstart='], text=True).strip(),
            'executable': subprocess.check_output(['ps', '-p', str(pid), '-o', 'comm='], text=True).strip(),
            'script': str((BASE / script).resolve()), 'cwd': str(ROOT), 'outputRoot': str(output)}
    assert not (output / 'accepted.json').exists()
    bootstrap.update(controller=actual_control(os.getpid(), 'run-allocated.py'),
        guardControllers=[actual_control(outer_pid, 'supervise-allocated.py')], publishedAtEpoch=time.time(),
        receiptPaths={'accept': str(output / 'primary-bootstrap-accepted.json'), 'full': str(output / 'release.json')},
        stage='Actual primary and outer Python before expensive carry census, no workload')
    (output / 'primary-bootstrap-accepted.json').write_text(json.dumps(bootstrap, indent=2) + '\n')
    pending_bootstrap = discovery.with_name('.' + discovery.name + '.' + str(os.getpid()) + '.tmp')
    pending_bootstrap.write_text(json.dumps(bootstrap, indent=2) + '\n'); os.replace(pending_bootstrap, discovery)
    link = ROOT / 'frontend/node_modules'; assert not os.path.lexists(link)
    assert (DEPS / '.pnpm/lock.yaml').read_bytes() == (ROOT / 'frontend/pnpm-lock.yaml').read_bytes()
    sources = [p for folder in ['frontend/src/entities/avatar/runtime/motion-adoption',
        'frontend/src/features/avatar/motion-adoption', 'frontend/src/features/avatar/__tests__/parade-ink-part-preview']
        for p in (ROOT / folder).rglob('*') if p.is_file()]
    sources += [Path(__file__), outer_source, BASE / 'git_carry.py', BASE / 'membership.py',
        ROOT / 'frontend/pnpm-lock.yaml', ROOT / 'frontend/tsconfig.json']
    pins = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}
    git_metadata = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
    dependency_logical = sum(p.stat().st_size for p in DEPS.rglob('*') if p.is_file())
    dependency_allocated = allocated(DEPS)
    dependency_charge = max(dependency_logical, dependency_allocated)
    python_charge = max(allocated(Path(sys.prefix)), size(Path(sys.prefix)))
    runtime_charge = NODE.stat().st_size
    chrome_logical = size(CHROME) if mode == 'native' else 0
    chrome_allocated = allocated(CHROME) if mode == 'native' else 0
    chrome_charge = max(chrome_logical, chrome_allocated)
    foundation = foundation_inventory(ROOT) if mode == 'native' else {'bytes': 0, 'postimages': []}
    foundation_charge = foundation['bytes']
    owned_git = inventory(ROOT, foundation=mode == 'native')
    retained = {'checkoutBytes': allocated(ROOT), 'gitMetadataBytes': allocated(git_metadata),
                'readOnlyDependencyLogicalBytes': dependency_logical, 'readOnlyDependencyAllocatedBytes': dependency_allocated,
                'readOnlyDependencyChargeBytes': dependency_charge, 'readOnlyNodeRuntimeBytes': runtime_charge,
                'readOnlyFoundation': foundation,
                'entirePythonPrefix': sys.prefix, 'entirePythonChargeBytes': python_charge,
                'readOnlyNodeSha256': hashlib.sha256(NODE.read_bytes()).hexdigest(),
                'readOnlyChromeLogicalBytes': chrome_logical, 'readOnlyChromeAllocatedBytes': chrome_allocated,
                'readOnlyChromeChargeBytes': chrome_charge,
                'ownedSharedGitInventory': owned_git,
                'sharedGitCommonDirReadOnly': subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT, text=True).strip(),
                'copiedGitCarryBytes': 0}
    baseline = retained['checkoutBytes'] + retained['gitMetadataBytes'] + dependency_charge + runtime_charge + chrome_charge + owned_git['bytes'] + python_charge + foundation_charge
    assert max(baseline, int(os.environ['MCR_PART_PRIOR_CARRY_BYTES'])) + caps['newBytes'] <= caps['wholeBytes']
    temporary = Path(tempfile.mkdtemp(prefix='mcr-partpreview-current-', dir='/private/tmp'))
    (outer_tmp / 'child-temporary.json').write_text(json.dumps(str(temporary)))
    report = {'allocation': os.environ['MCR_PART_ALLOCATION'], 'class': mode, 'candidate': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'acceptedUtc': os.environ['MCR_PART_ACCEPTED_UTC'], 'acceptedFreeBytes': admitted, 'otherReserveBytes': reserved,
              'workUtc': os.environ['MCR_PART_WORK_UTC'], 'hardUtc': os.environ['MCR_PART_HARD_UTC'], 'sourceSha256': pins,
              'guardianPid': os.getpid(), 'outerPid': outer_pid, 'standingBeforeChildrenUtc': utc(), 'retained': retained, 'caps': caps, 'peaks': {k: 0 for k in caps}, 'commands': [], 'status': 'FAIL'}
    groups = set(); child = None
    try:
        initial_rows = processes(groups)
        outer_rss = int(subprocess.check_output(['ps', '-p', str(outer_pid), '-o', 'rss='], text=True).strip()) * 1024
        initial = {'utc': utc(), 'pids': [pid for pid, _, _ in initial_rows] + [outer_pid], 'wholeBytes': max(baseline + allocated(temporary) + allocated(outer_tmp), int(os.environ['MCR_PART_PRIOR_CARRY_BYTES'])),
            'newBytes': int(os.environ.get('MCR_PART_NEW_PRIOR_BYTES', '0')) + allocated(temporary) + allocated(outer_tmp), 'temporaryBytes': allocated(temporary) + allocated(outer_tmp),
            'durableBytes': allocated(BASE), 'rssBytes': sum(rss for _, _, rss in initial_rows) + outer_rss, 'processes': len(initial_rows) + 1, 'freeBytes': free()}
        for key in caps: assert initial[key] <= caps[key]
        assert initial['freeBytes'] >= 8 * GIB
        report['initialSampleBeforeWorkload'] = initial
        (output / 'initial-sample.json').write_text(json.dumps(initial, indent=2) + '\n')
        resource = {'allocation': report['allocation'], 'state': 'ACTUAL_ACCEPT_LIVE', 'class': mode,
            'root': str(ROOT), 'candidate': report['candidate'], 'acceptedUtc': report['acceptedUtc'],
            'workUtc': report['workUtc'], 'hardUtc': report['hardUtc'], 'caps': caps,
            'pids': initial['pids'], 'pidBirth': {str(pid): subprocess.check_output(['ps', '-p', str(pid), '-o', 'lstart='], text=True).strip() for pid in [os.getpid(), outer_pid]},
            'initialSample': initial, 'retained': retained,
            'executables': {str(pid): subprocess.check_output(['ps', '-p', str(pid), '-o', 'comm='], text=True).strip() for pid in [os.getpid(), outer_pid]}}
        (output / 'resource.json').write_text(json.dumps(resource, indent=2) + '\n')
        (output / 'accepted.json').write_text(json.dumps(resource, indent=2) + '\n')
        publish_discovery(resource, output, os.getpid(), outer_pid)
        report['initialGlobalCensus'] = membership_sample()
        publish_census(output / 'primary-census.json', report['initialGlobalCensus'])
        env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ['PATH'], 'MCR_PART_SOURCE_TMP': str(temporary),
               'MCR_PART_PYTHON': sys.executable, 'MCR_PART_OUTPUT': str(output), 'MCR_PART_TMP': str(temporary), 'MCR_PART_DEPS_FRONTEND': str(DEPS.parent), 'TMPDIR': str(temporary), 'TMP': str(temporary), 'TEMP': str(temporary), 'RAYON_NUM_THREADS': '1', 'CI': 'true', 'XDG_CACHE_HOME': str(temporary / 'cache'), 'PYTHONDONTWRITEBYTECODE': '1'}
        # Private config resolves dependencies absolutely; source tree remains link-free.
        config = json.loads((ROOT / 'frontend/tsconfig.json').read_text())
        config['compilerOptions'].update({'paths': {'vite': [str(DEPS / 'vite/dist/node/index.d.ts')], '@vitejs/plugin-react': [str(DEPS / '@vitejs/plugin-react/dist/index.d.ts')], '@tailwindcss/vite': [str(DEPS / '@tailwindcss/vite/dist/index.d.mts')], 'react': [str(DEPS / '@types/react')], 'react/*': [str(DEPS / '@types/react/*')], 'react-dom': [str(DEPS / '@types/react-dom')], 'react-dom/*': [str(DEPS / '@types/react-dom/*')], '*': [str(DEPS / '*')]},
            'typeRoots': [str(DEPS / '@types')], 'types': [str(DEPS / 'vite/client')], 'incremental': False})
        config['include'] = [str(ROOT / 'frontend/src'), str(ROOT / 'frontend/vite.config.ts')]
        type_config = temporary / 'tsconfig.json'; type_config.write_text(json.dumps(config))
        testroot = ROOT / 'frontend/src/features/avatar/__tests__/parade-ink-part-preview'
        env.update(MCR_FRONTEND_DEPENDENCIES=str(DEPS.parent), MCR_MOTION_CACHE=str(temporary / 'motion-cache'),
                   MCR_PART_TMP=str(temporary), MCR_PART_OUTPUT=str(output))
        commands = [
            [str(NODE), str(DEPS / 'vitest/vitest.mjs'), 'run', '--config', str(testroot / 'vitest.config.mjs'), '--configLoader', 'native', '--no-cache', '--maxWorkers=1', '--reporter=json', '--outputFile=' + str(output / 'vitest.json')],
            [str(NODE), str(DEPS / 'typescript/bin/tsc'), '-p', str(type_config), '--pretty', 'false'],
            [str(DEPS / '.bin/oxlint'), '--deny-warnings', 'src/entities/avatar/runtime/motion-adoption/runtime.ts', 'src/entities/avatar/runtime/motion-adoption/compositor.ts',
             'src/entities/avatar/runtime/motion-adoption/__tests__/parade-ink-part-preview', 'src/features/avatar/motion-adoption/MotionOption.tsx',
             'src/features/avatar/motion-adoption/part-preview', 'src/features/avatar/__tests__/parade-ink-part-preview']]
        report['sourcePlan'] = source_plan
        if mode == 'source' and source_plan == 'types-lint': commands = commands[1:]
        if mode == 'source' and source_plan == 'lifetime-delta':
            commands[0] += ['src/entities/avatar/runtime/motion-adoption/__tests__/parade-ink-part-preview/runtime.test.ts',
                'src/features/avatar/__tests__/parade-ink-part-preview/option.test.tsx',
                '--testNamePattern', 'notifies once|clears all private|releases a live preview|removes all displayed pixels']
            commands[2] = [str(DEPS / '.bin/oxlint'), '--deny-warnings', 'src/entities/avatar/runtime/motion-adoption/runtime.ts',
                'src/entities/avatar/runtime/motion-adoption/__tests__/parade-ink-part-preview/runtime.test.ts']

        if mode == 'native':
            import socket
            with socket.socket() as listener:
                port = 5373; listener.bind(('127.0.0.1', port))
            env['MCR_PART_PORT'] = str(port)
            commands = [[str(NODE), str(testroot / 'native.mjs')]]
        for index, argv in enumerate(commands):
            assert time.time() < work and free() >= 8 * GIB
            began = time.monotonic(); item = {'argv': argv, 'cwd': '.' if mode == 'native' else 'frontend'}; report['commands'].append(item)
            with (output / f'{index:02}.log').open('wb') as log:
                child = subprocess.Popen(argv, cwd=ROOT if mode == 'native' else ROOT / 'frontend', env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                groups.add(child.pid)
                while child.poll() is None:
                    publish_census(output / 'primary-census.json', membership_sample())
                    owned_git['bytes'] = max(owned_git['bytes'], inventory(ROOT, foundation=mode == 'native')['bytes'])
                    rows = processes(groups)
                    for pid, group, _ in rows:
                        if pid != os.getpid() and group != os.getpgrp(): groups.add(group)
                    os.kill(outer_pid, 0)
                    temporary_bytes, durable_bytes = allocated(temporary) + allocated(outer_tmp), allocated(BASE)
                    whole = allocated(ROOT) + allocated(git_metadata) + dependency_charge + runtime_charge + chrome_charge + owned_git['bytes'] + python_charge + foundation_charge + temporary_bytes
                    sample = {'wholeBytes': max(whole, int(os.environ['MCR_PART_PRIOR_CARRY_BYTES'])), 'newBytes': int(os.environ.get('MCR_PART_NEW_PRIOR_BYTES', '0')) + max(0, whole - baseline), 'temporaryBytes': temporary_bytes, 'durableBytes': durable_bytes,
                              'rssBytes': sum(rss for _, _, rss in rows), 'processes': len(rows) + 1}
                    for key, value in sample.items():
                        report['peaks'][key] = max(report['peaks'][key], value)
                        assert value <= caps[key], 'Resource cap: ' + key
                    assert free() >= 8 * GIB and time.time() < work, 'Live free-space floor/work deadline'
                    time.sleep(.25)
            item.update(exitCode=child.returncode, elapsedSeconds=time.monotonic() - began)
            if child.returncode != 0:
                report.setdefault('commandFailures', []).append(index)
        if mode == 'native': assert foundation == foundation_inventory(ROOT)
        assert pins == {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}
        assert not subprocess.check_output(['git', 'diff', '--name-only', 'HEAD'], cwd=ROOT).strip()
        assert not report.get('commandFailures'), 'Commands failed; all original logs retained'
        report['status'] = 'PASS'
    except Exception as error:
        report['failure'] = str(error)
    finally:
        cleanup = time.monotonic()
        for group in groups:
            try: os.killpg(group, signal.SIGTERM)
            except ProcessLookupError: pass
        if child and child.poll() is None:
            try: child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                for group in groups:
                    try: os.killpg(group, signal.SIGKILL)
                    except ProcessLookupError: pass
                child.wait(timeout=2)
        for pid, _, _ in processes(groups):
            if pid != os.getpid():
                try: os.kill(pid, signal.SIGKILL)
                except ProcessLookupError: pass
        shutil.rmtree(temporary)
        if mode == 'native' and 'port' in locals():
            import socket
            with socket.socket() as listener:
                listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                listener.bind(('127.0.0.1', port)); listener.listen(1)
            report['portRebound'] = port
        remaining = [pid for pid, _, _ in processes(groups) if pid != os.getpid()]
        report.update(fullUtc=utc(), cleanupSeconds=time.monotonic() - cleanup, remainingProcesses=remaining,
                      dependencyLinkAbsent=not os.path.lexists(link), temporaryAbsent=not temporary.exists(), freeAfterBytes=free(),
                      finishedBeforeHardDeadline=time.time() < hard)
        report['released'] = not remaining and report['dependencyLinkAbsent'] and report['temporaryAbsent'] and report['finishedBeforeHardDeadline']
        if not report['released']: report['status'] = 'FAIL'
        report['logSha256'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in output.glob('*.log')}
        if (output / 'resource.json').exists():
            resource['state'] = 'ACTUAL_FULL_RELEASED' if report['released'] else 'RELEASE_FAILED'
            resource['fullUtc'] = report['fullUtc']; resource['remainingProcesses'] = remaining
            (output / 'resource.json').write_text(json.dumps(resource, indent=2) + '\n')
        (output / 'release.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2), flush=True)
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
