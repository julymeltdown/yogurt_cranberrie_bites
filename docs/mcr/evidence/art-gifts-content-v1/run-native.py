#!/usr/bin/env python3
"""Run only after a named offer. Retries retain the original accepted clock."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import time

PACKET = Path(__file__).resolve().parent
ROOT = PACKET.parents[3]
REL_ROOTS = ['frontend/public/reference-gifts-content-v1', 'docs/mcr/art/reference-gifts-content-v1.md',
             'docs/mcr/evidence/art-gifts-content-v1', 'frontend/public/reference-window-controls-v2',
             'docs/mcr/art/reference-window-controls-v2.md', 'docs/mcr/evidence/art-window-controls-v2']
ROOTS = [ROOT / p for p in REL_ROOTS]


def size(paths):
    total = 0
    for root in paths:
        root = Path(root)
        if root.is_file() or root.is_symlink():
            total += root.lstat().st_size
        elif root.is_dir():
            for base, dirs, files in os.walk(root, followlinks=False):
                for name in dirs + files:
                    p = Path(base) / name
                    if p.is_file() or p.is_symlink():
                        total += p.lstat().st_size
    return total


def snapshot():
    rows = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines():
        values = line.split()
        if len(values) == 4:
            rows[int(values[0])] = tuple(map(int, values[1:]))
    return rows


def port_free():
    listeners = subprocess.run(['lsof', '-nP', '-iTCP:5197', '-sTCP:LISTEN'], capture_output=True)
    sock = socket.socket()
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        sock.bind(('127.0.0.1', 5197))
        return listeners.returncode == 1
    except OSError:
        return False
    finally:
        sock.close()


def guardian(config):
    stop = False
    known = set()
    peaks = {key: 0 for key in ('processes', 'rssBytes', 'wholeBytes', 'tempBytes', 'cacheBytes', 'outputBytes')}
    minimum = shutil.disk_usage(ROOT).free
    reason = 'NORMAL_STOP'

    def finish(_signal, _frame):
        nonlocal stop
        stop = True

    signal.signal(signal.SIGTERM, finish)
    print('READY', flush=True)
    line = sys.stdin.readline().strip()
    if line:
        known.add(int(line))
    print('ARMED', flush=True)
    try:
        while not stop:
            rows = snapshot()
            changed = True
            while changed:
                changed = False
                for pid, values in rows.items():
                    if values[0] in known and pid not in known:
                        known.add(pid)
                        changed = True
            observed = (known & rows.keys()) | {os.getpid(), config['parent']}
            observed |= {pid for pid, values in rows.items() if values[0] == os.getpid()}
            own = size([ROOT, config['privateGit']])
            temp = size([PACKET / 'native-tmp'])
            cache = size([PACKET / 'private-cache'])
            values = {'processes': len(observed), 'rssBytes': sum(rows[p][2] * 1024 for p in observed if p in rows),
                      'wholeBytes': config['borrow'] + own, 'tempBytes': temp, 'cacheBytes': cache,
                      'outputBytes': own - temp - cache}
            for key, value in values.items():
                peaks[key] = max(peaks[key], value)
            free = shutil.disk_usage(ROOT).free
            minimum = min(minimum, free)
            if time.time() >= config['workDeadline']:
                reason = 'ORIGINAL_WORK_DEADLINE'
                break
            if (free < 8 * 1024**3 or values['processes'] > 16 or values['rssBytes'] > 2 * 1024**3
                    or values['wholeBytes'] > 2 * 1024**3 or temp > 64 * 1024**2
                    or cache > 24 * 1024**2 or values['outputBytes'] > 8 * 1024**2):
                reason = 'CAP_LIMIT'
                break
            try:
                os.kill(config['parent'], 0)
            except ProcessLookupError:
                reason = 'SUPERVISOR_GONE'
                break
            time.sleep(.1)
    finally:
        rows = snapshot()
        for pid in sorted(known, reverse=True):
            if pid in rows:
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        link = PACKET / 'node_modules'
        if link.is_symlink():
            link.unlink()
        for name in ('native-tmp', 'private-cache'):
            if (PACKET / name).exists():
                shutil.rmtree(PACKET / name)
        time.sleep(.1)
        result = {'reason': reason, 'peaks': peaks, 'minimumFreeBytes': minimum,
                  'remainingOwnedPids': sorted(known & snapshot().keys()), 'port5197Free': port_free(),
                  'ephemeralAbsent': {name: not (PACKET / name).exists() and not (PACKET / name).is_symlink()
                                      for name in ('node_modules', 'native-tmp', 'private-cache')}}
        (PACKET / f"{config['attempt']}-guardian.json").write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result), flush=True)


def run(args):
    assert 0 < args.work_seconds <= 120 and 0 < args.cleanup_seconds <= 60
    assert len(args.candidate) == 40 and all(c in '0123456789abcdef' for c in args.candidate)
    assert args.attempt.startswith('native-') and args.attempt[7:].isdigit()
    assert Path(args.node).is_file() and Path(args.browser).is_file() and Path(args.deps).is_dir()
    for name in ('node_modules', 'native-tmp', 'private-cache', args.attempt):
        assert not (PACKET / name).exists() and not (PACKET / name).is_symlink(), name
    assert not (PACKET / f'allocation-{args.attempt}.json').exists(), 'Never overwrite prior attempt proof'
    assert port_free()
    private_git = subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip()
    carry_roots = [ROOT, private_git]
    borrow = size([args.deps])
    assert borrow + size(carry_roots) < 2 * 1024**3 and shutil.disk_usage(ROOT).free >= 16 * 1024**3
    runtime = json.loads((PACKET / 'runtime-inputs.json').read_text())
    assert hashlib.sha256(Path(args.node).read_bytes()).hexdigest() == runtime['nodeExecutableSha256']
    packages = {}
    for name, key in [('package.json', 'frontendPackageSha256'), ('pnpm-lock.yaml', 'frontendPnpmLockSha256')]:
        own = subprocess.check_output(['git', 'show', f'{args.candidate}:frontend/{name}'], cwd=ROOT)
        retained = (Path(args.deps).parent / name).read_bytes()
        assert own == retained and hashlib.sha256(own).hexdigest() == runtime[key]
        packages[name] = hashlib.sha256(own).hexdigest()
    pins = []
    for root in ROOTS:
        for p in ([root] if root.is_file() else root.rglob('*') if root.exists() else []):
            if p.is_file():
                pins.append({'file': str(p.relative_to(ROOT)), 'bytes': p.stat().st_size,
                             'sha256': hashlib.sha256(p.read_bytes()).hexdigest()})
    accepted = dt.datetime.fromisoformat(args.original_accept) if args.original_accept else dt.datetime.now(dt.timezone.utc)
    assert accepted.timestamp() <= time.time() + .01, 'An original ACCEPT cannot be in the future'
    work = accepted.timestamp() + args.work_seconds
    full = work + args.cleanup_seconds
    assert time.time() < work, 'Original work cutoff has expired'
    record = {'offer': args.offer, 'acceptUtc': accepted.isoformat(), 'workDeadlineUtc': dt.datetime.fromtimestamp(work, dt.timezone.utc).isoformat(),
              'fullDeadlineUtc': dt.datetime.fromtimestamp(full, dt.timezone.utc).isoformat(), 'attempt': args.attempt,
              'startedUtc': dt.datetime.now(dt.timezone.utc).isoformat(), 'candidate': args.candidate,
              'actualHead': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'dirtyAtStart': subprocess.check_output(['git', 'status', '--short'], cwd=ROOT, text=True),
              'retainedLogicalBytes': borrow, 'preflightWholeBytes': borrow + size(carry_roots), 'freeBytes': shutil.disk_usage(ROOT).free,
              'packagePins': packages, 'browserSha256': hashlib.sha256(Path(args.browser).read_bytes()).hexdigest(),
              'sourcePins': sorted(pins, key=lambda p: p['file']), 'argv': ['node', str(Path(__file__).with_name('capture.mjs').relative_to(ROOT))]}
    config = {'parent': os.getpid(), 'borrow': borrow, 'privateGit': private_git, 'workDeadline': work, 'fullDeadline': full, 'attempt': args.attempt}
    guard = subprocess.Popen([sys.executable, __file__, '--guardian', json.dumps(config)], cwd=ROOT,
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
    job = None
    try:
        assert guard.stdout.readline().strip() == 'READY'
        (PACKET / 'node_modules').symlink_to(args.deps, target_is_directory=True)
        (PACKET / 'native-tmp').mkdir()
        env = os.environ.copy()
        env.update(PATH=str(Path(args.node).parent) + os.pathsep + env['PATH'], TMPDIR=str(PACKET / 'native-tmp'),
                   MCR_ART_SOURCE_COMMIT=args.candidate, MCR_ART_ATTEMPT=args.attempt, MCR_ART_BROWSER=args.browser)
        gate = "import os,sys;assert sys.stdin.readline().strip()=='GO';os.execv(sys.argv[1],[sys.argv[1],sys.argv[2]])"
        job = subprocess.Popen([sys.executable, '-c', gate, args.node, str(PACKET / 'capture.mjs')], cwd=ROOT, env=env,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        guard.stdin.write(str(job.pid) + '\n')
        guard.stdin.flush()
        assert guard.stdout.readline().strip() == 'ARMED'
        record['guardianArmedBeforeWorkload'] = True
        job.stdin.write('GO\n')
        job.stdin.flush()
        job.stdin.close()
        job.stdin = None
        try:
            out, err = job.communicate(timeout=max(.1, work - time.time()))
            code = job.returncode
        except subprocess.TimeoutExpired:
            os.killpg(job.pid, signal.SIGKILL)
            out, err = job.communicate()
            code = 124
        record.update(exitCode=code, stdout=out, stderr=err)
        (PACKET / f'{args.attempt}.stdout.log').write_text(out)
        (PACKET / f'{args.attempt}.stderr.log').write_text(err)
    finally:
        if job is not None and job.poll() is None:
            os.killpg(job.pid, signal.SIGKILL)
            job.wait()
        guard.stdin.close()
        guard.stdin = None
        if guard.poll() is None:
            guard.terminate()
        out, err = guard.communicate(timeout=max(.1, full - time.time()))
        record.update(guardianOutput=out, guardianError=err, guardianExitCode=guard.returncode)
        receipt = json.loads(out.strip().splitlines()[-1])
        record['resourceAndCleanupPassed'] = (receipt['reason'] == 'NORMAL_STOP'
            and not receipt['remainingOwnedPids'] and receipt['port5197Free']
            and all(receipt['ephemeralAbsent'].values()) and guard.returncode == 0)
        if not record['resourceAndCleanupPassed'] and record.get('exitCode') == 0:
            record['exitCode'] = 1
    record.update(finishedUtc=dt.datetime.now(dt.timezone.utc).isoformat(), postWholeBytes=borrow + size(carry_roots),
                  postFreeBytes=shutil.disk_usage(ROOT).free, port5197Free=port_free())
    (PACKET / f'allocation-{args.attempt}.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({k: v for k, v in record.items() if k not in ('sourcePins', 'stdout', 'stderr')}
                     | {'stdoutTail': record.get('stdout', '')[-2000:], 'stderrTail': record.get('stderr', '')[-3500:]}))
    return record.get('exitCode', 1)


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == '--guardian':
        guardian(json.loads(sys.argv[2]))
    else:
        parser = argparse.ArgumentParser()
        for name in ('offer', 'attempt', 'candidate', 'node', 'deps', 'browser'):
            parser.add_argument('--' + name, required=True)
        parser.add_argument('--original-accept')
        parser.add_argument('--work-seconds', type=int, default=120)
        parser.add_argument('--cleanup-seconds', type=int, default=60)
        raise SystemExit(run(parser.parse_args()))
