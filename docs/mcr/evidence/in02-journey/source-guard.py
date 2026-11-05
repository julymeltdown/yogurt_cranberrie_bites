"""Developer-owned finite IN02 source validation runner. Requires an actual accepted offer.

Adapted from developer204 FE10 guard at adec636683a37d9799e409f718a9aa15e60f00bd; no application or shared cache is modified.
The offer supplies exact commands, custody, capacity ceilings and deadlines.
"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import threading

ROOT = Path(__file__).resolve().parents[4]
FRONT = ROOT / 'frontend'
EVIDENCE = ROOT / 'docs/mcr/evidence/in02-journey'
OFFER = Path(sys.argv[1]).resolve()
assert OFFER.is_relative_to(EVIDENCE.resolve())
lease = json.loads(OFFER.read_text())
os.umask(0o077)
assert lease['resourceClass'] == 'SOURCE_ONLY_NO_BROWSER_NO_DB_NO_BUILD'
assert lease['accepted'] is True and lease['task'] == 'IN02'
assert os.environ.get('MCR_IN02_ACCEPTED') == lease['id']
assert ROOT == Path('/Users/lhs/Desktop/mcr-worktrees/fe-wall-tools-b-204')
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
assert head == lease['head'], 'Changed candidate needs explicit allocation coverage'
end = datetime.datetime.fromisoformat(lease['hardDeadline']).timestamp()
active_end = end - lease.get('cleanupSeconds', 60)
assert time.time() < active_end
scratch = Path(lease['scratch']).resolve()
assert scratch.is_relative_to(ROOT) and not scratch.exists()
assert os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize >= lease['startFloorBytes']
scratch.mkdir(parents=True, mode=0o700)
log = OFFER.parent / 'guardian.jsonl'
owned_link = FRONT / 'node_modules'
shared = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
created_link = False
peak = {'bytes': 0, 'rss': 0, 'processes': 0}
observed = set()
whole_stop = threading.Event()
whole = {'bytes': 0, 'entries': 0, 'measuredAt': 0, 'error': None}
whole_peak = {'bytes': lease.get('measuredWholeBytes', 0), 'entries': lease.get('measuredWholeEntries', 0)}

def census():
    roots = [Path(root).resolve() for root in lease['custodyRoots']]
    pending = list(roots); seen = set(); total = 208; entries = 4
    while pending:
        item = pending.pop()
        try:
            stat = item.lstat(); inode = (stat.st_dev, stat.st_ino)
            if inode in seen: continue
            seen.add(inode); total += stat.st_size; entries += 1
            if item.is_symlink():
                assert any(item.resolve().is_relative_to(root) for root in roots), 'Undeclared reachable custody target'
            elif item.is_dir(): pending.extend(item.iterdir())
        except FileNotFoundError: pass
    return {'bytes': total, 'entries': entries, 'measuredAt': time.monotonic(), 'error': None}

def census_loop():
    while not whole_stop.is_set():
        try:
            whole.update(census())
            whole_peak['bytes'] = max(whole_peak['bytes'], whole['bytes'])
            whole_peak['entries'] = max(whole_peak['entries'], whole['entries'])
        except Exception as error:
            whole['error'] = str(error)
            return
        whole_stop.wait(2)

whole_thread = None

def record(row):
    with log.open('a') as f:
        f.write(json.dumps({'time': time.time(), **row}) + '\n')

def process_table():
    result = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,args='], text=True).splitlines():
        values = line.split(maxsplit=4)
        if len(values) >= 4:
            pid, parent, group, rss = map(int, values[:4])
            result[pid] = {'parent': parent, 'group': group, 'rss': rss * 1024, 'args': values[4] if len(values) > 4 else ''}
    return result

def owned_bytes():
    paths = set()
    for value in lease.get('taskCarryRoots', [str(EVIDENCE)]) + [str(scratch)]:
        base = Path(value)
        if not base.exists():
            continue
        paths.add(base)
        if base.is_dir() and not base.is_symlink():
            paths.update(base.rglob('*'))
    return lease.get('gitCarryBytes', 0) + sum(item.lstat().st_size for item in paths if item.exists())

def stage(index, argv):
    output = OFFER.parent / f'{index:02d}.log'
    env = dict(os.environ, TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch),
               XDG_CACHE_HOME=str(scratch / 'cache'), MCR_IN02_EVIDENCE=str(OFFER.parent / 'native'), NODE_OPTIONS='--max-old-space-size=768', MCR_IN02_CACHE=str(scratch / 'vite-cache'), MCR_IN02_PRIVATE_CACHE=str(scratch / 'contract-cache'),
               PATH='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:' + os.environ['PATH'])
    # The independent parent guard samples BEFORE any command child starts.
    pre = process_table()
    owned = {pid for pid, row in pre.items() if pid == os.getpid() or row['parent'] == os.getpid() or str(scratch) in row['args']}
    assert time.time() < active_end
    disk = os.statvfs(ROOT)
    assert disk.f_bavail * disk.f_frsize >= lease['liveFloorBytes']
    assert owned_bytes() <= lease['maxBytes']
    assert len(owned) <= lease['maxProcesses'] and sum(pre[pid]['rss'] for pid in owned) <= lease['maxRssBytes']
    assert whole['error'] is None and time.monotonic() - whole['measuredAt'] < 10
    assert whole['bytes'] <= lease['maxWholeBytes'] and whole['entries'] <= lease['maxWholeEntries']
    record({'stage': index, 'status': 'pre-child-guard-sample', 'ownedProcesses': len(owned), 'freeBytes': disk.f_bavail * disk.f_frsize})
    with output.open('w') as f:
        child = subprocess.Popen(argv, cwd=FRONT, env=env, stdout=f, stderr=subprocess.STDOUT, start_new_session=True)
        ids = {child.pid}
        record({'stage': index, 'argv': argv, 'pid': child.pid, 'status': 'started'})
        try:
            while child.poll() is None:
                table = process_table()
                previous = set()
                while previous != ids:
                    previous = set(ids)
                    ids.update(pid for pid, row in table.items() if row['parent'] in ids)
                live = (ids & table.keys()) | {pid for pid, row in table.items() if row['parent'] == os.getpid() or row['group'] == child.pid or str(scratch) in row['args']}
                ids.update(live)
                observed.update(live)
                rss = sum(table[pid]['rss'] for pid in live | {os.getpid(), os.getppid()} if pid in table)
                size = owned_bytes()
                peak['bytes'] = max(peak['bytes'], size)
                peak['rss'] = max(peak['rss'], rss)
                peak['processes'] = max(peak['processes'], len(live) + 2)
                assert time.time() < active_end, 'Active deadline: release allocation'
                disk = os.statvfs(ROOT)
                assert disk.f_bavail * disk.f_frsize >= lease['liveFloorBytes'], 'Live disk floor'
                assert size <= lease['maxBytes'], 'Bounded NEW/growth and all task output/failure carry exceeds lease'
                assert whole['error'] is None, whole['error']
                assert time.monotonic() - whole['measuredAt'] < 10, 'Full custody census stalled'
                assert whole['bytes'] <= lease['maxWholeBytes'], 'Entire checkout/deps/common-Git WHOLE cap'
                assert whole['entries'] <= lease['maxWholeEntries'], 'Entire custody entry cap'
                assert rss <= lease['maxRssBytes'], 'Aggregate owned RSS exceeds lease'
                assert len(live) + 2 <= lease['maxProcesses'], 'Aggregate process count exceeds lease'
                time.sleep(.1)
        finally:
            # Browser subprocesses may create their own process group. Signal
            # only this child's observed descendant PIDs as well as its group.
            table = process_table()
            descendants = ((ids & table.keys()) | {pid for pid, row in table.items() if str(scratch) in row['args']}) - {child.pid, os.getpid()}
            for pid in descendants:
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            table = process_table()
            if any(row['group'] == child.pid for row in table.values()):
                time.sleep(.3)
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            for pid in descendants & process_table().keys():
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        remaining = (observed & process_table().keys()) - {os.getpid()}
        record({'stage': index, 'exit': child.returncode, 'status': 'finished', 'remainingOwnedPids': sorted(remaining)})
        assert not remaining, 'Owned descendants remain: release is not FULL'
        return child.returncode

try:
    whole.update(census())
    assert whole['bytes'] <= lease['maxWholeBytes'] and whole['entries'] <= lease['maxWholeEntries']
    whole_thread = threading.Thread(target=census_loop, daemon=True)
    whole_thread.start()
    assert not owned_link.exists() and not owned_link.is_symlink(), 'Absent-only dependency link custody required'
    assert shared.is_dir() and lease['dependencyLink'] == str(shared)
    assert hashlib.sha256((FRONT / 'pnpm-lock.yaml').read_bytes()).hexdigest() == lease['lockSha256']
    assert hashlib.sha256((shared.parent / 'pnpm-lock.yaml').read_bytes()).hexdigest() == lease['lockSha256']
    owned_link.symlink_to(shared, target_is_directory=True)
    created_link = True
    for name, text in lease.get('files', {}).items():
        path = (scratch / name).resolve()
        assert path.is_relative_to(scratch)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    record({'lease': lease['id'], 'head': head, 'guardianPid': os.getpid(), 'scratch': str(scratch), 'status': 'accepted'})
    commands = list(enumerate(lease['commands'], 1))
    unresolved = set()
    for repair_round in range(lease.get('maxRepairRounds', 0) + 1):
        exits = [stage(index + repair_round * 100, argv) for index, argv in commands]
        for (index, _), code in zip(commands, exits):
            if code == 0:
                unresolved.discard(index)
            else:
                unresolved.add(index)
        if not unresolved:
            break
        assert repair_round < lease.get('maxRepairRounds', 0), 'Retain failure and release/requeue'
        control = OFFER.parent / f'repair-{repair_round + 1}.json'
        record({'status': 'awaiting-developer-repair', 'round': repair_round, 'control': str(control), 'exits': exits})
        while not control.exists():
            assert time.time() < active_end, 'Repair deadline: release allocation'
            disk = os.statvfs(ROOT)
            assert disk.f_bavail * disk.f_frsize >= lease['liveFloorBytes']
            assert owned_bytes() <= lease['maxBytes']
            table = process_table()
            idle = {pid for pid, row in table.items() if pid == os.getpid() or row['parent'] == os.getpid() or str(scratch) in row['args']}
            assert len(idle | {os.getppid()}) <= lease['maxProcesses']
            assert sum(table[pid]['rss'] for pid in idle | {os.getppid()} if pid in table) <= lease['maxRssBytes']
            assert whole['error'] is None and time.monotonic() - whole['measuredAt'] < 10
            assert whole['bytes'] <= lease['maxWholeBytes'] and whole['entries'] <= lease['maxWholeEntries']
            time.sleep(.1)
        selected = json.loads(control.read_text())['stages']
        assert selected and all(isinstance(index, int) and 1 <= index <= len(lease['commands']) for index in selected)
        commands = [(index, lease['commands'][index - 1]) for index in selected]
finally:
    if created_link:
        assert owned_link.is_symlink() and owned_link.resolve() == shared.resolve()
        owned_link.unlink()
    # Only this run's new empty-or-generated scratch directory is removed.
    import shutil
    shutil.rmtree(scratch)
    whole_stop.set()
    if whole_thread is not None:
        whole_thread.join(timeout=5)
    try:
        final_whole = census()
    except Exception as error:
        final_whole = {'error': str(error)}
    record({'status': 'released', 'peakNew': peak, 'peakWhole': whole_peak, 'finalWhole': final_whole, 'observedPids': sorted(observed), 'dependencyLinkRemoved': created_link})
