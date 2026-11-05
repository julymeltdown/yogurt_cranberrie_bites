"""Finite PI12 SOURCE controller. Ordinary preparation never invokes main().

Accounting follows the preserved QA05 dual-control model, expanded to the entire
Python prefix and fresh global workflow membership. No inherited admission.
"""
import datetime as dt
import hashlib
import gzip
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = Path(__file__).parent
DESKTOP = ROOT.parent.parent
NODE_ROOT = DESKTOP / 'mcr-tools/node-v24.22.2-darwin-arm64'
NODE = NODE_ROOT / 'bin/node'
DEPS = DESKTOP / 'mcr/frontend/node_modules'
MEMBERS = DESKTOP / 'mcr-evidence/backend-staffing-01a1a21d/resource-membership-current.json'
RECEIPTS = MEMBERS.with_name('owner-resource-receipts')
OWNER = '01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c'


def verify_tools(offer):
    pins = json.loads((EVIDENCE / 'tool-pins.json').read_text())
    paths = {'node': NODE, 'python': Path(sys.executable).resolve(), 'git': Path(shutil.which('git')).resolve(),
             'vitest': DEPS / 'vitest/vitest.mjs', 'typescript': DEPS / 'typescript/bin/tsc',
             'oxlint': DEPS / 'oxlint/bin/oxlint', 'vite': DEPS / 'vite/package.json',
             'playwright': DEPS / '@playwright/test/package.json',
             'react-plugin': DEPS / '@vitejs/plugin-react/package.json', 'react-plugin-source': DEPS / '@vitejs/plugin-react/dist/index.js'}
    for name, p in paths.items():
        b = p.read_bytes()
        assert hashlib.sha256(b).hexdigest() == pins[name]['sha256'] and len(b) == pins[name]['bytes'], name
    for name, digest in offer['controlPins'].items():
        assert hashlib.sha256((EVIDENCE / name).read_bytes()).hexdigest() == digest, name


def git(*args):
    return sp.check_output(['git', *args], cwd=ROOT, text=True).strip()


def inventory(root):
    rows = {}
    if not root.exists():
        return rows
    if root.is_file() or root.is_symlink():
        s = root.lstat()
        return {str(root): (s.st_dev, s.st_ino, s.st_size, s.st_blocks * 512)}
    for parent, dirs, files in os.walk(root, followlinks=False):
        for name in ['.'] + files + [v for v in dirs if (Path(parent) / v).is_symlink()]:
            p = Path(parent) if name == '.' else Path(parent) / name
            try:
                s = p.lstat()
                rows[str(p)] = (s.st_dev, s.st_ino, s.st_size, s.st_blocks * 512)
            except FileNotFoundError:
                pass
    return rows


def census(browser=False):
    roots = [ROOT, DEPS, NODE_ROOT, Path(sys.prefix).resolve(),
             Path(shutil.which('git')).resolve().parents[1], Path(git('rev-parse', '--absolute-git-dir')),
             Path('/Users/lhs/.codex/visualizations/2026/11/03') / OWNER]
    for required in roots:
        assert required.exists(), 'Required WHOLE root missing: ' + str(required)
    for pattern in ['mcr-fe17-*', 'mcr-fe41-*', 'mcr-qa05-*', 'mcr-pi12-*']:
        roots.extend(Path('/private/tmp').glob(pattern))
    if browser:
        roots.append(Path('/Applications/Google Chrome.app'))
    rows = {}
    for root in dict.fromkeys(roots):
        rows.update(inventory(root))
    inodes = {(v[0], v[1]): v for v in rows.values()}
    objects = sorted({s.split()[0] for s in git('rev-list', '--objects', 'HEAD',
        'codex/mcr-fe-QA05-functional-recovery', 'codex/mcr-fe-FE17-original-completion',
        'codex/mcr-fe-AR03-persistence-motion', 'codex/mcr-fe-FE41-original-completion',
        'f543d43a96848d47021eaa4b0736a27e548ebadc').splitlines()})
    sizes = sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'], cwd=ROOT,
        input='\n'.join(objects) + '\n', text=True)
    history = sum(map(int, sizes.splitlines()))
    return {'roots': [str(p) for p in roots], 'rows': rows, 'historyBytes': history,
            'historyObjects': len(objects), 'wholeBytes': max(5 * 1024**3, history + sum(v[2] for v in inodes.values())),
            'allocatedBytes': sum(v[3] for v in inodes.values()), 'entries': len(inodes) + len(objects)}


def processes():
    return [line.strip().split(None, 4) for line in sp.check_output(
        ['ps', '-ww', '-axo', 'pid=,ppid=,pgid=,rss=,args='], text=True).splitlines() if line.strip()]


def identity(pid, script=None, output=None):
    def field(name):
        r = sp.run(['ps', '-ww', '-p', str(pid), '-o', name + '='], text=True, capture_output=True)
        return r.stdout.strip() if r.returncode == 0 else None
    return {'pid': pid, 'birth': field('lstart'), 'executable': field('comm'),
            'script': str(script) if script else None, 'outputRoot': str(output) if output else None, 'workingDirectory': str(Path.cwd())}


def publish_receipt(offer, state, controller, guards):
    target = RECEIPTS / (OWNER + '.json')
    previous = json.loads(target.read_text()) if target.exists() else {}
    value = {'closedHistory': previous.get('closedHistory', []), 'ownerThreadId': OWNER, 'offerId': offer['slot'], 'state': 'ACCEPTED' if state.startswith('ACCEPTED_') else state, 'phase': state,
             'acceptedAtEpoch': offer['acceptedEpoch'], 'workDeadlineEpoch': offer['workDeadline'],
             'fullDeadlineEpoch': offer['fullDeadline'], 'kind': offer['mode'].lower(),
             'processCap': offer['processCap'], 'rssCapBytes': offer['rssBytes'],
             'remainingNewUpperBytes': 0 if state == 'FULL' else offer['newBytes'],
             'controller': controller, 'guardControllers': guards,
             'receiptPaths': {'accept': offer['offerPath'], 'full': str(Path(offer['output']) / 'guardian.json')},
             'publishedAtEpoch': time.time()}
    if state == 'FULL':
        full = Path(offer['output']) / 'guardian.json'; data = json.loads(full.read_text())
        value['closedHistory'] = [x for x in value['closedHistory'] if x.get('offerId') != offer['slot']] + [{'offerId': offer['slot'], 'state': 'FULL', 'acceptedAtEpoch': offer['acceptedEpoch'], 'actualFullUTC': data['finishedUTC'], 'actualFullEpoch': dt.datetime.fromisoformat(data['finishedUTC']).timestamp(), 'resourcesReleased': data['resourcesReleased'], 'fullPath': str(full), 'fullSHA256': hashlib.sha256(full.read_bytes()).hexdigest()}]
    temporary = target.with_name(target.name + '.' + str(os.getpid()) + '.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n'); os.replace(temporary, target)


def sample(offer, ancestor, baseline):
    rows = processes(); by_pid = {int(v[0]): v for v in rows}
    def closure(seeds):
        owned = set(seeds)
        while True:
            expanded = owned | {int(v[0]) for v in rows if int(v[1]) in owned}
            if expanded == owned:
                return owned
            owned = expanded
    def live_descriptor(v):
        v = v or {}
        pid = v.get('pid')
        if not pid or pid not in by_pid or not v.get('birth'):
            return False
        actual = identity(pid)
        executable_matches = actual['executable'] and Path(actual['executable']).resolve() == Path(v['executable']).resolve()
        launcher_pair = {str(Path(sys.prefix) / 'bin' / name) for name in ['python3', 'python3.14']}
        runtime = str(Path(sys.prefix) / 'Resources/Python.app/Contents/MacOS/Python')
        executable_matches = executable_matches or (v.get('executable') in launcher_pair and actual['executable'] == runtime and v.get('script') and v['script'] in by_pid[pid][4])
        return actual['birth'] and actual['birth'].split() == v['birth'].split() and executable_matches and (not v.get('script') or v['script'] in by_pid[pid][4])
    peers = []; peer_roots = set(); ended_roots = set(); modern = set(); ordinary_roots = set()
    for path in RECEIPTS.glob('*.json'):
        v = json.loads(path.read_text()); owner = v.get('ownerThreadId'); modern.add(owner)
        ordinary = v.get('ordinaryObservation')
        if ordinary and owner != OWNER:
            observed_pid = ordinary.get('actualpid', ordinary.get('pid'))
            script = ordinary.get('script')
            if observed_pid and ordinary.get('birth') and ordinary.get('executable') and script and Path(script).is_absolute():
                descriptor = {**ordinary, 'pid': observed_pid}
                if live_descriptor(descriptor):
                    # Exact ordinary identity is accounted as a live peer, not a payload exemption.
                    ordinary_roots.add(observed_pid); peer_roots.add(observed_pid)
        if (v.get('state') == 'FULL' or v.get('kind') == 'ordinary') and ordinary and ordinary.get('ordinaryCommandsComplete') is True and ordinary.get('completedAt') and ordinary.get('role') == 'ordinary Main management authoring and Git only':
            continue
        if owner == OWNER:
            continue
        descriptors = [v.get('controller', {}), *v.get('guardControllers', [])]
        live = {d['pid'] for d in descriptors if live_descriptor(d)}
        if v.get('state') == 'FULL':
            ended_roots |= live
            continue
        if v.get('state') in ['DECLINED', 'SENT_UNACCEPTED', 'NOT_ACCEPTED'] and not v.get('acceptedAtEpoch'):
            assert not live, 'Unaccepted receipt has a live workflow root'
            continue
        if v.get('kind') == 'ordinary':
            peer_roots |= live
            continue
        # An accepted reservation stays until literal FULL even with no live PID.
        peers.append(v); peer_roots |= live
    membership = json.loads(MEMBERS.read_text())
    for v in membership['entries']:
        if v.get('owner') in modern or v.get('owner') == OWNER:
            continue
        state = v.get('state', '')
        if ('ACCEPT' in state or 'ACTIVE' in state) and 'UNACCEPTED' not in state and 'FULL' not in state:
            peers.append({'kind': v.get('kind'), 'processCap': v.get('processCap', 96),
                          'rssCapBytes': v.get('rssCapGiB', 18) * 1024**3,
                          'remainingNewUpperBytes': v.get('remainingNewConservativeUpperBytes', 0)})
            peer_roots |= {int(r[0]) for r in rows if any(m and m in r[4] for m in v.get('knownRootMatchers', []))}
    ordinary_owned = closure(ordinary_roots)
    for row in rows:
        if int(row[0]) not in ordinary_owned:
            continue
        args = row[4].split(); executable = Path(identity(int(row[0]))['executable'] or '').name.lower()
        assert not (executable.startswith(('node', 'cargo', 'rustc')) or 'chrome' in executable
                    or any(Path(a).name in ['pytest', 'vitest', 'tsc', 'playwright'] for a in args[1:])), 'Runtime payload inside an ordinary preparation identity'
    own = closure({ancestor}); other = closure(peer_roots); ended = closure(ended_roots)
    known = own | other | ended
    for r in rows:
        if int(r[0]) in known or len(r) < 5:
            continue
        args = r[4].split() if not r[4].startswith('[') else []
        if not args:
            continue
        name = Path(args[0]).name.lower()
        def task_path(a):
            return a.startswith('/') and (a.startswith(str(DESKTOP)) and '/mcr' in a or a.startswith('/private/tmp/mcr-'))
        scripts = [a for a in args[1:] if a.startswith('/') and a.endswith(('.py', '.js', '.mjs', '.cjs', '.ts', '.tsx'))]
        # CWD does not identify the program: the verified ChatGPT kernels remain
        # tool infrastructure, while unknown actual MCR programs fail closed.
        script_task = any(task_path(a) for a in scripts)
        manifest_task = any(task_path(args[i + 1]) for i, a in enumerate(args[:-1]) if a in ['--manifest-path', '--target-dir'])
        output_task = any('/private/tmp/mcr-' in a and a.startswith(('--user-data-dir=', '--target-dir=')) for a in args[1:])
        workload = name.startswith(('node', 'cargo', 'rustc')) or 'python' in name or ('Chrome' in r[4] and output_task)
        mcr = script_task or manifest_task or output_task
        if workload and mcr:
            raise AssertionError('Unknown live MCR workload identity ' + json.dumps({**identity(int(r[0])), 'script': next((p for p in scripts if task_path(p)), None), 'pgid': int(r[2])}))
    assert len(peers) + 1 <= 4
    assert sum(v.get('kind') == 'source' for v in peers) + (offer['mode'] == 'SOURCE') <= 3
    assert sum(v.get('kind') == 'browser' for v in peers) + (offer['mode'] == 'BROWSER') <= 2
    targets = sum(v.get('kind') == 'target' for v in peers); assert targets <= 1
    assert sum(v['processCap'] for v in peers) + offer['processCap'] <= 96
    assert sum(v['rssCapBytes'] for v in peers) + offer['rssBytes'] <= 18 * 1024**3
    rss = sum(int(v[3]) * 1024 for v in rows if int(v[0]) in own)
    global_pids = own | other
    assert len(own) <= offer['processCap'] and rss <= offer['rssBytes']
    assert len(global_pids) <= 96 and sum(int(v[3]) * 1024 for v in rows if int(v[0]) in global_pids) <= 18 * 1024**3
    current = {}
    for root in [ROOT, Path(git('rev-parse', '--absolute-git-dir')), Path(offer['temporary'])]:
        current.update(inventory(root))
    if offer.get('familyBaseline'):
        family_path = Path(offer['familyBaseline'])
        baseline = json.loads(gzip.decompress(family_path.read_bytes()) if family_path.suffix == '.gz' else family_path.read_bytes())['rows']
    observed_growth = offer.get('preacceptNewBytes', 0) + sum(max(0, v[3] - baseline.get(p, [0, 0, 0, 0])[3]) for p, v in current.items())
    # Historical temporary peaks remain charged to cumulative NEW, not misclassified as durable files.
    growth = max(observed_growth, offer.get('familyPreviousMaxBytes', 0))
    if offer['mode'] == 'BROWSER':
        images = [p for n in offer['nativeFamilyOutputs'] for p in Path(n).glob('*.png')]
        assert len(images) <= offer['maxPngCount'] and sum(p.stat().st_size for p in images) <= offer['maxPngBytes'], 'Native image cap'
    temporary_bytes = sum(v[3] for v in inventory(Path(offer['temporary'])).values())
    durable_growth = observed_growth - temporary_bytes
    assert temporary_bytes <= offer['temporaryBytes'] and durable_growth <= offer['durableBytes']
    assert offer['baselineEntries'] + len(set(current) - set(baseline)) <= offer['fileCap']
    free = shutil.disk_usage(ROOT).free
    other_reserved = max(offer['otherReservedBytes'], (5 if targets >= 2 else 4 if targets else 3) * 1024**3,
                         sum(v.get('remainingNewUpperBytes', 0) for v in peers) + 2684354560)
    assert time.time() < offer['workDeadline'] and growth <= offer['newBytes']
    assert offer['baselineWholeBytes'] + growth <= offer['wholeBytes']
    assert free >= 8 * 1024**3 and free - other_reserved - offer['newBytes'] >= 9 * 1024**3
    census = [{**identity(int(v[0])), 'ppid': int(v[1]), 'pgid': int(v[2]), 'rssBytes': int(v[3]) * 1024} for v in rows if int(v[0]) in global_pids]
    return {'utc': dt.datetime.now(dt.timezone.utc).isoformat(), 'ownedPids': sorted(own),
            'globalObservedPids': sorted(global_pids), 'processCensus': census,
            'rssBytes': rss, 'growthBytes': growth, 'observedGrowthBytes': observed_growth, 'durableGrowthBytes': durable_growth, 'temporaryBytes': temporary_bytes, 'freeBytes': free, 'otherReservedBytes': other_reserved}


def stop(child):
    # Popen retains the actual child identity. Never signal a numeric group after it ended.
    if child is None or child.poll() is not None:
        return
    rows = processes()
    owned = {child.pid}
    while True:
        expanded = owned | {int(v[0]) for v in rows if int(v[1]) in owned}
        if expanded == owned:
            break
        owned = expanded
    tracked = {pid: identity(pid) for pid in owned}
    assert os.getpid() not in tracked
    def same_live(pid, previous):
        current = identity(pid)
        return bool(previous['birth'] and current['birth'] == previous['birth']
                    and current['executable'] == previous['executable'])
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        for pid in sorted(tracked, reverse=True):
            if not same_live(pid, tracked[pid]):
                continue
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass
        try:
            child.wait(timeout=2)
        except sp.TimeoutExpired:
            continue
    assert not any(same_live(pid, previous) for pid, previous in tracked.items()), 'Owned child remains'


def main():
    offer = json.loads(Path(sys.argv[1]).read_text())
    output = Path(offer['output']); output.mkdir(exist_ok=True)
    assert offer['acceptance'] == 'ACCEPT' and offer['issuer'] == '01a1a21d-39ec-7912-84cb-910058e7a110'
    assert offer['sourceCommit'] == git('rev-parse', 'HEAD')
    verify_tools(offer)
    outer = identity(os.getppid(), EVIDENCE / 'guardian.py', output)
    current = identity(os.getpid(), __file__, output)
    publish_receipt(offer, 'ACCEPTED_INNER_BOOTSTRAP', current, [outer])
    bootstrap_temp = output / 'inner-bootstrap.tmp'
    bootstrap_temp.write_text(json.dumps({'controller': current, 'guardControllers': [outer]}))
    os.replace(bootstrap_temp, output / 'inner-bootstrap.json')
    while not (output / 'outer-initial.json').exists():
        assert time.time() < offer['workDeadline'] - 20, 'Outer initial deadline'
        time.sleep(.05)
    assert json.loads((output / 'outer-initial.json').read_text())['freeBytes'] >= offer['startFloorBytes']
    readset = json.loads((EVIDENCE / 'readset.json').read_text())
    for p, digest in readset['files'].items():
        assert hashlib.sha256((ROOT / p).read_bytes()).hexdigest() == digest, p
    census_path = output / ('outer-census.json.gz' if offer.get('compressedCensus') else 'outer-census.json')
    baseline = json.loads(gzip.decompress(census_path.read_bytes()) if offer.get('compressedCensus') else census_path.read_bytes())['rows']
    publish_receipt(offer, 'ACCEPTED_INNER_CENSUS', identity(os.getpid(), __file__, output), [outer])
    carried = census(offer['mode'] == 'BROWSER'); assert carried['wholeBytes'] <= offer['baselineWholeBytes'] <= offer['wholeBytes']
    census_bytes = json.dumps(carried,separators=(',',':')).encode()
    (output / ('inner-census.json.gz' if offer.get('compressedCensus') else 'inner-census.json')).write_bytes(gzip.compress(census_bytes) if offer.get('compressedCensus') else census_bytes)
    report = {'status': 'FAIL', 'commands': [], 'samples': [], 'whole': {k: v for k, v in carried.items() if k != 'rows'}}
    child = None; link = ROOT / 'frontend/node_modules'; created = False
    def terminate(_sig, _frame):
        raise RuntimeError('Outer guard requested bounded cleanup')
    signal.signal(signal.SIGTERM, terminate)
    try:
        first = sample(offer, os.getppid(), baseline)
        (output / 'inner-initial.json').write_text(json.dumps(first))
        assert outer['pid'] == os.getppid() and identity(outer['pid'])['birth'].split() == outer['birth'].split()
        publish_receipt(offer, 'ACCEPTED_RUNNING', identity(os.getpid(), __file__, output), [outer])
        assert not link.exists() and not link.is_symlink() and offer['allowAbsentOnlyDependencyLink']
        link.symlink_to(DEPS, target_is_directory=True); created = True
        temp = Path(offer['temporary']); temp.mkdir(exist_ok=True)
        env = {**os.environ, 'TMPDIR': str(temp), 'TMP': str(temp), 'TEMP': str(temp), 'XDG_CACHE_HOME': str(temp / 'cache'), 'MCR_PI12_TEST_ROOT': str(ROOT / 'frontend'), 'MCR_PI12_TMP': str(temp), 'MCR_PI12_VITEST_PACKAGE': str(DEPS / 'vitest/package.json')}
        own = [p for p in readset['files'] if p.endswith(('.tsx', '.ts', '.mjs')) and
               ('/parade-ink-route-status/' in p or '/parade-ink-report/' in p or p in readset['changedExisting'])]
        commands = [
            ('scope', [sys.executable, '-B', str(EVIDENCE / 'scope.py')]),
            ('changed-tests', [str(NODE), '--input-type=module', '-e', """
import {createRequire} from 'node:module';import{pathToFileURL}from'node:url';
const require=createRequire(process.env.MCR_PI12_VITEST_PACKAGE);
const {startVitest}=await import(pathToFileURL(require.resolve('vitest/node')).href);
const ctx=await startVitest('test',['src/app/__tests__/parade-ink-route-status/status.test.tsx','src/features/safety/__tests__/parade-ink-report/report.test.tsx'],{root:process.env.MCR_PI12_TEST_ROOT,config:false,watch:false,maxWorkers:1,fileParallelism:false},{cacheDir:process.env.MCR_PI12_TMP+'/vite',esbuild:{jsx:'automatic'}});
if(!ctx)process.exitCode=1;else{process.exitCode=ctx.state.getFiles().some(f=>f.result?.state==='fail')||ctx.state.getUnhandledErrors().length?1:0;await ctx.close();}
"""]),
            ('types', [str(NODE), str(DEPS / 'typescript/bin/tsc'), '--noEmit', '--incremental', 'false', '--tsBuildInfoFile', str(temp / 'types.tsbuildinfo'), '-p', 'src/app/__tests__/parade-ink-route-status/tsconfig.source.json']),
            ('lint', [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', *[str(ROOT / p) for p in own]]),
            ('native-syntax', [str(NODE), '--check', str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/native.mjs')]),
            ('overlay-syntax', [str(NODE), '--check', str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/foundation-overlay.mjs')]),
            ('whitespace', ['git', 'diff', '--check', '6414d3c9aa0d7a10224b3fed89f6f0e5e45d4997']),
        ]
        if offer.get('repairFixtureSpread'):
            repair = "from pathlib import Path;import hashlib,json;p=Path(" + repr(str(ROOT / 'frontend/src/app/__tests__/parade-ink-route-status/qualified.tsx')) + ");s=p.read_text();a='for(const fn of [...listeners])fn()';b='for(const fn of listeners)fn()';assert s.count(a)==1;p.write_text(s.replace(a,b));print(json.dumps({'repair':'redundant iterable spread in browser fixture only','sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'focused13CasesUnchanged':True}))"
            commands.insert(0, ('fixture-lint-fix', [sys.executable, '-B', '-c', repair]))
        if offer.get('remainderOnly'):
            commands = [(label, argv) for label, argv in commands if label != 'changed-tests']
            report['retainedQualification'] = offer['retainedQualification']
        if offer['mode'] == 'BROWSER':
            assert offer['group'] in ['network', 'report-errors', 'both']
            env.update({'MCR_READONLY_FRONTEND': str(DEPS.parent), 'MCR_PI12_OUTPUT': str(output),
                'MCR_PI12_TMP': str(temp), 'MCR_PI12_WORK_DEADLINE': dt.datetime.fromtimestamp(offer['workDeadline'], dt.timezone.utc).isoformat(),
                'MCR_PI12_PORT': str(offer['port']), 'MCR_PI12_GROUP': offer['group']})
            if offer.get('retainedNative'):
                env['MCR_PI12_RETAINED_NATIVE'] = json.dumps(offer['retainedNative'])
            if offer.get('retainedImages'):
                env['MCR_PI12_RETAINED_IMAGES'] = json.dumps(offer['retainedImages'])
            commands = [('native-' + offer['group'], [str(NODE), str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/native.mjs')])]
            if offer.get('changedNativeHelper'):
                commands = [('changed-native-lint', [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/native.mjs')]),
                            ('changed-native-syntax', [str(NODE), '--check', str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/native.mjs')])] + commands
        if offer.get('changedFixture'):
            fixture = str(ROOT / 'frontend/src/app/__tests__/parade-ink-route-status/qualified.tsx')
            commands = [('changed-fixture-lint', [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', fixture]),
                        ('changed-fixture-types', [str(NODE), str(DEPS / 'typescript/bin/tsc'), '-p', str(ROOT / 'frontend/src/app/__tests__/parade-ink-route-status/tsconfig.source.json'), '--noEmit'])] + commands
        else:
            assert offer['mode'] in ['SOURCE', 'BROWSER']
        if offer.get('helpersOnly'):
            assert offer['mode'] == 'SOURCE'
            commands = [('helper-lint', [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/native.mjs'), str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/foundation-overlay.mjs')]),
                        ('native-helper-syntax', [str(NODE), '--check', str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/native.mjs')]),
                        ('overlay-helper-syntax', [str(NODE), '--check', str(ROOT / 'frontend/e2e/visual/parade-ink-route-status/foundation-overlay.mjs')])]
        for label, argv in commands:
            report['samples'].append(sample(offer, os.getppid(), baseline))
            with (output / (label + '.log')).open('wb') as log:
                child = sp.Popen(argv, cwd=ROOT / 'frontend', env=env, stdout=log, stderr=sp.STDOUT, start_new_session=True)
                while child.poll() is None:
                    report['samples'].append(sample(offer, os.getppid(), baseline)); time.sleep(.5)
            report['commands'].append({'label': label, 'exit': child.returncode}); stop(child)
            assert child.returncode == 0, label + ' failed; retain evidence and fix in a usable fresh offer'
        report['status'] = 'PASS'
    except BaseException as error:
        report['failure'] = str(error)
    finally:
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        stop(child)
        if created and link.is_symlink() and link.resolve() == DEPS.resolve():
            link.unlink()
        # Only this offer's private regenerable temporary root may be removed.
        temp = Path(offer['temporary']); assert str(temp).startswith('/private/tmp/mcr-pi12-')
        if temp.exists():
            shutil.rmtree(temp)
        report['finishedUTC'] = dt.datetime.now(dt.timezone.utc).isoformat()
        report['dependencyLinkAbsent'] = not link.exists() and not link.is_symlink()
        report['temporaryAbsent'] = not temp.exists()
        (output / 'controller.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ['samples', 'whole']}))
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
