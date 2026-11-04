"""Sequential, bounded developer qualification. Requires an accepted allocator offer.

Adapted from reviewed-persistence-motion/qualify.py at a10e4f409; this round owns
only the continuity paths and the three explicitly transferred selector/footer hunks.
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
from guardian import sample, breach, fresh_bytes

ROOT = Path(__file__).resolve().parents[4]
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend')
TESTS = 'src/features/avatar/__tests__/integration-continuity'
EVIDENCE = ROOT / 'docs/mcr/evidence/avatar-integration-continuity'
BASE = 'a10e4f409f5727cb25b6cc37d75f499a73e9bb70'


def size(path):
    return sum(p.lstat().st_size for p in path.rglob('*') if p.is_file() or p.is_symlink()) if path.exists() else 0


def processes():
    return [tuple(map(int, row.split())) for row in sp.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines() if row.strip()]


def inventory():
    prefixes = ['frontend/src/entities/avatar', 'frontend/src/features/avatar', 'frontend/src/world/avatar',
                'frontend/src/app/character-entry', 'frontend/public/world/avatars', 'docs/mcr/art',
                'docs/mcr/evidence/avatar-motion-adoption', 'docs/mcr/evidence/customize-reviewed-production',
                'docs/mcr/evidence/avatar-part-controls', 'docs/mcr/evidence/reviewed-persistence-motion',
                'docs/mcr/evidence/avatar-integration-continuity']
    result = {path: size(ROOT / path) for path in prefixes}
    objects = {row.split()[0] for row in sp.check_output(['git', 'rev-list', '--objects', BASE + '..HEAD'], cwd=ROOT, text=True).splitlines()}
    result['sourceGitObjects'] = sum(int(row) for row in sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'],
        cwd=ROOT, input='\n'.join(sorted(objects)) + '\n', text=True).splitlines()) if objects else 0
    result['worktreeGit'] = size(Path(sp.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip()))
    return result


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
    offer = json.loads(os.environ['MCR_CONTINUITY_ACCEPTED_OFFER'])
    deadline = dt.datetime.fromisoformat(offer['deadline']).timestamp()
    mode, attempt = os.environ['MCR_CONTINUITY_MODE'], os.environ['MCR_CONTINUITY_ATTEMPT']
    assert mode in ['source', 'native'] and attempt.replace('-', '').isalnum()
    assert deadline - time.time() > 75
    assert shutil.disk_usage(ROOT).free >= offer['admissionFreeBytes']
    assert shutil.disk_usage(ROOT).free - offer['newBytes'] - offer['otherUpperBytes'] >= offer['afterAllFreeBytes']
    assert hashlib.sha256((ROOT / 'frontend/pnpm-lock.yaml').read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
    carry = inventory()
    assert sum(carry.values()) < offer['wholeBytes']
    output = EVIDENCE / attempt
    assert not output.exists()
    output.mkdir(parents=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-avatar-continuity-', dir='/private/tmp'))
    env = os.environ.copy()
    env.update(TMPDIR=str(tmp), PYTHONDONTWRITEBYTECODE='1', XDG_CACHE_HOME=str(tmp / 'cache'), MCR_MOTION_CACHE=str(tmp / 'vite'),
               MCR_FRONTEND_DEPENDENCIES=str(DEPS), MCR_CONTINUITY_SCRATCH=str(tmp),
               MCR_CONTINUITY_OUTPUT=str(output / 'native'), PATH=str(NODE.parent) + ':' + env.get('PATH', ''))
    commands = [('tests', [str(NODE), str(DEPS / 'node_modules/vitest/vitest.mjs'), 'run', '--configLoader', 'runner',
                           '--config', TESTS + '/vitest.config.mjs', '--no-cache', '--reporter', 'dot']),
                ('lint', [str(NODE), str(DEPS / 'node_modules/oxlint/bin/oxlint'), '--deny-warnings', TESTS,
                          'src/features/avatar/motion-adoption/MotionEditor.tsx']),
                ('types', [str(NODE), '--max-old-space-size=2048', TESTS + '/typecheck.mjs']),
                ('syntax', [str(NODE), '--check', TESTS + '/native.mjs'])] if mode == 'source' else [
                ('native', [str(NODE), TESTS + '/native.mjs'])]
    report = {'offer': offer, 'mode': mode, 'source': sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'commands': [], 'inventory': carry, 'wholePeak': sum(carry.values()), 'status': 'FAIL', 'startedUTC': dt.datetime.now(dt.timezone.utc).isoformat(),
              'rssPeak': 0, 'processPeak': 0, 'temporaryPeak': 0, 'durablePeak': 0, 'freeMinimum': shutil.disk_usage(ROOT).free}
    child = None
    owned = set()
    config = {'root': str(ROOT), 'base': BASE, 'supervisor': os.getpid(), 'offer': offer,
              'temporary': str(tmp), 'evidence': str(EVIDENCE), 'fixedCarry': sum(carry.values()),
              'workDeadline': deadline - 60, 'stop': str(output / 'guard.stop'),
              'ready': str(output / 'guard.ready'),
              'activeChild': str(output / 'active-child.pid'), 'report': str(output / 'guardian.json'),
              'freshPaths': ['frontend/src/features/avatar/__tests__/integration-continuity',
                  'frontend/src/features/avatar/integration-continuity', 'docs/mcr/frontend/avatar-integration-continuity.md',
                  'docs/mcr/evidence/avatar-integration-continuity',
                  'frontend/src/features/avatar/motion-adoption/MotionEditor.tsx',
                  'frontend/src/features/avatar/__tests__/reviewed-production/owner.test.tsx',
                  'frontend/src/app/character-entry/__tests__/reviewed-production/default-entry/native.mjs']}
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
