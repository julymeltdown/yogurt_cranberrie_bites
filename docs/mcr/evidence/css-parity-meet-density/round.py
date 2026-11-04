"""Bounded CSS-MEET-DENSITY round. Adapted from213 first-view/round.py at7a87414a.
Source authoring is independent; invoke only after named21d OFFER and ACCEPT.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import tempfile
import time

p = argparse.ArgumentParser()
p.add_argument('--job', required=True)
p.add_argument('--cleanup-deadline-epoch', type=float, required=True)
p.add_argument('--port', type=int, required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--baseline', type=Path, required=True)
a = p.parse_args()
repo = Path(__file__).resolve().parents[4]
frontend = repo / 'frontend'
node = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
dependencies = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
link = frontend / 'node_modules'
out = a.output.resolve()
work_deadline = a.cleanup_deadline_epoch - 60
assert out.is_relative_to(repo / 'docs/mcr/evidence/css-parity-meet-density') and not out.exists()
assert not link.exists() and not link.is_symlink()
assert 0 < a.cleanup_deadline_epoch - time.time() <= 300
assert time.time() < work_deadline
assert shutil.disk_usage(repo).free >= 16 * 1024**3, 'Fresh16GiB start floor'
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (frontend / name).read_bytes() == (dependencies.parent / name).read_bytes()
assert subprocess.check_output([str(node), '--version'], text=True).strip() == 'v24.22.2'
with socket.socket() as probe:
    probe.bind(('127.0.0.1', a.port))
out.mkdir(parents=True)
temporary = Path(tempfile.mkdtemp(prefix='mcr-meet-density-', dir='/private/tmp'))
env = {**os.environ, 'PATH': str(node.parent) + os.pathsep + os.environ['PATH'],
       'TMPDIR': str(temporary), 'MCR_FIRST_VIEW_JOB': a.job,
       'MCR_FIRST_VIEW_PORT': str(a.port), 'MCR_FIRST_VIEW_OUTPUT': str(out / 'capture'),
       'MCR_DENSITY_BASELINE': str(a.baseline.resolve()), 'NO_COLOR': '1'}
report = {'task': 'CSS-MEET-DENSITY', 'job': a.job, 'status': 'IN_PROGRESS',
          'startedEpoch': time.time(), 'workDeadline': work_deadline,
          'cleanupDeadline': a.cleanup_deadline_epoch,
          'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
          'limits': {'totalSeconds': 300, 'cleanupReserveSeconds': 60,
                     'wholeBytes': 2 * 1024**3, 'privateGitReserveBytes': 2 * 1024**2,
                     'tempBytes': 200 * 1024**2, 'durableBytes': 12 * 1024**2,
                     'logsBytes': 2 * 1024**2, 'newPngCount': 15, 'newPngBytes': 4 * 1024**2,
                     'processes': 24, 'rssMiB': 4096}, 'commands': [],
          'peakProcesses': 0, 'peakRssKiB': 0, 'peakWholeBytes': 0,
          'peakTempBytes': 0, 'peakOutputBytes': 0}
child = None
seen = {}

def size(path):
    total = 0
    for f in path.rglob('*'):
        try:
            if f.is_file() and not f.is_symlink():
                total += f.stat().st_size
        except FileNotFoundError:
            pass
    return total

def processes():
    result = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,command='], text=True).splitlines():
        parts = line.split(None, 3)
        if len(parts) == 4:
            pid, parent, rss = map(int, parts[:3])
            result[pid] = {'pid': pid, 'parent': parent, 'rss': rss,
                           'commandHash': hashlib.sha256(parts[3].encode()).hexdigest()}
    return result

def family(rows):
    ids = {os.getpid()}
    while True:
        expanded = ids | {pid for pid, row in rows.items() if row['parent'] in ids}
        if expanded == ids:
            return [row for pid, row in rows.items() if pid in ids]
        ids = expanded

def reap():
    # Retain identities because Chromium helpers may acquire separate groups.
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        rows = processes()
        for pid, digest in reversed(list(seen.items())):
            if pid != os.getpid() and pid in rows and rows[pid]['commandHash'] == digest:
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
        if child is not None:
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass

def stop(signum, frame):
    raise RuntimeError('Supervisor interrupted: ' + str(signum))

signal.signal(signal.SIGTERM, stop)
try:
    # All supervision limits exist before this link and before any child job.
    link.symlink_to(dependencies, target_is_directory=True)
    driver = 'docs/mcr/evidence/css-parity-meet-density/native.mjs'
    for label, command in [('syntax', [str(node), '--check', driver]),
                           ('native', [str(node), driver])]:
        record = {'label': label, 'command': command, 'startedEpoch': time.time()}
        report['commands'].append(record)
        with (out / (label + '.log')).open('wb') as log:
            child = subprocess.Popen(command, cwd=repo, env=env, stdout=log,
                                     stderr=subprocess.STDOUT, start_new_session=True)
            while child.poll() is None:
                assert time.time() < work_deadline, 'Original work cutoff'
                assert shutil.disk_usage(repo).free >= 8 * 1024**3, 'Live8GiB floor'
                current = family(processes())
                for row in current:
                    seen[row['pid']] = row['commandHash']
                count, rss = len(current), sum(row['rss'] for row in current)
                whole = sum(int(row.split()[0]) * 1024 for row in subprocess.check_output(
                    ['du', '-sk', str(repo), str(temporary)], text=True).splitlines()) + 2 * 1024**2
                temp_bytes, output_bytes = size(temporary), size(out)
                report['peakProcesses'] = max(report['peakProcesses'], count)
                report['peakRssKiB'] = max(report['peakRssKiB'], rss)
                report['peakWholeBytes'] = max(report['peakWholeBytes'], whole)
                report['peakTempBytes'] = max(report['peakTempBytes'], temp_bytes)
                report['peakOutputBytes'] = max(report['peakOutputBytes'], output_bytes)
                assert count <= 24 and rss <= 4096 * 1024, 'Descendant/RSS limit'
                assert whole <= 2 * 1024**3 and temp_bytes <= 200 * 1024**2, 'Whole/temp limit'
                assert output_bytes <= 12 * 1024**2, 'Durable output limit'
                assert sum(f.stat().st_size for f in out.rglob('*.log')) <= 2 * 1024**2, 'Log limit'
                pngs = list(out.rglob('*.png'))
                assert len(pngs) <= 15 and sum(f.stat().st_size for f in pngs) <= 4 * 1024**2, 'PNG limit'
                time.sleep(.25)
            record.update(exitCode=child.returncode, finishedEpoch=time.time())
            assert child.returncode == 0, label + ' failed; retain evidence'
    report['status'] = 'PASS'
except BaseException as error:
    report.update(status='FAIL', error=str(error))
    raise
finally:
    reap()
    if link.is_symlink() and link.resolve() == dependencies.resolve():
        link.unlink()
    shutil.rmtree(temporary)
    rows = processes()
    report['remainingOwnDescendants'] = [pid for pid, digest in seen.items()
        if pid != os.getpid() and pid in rows and rows[pid]['commandHash'] == digest]
    report['finishedEpoch'] = time.time()
    report['cleanup'] = {'linkAbsent': not link.exists() and not link.is_symlink(),
        'tempAbsent': not temporary.exists(), 'beforeOriginalDeadline': time.time() < a.cleanup_deadline_epoch}
    with socket.socket() as check:
        report['cleanup']['portClosed'] = check.connect_ex(('127.0.0.1', a.port)) != 0
    if report['remainingOwnDescendants'] or not all(report['cleanup'].values()):
        report['status'] = 'FAIL'
    (out / 'guardian.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'peakProcesses': report['peakProcesses'],
        'peakRssKiB': report['peakRssKiB'], 'cleanup': report['cleanup'],
        'remainingOwnDescendants': report['remainingOwnDescendants']}))
