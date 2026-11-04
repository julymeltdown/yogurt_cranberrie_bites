"""Developer-owned finite validation runner. Requires an actual accepted offer.

Adapted from developer204 C04-B finite runner for FE10; no application or shared cache is modified.
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

ROOT = Path(__file__).resolve().parents[4]
FRONT = ROOT / 'frontend'
EVIDENCE = ROOT / 'docs/mcr/evidence/signup-consent-admission'
OFFER = Path(sys.argv[1]).resolve()
assert OFFER.is_relative_to(EVIDENCE.resolve())
lease = json.loads(OFFER.read_text())
assert lease['accepted'] is True and lease['task'] == 'FE10-CONSENT-ADMISSION'
assert os.environ.get('MCR_FE10_ACCEPTED') == lease['id']
assert ROOT == Path('/Users/lhs/Desktop/mcr-worktrees/fe-wall-tools-b-204')
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
    return lease.get('gitCarryBytes', 0) + lease.get('sourceCarryBytes', 0) + sum(p.stat().st_size for base in (EVIDENCE, scratch) if base.exists()
               for p in base.rglob('*') if p.is_file() and not p.is_symlink())

def stage(index, argv):
    output = OFFER.parent / f'{index:02d}.log'
    env = dict(os.environ, TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch),
               XDG_CACHE_HOME=str(scratch / 'cache'), NODE_OPTIONS='--max-old-space-size=768', MCR_FE10_CACHE=str(scratch / 'vite-cache'),
               PATH='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:' + os.environ['PATH'])
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
                rss = sum(table[pid]['rss'] for pid in live | {os.getpid()} if pid in table)
                size = owned_bytes()
                peak['bytes'] = max(peak['bytes'], size)
                peak['rss'] = max(peak['rss'], rss)
                peak['processes'] = max(peak['processes'], len(live) + 1)
                assert time.time() < active_end, 'Active deadline: release allocation'
                disk = os.statvfs(ROOT)
                assert disk.f_bavail * disk.f_frsize >= 8 * 1024**3, 'Live disk floor'
                assert size <= lease['maxBytes'], 'All retained failure/output carry exceeds lease'
                assert rss <= lease['maxRssBytes'], 'Aggregate owned RSS exceeds lease'
                assert len(live) + 1 <= lease['maxProcesses'], 'Aggregate process count exceeds lease'
                time.sleep(.25)
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
        record({'stage': index, 'exit': child.returncode, 'status': 'finished'})
        assert child.returncode == 0, 'Retain failure, repair within this accepted round or release/requeue'

try:
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
    for index, argv in enumerate(lease['commands'], 1):
        stage(index, argv)
finally:
    if created_link:
        assert owned_link.is_symlink() and owned_link.resolve() == shared.resolve()
        owned_link.unlink()
    # Only this run's new empty-or-generated scratch directory is removed.
    import shutil
    shutil.rmtree(scratch)
    record({'status': 'released', 'peak': peak, 'observedPids': sorted(observed), 'dependencyLinkRemoved': created_link})
