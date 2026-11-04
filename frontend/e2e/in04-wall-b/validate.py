#!/usr/bin/env python3
"""Execute only an actually allocated source round, retaining every failed command.
No browser/server/stack/install/build commands. An offer file records scheduling;
it does not itself authorize execution. The human allocator's accepted lease is required.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import shutil
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
LINK = ROOT / 'frontend/node_modules'
EVIDENCE = ROOT / 'docs/mcr/evidence/in04-wall-b'
# Future bounded SOURCE admissions only; never alter an accepted clock/offer.
START_FLOOR_BYTES = 11 * 1024**3
LIVE_FLOOR_BYTES = 8 * 1024**3
RESIDUAL_BYTES = 9 * 1024**3
MAX_BOUNDED_NEW_BYTES = 2 * 1024**3


def owned_bytes():
    total = 0
    for base in (HERE, EVIDENCE):
        for directory, dirs, files in os.walk(base, followlinks=False):
            dirs[:] = [d for d in dirs if not Path(directory, d).is_symlink()]
            for name in files:
                path = Path(directory, name)
                if not path.is_symlink():
                    total += path.stat().st_size
    # Conservative uncompressed bytes for this task's committed Git blobs; no
    # shared-object pruning, target inspection or peer-file deletion.
    paths = subprocess.check_output(['git', 'ls-files', 'frontend/e2e/in04-wall-b', 'docs/mcr/qa/in04-wall-b.md', 'docs/mcr/evidence/in04-wall-b'], cwd=ROOT, text=True).splitlines()
    for path in paths:
        result = subprocess.run(['git', 'cat-file', '-s', 'HEAD:' + path], cwd=ROOT, text=True, capture_output=True)
        if result.returncode == 0:
            total += int(result.stdout)
    return total


def descendants():
    observer = subprocess.Popen(['ps', '-axo', 'pid=,ppid=,rss='], text=True, stdout=subprocess.PIPE)
    rows = observer.communicate()[0].splitlines()
    assert observer.returncode == 0
    table = [tuple(map(int, row.split())) for row in rows]
    mine = {os.getpid()}
    for _ in table:
        before = len(mine)
        mine.update(pid for pid, parent, _ in table if parent in mine)
        if len(mine) == before:
            break
    mine.discard(observer.pid)
    return mine, sum(rss * 1024 for pid, _, rss in table if pid in mine)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--accepted-offer', required=True, type=Path)
    parser.add_argument('--attempt', default='attempt01')
    parser.add_argument('--profile', choices=('wall-b', 'reconnect'), default='wall-b')
    args = parser.parse_args()
    offer = json.loads(args.accepted_offer.read_text())
    required = {'allocation', 'accepted', 'work_deadline_epoch', 'hard_deadline_epoch', 'max_owned_bytes', 'max_new_bytes', 'max_rss_bytes', 'max_pids', 'live_floor_bytes', 'all_new_bytes'}
    assert required <= offer.keys() and offer['accepted'] is True
    assert time.time() < offer['work_deadline_epoch'] < offer['hard_deadline_epoch']
    allocation_root = EVIDENCE / offer['allocation']
    assert allocation_root.parent == EVIDENCE
    out = allocation_root / args.attempt
    assert out.parent == allocation_root and not out.exists(), 'Use a fresh attempt ID; failed rounds are immutable'
    assert NODE.is_file() and DEPS.is_dir() and not os.path.lexists(LINK)
    start_floor = offer.get('start_floor_bytes', START_FLOOR_BYTES)
    assert start_floor >= START_FLOOR_BYTES and offer['live_floor_bytes'] >= LIVE_FLOOR_BYTES
    assert 0 < offer['max_new_bytes'] <= MAX_BOUNDED_NEW_BYTES, 'Only bounded SOURCE NEW <=2GiB is supported'
    assert isinstance(offer['all_new_bytes'], int) and offer['all_new_bytes'] >= offer['max_new_bytes']
    free = os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize
    assert free >= start_floor, 'Fresh start floor required'
    # ALL outstanding/proposed worst-case additional NEW is allocator-supplied;
    # retained WHOLE/carry already occupies measured disk and is not subtracted.
    assert free - offer['all_new_bytes'] >= RESIDUAL_BYTES, '9GiB residual after ALL NEW required'
    assert hashlib.sha256((ROOT / 'frontend/pnpm-lock.yaml').read_bytes()).digest() == hashlib.sha256((DEPS.parent / 'pnpm-lock.yaml').read_bytes()).digest(), 'Retained dependency lock mismatch'
    out.mkdir(parents=True)
    (out / 'offer.json').write_text(json.dumps(offer, indent=2) + '\n')
    baseline_file = allocation_root / 'initial-owned-bytes.json'
    if not baseline_file.exists():
        baseline_file.write_text(json.dumps({'bytes': owned_bytes()}) + '\n')
    initial = json.loads(baseline_file.read_text())['bytes']
    peak = {'ownedBytes': initial, 'positiveNewBytes': 0, 'rssBytes': 0, 'pids': 0}
    child = None
    logs = []
    outcome = 'FAIL'
    guard = out / 'guardian.jsonl'
    env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ['PATH'], 'NO_COLOR': '1'}
    env['PWTEST_CACHE_DIR'] = str(out / 'scratch/playwright-transform')
    env.pop('NODE_COMPILE_CACHE', None)
    env.pop('MCR_IN04_CONFIG', None)  # Missing-input negative must never load real credentials.
    env['MCR_IN04_B_ARTIFACT_ROOT'] = str(out / 'unused-browser-output')
    commands = [
        ([str(NODE), '--experimental-transform-types', 'frontend/e2e/in04-wall-b/' + ('reconnect-check.ts' if args.profile == 'reconnect' else 'check.ts')], 0),
        ([str(NODE), 'frontend/node_modules/typescript/bin/tsc', '--noEmit', '--incremental', 'false', '-p', 'frontend/e2e/in04-wall-b/tsconfig.json', '--pretty', 'false'], 0),
        ([str(DEPS / '.bin/oxlint'), *(['frontend/e2e/in04-wall-b/' + name for name in ('reconnect.ts', 'reconnect-check.ts', 'wall-b.spec.ts')] if args.profile == 'reconnect' else ['frontend/e2e/in04-wall-b'])], 0),
        ([str(NODE), 'frontend/node_modules/@playwright/test/cli.js', 'test', '--config', 'frontend/e2e/in04-wall-b/playwright.config.ts', '--list'], 0),
        ([str(NODE), '--experimental-transform-types', 'frontend/e2e/in04-wall-b/preflight.ts'], 2),
    ]

    def sample():
        current = owned_bytes()
        pids, rss = descendants()
        free = os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize
        data = {'time': time.time(), 'ownedBytes': current, 'positiveNewBytes': max(0, current - initial), 'rssBytes': rss, 'pids': len(pids), 'freeBytes': free}
        for name in peak:
            peak[name] = max(peak[name], data[name])
        with guard.open('a') as stream:
            stream.write(json.dumps(data) + '\n')
        assert data['time'] < offer['work_deadline_epoch'], 'Work cutoff reached'
        assert free >= offer['live_floor_bytes'], 'Free-space floor breached'
        assert current <= offer['max_owned_bytes'], 'Whole owned-byte ceiling breached'
        assert data['positiveNewBytes'] <= offer['max_new_bytes'], 'Positive new-byte ceiling breached'
        assert rss <= offer['max_rss_bytes'] and len(pids) <= offer['max_pids'], 'Aggregate process/RSS ceiling breached'

    def stop_signal(signum, _frame):
        raise RuntimeError('Allocated source guardian interrupted by signal ' + str(signum))

    signal.signal(signal.SIGTERM, stop_signal)
    signal.signal(signal.SIGINT, stop_signal)
    try:
        LINK.symlink_to(DEPS, target_is_directory=True)
        for index, (argv, expected) in enumerate(commands):
            sample()  # Independent pre-child check; never infer capacity from a sent request.
            with (out / f'{index:02}.log').open('wb') as log:
                child = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                while child.poll() is None:
                    sample()
                    time.sleep(.25)
                logs.append({'argv': argv, 'exit': child.returncode, 'expected': expected})
                assert child.returncode == expected, 'Command failed; preserve evidence and repair within the accepted clock'
                child = None
        sample()
        outcome = 'PASS'
    finally:
        if child:
            try:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait(timeout=3)
            except ProcessLookupError:
                pass
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=3)
            if not logs or logs[-1]['argv'] != argv:
                logs.append({'argv': argv, 'exit': child.returncode, 'expected': expected})
        if LINK.is_symlink() and LINK.resolve() == DEPS:
            LINK.unlink()
        scratch = out / 'scratch'
        if scratch.exists():
            assert scratch.is_dir() and not scratch.is_symlink()
            shutil.rmtree(scratch)
        remaining, rss = descendants()
        # ps itself is a transient observer; no persistent child may remain.
        remaining.discard(os.getpid())
        source_hashes = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in HERE.glob('*') if p.is_file()}
        receipt = {'profile': args.profile, 'status': outcome, 'realIN04': 'NOT_RUN', 'commands': logs, 'peak': peak, 'finished_epoch': time.time(),
                   'sourceRevision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                   'sourceHashes': source_hashes, 'dependencyLinkRemaining': os.path.lexists(LINK), 'observedPidsAtCleanup': sorted(remaining),
                   'scratchOrServerAllocated': False, 'rssAtCleanup': rss}
        (out / 'release.json').write_text(json.dumps(receipt, indent=2) + '\n')
        print(json.dumps(receipt))
        assert not remaining and not os.path.lexists(LINK), 'FULL cleanup requires no descendants or dependency link'
        assert time.time() < offer['hard_deadline_epoch'], 'Hard cleanup deadline exceeded'
    return 0 if outcome == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
