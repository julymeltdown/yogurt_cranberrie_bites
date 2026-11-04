"""FE34-PROFILE-ACTIVITY-MEDIA-01 bounded native checks; adapted from this task source guardian and published first-view guardian at30a9c07b."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

p = argparse.ArgumentParser()
p.add_argument('--job', required=True)
p.add_argument('--deadline-epoch', type=float, required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--port', type=int, required=True)
a = p.parse_args()
repo = Path(__file__).resolve().parents[6]
frontend = repo / 'frontend'
node = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
dependencies = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
link = frontend / 'node_modules'
out = a.output.resolve()
assert out.is_relative_to(repo / 'docs/mcr/evidence/profile-activity-media') and not out.exists()
assert not link.exists() and not link.is_symlink()
assert 0 < a.deadline_epoch - time.time() <= 300
assert shutil.disk_usage(repo).free >= 12 * 1024**3
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (frontend / name).read_bytes() == (dependencies.parent / name).read_bytes()
assert subprocess.check_output([str(node), '--version'], text=True).strip() == 'v24.22.2'
out.mkdir(parents=True)
temporary = Path(tempfile.mkdtemp(prefix='mcr-profile-activity-media-', dir='/private/tmp'))
env = {**os.environ, 'PATH': str(node.parent) + os.pathsep + os.environ['PATH'], 'TMPDIR': str(temporary),
       'MCR_PROFILE_MEDIA_TMP': str(temporary), 'MCR_PROFILE_MEDIA_OUTPUT': str(out / 'capture'),
       'MCR_PROFILE_MEDIA_JOB': a.job, 'MCR_PROFILE_MEDIA_PORT': str(a.port), 'NO_COLOR': '1'}
roots = [repo, repo.parent / 'fe-mural-editor', repo.parent / 'fe-wall-reference-target', repo.parent / 'be-party-runtime']
report = {'job': a.job, 'status': 'IN_PROGRESS', 'startedEpoch': time.time(), 'workDeadline': a.deadline_epoch,
          'cleanupDeadline': a.deadline_epoch + 60, 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
          'limits': {'workSeconds': 300, 'cleanupSeconds': 60, 'wholeBytes': 2 * 1024**3, 'privateGitReserveBytes': 2 * 1024**2, 'tempBytes': 100 * 1024**2, 'additionalDurableBytes': 16 * 1024**2, 'logsBytes': 2 * 1024**2, 'newGrowthBytes': 128 * 1024**2, 'processes': 24, 'rssMiB': 4096, 'pngs': 9, 'pngBytes': 4 * 1024**2},
          'commands': [], 'peakProcesses': 0, 'peakRssKiB': 0, 'peakWholeBytes': 0, 'peakTempBytes': 0, 'peakOutputBytes': 0}
child = None
seen = {}

def size(path):
    total = 0
    for f in path.rglob('*'):
        try:
            if f.is_file() and not f.is_symlink():
                total += f.stat().st_size
        except FileNotFoundError:
            # Vite/Vitest atomically replace private cache files during sampling.
            continue
    return total

def temporary_allocated_bytes():
    total = temporary.stat().st_blocks * 512
    for f in temporary.rglob('*'):
        try:
            if not f.is_symlink():
                total += f.stat().st_blocks * 512
        except FileNotFoundError:
            continue
    return total

def processes():
    text = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,command='], text=True)
    rows = {}
    for line in text.splitlines():
        parts = line.split(None, 3)
        if len(parts) == 4:
            pid, parent, rss = map(int, parts[:3])
            rows[pid] = {'pid': pid, 'parent': parent, 'rss': rss, 'commandHash': hashlib.sha256(parts[3].encode()).hexdigest()}
    return rows

def family(rows):
    ids = {os.getpid()}
    while True:
        expanded = ids | {pid for pid, row in rows.items() if row['parent'] in ids}
        if expanded == ids:
            return [row for pid, row in rows.items() if pid in ids]
        ids = expanded

def reap():
    # Chrome may use separate process groups; match retained descendant identity.
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

try:
    assert subprocess.run(['lsof', '-nP', '-iTCP:' + str(a.port), '-sTCP:LISTEN'], capture_output=True).returncode == 1, 'Owned port must be absent before start'
    link.symlink_to(dependencies, target_is_directory=True)
    # Source45 and leaf25 already qualified; run only new driver syntax/lint/native.
    commands = [
        ('syntax', [str(node), '--check', 'src/features/me/__tests__/profile-activity-media/native.mjs']),
        ('native-lint', [str(dependencies / '.bin/oxlint'), 'src/features/me/__tests__/profile-activity-media/native.mjs']),
        ('native', [str(node), 'src/features/me/__tests__/profile-activity-media/native.mjs']),
    ]
    report['initialWholeBytes'] = sum(int(row.split()[0]) * 1024 for row in subprocess.check_output(['du', '-sk', *map(str, roots)], text=True).splitlines()) + report['limits']['privateGitReserveBytes']
    for label, command in commands:
        record = {'label': label, 'command': command, 'startedEpoch': time.time()}
        report['commands'].append(record)
        with (out / (label + '.log')).open('wb') as log:
            child = subprocess.Popen(command, cwd=frontend, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            while child.poll() is None:
                assert time.time() < a.deadline_epoch, 'Original work deadline'
                assert shutil.disk_usage(repo).free >= 8 * 1024**3, '8GiB live floor'
                current = family(processes())
                for row in current:
                    seen[row['pid']] = row['commandHash']
                count, rss = len(current), sum(row['rss'] for row in current)
                whole = sum(int(row.split()[0]) * 1024 for row in subprocess.check_output(['du', '-sk', *map(str, roots)], text=True).splitlines()) + temporary_allocated_bytes() + report['limits']['privateGitReserveBytes']
                evidence_roots = [repo / 'docs/mcr/evidence/profile-activity-media']
                temp_bytes, output_bytes = size(temporary), sum(size(p) for p in evidence_roots)
                report['peakProcesses'] = max(report['peakProcesses'], count)
                report['peakRssKiB'] = max(report['peakRssKiB'], rss)
                report['peakWholeBytes'] = max(report['peakWholeBytes'], whole)
                report['peakTempBytes'] = max(report['peakTempBytes'], temp_bytes)
                report['peakOutputBytes'] = max(report['peakOutputBytes'], output_bytes)
                assert count <= report['limits']['processes'] and rss <= report['limits']['rssMiB'] * 1024, 'Whole descendant process/RSS cap'
                assert whole <= 2 * 1024**3 and temp_bytes <= 100 * 1024**2 and output_bytes <= 16 * 1024**2, 'Whole/temp/output cap'
                assert sum(f.stat().st_size for p in evidence_roots for f in p.rglob('*.log')) <= 2 * 1024**2, '2MiB complete log cap'
                assert whole - report['initialWholeBytes'] <= 128 * 1024**2, '128MiB positive growth cap'
                pngs = [f for root in evidence_roots for f in root.rglob('*.png')]
                assert len(pngs) <= 9 and sum(f.stat().st_size for f in pngs) <= 4 * 1024**2, '9PNG/4MiB cap'
                report['pngCount'] = len(pngs)
                time.sleep(.25)
            record.update(exitCode=child.returncode, finishedEpoch=time.time())
            assert child.returncode == 0, label + ' failed; retained log/report'
    report['status'] = 'PASS'
except BaseException as error:
    report['error'] = str(error)
    report['status'] = 'FAIL'
    raise
finally:
    reap()
    if link.is_symlink() and link.resolve() == dependencies.resolve():
        link.unlink()
    shutil.rmtree(temporary)
    rows = processes()
    report['remainingOwnDescendants'] = [pid for pid, digest in seen.items() if pid != os.getpid() and pid in rows and rows[pid]['commandHash'] == digest]
    report['finishedEpoch'] = time.time()
    report['cleanup'] = {'linkAbsent': not link.is_symlink(), 'tempAbsent': not temporary.exists(), 'beforeOriginalDeadline': time.time() < a.deadline_epoch + 60, 'portAbsent': subprocess.run(['lsof', '-nP', '-iTCP:' + str(a.port), '-sTCP:LISTEN'], capture_output=True).returncode == 1}
    if report['remainingOwnDescendants'] or not all(report['cleanup'].values()):
        report['status'] = 'FAIL'
    (out / 'guardian.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'peakProcesses': report['peakProcesses'], 'peakRssKiB': report['peakRssKiB'], 'cleanup': report['cleanup'], 'remainingOwnDescendants': report['remainingOwnDescendants']}))
