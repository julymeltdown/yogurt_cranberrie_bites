"""QA06 finite software remainder guard; adapted from own reflow guard c18d1bfe (shared-chrome ancestor82787782); no shared-file write."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import time

p = argparse.ArgumentParser()
p.add_argument('--job', required=True)
p.add_argument('--round', required=True)
p.add_argument('--cohort', required=True)
p.add_argument('--admission', required=True)
p.add_argument('--resume')
p.add_argument('--deadline', type=float, required=True)
p.add_argument('--port', type=int, required=True)
p.add_argument('--aggregate-reserve', type=int, required=True, help='All accepted worst-case NEW bytes, including this job')
p.add_argument('--candidate', required=True)
a = p.parse_args()
repo = Path(__file__).resolve().parents[4]
node = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
deps = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
link = repo / 'frontend/node_modules'
assert a.cohort.replace('-', '').isalnum()
out = repo / 'docs/mcr/evidence/visual/qa06/software-remainder' / a.cohort / a.round
temporary = Path('/private/tmp/mcr-qa06-software-' + a.cohort + '-' + a.round)
private_git = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=repo, text=True).strip())
assert a.round.replace('-', '').isalnum() and not out.exists() and not temporary.exists()
assert not link.exists() and not link.is_symlink()
assert 0 < a.deadline - time.time() <= 300
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip() == a.candidate
admission = json.loads(Path(a.admission).read_text())
assert admission['job'] == a.job and admission['candidate'] == a.candidate and admission['cohort'] == a.cohort
assert admission['workDeadline'] == a.deadline and admission['aggregateWorstNewBytes'] == a.aggregate_reserve
assert admission['guardSha256'] == hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
assert admission['startFloorBytes'] == 11 * 1024**3
assert a.aggregate_reserve >= 256 * 1024**2
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (repo / 'frontend' / name).read_bytes() == (deps.parent / name).read_bytes()
assert hashlib.sha256((repo / 'frontend/pnpm-lock.yaml').read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
assert shutil.disk_usage(repo).free >= admission['startFloorBytes']
assert shutil.disk_usage(repo).free - a.aggregate_reserve >= 9 * 1024**3
def port_free():
    with socket.socket() as s:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            s.bind(('127.0.0.1', a.port))
            return True
        except OSError:
            return False
assert port_free()
def size(path):
    total = 0
    for root, dirs, files in os.walk(path, followlinks=False):
        dirs[:] = [name for name in dirs if not (Path(root) / name).is_symlink()]
        for name in files:
            try:
                stat = (Path(root) / name).lstat()
                total += max(stat.st_size, stat.st_blocks * 512)
            except FileNotFoundError:
                pass
    return total
dependency_bytes = size(deps)
physical_carried = size(repo) + size(private_git) + dependency_bytes
carried = max(physical_carried, admission['chargedCarryBytes'], 1288548352)
# An in-lease repair round charges the original job baseline and all retained output.
first_receipt = out.parent / 'round-01/supervisor.json'
job_baseline = carried
if a.round != 'round-01' and first_receipt.exists():
    previous = json.loads(first_receipt.read_text())
    assert previous['job'] == a.job and previous['workDeadline'] == a.deadline
    assert previous['FULL']
    job_baseline = min(carried, previous['carriedWholeBytes'])
info = {'job': a.job, 'round': a.round, 'started': time.time(), 'workDeadline': a.deadline,
        'cleanupDeadline': a.deadline + 60, 'candidate': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
        'carriedWholeBytes': carried, 'actualPhysicalCarryBytes': physical_carried, 'originalJobCarryBytes': job_baseline, 'retainedDependencyBytes': dependency_bytes, 'aggregateNewReserve': a.aggregate_reserve,
        'caps': {'whole': 2 * 1024**3, 'new': 256 * 1024**2, 'temp': 200 * 1024**2, 'durable': 32 * 1024**2, 'allPids': 24, 'rss': 4 * 1024**3, 'pngs': 8},
        'peaks': {}, 'commands': []}
assert carried + info['caps']['new'] < info['caps']['whole']
out.mkdir(parents=True)
temporary.mkdir()
child = None
seen = set()
env = {**os.environ, 'PATH': str(node.parent) + os.pathsep + os.environ['PATH'],
       'TMPDIR': str(temporary), 'QA06_SOFTWARE_TEMP': str(temporary), 'QA06_SOFTWARE_OUTPUT': str(out),
       'QA06_SOFTWARE_JOB': a.job, 'QA06_SOFTWARE_PORT': str(a.port), 'NO_COLOR': '1'}
env['QA06_SOFTWARE_CANDIDATE'] = a.candidate
env['QA06_SOFTWARE_COHORT'] = a.cohort
env['QA06_SOFTWARE_ADMISSION'] = str(Path(a.admission).resolve())
if a.resume:
    env['QA06_SOFTWARE_RESUME'] = str(Path(a.resume).resolve())
def process_rows():
    return [list(map(int, row.split())) for row in subprocess.check_output(['ps', '-axo', 'pid,ppid,rss'], text=True).splitlines()[1:] if len(row.split()) == 3]
def measure():
    rows = process_rows()
    owned = {os.getpid()} | seen
    for _ in range(20):
        expanded = owned | {pid for pid, parent, rss in rows if parent in owned}
        if expanded == owned:
            break
        owned = expanded
    seen.update(owned - {os.getpid()})
    actual = [row for row in rows if row[0] in owned]
    free = shutil.disk_usage(repo).free
    info['minFree'] = min(info.get('minFree', free), free)
    # Full checkout/private Git and the retained read-only dependency footprint
    # are charged to WHOLE; only growth is charged to NEW.
    now = time.time()
    if now - info.get('wholeMeasuredAt', 0) > 4:
        info['currentCarry'] = max(size(repo) + size(private_git) + dependency_bytes, carried)
        info['wholeMeasuredAt'] = now
    values = {'temp': size(temporary), 'durable': size(out.parent),
              'allPids': len(actual), 'rss': sum(row[2] * 1024 for row in actual), 'pngs': len(list(out.parent.rglob('*.png')))}
    values['new'] = max(0, info['currentCarry'] - job_baseline) + values['temp']
    values['whole'] = info['currentCarry'] + values['temp']
    for key, value in values.items():
        info['peaks'][key] = max(info['peaks'].get(key, 0), value)
        if value > info['caps'][key]:
            return key + ' cap'
    if free < 8 * 1024**3:
        return '8GiB live floor'
    if now >= a.deadline - 3:
        return 'absolute work deadline'
    return None
def terminate_owned():
    live = {row[0] for row in process_rows()} & seen
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        for pid in live:
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass
        time.sleep(.3)
        live = {row[0] for row in process_rows()} & seen
        if not live:
            break
    return sorted(live)
try:
    link.symlink_to(deps, target_is_directory=True)
    for args in [[str(node), '--check', str(Path(__file__).with_name('software.mjs'))],
                 [str(node), str(Path(__file__).with_name('software.mjs'))]]:
        assert not measure()
        info['commands'].append(args)
        with (out / ('syntax.log' if '--check' in args else 'native.log')).open('w') as log:
            child = subprocess.Popen(args, cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            seen.add(child.pid)
            while child.poll() is None:
                reason = measure()
                if reason:
                    info['stopReason'] = reason
                    terminate_owned()
                    break
                time.sleep(.1)
            info['exitCode'] = child.wait()
        if info['exitCode']:
            break
finally:
    info['remainingOwnedPids'] = terminate_owned()
    if link.is_symlink() and link.resolve() == deps:
        link.unlink()
    if temporary.exists():
        shutil.rmtree(temporary)
    info.update(finished=time.time(), ownLinkAbsent=not link.exists() and not link.is_symlink(),
                ownTempAbsent=not temporary.exists(), portClosed=port_free(),
                cleanupWithinDeadline=time.time() < a.deadline + 60, finalFree=shutil.disk_usage(repo).free)
    info['FULL'] = not info['remainingOwnedPids'] and info['ownLinkAbsent'] and info['ownTempAbsent'] and info['portClosed'] and info['cleanupWithinDeadline']
    (out / 'supervisor.json').write_text(json.dumps(info, indent=2) + '\n')
    print(json.dumps(info))
raise SystemExit(info.get('exitCode', 1) if info['FULL'] else 2)
