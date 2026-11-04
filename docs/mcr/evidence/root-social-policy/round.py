"""Finite Root Wall source/native runner; requires a fresh explicitly accepted allocation."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
FRONT = ROOT / 'frontend'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
EVIDENCE = ROOT / 'docs/mcr/evidence/root-social-policy'
offer = Path(sys.argv[1]).resolve()
assert offer.is_relative_to(EVIDENCE.resolve())
lease = json.loads(offer.read_text())
assert lease['accepted'] is True and lease['task'] == 'ROOT-SOCIAL-POLICY'
assert os.environ.get('MCR_MURAL_ACCEPTED') == lease['id']
assert ROOT == Path('/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish')
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == lease['head']
deadline = datetime.datetime.fromisoformat(lease['hardDeadline']).timestamp()
work_end = deadline - lease['cleanupSeconds']
assert time.time() < work_end
assert lease['startupFloorBytes'] >= 12 * 1024**3
assert shutil.disk_usage(ROOT).free >= lease['startupFloorBytes'], 'named allocation startup floor'
if lease['kind'] == 'source':
    assert lease['limits']['newTotalBytes'] <= 128 * 1024**2
link = FRONT / 'node_modules'
assert not link.exists() and not link.is_symlink()
for name in ('package.json', 'pnpm-lock.yaml'):
    assert (FRONT / name).read_bytes() == (DEPS.parent / name).read_bytes()
out = offer.parent / 'run'
assert not out.exists()
out.mkdir()
scratch = Path(tempfile.mkdtemp(prefix='mcr-root-wall-', dir='/private/tmp'))
env = dict(os.environ, PATH=str(NODE.parent) + os.pathsep + os.environ['PATH'], TMPDIR=str(scratch),
           TMP=str(scratch), TEMP=str(scratch), XDG_CACHE_HOME=str(scratch / 'cache'),
           NODE_OPTIONS='--max-old-space-size=1536', MCR_MURAL_SCRATCH=str(scratch),
           MCR_MURAL_OUTPUT=str(out / 'capture'), MCR_MURAL_PORT=str(lease.get('port', 5317)), NO_COLOR='1')
report = {'job': lease['id'], 'head': lease['head'], 'status': 'IN_PROGRESS', 'start': time.time(),
          'workDeadline': work_end, 'hardDeadline': deadline, 'commands': [],
          'peakProcesses': 0, 'peakRssBytes': 0, 'peakWholeBytes': 0, 'peakTempBytes': 0, 'peakOutputBytes': 0, 'peakNewTotalBytes': 0}
borrowed_bytes = int(subprocess.check_output(['du', '-sk', str(DEPS)], text=True).split()[0]) * 1024
report['borrowedDependencyBytes'] = borrowed_bytes
seen = {}
child = None

def table():
    rows = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,command='], text=True).splitlines():
        values = line.split(None, 3)
        if len(values) == 4:
            pid, parent, rss = map(int, values[:3])
            rows[pid] = {'parent': parent, 'rss': rss * 1024, 'identity': hashlib.sha256(values[3].encode()).hexdigest()}
    return rows

def size(path):
    total = 0
    for item in path.rglob('*'):
        try:
            if item.is_file() and not item.is_symlink():
                total += item.stat().st_size
        except FileNotFoundError:
            continue
    return total

def sample():
    rows = table()
    ids = {os.getpid()}
    while True:
        expanded = ids | {pid for pid, row in rows.items() if row['parent'] in ids}
        if expanded == ids:
            break
        ids = expanded
    for pid in ids:
        if pid in rows:
            seen[pid] = rows[pid]['identity']
    count, rss = len(ids), sum(rows[pid]['rss'] for pid in ids if pid in rows)
    temp, output = size(scratch), size(EVIDENCE)
    git_reserve = 32 * 1024**2
    whole = int(subprocess.check_output(['du', '-sk', str(ROOT)], text=True).split()[0]) * 1024 + borrowed_bytes + temp + git_reserve
    # All caches stay in scratch; all durable outputs, including retained failures,
    # remain in this one evidence root. The external Git growth reserve is charged.
    new_total = temp + output + git_reserve
    for key, value in [('peakProcesses', count), ('peakRssBytes', rss), ('peakWholeBytes', whole), ('peakTempBytes', temp), ('peakOutputBytes', output), ('peakNewTotalBytes', new_total)]:
        report[key] = max(report[key], value)
    limits = lease['limits']
    assert count <= limits['processes'] and rss <= limits['rssBytes'], 'descendant process/RSS cap'
    assert whole <= limits['wholeBytes'] and temp <= limits['tempBytes'] and output <= limits['outputBytes'], 'whole/temp/output cap'
    assert new_total <= limits['newTotalBytes'], 'all owned new bytes plus Git reserve'
    assert shutil.disk_usage(ROOT).free >= 8 * 1024**3, '8GiB live floor'
    assert time.time() < work_end, 'original work deadline'

try:
    link.symlink_to(DEPS, target_is_directory=True)
    for index, command in enumerate(lease['commands']):
        sample()  # Admit each child only after the outer guard checks current limits.
        record = {'argv': command, 'start': time.time()}
        report['commands'].append(record)
        with (out / f'{index:02d}.log').open('wb') as log:
            child = subprocess.Popen(command, cwd=FRONT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            while child.poll() is None:
                sample()
                time.sleep(0.25)
            record.update(exitCode=child.returncode, end=time.time())
            assert child.returncode == 0, f'stage {index} failed; retained log'
    report['status'] = 'PASS'
except BaseException as error:
    report.update(status='FAIL', error=str(error))
    raise
finally:
    for sig in (signal.SIGTERM, signal.SIGKILL):
        rows = table()
        for pid, identity in reversed(list(seen.items())):
            if pid != os.getpid() and pid in rows and rows[pid]['identity'] == identity:
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
        if child is not None:
            try:
                os.killpg(child.pid, sig)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass
    if link.is_symlink() and link.resolve() == DEPS.resolve():
        link.unlink()
    shutil.rmtree(scratch)
    rows = table()
    remaining = [pid for pid, identity in seen.items() if pid != os.getpid() and pid in rows and rows[pid]['identity'] == identity]
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(('127.0.0.1', lease.get('port', 5317)))
    report.update(fullRelease=time.time(), remainingPids=remaining, dependencyLinkPresent=link.exists() or link.is_symlink(),
                  scratchPresent=scratch.exists(), portReuseBind=True)
    (out / 'release.json').write_text(json.dumps(report, indent=2) + '\n')
    assert not remaining and not link.exists() and not link.is_symlink() and not scratch.exists()
    assert time.time() < deadline, 'original hard deadline'
    print(json.dumps(report))
