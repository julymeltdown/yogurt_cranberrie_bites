"""Changed-only QA05 source checks. Requires a fresh literal accepted SOURCE offer.

Provenance: bounded process/carry accounting from the preserved FE17 supervisor;
no old allocation, source permissions or runtime authority is inherited.
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

ROOT = Path(__file__).resolve().parents[4]
NODE_ROOT = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64')
NODE = NODE_ROOT / 'bin/node'
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
EVIDENCE = ROOT / 'docs/mcr/evidence/qa05-frontend-recovery'
PREFIXES = ('frontend/e2e/qa05-functional-recovery/',
            'frontend/src/world/network/__tests__/qa05-functional-recovery/',
            'docs/mcr/evidence/qa05-frontend-recovery/')
DOC = 'docs/mcr/qa/qa05-frontend-recovery.md'
PRESERVED = ('8ca330d8966e4551ef0fade957e5ce907646108b',
             'defdc8fc429e658c7f53523304208056477c6a88',
             '4d49d95e6f221cbcc8040f31b0499aa2151ad7ff',
             'f543d43a96848d47021eaa4b0736a27e548ebadc')
BASE = 'a0cb4396d2fe5c333e7863aa5bf4c8b88a357ac9'


def git(*args):
    return sp.check_output(['git', *args], cwd=ROOT, text=True).strip()


def inventory(root):
    total = count = 0
    if root.is_file():
        return {'bytes': root.lstat().st_size, 'entries': 1}
    for parent, dirs, files in os.walk(root, followlinks=False):
        for name in files + [name for name in dirs if (Path(parent) / name).is_symlink()]:
            try:
                total += (Path(parent) / name).lstat().st_size
                count += 1
            except FileNotFoundError:
                pass
    return {'bytes': total, 'entries': count}


def census():
    objects = sorted({line.split()[0] for line in git('rev-list', '--objects', 'HEAD', *PRESERVED).splitlines()})
    sizes = sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'], cwd=ROOT,
                            input='\n'.join(objects) + '\n', text=True)
    roots = [ROOT, DEPS, NODE_ROOT, Path(git('rev-parse', '--absolute-git-dir'))]
    for pattern in ('mcr-fe41-*', 'mcr-fe17-*', 'mcr-qa05-*'):
        roots.extend(Path('/private/tmp').glob(pattern))
    rows = [{'root': str(path), **inventory(path)} for path in dict.fromkeys(roots)]
    history = sum(map(int, sizes.splitlines()))
    return {'roots': rows, 'historyBytes': history, 'historyObjects': len(objects),
            'bytes': history + sum(row['bytes'] for row in rows),
            'entries': len(objects) + sum(row['entries'] for row in rows)}


def processes():
    return [tuple(map(int, line.split())) for line in sp.check_output(
        ['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines() if line.strip()]


def stop(child):
    if child is None:
        return
    # Each child has a new session/process group; only that owned group is killed.
    for sig in (signal.SIGTERM, signal.SIGKILL):
        if any(group == child.pid for _, _, group, _ in processes()):
            try:
                os.killpg(child.pid, sig)
            except ProcessLookupError:
                pass
        try:
            child.wait(timeout=2)
        except sp.TimeoutExpired:
            continue
        if not any(group == child.pid for _, _, group, _ in processes()):
            return
    raise RuntimeError('Owned process group survived cleanup')


def main():
    offer_path, attempt = Path(sys.argv[1]), sys.argv[2]
    assert offer_path.is_absolute() and offer_path.stat().st_mode & 0o077 == 0
    offer = json.loads(offer_path.read_text())
    assert offer['issuer'] == '01a1a21d-39ec-7912-84cb-910058e7a110'
    assert offer['acceptance'] == 'ACCEPT' and offer['mode'] == 'SOURCE'
    assert offer['slot'] and offer['sourceCommit'] == git('rev-parse', 'HEAD')
    deadline = dt.datetime.fromisoformat(offer['workByUTC']).timestamp()
    assert 15 < deadline - time.time() <= 120
    assert ROOT == Path('/Users/lhs/Desktop/mcr-worktrees/fe-ref-map') and Path.cwd() == ROOT
    assert git('branch', '--show-current') == 'codex/mcr-fe-QA05-functional-recovery'
    assert hashlib.sha256(NODE.read_bytes()).hexdigest() == 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a'
    readset = json.loads((EVIDENCE / 'readset.json').read_text())
    for path, digest in {**readset['files'], **readset['readonly']}.items():
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == digest, 'Frozen readset mismatch: ' + path
    assert not (ROOT / 'frontend/node_modules').exists() and not (ROOT / 'frontend/node_modules').is_symlink()
    changed = git('diff', '--name-only', BASE).splitlines()
    untracked = git('ls-files', '--others', '--exclude-standard').splitlines()
    assert all(path == DOC or path.startswith(PREFIXES) for path in changed + untracked)
    source = sorted(path for path in set(changed + untracked) if path.endswith('.mjs') and path.startswith('frontend/'))
    assert source and all((ROOT / path).is_file() for path in source)
    pins = {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in source}
    caps = offer['caps']
    carried = census()
    assert carried['bytes'] <= caps['wholeBytes'] and carried['entries'] <= caps['entries']
    free = shutil.disk_usage(ROOT).free
    assert free >= offer['startFloorBytes']
    assert free - offer['otherReservedBytes'] - caps['newBytes'] >= 9 * 1024**3
    assert attempt.startswith('source-') and '/' not in attempt
    output = EVIDENCE / attempt
    assert not output.exists()
    output.mkdir()
    temporary = Path(tempfile.mkdtemp(prefix='mcr-qa05-source-', dir='/private/tmp'))
    env = {**os.environ, 'TMPDIR': str(temporary), 'TMP': str(temporary), 'TEMP': str(temporary), 'XDG_CACHE_HOME': str(temporary / 'cache')}
    commands = [('oracles', [str(NODE), '--test', str(ROOT / 'frontend/src/world/network/__tests__/qa05-functional-recovery/oracles.test.mjs')]),
                ('lint', [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', *source]),
                *[(f'syntax-{index}', [str(NODE), '--check', path]) for index, path in enumerate(source)],
                ('whitespace', ['git', 'diff', '--check', BASE]),
                ('plan', ['python3', 'scripts/mcr/check-plan.py'])]
    report = {'slot': offer['slot'], 'sourceCommit': offer['sourceCommit'], 'sourcePins': pins,
              'inventory': carried, 'offer': offer, 'commands': [], 'status': 'FAIL',
              'rssPeak': 0, 'processPeak': 0, 'newPeak': 0, 'minimumFree': free,
              'qualification': 'Synthetic changed-source checks only; combined actual runtime NOT_RUN.'}
    child = None
    try:
        for label, argv in commands:
            assert time.time() < deadline - 15
            with (output / (label + '.log')).open('wb') as log:
                child = sp.Popen(argv, cwd=ROOT, env=env, stdout=log, stderr=sp.STDOUT, start_new_session=True)
                started = time.time()
                while True:
                    rows = processes()
                    owned = {os.getpid()}
                    while True:
                        added = owned | {pid for pid, parent, _, _ in rows if parent in owned}
                        if added == owned:
                            break
                        owned = added
                    rss = sum(rss * 1024 for pid, _, _, rss in rows if pid in owned)
                    temp, durable = inventory(temporary), inventory(output)
                    growth, free = temp['bytes'] + durable['bytes'], shutil.disk_usage(ROOT).free
                    report['rssPeak'] = max(report['rssPeak'], rss)
                    report['processPeak'] = max(report['processPeak'], len(owned))
                    report['newPeak'] = max(report['newPeak'], growth)
                    report['minimumFree'] = min(report['minimumFree'], free)
                    assert time.time() < deadline - 15 and time.time() - started < 60
                    assert rss <= caps['rssBytes'] and len(owned) <= caps['processes']
                    assert temp['bytes'] <= caps['temporaryBytes'] and durable['bytes'] <= caps['durableBytes']
                    assert growth <= caps['newBytes'] and carried['bytes'] + growth <= caps['wholeBytes']
                    assert carried['entries'] + temp['entries'] + durable['entries'] <= caps['entries']
                    assert free >= 8 * 1024**3 and free - offer['otherReservedBytes'] - caps['newBytes'] >= 9 * 1024**3
                    if child.poll() is not None:
                        break
                    time.sleep(.25)
                report['commands'].append({'label': label, 'argv': argv, 'exit': child.returncode})
                stop(child)
                assert child.returncode == 0, label + ' failed; retain raw log and fix before another run'
        assert pins == {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in source}
        report['status'] = 'PASS'
    except BaseException as error:
        report['failure'] = type(error).__name__ + ': ' + str(error)
    finally:
        stop(child)
        shutil.rmtree(temporary)
        report['temporaryAbsent'] = not temporary.exists()
        report['ownedProcessGroupsAbsent'] = child is None or not any(group == child.pid for _, _, group, _ in processes())
        report['finishedUTC'] = dt.datetime.now(dt.timezone.utc).isoformat()
        (output / 'supervisor.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({key: report[key] for key in ('status', 'commands', 'rssPeak', 'processPeak', 'newPeak', 'minimumFree', 'temporaryAbsent', 'ownedProcessGroupsAbsent', 'finishedUTC')}))
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
