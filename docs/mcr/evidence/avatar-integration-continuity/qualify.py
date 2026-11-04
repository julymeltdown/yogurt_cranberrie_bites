"""Sequential, bounded developer qualification. Requires an accepted allocator offer.

Adapted from reviewed-persistence-motion/qualify.py at a10e4f409; this round owns
only the continuity paths and the three explicitly transferred selector/footer hunks.
"""
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend')
TESTS = 'src/features/avatar/__tests__/integration-continuity'
EVIDENCE = ROOT / 'docs/mcr/evidence/avatar-integration-continuity'


def size(path):
    return sum(p.lstat().st_size for p in path.rglob('*') if p.is_file() or p.is_symlink()) if path.exists() else 0


def processes():
    return [tuple(map(int, row.split())) for row in sp.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines() if row.strip()]


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
    output = EVIDENCE / attempt
    assert not output.exists()
    output.mkdir(parents=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-avatar-continuity-', dir='/private/tmp'))
    env = os.environ.copy()
    env.update(TMPDIR=str(tmp), XDG_CACHE_HOME=str(tmp / 'cache'), MCR_MOTION_CACHE=str(tmp / 'vite'),
               MCR_FRONTEND_DEPENDENCIES=str(DEPS), MCR_CONTINUITY_SCRATCH=str(tmp),
               MCR_CONTINUITY_OUTPUT=str(output / 'native'), PATH=str(NODE.parent) + ':' + env.get('PATH', ''))
    commands = [('tests', [str(NODE), str(DEPS / 'node_modules/vitest/vitest.mjs'), 'run', '--configLoader', 'runner',
                           '--config', TESTS + '/vitest.config.mjs', '--no-cache', '--reporter', 'dot']),
                ('lint', [str(NODE), str(DEPS / 'node_modules/oxlint/bin/oxlint'), '--deny-warnings', TESTS,
                          'src/features/avatar/motion-adoption/MotionEditor.tsx']),
                ('syntax', [str(NODE), '--check', TESTS + '/native.mjs'])] if mode == 'source' else [
                ('native', [str(NODE), TESTS + '/native.mjs'])]
    report = {'offer': offer, 'mode': mode, 'source': sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'commands': [], 'status': 'FAIL', 'startedUTC': dt.datetime.now(dt.timezone.utc).isoformat(),
              'rssPeak': 0, 'processPeak': 0, 'temporaryPeak': 0, 'durablePeak': 0, 'freeMinimum': shutil.disk_usage(ROOT).free}
    child = None
    owned = set()
    try:
        for label, argv in commands:
            assert time.time() < deadline - 60
            with (output / (label + '.log')).open('wb') as stream:
                child = sp.Popen(argv, cwd=ROOT / 'frontend', env=env, stdout=stream, stderr=sp.STDOUT, start_new_session=True)
                row = {'label': label, 'argv': argv, 'pid': child.pid}; report['commands'].append(row)
                while True:
                    rows = processes(); ids = {child.pid}
                    while True:
                        more = ids | {pid for pid, parent, group, _ in rows if parent in ids or group == child.pid}
                        if more == ids:
                            break
                        ids = more
                    owned.update(ids)
                    rss = sum(rss * 1024 for pid, _, _, rss in rows if pid in ids)
                    temporary, durable, free = size(tmp), size(EVIDENCE), shutil.disk_usage(ROOT).free
                    for key, value in [('rssPeak', rss), ('processPeak', len(ids)), ('temporaryPeak', temporary), ('durablePeak', durable)]:
                        report[key] = max(report[key], value)
                    report['freeMinimum'] = min(report['freeMinimum'], free)
                    assert time.time() < deadline - 60, 'Cleanup deadline'
                    assert free >= offer['continuousFreeBytes'], 'Continuous free floor'
                    assert temporary <= offer['temporaryBytes'] and durable <= offer['durableBytes'], 'Output byte cap'
                    assert rss <= offer['rssBytes'] and len(ids) <= offer['processes'], 'Process/RSS cap'
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
