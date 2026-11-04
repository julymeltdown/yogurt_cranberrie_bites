"""Independent source-only guardian. Requires an actual fresh accepted offer.

Counts the whole checkout, shared Git store, retained dependency/tool trees and
this round's scratch. Never starts Vite, a browser, a backend or an installer.
"""
import argparse
import base64
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
OUT = ROOT / 'docs/mcr/evidence/in04-runtime-adapter'
TAIL = Path('/Users/lhs/Desktop/mcr-worktrees/fe-ref-map/frontend/src/app/errors/fe41-lifecycle/native.mjs')
TAIL_SHA256 = 'c8ee10c427f023a9a7622dd6320729b6ff5488d6a97c1df52d7ed3e3f8878122'


def inventory_records(bases):
    records = {}
    for base in bases:
        for directory, dirs, files in os.walk(base, followlinks=False):
            dirs[:] = [d for d in dirs if not Path(directory, d).is_symlink()]
            for name in files:
                path = Path(directory, name)
                if path.is_symlink():
                    continue
                try:
                    stat = path.stat()
                except FileNotFoundError:
                    continue
                key = stat.st_dev, stat.st_ino
                records[key] = stat.st_size
    return records


def inventory(bases):
    return sum(inventory_records(bases).values())


def processes(root_pid=None):
    observer = subprocess.Popen(['ps', '-axo', 'pid=,ppid=,rss='], stdout=subprocess.PIPE, text=True)
    rows = [tuple(map(int, line.split())) for line in observer.communicate()[0].splitlines()]
    assert observer.returncode == 0
    mine = {root_pid or os.getpid()}
    while True:
        more = mine | {pid for pid, parent, _ in rows if parent in mine}
        if more == mine:
            break
        mine = more
    mine.discard(observer.pid)
    return mine, sum(rss * 1024 for pid, _, rss in rows if pid in mine)


def independent_watch(offer, out, bases, root_pid):
    assert os.getppid() == root_pid
    baseline = inventory_records(bases)
    highwater = {}
    peak = dict(ownedBytes=sum(baseline.values()), positiveNewBytes=0, pids=0, rssBytes=0)
    failure = None
    try:
        while not (out / 'watch-stop').exists():
            ids, rss = processes(root_pid)
            current = inventory_records(bases)
            for key, value in current.items():
                highwater[key] = max(highwater.get(key, 0), max(0, value - baseline.get(key, 0)))
            data = dict(time=time.time(), ownedBytes=sum(current.values()), positiveNewBytes=sum(highwater.values()),
                        pids=len(ids), rssBytes=rss, freeBytes=os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize)
            for name in peak:
                peak[name] = max(peak[name], data[name])
            with (out / 'independent.jsonl').open('a') as stream:
                stream.write(json.dumps(data) + '\n')
            assert data['time'] < offer['work_deadline_epoch'] and data['freeBytes'] >= offer['live_floor_bytes']
            assert max(peak['ownedBytes'], offer.get('carry_bytes', peak['ownedBytes'])) + data['positiveNewBytes'] <= offer['max_owned_bytes']
            assert data['positiveNewBytes'] <= offer['max_new_bytes']
            assert len(ids) <= offer['max_pids'] and rss <= offer['max_rss_bytes']
            (out / 'watch-ready').touch()
            time.sleep(.1)
    except BaseException as error:
        failure = type(error).__name__
        os.kill(root_pid, signal.SIGTERM)
    finally:
        (out / 'independent-release.json').write_text(json.dumps(dict(peak=peak, failure=failure)) + '\n')
    return failure is None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--accepted-offer', required=True, type=Path)
    parser.add_argument('--attempt', required=True)
    parser.add_argument('--watch-pid', type=int)
    args = parser.parse_args()
    offer = json.loads(args.accepted_offer.read_text())
    assert offer['accepted'] is True and time.time() < offer['work_deadline_epoch'] < offer['hard_deadline_epoch']
    assert offer['max_new_bytes'] <= 128 * 1024**2
    assert offer['start_floor_bytes'] >= 11 * 1024**3 and offer['live_floor_bytes'] >= 8 * 1024**3
    assert offer['all_new_bytes'] >= offer['max_new_bytes']
    out = OUT / offer['allocation'] / args.attempt
    assert out.parent.parent == OUT and out.parent.name == offer['allocation'] and out.name == args.attempt
    common = Path(subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT, text=True).strip())
    bases = [ROOT, common, DEPS, NODE.parent.parent]
    if args.watch_pid:
        return independent_watch(offer, out, bases, args.watch_pid)
    assert not out.exists()
    free = os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize
    assert free >= offer['start_floor_bytes'] and free - offer['all_new_bytes'] >= 9 * 1024**3
    baseline = inventory_records(bases)
    initial = sum(baseline.values())
    highwater = {}
    assert initial <= offer['max_owned_bytes']
    out.mkdir(parents=True)
    scratch = out / 'scratch'
    scratch.mkdir()
    (out / 'offer.json').write_text(json.dumps(offer, indent=2) + '\n')
    source = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in HERE.iterdir() if p.suffix in ('.mjs', '.py')}
    include_tail = offer.get('include_fe41_tail', True)
    if include_tail:
        assert hashlib.sha256(TAIL.read_bytes()).hexdigest() == TAIL_SHA256
    (out / 'readset.json').write_text(json.dumps(source, indent=2) + '\n')
    if include_tail:
        (out / 'fe41-readonly-input.json').write_text(json.dumps({'path': str(TAIL), 'sha256': TAIL_SHA256,
            'source': '4d49d95e6f221cbcc8040f31b0499aa2151ad7ff', 'mode': 'read-only final harness checks; no product test replay'}, indent=2) + '\n')
    env = {**os.environ, 'TMPDIR': str(scratch), 'NO_COLOR': '1', 'PATH': str(NODE.parent) + ':' + os.environ['PATH'],
           'MCR_IN04_SOURCE_REPO': str(ROOT), 'MCR_IN04_SOURCE_CANDIDATE': offer['candidate']}
    env.pop('NODE_COMPILE_CACHE', None)
    relay_only = offer.get('qa05_relay', False)
    commands = ([[str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', str(TAIL)],
                 [str(NODE), '--check', str(TAIL)]] if include_tail else [])
    if offer.get('deferred_bundle', False):
        commands += [[str(NODE), str(HERE / 'deferred.mjs'), str(ROOT), offer['backend_source'], offer['backend_sha256'], str(HERE / 'qualified-runtime.mjs')],
                     [str(NODE), '--check', str(HERE / 'qualified-runtime.mjs')]]
    if relay_only:
        assert not include_tail and not offer.get('deferred_bundle', False)
        factory = subprocess.check_output(['git', 'show', offer['frame_factory_source'] + ':frontend/e2e/qa05-functional-recovery/frame-faults.mjs'], cwd=ROOT)
        assert hashlib.sha256(factory).hexdigest() == 'ad76fd4be3702d90676ed5b156c8831e2b5f5f016595f5c8130ec8020e975a05'
        env['MCR_QA05_FRAME_FACTORY'] = 'data:text/javascript;base64,' + base64.b64encode(factory).decode()
        paths = [str(HERE / name) for name in ['qa05-relay.mjs', 'qa05-relay.test.mjs']]
        commands += [[str(NODE), '--check', path] for path in paths]
        commands += [[str(NODE), '--test', '--test-isolation=none', paths[1]],
                     [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', *paths]]
    else:
        commands += [[str(NODE), '--check', str(p)] for p in sorted(HERE.glob('*.mjs'))]
        commands += [[str(NODE), '--test', '--test-isolation=none', str(HERE / 'source.test.mjs'), str(HERE / 'deferred.test.mjs')],
                     [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', str(HERE)]]
    child, history, observed = None, [], set()
    peak = dict(ownedBytes=initial, positiveNewBytes=0, pids=0, rssBytes=0)
    status = 'FAIL'

    def sample():
        ids, rss = processes()
        current = inventory_records(bases)
        owned = sum(current.values())
        for key, value in current.items():
            highwater[key] = max(highwater.get(key, 0), max(0, value - baseline.get(key, 0)))
        observed.update(ids)
        data = dict(time=time.time(), ownedBytes=owned, positiveNewBytes=sum(highwater.values()), pids=len(ids), rssBytes=rss,
                    freeBytes=os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize)
        for name in peak:
            peak[name] = max(peak[name], data[name])
        with (out / 'guardian.jsonl').open('a') as stream:
            stream.write(json.dumps(data) + '\n')
        assert data['time'] < offer['work_deadline_epoch'] and data['freeBytes'] >= offer['live_floor_bytes']
        assert max(peak['ownedBytes'], offer.get('carry_bytes', initial)) + data['positiveNewBytes'] <= offer['max_owned_bytes']
        assert data['positiveNewBytes'] <= offer['max_new_bytes']
        assert len(ids) <= offer['max_pids'] and rss <= offer['max_rss_bytes']

    def stop(signum, _):
        raise RuntimeError('Guardian received signal ' + str(signum))

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    watcher = None
    try:
        if relay_only:
            watcher = subprocess.Popen([str(Path(os.sys.executable)), '-B', str(HERE / 'validate.py'),
                '--accepted-offer', str(args.accepted_offer), '--attempt', args.attempt, '--watch-pid', str(os.getpid())], start_new_session=True)
            ready_deadline = time.monotonic() + 10
            while not (out / 'watch-ready').exists():
                assert watcher.poll() is None and time.monotonic() < ready_deadline
                time.sleep(.05)
        for index, argv in enumerate(commands):
            sample()
            with (out / f'{index:02}.log').open('wb') as stream:
                child = subprocess.Popen(argv, cwd=TAIL.parents[5] if str(TAIL) in argv else ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
                while child.poll() is None:
                    sample()
                    time.sleep(.25)
                history.append(dict(argv=argv, exitCode=child.returncode))
                assert child.returncode == 0, 'Source check failed; preserve failure, repair within actual accepted clock'
                child = None
        sample()
        assert source == {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in source}
        if include_tail:
            assert hashlib.sha256(TAIL.read_bytes()).hexdigest() == TAIL_SHA256
        status = 'PASS'
    finally:
        if child and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=3)
        if watcher is not None:
            (out / 'watch-stop').touch()
            try:
                watcher.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(watcher.pid, signal.SIGTERM)
                watcher.wait(timeout=3)
            if watcher.returncode:
                status = 'FAIL'
        for path in scratch.iterdir():
            assert path.is_file() and not path.is_symlink(), 'Unexpected scratch custody; retain for owner cleanup'
            path.unlink()
        scratch.rmdir()
        live, _ = processes()
        live.discard(os.getpid())
        assert not live and time.time() < offer['hard_deadline_epoch']
        (out / 'release.json').write_text(json.dumps(dict(status=status, finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
            commands=history, peak=peak, observedPids=sorted(observed), remainingDescendants=sorted(live), scratchRemaining=False), indent=2) + '\n')
    return status == 'PASS'


if __name__ == '__main__':
    assert main()
