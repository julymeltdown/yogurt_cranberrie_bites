"""Developer-owned finite browser validation runner with exact singleton custody. Requires an actual accepted offer.

Adapted from developer204 C04-B finite runner for FE10; no application or shared cache is modified.
The offer supplies exact commands, custody, capacity ceilings and deadlines.
Singleton discovery adapted from developer83 qualify-browser.py at62f1c3a37eb755ef4cf9292db65443ed2001589b; no peer TMP ownership granted.
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
EVIDENCE = ROOT / 'docs/mcr/evidence/signup-consent-admission'
OFFER = Path(sys.argv[1]).resolve()
assert OFFER.is_relative_to(EVIDENCE.resolve())
lease = json.loads(OFFER.read_text())
assert lease['accepted'] is True and lease['task'] == 'FE10-CONSENT-ADMISSION'
assert os.environ.get('MCR_FE10_ACCEPTED') == lease['id']
assert ROOT == Path('/Users/lhs/Desktop/mcr-worktrees/fe-wall-tools-b-204')
assert lease.get('allowOwnedExternalSingleton') is True, 'Explicit browser custody allocation required'
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
assert head == lease['head'], 'Changed candidate needs explicit allocation coverage'
end = datetime.datetime.fromisoformat(lease['hardDeadline']).timestamp()
active_end = end - lease.get('cleanupSeconds', 60)
assert time.time() < active_end
scratch = Path(lease['scratch']).resolve()
assert scratch.is_relative_to(ROOT) and not scratch.exists()
assert os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize >= 12 * 1024**3
scratch.mkdir(parents=True, mode=0o700)
log = OFFER.parent / 'guardian.jsonl'
owned_link = FRONT / 'node_modules'
shared = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
created_link = False
peak = {'bytes': 0, 'rss': 0, 'processes': 0}
observed = set()
fingerprints = {}
whole_stop = threading.Event()
whole = {'bytes': 0, 'entries': 0, 'measuredAt': 0, 'error': None}
whole_peak = {'bytes': lease.get('measuredWholeBytes', 0), 'entries': lease.get('measuredWholeEntries', 0)}

# Only an exact symlink in our new private scratch may introduce an external
# Chrome SingletonSocket root. No external TMP glob, name/mtime ownership guess.
external_singletons = {}
external_lock = threading.RLock()
def discover_singletons():
    for base, directories, files in os.walk(scratch, followlinks=False):
        for name in directories + files:
            item = Path(base) / name
            if name != 'SingletonSocket' or not item.is_symlink():
                continue
            try:
                target = item.resolve()
                if target.is_relative_to(scratch):
                    continue
                assert target.name == 'SingletonSocket', f'Unexpected singleton target {item} -> {target}'
                assert target.parent.name.startswith(('com.google.Chrome.', 'scoped_dir')), f'Unconfirmed singleton target {target}'
                assert target.parent.stat().st_uid == os.getuid(), f'Unowned singleton target {target}'
                assert set(path.name for path in target.parent.iterdir()) <= {'SingletonSocket', 'SingletonCookie'}, f'Unexpected singleton entries {target.parent}'
                with external_lock:
                    if str(target.parent) not in external_singletons:
                        row = {'source': str(item), 'target': str(target), 'root': str(target.parent), 'observedAt': time.time()}
                        external_singletons[str(target.parent)] = row
                        record({'status': 'exact-own-profile-singleton-discovery', **row})
            except FileNotFoundError:
                continue

def count_roots(roots):
    total = 0; entries = 0
    for root in roots:
        if not root.exists():
            continue
        # Charge external directory metadata as well as every entry.
        if str(root) in external_singletons:
            total += root.lstat().st_size; entries += 1
        for base, directories, files in os.walk(root, followlinks=False):
            for name in directories + files:
                item = Path(base) / name
                try:
                    total += item.lstat().st_size; entries += 1
                    if item.is_symlink():
                        target = item.resolve()
                        assert any(target.is_relative_to(declared) for declared in roots), f'Undeclared reachable symlink target: {item} -> {target}'
                except FileNotFoundError:
                    pass
    return total, entries

def census():
    discover_singletons()
    with external_lock:
        roots = [Path(root).resolve() for root in lease['custodyRoots']] + [Path(root) for root in external_singletons]
        total, entries = count_roots(roots)
    return {'bytes': total + 208, 'entries': entries + 4, 'measuredAt': time.monotonic(), 'error': None}

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

def fingerprint(pid):
    result = subprocess.run(['ps', '-p', str(pid), '-o', 'lstart=', '-o', 'comm='], capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else None

def remember(pids):
    for pid in pids:
        if pid not in fingerprints:
            value = fingerprint(pid)
            if value: fingerprints[pid] = value

def same_process(pid):
    return pid in fingerprints and fingerprint(pid) == fingerprints[pid]

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
    with external_lock:
        for value in external_singletons:
            base = Path(value)
            if base.exists():
                paths.add(base); paths.update(base.rglob('*'))
    return lease.get('gitCarryBytes', 0) + sum(item.lstat().st_size for item in paths if item.exists() or item.is_symlink())

def stage(index, argv):
    output = OFFER.parent / f'{index:02d}.log'
    env = dict(os.environ, TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch),
               XDG_CACHE_HOME=str(scratch / 'cache'), MCR_FE10_EVIDENCE=str(OFFER.parent / 'native'), NODE_OPTIONS='--max-old-space-size=768', MCR_FE10_CACHE=str(scratch / 'vite-cache'),
               PATH='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:' + os.environ['PATH'])
    # The independent parent guard samples BEFORE any command child starts.
    pre = process_table()
    owned = {pid for pid, row in pre.items() if pid == os.getpid() or row['parent'] == os.getpid() or str(scratch) in row['args']}
    assert time.time() < active_end
    disk = os.statvfs(ROOT)
    assert disk.f_bavail * disk.f_frsize >= 8 * 1024**3
    assert owned_bytes() <= lease['maxBytes']
    assert len(owned) <= lease['maxProcesses'] and sum(pre[pid]['rss'] for pid in owned) <= lease['maxRssBytes']
    assert whole['error'] is None and time.monotonic() - whole['measuredAt'] < 10
    assert whole['bytes'] <= lease['maxWholeBytes'] and whole['entries'] <= lease['maxWholeEntries']
    record({'stage': index, 'status': 'pre-child-guard-sample', 'ownedProcesses': len(owned), 'freeBytes': disk.f_bavail * disk.f_frsize})
    with output.open('w') as f:
        child = subprocess.Popen(argv, cwd=FRONT, env=env, stdout=f, stderr=subprocess.STDOUT, start_new_session=True)
        ids = {child.pid}
        remember(ids)
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
                remember(live)
                rss = sum(table[pid]['rss'] for pid in live | {os.getpid()} if pid in table)
                size = owned_bytes()
                peak['bytes'] = max(peak['bytes'], size)
                peak['rss'] = max(peak['rss'], rss)
                peak['processes'] = max(peak['processes'], len(live) + 1)
                assert time.time() < active_end, 'Active deadline: release allocation'
                disk = os.statvfs(ROOT)
                assert disk.f_bavail * disk.f_frsize >= 8 * 1024**3, 'Live disk floor'
                assert size <= lease['maxBytes'], 'Bounded NEW/growth and all task output/failure carry exceeds lease'
                assert whole['error'] is None, whole['error']
                assert time.monotonic() - whole['measuredAt'] < 10, 'Full custody census stalled'
                assert whole['bytes'] <= lease['maxWholeBytes'], 'Entire checkout/deps/common-Git WHOLE cap'
                assert whole['entries'] <= lease['maxWholeEntries'], 'Entire custody entry cap'
                assert rss <= lease['maxRssBytes'], 'Aggregate owned RSS exceeds lease'
                assert len(live) + 1 <= lease['maxProcesses'], 'Aggregate process count exceeds lease'
                discover_singletons()
                time.sleep(.1)
        finally:
            # Browser subprocesses may create their own process group. Signal
            # only this child's observed descendant PIDs as well as its group.
            table = process_table()
            descendants = ((ids & table.keys()) | {pid for pid, row in table.items() if str(scratch) in row['args']}) - {child.pid, os.getpid()}
            remember(descendants)
            for pid in descendants:
                if not same_process(pid): continue
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            try:
                if same_process(child.pid): os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                if same_process(child.pid): os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            table = process_table()
            if any(row['group'] == child.pid for row in table.values()):
                time.sleep(.3)
                try:
                    if same_process(child.pid): os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            for pid in descendants & process_table().keys():
                if not same_process(pid): continue
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        remaining = {pid for pid in (observed & process_table().keys()) - {os.getpid()} if same_process(pid)}
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
            assert disk.f_bavail * disk.f_frsize >= 8 * 1024**3
            assert owned_bytes() <= lease['maxBytes']
            table = process_table()
            idle = {pid for pid, row in table.items() if pid == os.getpid() or row['parent'] == os.getpid() or str(scratch) in row['args']}
            assert len(idle) <= lease['maxProcesses']
            assert sum(table[pid]['rss'] for pid in idle) <= lease['maxRssBytes']
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
    record({'status': 'released', 'peakNew': peak, 'peakWhole': whole_peak, 'finalWhole': final_whole, 'observedPids': sorted(observed), 'dependencyLinkRemoved': created_link, 'exactExternalSingletons': list(external_singletons.values()), 'retainedExternalSingletonRoots': [root for root in external_singletons if Path(root).exists()]})
    assert not any(Path(root).exists() for root in external_singletons), 'Retain exact external singleton carry; do not claim FULL or delete by name'
