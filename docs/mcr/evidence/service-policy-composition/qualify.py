"""Sequential, bounded developer qualification. Requires an accepted allocator offer.

Adapted from reviewed-persistence-motion/qualify.py at a10e4f409; this round owns
only the exact Root-delegated servicePolicy composition hunks and NEW integration test/docs grant. Whole accounting uses a full inode-deduplicated census including retained dependencies/tools/shared Git.
"""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import sys
import tempfile
import time
sys.dont_write_bytecode = True
from guardian import sample, breach, fresh_bytes, census, atomic, process_rows, descendants, matches, signal_owned, RECEIPTS, OWNER

ROOT = Path(__file__).resolve().parents[4]
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend')
TESTS = 'src/app/operations-composition/__tests__/service-policy-integration'
EVIDENCE = ROOT / 'docs/mcr/evidence/service-policy-composition'
BASE = 'bd087fde2c67933b53d1666d3656af5ac98a7c19'


def size(path):
    if path.is_file():
        return path.stat().st_size
    return sum(p.lstat().st_size for p in path.rglob('*') if p.is_file() or p.is_symlink()) if path.exists() else 0


def inventory(browser=False):
    common = Path(sp.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=ROOT, text=True).strip())
    if not common.is_absolute(): common = ROOT / common
    roots = [str(ROOT), str(DEPS / 'node_modules'), '/Users/lhs/Desktop/mcr-tools', str(common.resolve())]
    roots += [str(Path(sys.executable).resolve().parent.parent),
              str(Path(shutil.which('git')).resolve().parent.parent),
              sp.check_output(['git', '--exec-path'], cwd=ROOT, text=True).strip(),
              shutil.which('git')]
    if browser:
        roots.append('/Applications/Google Chrome.app')
    return roots


def retire(child, known):
    errors = []
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        errors.extend(signal_owned(known, sig))
        if child is not None:
            try:
                child.wait(timeout=2)
            except sp.TimeoutExpired:
                pass
        time.sleep(.1)
    return errors


def publish(offer, identity, output, state, guards=(), full=None):
    value = {'ownerThreadId': OWNER, 'offerId': offer['id'], 'state': state, 'kind': offer.get('kind', 'source'),
             'acceptedAtEpoch': dt.datetime.fromisoformat(offer['acceptedUTC']).timestamp(),
             'workDeadlineEpoch': dt.datetime.fromisoformat(offer['workDeadline']).timestamp(),
             'fullDeadlineEpoch': dt.datetime.fromisoformat(offer['deadline']).timestamp(),
             'processCap': offer['processes'], 'rssCapBytes': offer['rssBytes'],
             'remainingNewUpperBytes': offer['newBytes'] if state == 'ACCEPTED' else 0,
             'controller': {**identity, 'script': str(Path(__file__).resolve()), 'cwd': str(ROOT), 'outputRoot': str(output)},
             'guardControllers': list(guards), 'receiptPaths': {'accept': str(output / 'ACCEPT.json'), 'full': str(output / 'FULL.json') if full else None},
             'observedAtEpoch': time.time(), 'phase': 'FULL_AFTER_OWNED_CLEANUP' if full else 'ACTUAL_EXECUTION_IDENTITY_SAME_AS_BOOTSTRAP' if guards else 'BOOTSTRAP_BEFORE_HEAVY_CENSUS',
             'caps': offer, 'historicalFullPaths': offer.get('priorFullPaths', [])}
    if full:
        value.update(fullAtEpoch=full['fullAtEpoch'], result=full['status'], remainingOwned=full['remainingOwned'])
    atomic(RECEIPTS / (OWNER + '.json'), value)


def main():
    offer = json.loads(os.environ['MCR_FE38_ACCEPTED_OFFER'])
    mode, attempt = os.environ['MCR_FE38_MODE'], os.environ['MCR_FE38_ATTEMPT']
    assert mode in ['source', 'native'] and attempt.replace('-', '').isalnum()
    accepted = time.time()
    assert accepted < dt.datetime.fromisoformat(offer['acceptByUTC']).timestamp()
    assert offer['id'] in ['FE38-COMPOSITION-SOURCE03-0120', 'FE38-COMPOSITION-SOURCE04-0120', 'FE38-COMPOSITION-SOURCE05-0120', 'FE38-DEFAULTADMIN-NATIVE01-0133', 'FE38-DEFAULTADMIN-NATIVE02-0206', 'FE38-DEFAULTADMIN-NATIVE03-0206', 'FE38-DEFAULTADMIN-NATIVE04-0212', 'FE38-DEFAULTADMIN-NATIVE05-0212', 'FE38-DEFAULTADMIN-NATIVE06-0215', 'FE38-DEFAULTADMIN-NATIVE07-0215']
    assert (mode == 'native') == (offer.get('kind') == 'browser')
    for full_path in offer['priorFullPaths']:
        prior = json.loads(Path(full_path).read_text())
        assert prior.get('state') == 'ACTUAL_FULL' or prior.get('status', '').startswith('ACTUAL_FULL')
        assert prior['remainingOwned'] == []
        assert prior.get('privateTempAbsent', prior.get('privateTemporaryAbsent')) is True
        assert prior['dependencyLinkAbsent'] is True
    work_seconds, full_seconds = (300, 360) if mode == 'native' else (180, 240)
    offer.update(acceptedUTC=dt.datetime.fromtimestamp(accepted, dt.timezone.utc).isoformat(),
                 workDeadline=dt.datetime.fromtimestamp(accepted + work_seconds, dt.timezone.utc).isoformat(),
                 deadline=dt.datetime.fromtimestamp(accepted + full_seconds, dt.timezone.utc).isoformat())
    deadline = accepted + full_seconds
    output = EVIDENCE / attempt
    assert not output.exists()
    output.mkdir(parents=True)
    identity = process_rows()[os.getpid()]
    assert identity['executable'] and identity['birth']
    acceptance = {'state': 'ACTUAL_ACCEPT', 'offer': offer, 'controller': identity,
                  'phase': 'BOOTSTRAP_BEFORE_HEAVY_CENSUS', 'source': offer['source']}
    atomic(output / 'ACCEPT.json', acceptance)
    publish(offer, identity, output, 'ACCEPTED')
    # No census or workload precedes the immutable bootstrap receipt above.
    child = guardian = None
    tmp = None
    config = None
    known = {}
    report = {'offer': offer, 'mode': mode, 'source': offer['source'], 'commands': [], 'status': 'FAIL',
              'startedUTC': offer['acceptedUTC'], 'rssPeak': 0, 'processPeak': 0, 'temporaryPeak': 0,
              'durablePeak': 0, 'wholePeak': offer['minimumCarryBytes'], 'freeMinimum': shutil.disk_usage(ROOT).free}
    signal.signal(signal.SIGTERM, lambda _signal, _frame: (_ for _ in ()).throw(RuntimeError('Independent guardian interrupted allocation')))
    try:
        run(offer, deadline, output, identity, report, known)
    except BaseException as exc:
        report['failure'] = type(exc).__name__ + ': ' + str(exc)
    finally:
        # run stores resources as soon as created, including partial startup.
        config = report.pop('_config', None)
        child = report.pop('_child', None)
        guardian = report.pop('_guardian', None)
        tmp_name = report.pop('_temporary', None)
        tmp = Path(tmp_name) if tmp_name else None
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        # Capture actual descendants even if interruption happened between
        # Popen and its first normal sample. Identity is recorded before signals.
        current = process_rows()
        if matches(current.get(os.getpid()), identity):
            for pid in descendants(current, {os.getpid()}) - {os.getpid()}:
                known[pid] = current[pid]
        if config:
            Path(config['stop']).touch()
        workload = {pid: item for pid, item in known.items() if not guardian or pid != guardian.pid}
        report['cleanupErrors'] = retire(child, workload)
        if guardian:
            try:
                guardian.wait(timeout=5)
            except sp.TimeoutExpired:
                report['cleanupErrors'].extend(retire(guardian, {guardian.pid: known[guardian.pid]}))
            if config and Path(config['report']).exists():
                report['guardian'] = json.loads(Path(config['report']).read_text())
            if guardian.returncode != 0 or report.get('guardian', {}).get('breach'):
                report['status'] = 'FAIL'
        rows = process_rows()
        remaining = [item for pid, item in known.items() if matches(rows.get(pid), item)]
        if tmp and not remaining:
            try:
                shutil.rmtree(tmp)
            except OSError as exc:
                report['cleanupErrors'].append({'temporary': str(tmp), 'error': str(exc)})
        if config:
            Path(config['activeChild']).unlink(missing_ok=True)
        report.update(ownedPids=sorted(known), aliveOwnedPids=[item['pid'] for item in remaining],
                      privateTempAbsent=tmp is None or not tmp.exists(),
                      dependencyLinkAbsent=not (ROOT / 'frontend/node_modules').exists(), endedUTC=dt.datetime.now(dt.timezone.utc).isoformat())
        all_zero = not remaining and report['privateTempAbsent'] and report['dependencyLinkAbsent']
        if mode == 'native':
            probe = sp.run(['/usr/sbin/lsof', '-nP', '-iTCP:5384', '-sTCP:LISTEN'], capture_output=True, text=True)
            report['strictPort5384Closed'] = probe.returncode == 1 and not probe.stdout
            all_zero = all_zero and report['strictPort5384Closed']
        if not all_zero:
            report['status'] = 'FAIL'
        atomic(output / 'supervisor.json', report)
        if all_zero:
            full = {'state': 'ACTUAL_FULL', 'status': report['status'], 'fullAtEpoch': time.time(),
                    'offer': offer, 'remainingOwned': [], 'privateTempAbsent': True, 'dependencyLinkAbsent': True,
                    'browserServerTargetResources': 0, 'supervisor': str(output / 'supervisor.json'),
                    'qualification': [{'label': row['label'], 'exit': row.get('exit')} for row in report['commands']]}
            atomic(output / 'FULL.json', full)
            publish(offer, identity, output, 'FULL', full=full)
        else:
            report['fullNotPublished'] = 'Owned resources remain; original reservation stays accepted'
            atomic(output / 'supervisor.json', report)
    print(json.dumps({'status': report['status'], 'failure': report.get('failure'), 'commands': [
        {'label': row['label'], 'exit': row.get('exit')} for row in report['commands']],
        'output': str(output), 'fullPublished': (output / 'FULL.json').exists()}))
    return 0 if report['status'] == 'PASS' else 1


def run(offer, deadline, output, identity, report, known):
    assert not (ROOT / 'frontend/node_modules').exists(), 'No temporary dependency link permitted'
    assert shutil.disk_usage(ROOT).free >= offer['admissionFreeBytes']
    assert shutil.disk_usage(ROOT).free - offer['newBytes'] - offer['otherUpperBytes'] >= offer['afterAllFreeBytes']
    assert hashlib.sha256((ROOT / 'frontend/pnpm-lock.yaml').read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
    browser = offer.get('kind') == 'browser'
    if browser:
        probe = sp.run(['/usr/sbin/lsof', '-nP', '-iTCP:5384', '-sTCP:LISTEN'], capture_output=True, text=True)
        assert probe.returncode == 1 and not probe.stdout, 'Strict5384 already has a listener; no peer cleanup'
    roots = inventory(browser)
    carry_bound = offer['minimumCarryBytes']
    assert carry_bound < offer['wholeBytes']
    tmp = Path(tempfile.mkdtemp(prefix='mcr-fe38-composition-', dir='/private/tmp'))
    report['_temporary'] = str(tmp)
    env = os.environ.copy()
    env.update(TMPDIR=str(tmp), PYTHONDONTWRITEBYTECODE='1', XDG_CACHE_HOME=str(tmp / 'cache'), MCR_MOTION_CACHE=str(tmp / 'vite'),
               MCR_FRONTEND_DEPENDENCIES=str(DEPS), MCR_FE38_SCRATCH=str(tmp),
               MCR_FE38_OUTPUT=str(output / 'native'), PATH=str(NODE.parent) + ':' + env.get('PATH', ''))
    assert sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == offer['source']
    commands = ([('receipt-regression', [sys.executable, '-B', str(EVIDENCE / 'receipt-regression.py')])] if offer.get('controlRegressionAuthorized') else []) + [('tests', [str(NODE), str(DEPS / 'node_modules/vitest/vitest.mjs'), 'run', '--configLoader', 'runner',
                           '--config', TESTS + '/vitest.config.mjs', '--no-cache', '--reporter', 'dot']),
                ('lint', [str(NODE), str(DEPS / 'node_modules/oxlint/bin/oxlint'), '--deny-warnings', 'src/app/operations-composition/owner.ts', 'src/app/operations-composition/panel.tsx', TESTS]),
                ('types', [str(NODE), '--max-old-space-size=2048', TESTS + '/typecheck.mjs']),
                *[(name + '-syntax', [str(NODE), '--check', TESTS + '/' + name + '.mjs']) for name in ['native', 'vitest.config', 'typecheck']],
                ('control-syntax', [sys.executable, '-B', '-c', 'import ast, pathlib; [ast.parse(pathlib.Path(p).read_text(), filename=p) for p in __import__("sys").argv[1:]]', str(EVIDENCE / 'qualify.py'), str(EVIDENCE / 'guardian.py')])]
    if offer.get('remainingCommands'):
        remaining = set(offer['remainingCommands'])
        assert remaining <= {label for label, _ in commands} and 'receipt-regression' in remaining
        commands = [(label, argv) for label, argv in commands if label in remaining]
    if browser:
        commands = [('actual-default-app-native', [str(NODE), TESTS + '/default-app-native.mjs'])]
    report.update(carryBoundBytes=carry_bound, wholePeak=carry_bound)
    config = {'root': str(ROOT), 'base': BASE, 'supervisor': os.getpid(), 'supervisorIdentity': identity, 'offer': offer, 'mode': 'native' if browser else 'source',
              'temporary': str(tmp), 'evidence': str(EVIDENCE), 'fixedCarry': carry_bound, 'wholeRoots': roots,
              'workDeadline': deadline - 60, 'stop': str(output / 'guard.stop'),
              'ready': str(output / 'guard.ready'),
              'start': str(output / 'guard.start'),
              'activeChild': str(output / 'active-child.pid'), 'report': str(output / 'guardian.json'),
              'spentTemporaryBytes': offer.get('spentTemporaryBytes', 19135232), 'ordinaryReservedBytes': offer.get('ordinaryReservedBytes', 8 * 1024**2),
              'pngDirectory': str(output / 'native') if browser else None,
              'priorPngCount': offer.get('priorPngCount', 0), 'priorPngBytes': offer.get('priorPngBytes', 0),
              'primaryCensus': str(output / 'primary-census.json'), 'independentCensus': str(output / 'independent-census.json'),
              'ordinaryPrograms': offer['ordinaryPrograms'],
              'ordinaryRolePins': offer.get('ordinaryRolePins', []),
              'peerFullReceipts': offer.get('peerFullReceipts', []),
              'freshPaths': ['frontend/src/features/admin/service-policy', 'docs/mcr/frontend/admin-service-policy.md', 'docs/mcr/evidence/admin-service-policy', 'frontend/src/app/operations-composition/owner.ts', 'frontend/src/app/operations-composition/panel.tsx', 'frontend/src/app/operations-composition/__tests__/service-policy-integration', 'docs/mcr/frontend/service-policy-composition.md', 'docs/mcr/evidence/service-policy-composition']}
    report['_config'] = config
    config_path = output / 'guard-config.json'; config_path.write_text(json.dumps(config, indent=2) + '\n')
    guardian = sp.Popen([sys.executable, str(EVIDENCE / 'guardian.py'), str(config_path)], env=env)
    report['_guardian'] = guardian
    try:
        guard_identity = process_rows()[guardian.pid]
        guard_identity['script'] = str(EVIDENCE / 'guardian.py')
        guard_identity['cwd'] = str(ROOT)
        assert guard_identity['executable'] and guard_identity['birth']
        known[guardian.pid] = guard_identity
        publish(offer, identity, output, 'ACCEPTED', [guard_identity])
        Path(config['start']).touch()
        readiness_limit = time.time() + 25
        while not Path(config['ready']).exists():
            assert guardian.poll() is None and time.time() < readiness_limit, 'Independent prechild guard not ready'
            time.sleep(.1)
        initial = sample(config)
        report['inventory'] = initial['wholeInventory']
        report['carryBoundBytes'] = max(carry_bound, initial['wholeInventory']['total'])
        report['initial'] = initial
        atomic(output / 'primary-initial.json', initial)
        atomic(config['primaryCensus'], initial)
        assert breach(config, initial) is None, 'Supervisor prechild guard: ' + str(breach(config, initial))
        independent_initial = json.loads(Path(config['ready']).read_text())
        assert breach(config, independent_initial) is None, 'Independent initial sample failed'
        for label, argv in commands:
            assert time.time() < deadline - 60
            with (output / (label + '.log')).open('wb') as stream:
                child = sp.Popen(argv, cwd=ROOT / 'frontend', env=env, stdout=stream, stderr=sp.STDOUT, start_new_session=True)
                report['_child'] = child
                current = process_rows()
                for pid in descendants(current, {os.getpid()}) - {os.getpid()}:
                    known[pid] = current[pid]
                atomic(config['activeChild'], known)
                row = {'label': label, 'argv': argv, 'pid': child.pid}; report['commands'].append(row)
                while True:
                    values = sample(config); ids = set(values['pids'])
                    atomic(config['primaryCensus'], values)
                    for item in values['ownedIdentities']:
                        if item['pid'] != os.getpid():
                            known[item['pid']] = item
                    atomic(config['activeChild'], known)
                    rss, temporary, durable, free, whole = (values[key] for key in ['rss', 'temporary', 'durable', 'free', 'whole'])
                    report['wholePeak'] = max(report['wholePeak'], whole)
                    for key, value in [('rssPeak', rss), ('processPeak', len(ids)), ('temporaryPeak', temporary), ('durablePeak', durable)]:
                        report[key] = max(report[key], value)
                    report['freeMinimum'] = min(report['freeMinimum'], free)
                    report['newPeak'] = max(report.get('newPeak', 0), values['new'])
                    report['lastGlobal'] = values['global']
                    assert guardian.poll() is None, 'Independent guard ended early'
                    assert breach(config, values) is None, str(breach(config, values))
                    if child.poll() is not None:
                        break
                    time.sleep(.3)
                row['exit'] = child.returncode
                # Independent commands still run after a source failure when
                # the original work clock and guard limits remain satisfied.
                atomic(output / 'commands-progress.json', report['commands'])
                if label == 'receipt-regression':
                    assert child.returncode == 0, 'Owned receipt/cleanup regression failed; product commands withheld'
        report['status'] = 'PASS' if all(row.get('exit') == 0 for row in report['commands']) else 'FAIL'
    finally:
        # Main's exception-resilient finalizer owns all cleanup and FULL.
        pass


if __name__ == '__main__':
    raise SystemExit(main())
