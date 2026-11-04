#!/usr/bin/env python3
"""Independent live outer guard. Pins and stands before launching the primary."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode = True
from git_carry import inventory

ROOT = Path(__file__).resolve().parents[4]
BASE = Path(__file__).resolve().parent
GIB = 1024 ** 3
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
CHROME = Path('/Applications/Google Chrome.app')


def allocated(path):
    return int(subprocess.check_output(['/usr/bin/du', '-sk', str(path)], text=True).split()[0]) * 1024


def free():
    fs = os.statvfs(ROOT)
    return fs.f_bavail * fs.f_frsize


def stamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def main():
    mode = os.environ['MCR_OPS_CLASS']
    assert mode in ('source', 'native')
    source_plan = os.environ.get('MCR_OPS_SOURCE_PLAN', 'full')
    assert source_plan in ('full', 'types-lint')
    process_cap = 8 if mode == 'source' and source_plan == 'types-lint' else 32
    run = os.environ['MCR_OPS_RUN']
    assert run.startswith(mode + '-') and all(c.islower() or c.isdigit() or c == '-' for c in run)
    work = datetime.datetime.fromisoformat(os.environ['MCR_OPS_WORK_UTC'].replace('Z', '+00:00')).timestamp()
    hard = datetime.datetime.fromisoformat(os.environ['MCR_OPS_HARD_UTC'].replace('Z', '+00:00')).timestamp()
    assert time.time() < work < hard
    assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT).strip()
    primary = BASE / 'run-allocated.py'
    controls = (primary, Path(__file__), BASE / 'git_carry.py')
    pins = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in controls}
    gitdir = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
    dependency = max(allocated(DEPS), sum(p.stat().st_size for p in DEPS.rglob('*') if p.is_file()))
    chrome_charge = max(allocated(CHROME), sum(p.stat().st_size for p in CHROME.rglob('*')
                        if p.is_file() and not p.is_symlink())) if mode == 'native' else 0
    whole_cap = (2 if mode == 'source' else 4) * GIB
    owned_git = inventory(ROOT)
    carry = max(allocated(ROOT) + allocated(gitdir) + dependency + NODE.stat().st_size + chrome_charge + owned_git['bytes'],
                int(os.environ['MCR_OPS_PRIOR_CARRY_BYTES']))
    new_cap = (128 if mode == 'source' else 200) * 1024 ** 2
    assert carry < whole_cap and free() >= 12 * GIB
    assert free() - int(os.environ['MCR_OPS_OTHER_RESERVE_BYTES']) - new_cap >= 9 * GIB
    temporary = Path(tempfile.mkdtemp(prefix='mcr-reports-outer-', dir='/private/tmp'))
    report = {'allocation': os.environ['MCR_OPS_ALLOCATION'], 'outerPid': os.getpid(), 'pins': pins,
              'standingUtc': stamp(), 'carryBytes': carry, 'ownedSharedGitInventory': owned_git, 'samples': [], 'status': 'FAIL'}
    (temporary / 'standing.json').write_text(json.dumps(report))
    env = {**os.environ, 'MCR_OPS_OUTER_PID': str(os.getpid()), 'MCR_OPS_OUTER_TMP': str(temporary),
           'MCR_OPS_OUTER_SHA256': pins[Path(__file__).name]}
    child = None
    groups = set()
    known = set()
    try:
        with (temporary / 'primary.log').open('wb') as log:
            child = subprocess.Popen([sys.executable, str(primary)], cwd=ROOT, env=env,
                                     stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            groups.add(child.pid)
            while child.poll() is None:
                owned_git['bytes'] = max(owned_git['bytes'], inventory(ROOT)['bytes'])
                rows = [list(map(int, line.split())) for line in subprocess.check_output(
                    ['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines()]
                selected = {os.getpid()} | known
                while True:
                    expanded = selected | {pid for pid, parent, group, _ in rows if parent in selected or group in groups}
                    if expanded == selected: break
                    selected = expanded
                live = [(pid, group, rss * 1024) for pid, _, group, rss in rows if pid in selected]
                known.update(pid for pid, _, _ in live if pid != os.getpid())
                groups.update(group for pid, group, _ in live if pid != os.getpid() and group != os.getpgrp())
                whole = allocated(ROOT) + allocated(gitdir) + dependency + NODE.stat().st_size + chrome_charge + owned_git['bytes']
                temporary_bytes = allocated(temporary)
                child_temporary = temporary / 'child-temporary.json'
                if child_temporary.is_file():
                    path = Path(json.loads(child_temporary.read_text()))
                    assert path.parent == Path('/private/tmp') and path.name.startswith('mcr-operations-current-')
                    if path.exists(): temporary_bytes += allocated(path)
                sample = {'utc': stamp(), 'pids': [pid for pid, _, _ in live], 'processes': len(live),
                          'rssBytes': sum(rss for _, _, rss in live), 'wholeBytes': whole + temporary_bytes,
                          'newBytes': max(0, whole + temporary_bytes - carry), 'temporaryBytes': temporary_bytes,
                          'durableBytes': allocated(BASE), 'freeBytes': free()}
                report['samples'].append(sample)
                assert sample['processes'] <= process_cap and sample['rssBytes'] <= 4 * GIB
                assert sample['wholeBytes'] <= whole_cap and sample['newBytes'] <= new_cap
                assert temporary_bytes <= (96 if mode == 'source' else 160) * 1024 ** 2
                assert sample['durableBytes'] <= (32 if mode == 'source' else 40) * 1024 ** 2
                assert sample['freeBytes'] >= 8 * GIB
                if time.time() >= work:
                    report['workDeadlineReached'] = True
                    for group in groups - {child.pid}:
                        try: os.killpg(group, signal.SIGTERM)
                        except ProcessLookupError: pass
                assert time.time() < hard - 5, 'Outer deadline leaves five seconds for final reap'
                time.sleep(.25)
        report['exitCode'] = child.returncode
        assert child.returncode == 0, 'Primary failed; logs preserved'
        assert not report.get('workDeadlineReached'), 'Work deadline reached; cleanup preserved'
        assert pins == {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in controls}
        report['status'] = 'PASS'
    except Exception as error:
        report['failure'] = str(error)
    finally:
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
        rows = [list(map(int, line.split())) for line in subprocess.check_output(
            ['ps', '-axo', 'pid=,ppid=,pgid='], text=True).splitlines()]
        remaining = [pid for pid, _, group in rows if group in groups and pid != os.getpid()]
        for pid in remaining:
            try: os.kill(pid, signal.SIGKILL)
            except ProcessLookupError: pass
        report['remainingBeforeFinalReap'] = remaining
        rows = [list(map(int, line.split())) for line in subprocess.check_output(
            ['ps', '-axo', 'pid=,ppid=,pgid='], text=True).splitlines()]
        report['remainingProcesses'] = [pid for pid, _, group in rows if group in groups and pid != os.getpid()]
        if report['remainingProcesses']: report['status'] = 'FAIL'
        output = BASE / 'runs' / run
        output.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(temporary / 'primary.log', output / 'outer-primary.log')
        shutil.rmtree(temporary)
        report.update(fullUtc=stamp(), outerTemporaryAbsent=not temporary.exists(),
                      finishedBeforeHardDeadline=time.time() < hard, freeAfterBytes=free())
        if not report['finishedBeforeHardDeadline']: report['status'] = 'FAIL'
        (output / 'outer-release.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({k: v for k, v in report.items() if k != 'samples'}), flush=True)
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
