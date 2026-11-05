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
    assert source_plan in ('full', 'types-lint', 'lint-native')
    process_cap = 8 if mode == 'source' else 32
    run = os.environ['MCR_OPS_RUN']
    assert run.startswith(mode + '-') and all(c.islower() or c.isdigit() or c == '-' for c in run)
    work = datetime.datetime.fromisoformat(os.environ['MCR_OPS_WORK_UTC'].replace('Z', '+00:00')).timestamp()
    hard = datetime.datetime.fromisoformat(os.environ['MCR_OPS_HARD_UTC'].replace('Z', '+00:00')).timestamp()
    assert time.time() < work < hard
    assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT).strip()
    discovery = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a230-6acc-7e82-9e4b-92506decc745.json')
    bootstrap = {'ownerThreadId': '01a1a230-6acc-7e82-9e4b-92506decc745', 'offerId': os.environ['MCR_OPS_ALLOCATION'],
        'state': 'ACCEPTED', 'acceptedAtEpoch': datetime.datetime.fromisoformat(os.environ['MCR_OPS_ACCEPTED_UTC']).timestamp(),
        'workDeadlineEpoch': work, 'fullDeadlineEpoch': hard, 'kind': 'browser' if mode == 'native' else 'source',
        'processCap': process_cap, 'rssCapBytes': (4 if mode == 'native' else 3) * GIB,
        'remainingNewUpperBytes': (200 if mode == 'native' else 128) * 1024 ** 2,
        'controller': {'pid': os.getpid(), 'birth': subprocess.check_output(['ps', '-p', str(os.getpid()), '-o', 'lstart='], text=True).strip(),
            'executable': subprocess.check_output(['ps', '-p', str(os.getpid()), '-o', 'comm='], text=True).strip(),
            'script': str(Path(__file__).resolve()), 'cwd': str(ROOT), 'outputRoot': str(BASE / 'runs' / run)},
        'guardControllers': [], 'receiptPaths': {'accept': str(BASE / 'runs' / run / 'bootstrap-accepted.json'),
            'full': str(BASE / 'runs' / run / 'outer-release.json')}, 'publishedAtEpoch': time.time(),
        'stage': 'Actual outer Python, before expensive carry census and before primary launch'}
    pending = discovery.with_name('.' + discovery.name + '.' + str(os.getpid()) + '.tmp')
    pending.write_text(json.dumps(bootstrap, indent=2) + '\n'); os.replace(pending, discovery)
    primary = BASE / 'run-allocated.py'
    controls = (primary, Path(__file__), BASE / 'git_carry.py')
    pins = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in controls}
    gitdir = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
    dependency = max(allocated(DEPS), sum(p.stat().st_size for p in DEPS.rglob('*') if p.is_file()))
    chrome_charge = max(allocated(CHROME), sum(p.stat().st_size for p in CHROME.rglob('*')
                        if p.is_file() and not p.is_symlink())) if mode == 'native' else 0
    whole_cap = 8 * GIB
    python_root = Path(sys.prefix)
    python_charge = max(allocated(python_root), sum(p.stat().st_size for p in python_root.rglob('*') if p.is_file()))
    foundation = {'bytes': 0, 'postimages': [], 'qualification': 'Canonical source only; no PI01 overlay'}
    foundation_charge = foundation['bytes']
    owned_git = inventory(ROOT)
    actual_baseline = allocated(ROOT) + allocated(gitdir) + dependency + NODE.stat().st_size + chrome_charge + owned_git['bytes'] + python_charge + foundation_charge
    carry = max(allocated(ROOT) + allocated(gitdir) + dependency + NODE.stat().st_size + chrome_charge + owned_git['bytes'] + python_charge + foundation_charge,
                int(os.environ['MCR_OPS_PRIOR_CARRY_BYTES']))
    new_cap = (128 if mode == 'source' else 200) * 1024 ** 2
    assert carry + new_cap <= whole_cap and free() >= 12 * GIB
    assert free() - int(os.environ['MCR_OPS_OTHER_RESERVE_BYTES']) - new_cap >= 9 * GIB
    temporary = Path(tempfile.mkdtemp(prefix='mcr-fe37-mc-outer-', dir='/private/tmp'))
    report = {'allocation': os.environ['MCR_OPS_ALLOCATION'], 'outerPid': os.getpid(), 'pins': pins,
              'readOnlyFoundation': foundation, 'standingUtc': stamp(), 'carryBytes': carry, 'entirePythonPrefix': sys.prefix, 'entirePythonChargeBytes': python_charge, 'ownedSharedGitInventory': owned_git, 'samples': [], 'status': 'FAIL'}
    env = {**os.environ, 'MCR_OPS_OUTER_PID': str(os.getpid()), 'MCR_OPS_OUTER_TMP': str(temporary),
           'MCR_OPS_OUTER_SHA256': pins[Path(__file__).name]}
    child = None
    groups = set()
    known = set()
    try:
        initial_rows = [list(map(int, line.split())) for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss='], text=True).splitlines()]
        initial_live = [(pid, rss * 1024) for pid, parent, rss in initial_rows if pid == os.getpid() or parent == os.getpid()]
        initial_rss = sum(rss for _, rss in initial_live)
        initial = {'utc': stamp(), 'pids': [pid for pid, _ in initial_live], 'processes': len(initial_live), 'rssBytes': initial_rss,
            'wholeBytes': max(carry, actual_baseline + allocated(temporary)), 'newBytes': allocated(temporary),
            'temporaryBytes': allocated(temporary), 'durableBytes': allocated(BASE), 'freeBytes': free()}
        assert initial['processes'] <= process_cap and initial['rssBytes'] <= (3 if mode == 'source' else 4) * GIB
        assert initial['wholeBytes'] <= whole_cap and initial['newBytes'] <= new_cap
        assert initial['temporaryBytes'] <= (96 if mode == 'source' else 160) * 1024 ** 2
        assert initial['durableBytes'] <= (32 if mode == 'source' else 40) * 1024 ** 2
        assert initial['freeBytes'] >= 8 * GIB
        report['initialSampleBeforePrimary'] = initial; report['samples'].append(initial)
        (temporary / 'standing.json').write_text(json.dumps(report))
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
                whole = allocated(ROOT) + allocated(gitdir) + dependency + NODE.stat().st_size + chrome_charge + owned_git['bytes'] + python_charge + foundation_charge
                temporary_bytes = allocated(temporary)
                child_temporary = temporary / 'child-temporary.json'
                if child_temporary.is_file():
                    path = Path(json.loads(child_temporary.read_text()))
                    assert path.parent == Path('/private/tmp') and path.name.startswith('mcr-fe37-mc-current-')
                    if path.exists(): temporary_bytes += allocated(path)
                sample = {'utc': stamp(), 'pids': [pid for pid, _, _ in live], 'processes': len(live),
                          'rssBytes': sum(rss for _, _, rss in live), 'wholeBytes': max(carry, whole + temporary_bytes),
                          'newBytes': max(0, whole - actual_baseline) + temporary_bytes, 'temporaryBytes': temporary_bytes,
                          'durableBytes': allocated(BASE), 'freeBytes': free()}
                report['samples'].append(sample)
                assert sample['processes'] <= process_cap and sample['rssBytes'] <= (3 if mode == 'source' else 4) * GIB
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
        (output / 'bootstrap-accepted.json').write_text(json.dumps(bootstrap, indent=2) + '\n')
        if (temporary / 'primary.log').exists(): shutil.copyfile(temporary / 'primary.log', output / 'outer-primary.log')
        shutil.rmtree(temporary)
        report.update(fullUtc=stamp(), outerTemporaryAbsent=not temporary.exists(),
                      finishedBeforeHardDeadline=time.time() < hard, freeAfterBytes=free())
        if not report['finishedBeforeHardDeadline']: report['status'] = 'FAIL'
        (output / 'outer-release.json').write_text(json.dumps(report, indent=2) + '\n')
        discovery = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a230-6acc-7e82-9e4b-92506decc745.json')
        if discovery.is_file() and not report['remainingProcesses'] and report['outerTemporaryAbsent']:
            document = json.loads(discovery.read_text())
            release = json.loads((output / 'release.json').read_text()) if (output / 'release.json').is_file() else {}
            if document.get('offerId') == report['allocation'] and release.get('released'):
                document.update(state='FULL', actualFullEpoch=datetime.datetime.fromisoformat(report['fullUtc']).timestamp(),
                    remainingNewUpperBytes=0, resources=0, publishedAtEpoch=time.time())
                document['receiptPaths']['outerFull'] = str(output / 'outer-release.json')
                pending = discovery.with_name('.' + discovery.name + '.' + str(os.getpid()) + '.tmp')
                pending.write_text(json.dumps(document, indent=2) + '\n'); os.replace(pending, discovery)
        print(json.dumps({k: v for k, v in report.items() if k != 'samples'}), flush=True)
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
