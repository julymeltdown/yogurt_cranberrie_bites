"""Source-only preparation: execute only after a fresh explicit Root reviewed characters and default entry allocation."""
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import tempfile
import time

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish')
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')

def bytes_in(root):
    total = 0
    for directory, _, files in os.walk(root, followlinks=False):
        for name in files:
            try:
                total += (Path(directory) / name).lstat().st_size
            except FileNotFoundError:
                pass  # Chromium retires transient profile entries while it runs.
    return total

def processes():
    return [tuple(map(int, line.split())) for line in sp.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines() if line.strip()]

def descendants(pid, rows):
    found = {pid}
    while True:
        expanded = found | {p for p, parent, _, _ in rows if parent in found}
        if expanded == found:
            return found
        found = expanded

def retire(child):
    if child is None:
        return
    # The entire original session group is ours, including a surviving browser
    # after the Node leader exits. Never kill unrelated processes by name.
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
        if not any(group == child.pid for _, _, group, _ in processes()):
            return
        time.sleep(.2)

def main():
    slot, pin = os.environ['MCR_ROOT_REVIEWED_SLOT'], os.environ['MCR_ROOT_REVIEWED_PIN']
    deadline = dt.datetime.fromisoformat(os.environ['MCR_ROOT_REVIEWED_DEADLINE']).timestamp()
    carry = int(os.environ['MCR_ROOT_REVIEWED_CARRY_BYTES'])
    output = (ROOT / os.environ['MCR_ROOT_REVIEWED_OUTPUT']).resolve()
    assert slot.startswith('ROOT-REVIEWED-FE41-NATIVE-')
    assert sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == pin
    assert not sp.check_output(['git', 'status', '--porcelain'], cwd=ROOT)
    assert output.is_relative_to(ROOT / 'docs/mcr/evidence/customize-root-adoption') and not output.exists()
    assert 120 < deadline - time.time() <= 420
    assert shutil.disk_usage(ROOT).free >= 16 * 1024**3
    link = ROOT / 'frontend/node_modules'
    assert not link.exists() and not link.is_symlink()
    output.mkdir(parents=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-root-reviewed-fe41-', dir='/private/tmp'))
    env = {**os.environ, 'TMPDIR': str(tmp), 'XDG_CACHE_HOME': str(tmp / 'cache'), 'PATH': str(NODE.parent) + ':' + os.environ.get('PATH', ''),
           'MCR_REVIEWED_NATIVE_SCRATCH': str(tmp / 'reviewed'), 'MCR_REVIEWED_NATIVE_OUTPUT': str(output / 'reviewed'),
           'FE41_SCRATCH': str(tmp / 'fe41'), 'FE41_OUTPUT': str(output / 'fe41')}
    driver = ROOT / 'frontend/src/app/character-entry/__tests__/reviewed-production/default-entry/native.mjs'
    fe41 = ROOT / 'frontend/src/app/errors/__tests__/default-entry/native.mjs'
    report = {'slot': slot, 'pin': pin, 'started': dt.datetime.now(dt.timezone.utc).isoformat(),
              'qualification': 'ACTUAL_MAIN_SYNTHETIC_REVIEWED_AND_FE41_NOT_LIVE', 'commands': [], 'carryBytes': carry,
              'wholePeak': carry, 'temporaryPeak': 0, 'durablePeak': 0, 'rssPeak': 0, 'ownedPids': [], 'processPeak': 0}
    child = None
    owned = set()
    status = 'FAIL'
    try:
        link.symlink_to(DEPS, target_is_directory=True)
        commands = [('syntax-fe41', [str(NODE), '--check', str(fe41)]), ('syntax-reviewed', [str(NODE), '--check', str(driver)]),
                    ('fe41-native', [str(NODE), str(fe41)]), ('reviewed-native', [str(NODE), str(driver)])]
        report['retainedPriorTypes'] = 'Full application noEmit incrementalfalse at32fed3eb passed in initial1417allocation; all TypeScript postimages remain exact; new changes are native MJS harness and evidence only'
        for label, argv in commands:
            assert time.time() < deadline - 60
            with (output / (label + '.log')).open('wb') as log:
                child = sp.Popen(argv, cwd=ROOT / 'frontend', env=env, stdout=log, stderr=sp.STDOUT, start_new_session=True)
                record = {'label': label, 'argv': argv, 'pid': child.pid, 'startedEpoch': time.time()}
                report['commands'].append(record)
                owned.add(child.pid)
                while child.poll() is None:
                    rows = processes(); ids = descendants(os.getpid(), rows); owned.update(ids - {os.getpid()})
                    temporary, durable = bytes_in(tmp), bytes_in(output)
                    rss = sum(r * 1024 for p, _, _, r in rows if p in ids)
                    free = shutil.disk_usage(ROOT).free
                    report['wholePeak'] = max(report['wholePeak'], carry + temporary + durable)
                    report['temporaryPeak'] = max(report['temporaryPeak'], temporary)
                    report['durablePeak'] = max(report['durablePeak'], durable)
                    report['rssPeak'] = max(report['rssPeak'], rss)
                    report['processPeak'] = max(report['processPeak'], len(ids))
                    report['minimumFree'] = min(report.get('minimumFree', free), free)
                    if (time.time() >= deadline - 60 or time.time() - record['startedEpoch'] >= 300
                            or carry + temporary + durable > 224 * 1024**2 or temporary > 96 * 1024**2
                            or durable > 32 * 1024**2 or rss > 4 * 1024**3 or len(ids) > 48 or free < 8 * 1024**3):
                        raise RuntimeError('Original time/whole/TMP/durable/RSS/process/free bound reached')
                    time.sleep(.2)
                record.update(exit=child.returncode, endedEpoch=time.time())
                if child.returncode:
                    raise RuntimeError(label + ' failed; retained raw log and partial evidence')
                retire(child)
        status = 'PASS'
    except BaseException as exc:
        report['failure'] = type(exc).__name__ + ': ' + str(exc)
    finally:
        retire(child)
        if link.is_symlink() and link.resolve() == DEPS:
            link.unlink()
        shutil.rmtree(tmp)
        alive = {p for p, _, _, _ in processes()}
        report['ownedPids'] = sorted(owned)
        report['aliveOwnedPids'] = sorted(owned & alive)
        if report['aliveOwnedPids']:
            status = 'FAIL'
        report.update(status=status, ended=dt.datetime.now(dt.timezone.utc).isoformat(),
                      linkAbsent=not link.exists() and not link.is_symlink(), privateTempAbsent=not tmp.exists(), finalFree=shutil.disk_usage(ROOT).free)
        (output / 'supervisor.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))
    return 0 if status == 'PASS' else 1

if __name__ == '__main__':
    raise SystemExit(main())
