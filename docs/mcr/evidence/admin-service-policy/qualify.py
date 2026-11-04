"""Sequential, bounded developer qualification. Requires an accepted allocator offer.

Adapted from reviewed-persistence-motion/qualify.py at a10e4f409; this round owns
only the NEW FE38 service-policy module/test/docs grant. Whole accounting uses a full inode-deduplicated census including retained dependencies/tools/shared Git.
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
from guardian import sample, breach, fresh_bytes, census

ROOT = Path(__file__).resolve().parents[4]
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend')
TESTS = 'src/features/admin/service-policy/__tests__'
EVIDENCE = ROOT / 'docs/mcr/evidence/admin-service-policy'
BASE = 'bd087fde2c67933b53d1666d3656af5ac98a7c19'


def size(path):
    if path.is_file():
        return path.stat().st_size
    return sum(p.lstat().st_size for p in path.rglob('*') if p.is_file() or p.is_symlink()) if path.exists() else 0


def processes():
    return [tuple(map(int, row.split())) for row in sp.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines() if row.strip()]


def inventory():
    common = Path(sp.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=ROOT, text=True).strip())
    if not common.is_absolute(): common = ROOT / common
    roots = [str(ROOT), str(DEPS / 'node_modules'), '/Users/lhs/Desktop/mcr-tools', str(common.resolve())]
    result = census(roots)
    return roots, result


def retire(child):
    if child is None:
        return
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        if any(group == child.pid for _, _, group, _ in processes()):
            try:
                os.killpg(child.pid, sig)
            except ProcessLookupError:
                pass
        try:
            child.wait(timeout=3)
        except sp.TimeoutExpired:
            continue


def main():
    offer = json.loads(os.environ['MCR_FE38_ACCEPTED_OFFER'])
    deadline = dt.datetime.fromisoformat(offer['deadline']).timestamp()
    mode, attempt = os.environ['MCR_FE38_MODE'], os.environ['MCR_FE38_ATTEMPT']
    assert mode in ['source', 'native'] and attempt.replace('-', '').isalnum()
    assert deadline - time.time() > 75
    assert not (ROOT / 'frontend/node_modules').exists(), 'No temporary dependency link permitted'
    assert shutil.disk_usage(ROOT).free >= offer['admissionFreeBytes']
    assert shutil.disk_usage(ROOT).free - offer['newBytes'] - offer['otherUpperBytes'] >= offer['afterAllFreeBytes']
    assert hashlib.sha256((ROOT / 'frontend/pnpm-lock.yaml').read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
    roots, carry = inventory()
    carry_bound = max(carry['total'], offer.get('minimumCarryBytes', 0))
    assert carry_bound < offer['wholeBytes']
    output = EVIDENCE / attempt
    assert not output.exists()
    output.mkdir(parents=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-fe38-policy-', dir='/private/tmp'))
    env = os.environ.copy()
    env.update(TMPDIR=str(tmp), PYTHONDONTWRITEBYTECODE='1', XDG_CACHE_HOME=str(tmp / 'cache'), MCR_MOTION_CACHE=str(tmp / 'vite'),
               MCR_FRONTEND_DEPENDENCIES=str(DEPS), MCR_FE38_SCRATCH=str(tmp),
               MCR_FE38_OUTPUT=str(output / 'native'), PATH=str(NODE.parent) + ':' + env.get('PATH', ''))
    assert sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == offer['source']
    commands = [('tests', [str(NODE), str(DEPS / 'node_modules/vitest/vitest.mjs'), 'run', '--configLoader', 'runner',
                           '--config', TESTS + '/vitest.config.mjs', '--no-cache', '--reporter', 'dot']),
                ('lint', [str(NODE), str(DEPS / 'node_modules/oxlint/bin/oxlint'), '--deny-warnings', 'src/features/admin/service-policy']),
                ('types', [str(NODE), '--max-old-space-size=2048', TESTS + '/typecheck.mjs']),
                *[(name + '-syntax', [str(NODE), '--check', TESTS + '/' + name + '.mjs']) for name in ['native', 'vitest.config', 'typecheck']]] if mode == 'source' else [
                ('native', [str(NODE), TESTS + '/native.mjs'])]
    report = {'offer': offer, 'mode': mode, 'source': sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'commands': [], 'inventory': carry, 'carryBoundBytes': carry_bound, 'wholePeak': carry_bound, 'status': 'FAIL', 'startedUTC': dt.datetime.now(dt.timezone.utc).isoformat(),
              'rssPeak': 0, 'processPeak': 0, 'temporaryPeak': 0, 'durablePeak': 0, 'freeMinimum': shutil.disk_usage(ROOT).free}
    child = None
    owned = set()
    config = {'root': str(ROOT), 'base': BASE, 'supervisor': os.getpid(), 'offer': offer,
              'temporary': str(tmp), 'evidence': str(EVIDENCE), 'fixedCarry': carry_bound, 'wholeRoots': roots,
              'workDeadline': deadline - 60, 'stop': str(output / 'guard.stop'),
              'ready': str(output / 'guard.ready'),
              'activeChild': str(output / 'active-child.pid'), 'report': str(output / 'guardian.json'),
              'freshPaths': ['frontend/src/features/admin/service-policy', 'docs/mcr/frontend/admin-service-policy.md', 'docs/mcr/evidence/admin-service-policy']}
    assert fresh_bytes(config) + size(tmp) <= offer['newBytes']
    config_path = output / 'guard-config.json'; config_path.write_text(json.dumps(config, indent=2) + '\n')
    guardian = sp.Popen([sys.executable, str(EVIDENCE / 'guardian.py'), str(config_path)], env=env)
    owned.add(guardian.pid)
    def interrupted(_signal, _frame):
        raise RuntimeError('Independent guardian interrupted allocation')
    signal.signal(signal.SIGTERM, interrupted)
    try:
        readiness_limit = time.time() + 5
        while not Path(config['ready']).exists():
            assert guardian.poll() is None and time.time() < readiness_limit, 'Independent prechild guard not ready'
            time.sleep(.1)
        assert breach(config, sample(config)) is None, 'Supervisor prechild guard'
        for label, argv in commands:
            assert time.time() < deadline - 60
            with (output / (label + '.log')).open('wb') as stream:
                child = sp.Popen(argv, cwd=ROOT / 'frontend', env=env, stdout=stream, stderr=sp.STDOUT, start_new_session=True)
                Path(config['activeChild']).write_text(str(child.pid))
                row = {'label': label, 'argv': argv, 'pid': child.pid}; report['commands'].append(row)
                while True:
                    values = sample(config); ids = set(values['pids']); owned.update(ids - {os.getpid()})
                    rss, temporary, durable, free, whole = (values[key] for key in ['rss', 'temporary', 'durable', 'free', 'whole'])
                    report['wholePeak'] = max(report['wholePeak'], whole)
                    for key, value in [('rssPeak', rss), ('processPeak', len(ids)), ('temporaryPeak', temporary), ('durablePeak', durable)]:
                        report[key] = max(report[key], value)
                    report['freeMinimum'] = min(report['freeMinimum'], free)
                    report['newPeak'] = max(report.get('newPeak', 0), values['new'])
                    assert guardian.poll() is None, 'Independent guard ended early'
                    assert breach(config, values) is None, str(breach(config, values))
                    if child.poll() is not None:
                        break
                    time.sleep(.3)
                row['exit'] = child.returncode
                assert child.returncode == 0, label + ' failed'
        report['status'] = 'PASS'
    except BaseException as exc:
        report['failure'] = type(exc).__name__ + ': ' + str(exc)
    finally:
        retire(child)
        Path(config['stop']).touch()
        guardian.wait(timeout=5)
        report['guardian'] = json.loads(Path(config['report']).read_text())
        if guardian.returncode != 0:
            report['status'] = 'FAIL'
        Path(config['activeChild']).unlink(missing_ok=True)
        shutil.rmtree(tmp)
        live = {pid for pid, _, _, _ in processes()}
        report.update(ownedPids=sorted(owned), aliveOwnedPids=sorted(owned & live), privateTempAbsent=not tmp.exists(),
                      dependencyLinkAbsent=not (ROOT / 'frontend/node_modules').exists(), endedUTC=dt.datetime.now(dt.timezone.utc).isoformat())
        if report['aliveOwnedPids']:
            report['status'] = 'FAIL'
        (output / 'supervisor.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
