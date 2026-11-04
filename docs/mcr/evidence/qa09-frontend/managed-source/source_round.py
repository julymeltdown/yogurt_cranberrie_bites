"""Finite QA09 source-only controller, adapted from qualified TOOLS-FS-01 runner.

Inventory mode admits no workload. Run mode requires a literal finite 21d offer.
No dependency install, link, browser, socket, PG, build, stack or target access.
"""
from pathlib import Path
import argparse
import json
import os
import signal
import subprocess
import sys
import time
from resource_guard import api, Guard, HELPER, HELPER_SHA, collect_whole, digest, processes, write

REPO = Path('/Users/lhs/Desktop/mcr-worktrees/qa-in01-world')
GIT = Path('/Users/lhs/Desktop/mcr/.git')
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
HERE = Path(__file__).resolve().parent
SOURCE = REPO / 'frontend/e2e/real/qa09-recovery'


def roots():
    # Whole checkout includes physical node_modules and ALL IN01/QA09 proof.
    # Child symlinks are never followed. Their external execution targets must
    # be separately declared; Node/Python/helper trees are explicitly included.
    scopes = {'checkout': REPO, 'sharedGit': GIT, 'nodeToolchain': NODE.parent.parent,
              'pythonRuntime': Path(sys.base_prefix), 'systemBin': Path('/bin'),
              'accountingHelper': HELPER.parent}
    return {key: api.RootIdentity.capture(path) for key, path in scopes.items()}


def pins():
    paths = [SOURCE / name for name in ['managed-admission.mjs', 'managed-admission.test.mjs',
                                       'journey.mjs', 'actions.mjs', 'cases.mjs']]
    paths += [HERE / 'resource_guard.py', HERE / 'source_round.py', HELPER, NODE]
    return [{'path': str(path), 'sha256': digest(path),
             'bytes': path.stat().st_size} for path in paths]


def inventory(path):
    started = time.monotonic()
    deadline = started + 45
    def check():
        if time.monotonic() >= deadline:
            raise RuntimeError('inventory deadline')
    bindings = roots()
    result = collect_whole(bindings, deadline=deadline, checkpoint=check)
    result.update(kind='QA09-complete-readonly-inode-union', sampledEpoch=time.time(),
                  elapsedSeconds=time.monotonic() - started,
                  roots={key: value.__dict__ for key, value in bindings.items()},
                  helper={'path': str(HELPER), 'sha256': HELPER_SHA}, pins=pins(),
                  workloadAdmitted=False, ownedWorkflow=0,
                  note='Read-only source-intake inventory; not an execution offer or live-case evidence.')
    write(path, result)
    print(json.dumps({'complete': True, 'bytes': result['bytes'], 'scopes': result['scopes'],
                      'elapsedSeconds': result['elapsedSeconds']}))


def run(args):
    directory = Path(args.run_dir).resolve()
    if not directory.is_relative_to(HERE) or directory.exists():
        raise RuntimeError('new exact QA09 evidence directory required')
    directory.mkdir(mode=0o700)
    started, epoch = time.monotonic(), time.time()
    deadline, full_deadline = started + 60, started + 90
    config = {'offer': args.offer, 'parent': os.getpid(), 'source': args.source,
              'checkout': str(REPO), 'branchRef': str(GIT / 'refs/heads/codex/mcr-fe-QA09'),
              'branchRefName': 'refs/heads/codex/mcr-fe-QA09', 'gitCommon': str(GIT),
              'headPath': str(GIT / 'worktrees/qa-in01-world/HEAD'),
              'acceptedEpoch': epoch, 'deadline': deadline, 'fullDeadline': full_deadline,
              'processLock': str(directory / 'process-measurement.lock'),
              'retainedNew': args.retained_new, 'priorWhole': args.prior_whole,
              'reservation': args.reservation, 'pins': pins(),
              'caps': {'free_floor': args.live_free, 'new': args.new_cap,
                       'whole': args.whole_cap, 'processes': 4, 'rss': 256 * 1024 ** 2}}
    failure = None
    child = guardian = None
    guard = None
    commands = []
    def interrupted(*_):
        raise RuntimeError('signal or independent guard')
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    try:
        space = os.statvfs(REPO)
        if space.f_bavail * space.f_frsize < args.start_free:
            raise RuntimeError('start free gate')
        bindings = roots()
        config['roots'] = {key: value.__dict__ for key, value in bindings.items()}
        last_initial = 0
        def initial_check():
            nonlocal last_initial
            if time.monotonic() >= deadline:
                raise RuntimeError('deadline before workload')
            if time.monotonic() - last_initial < .1:
                return
            current = processes(os.getpid(), config['processLock'], deadline)
            last_initial = time.monotonic()
            space = os.statvfs(REPO)
            if (space.f_bavail * space.f_frsize < args.live_free
                    or current['processes'] > 4 or current['rss'] > 256 * 1024 ** 2):
                raise RuntimeError('pre-workload resource limit')
        baseline = collect_whole(bindings, deadline=deadline, checkpoint=initial_check)
        config['baseline'] = {key: value['bytes'] for key, value in baseline['scopes'].items()}
        write(directory / 'baseline.json', baseline)
        write(directory / 'config.json', config)
        guard = Guard(config)
        guard.sample()
        guardian = subprocess.Popen([sys.executable, '-B', str(HERE / 'resource_guard.py'),
                                     str(directory / 'config.json')], start_new_session=True)
        while not (directory / 'guardian-ready.json').exists():
            guard.live(True)
            if guardian.poll() is not None:
                raise RuntimeError('guardian did not become ready')
            time.sleep(.025)
        workload = [('new-tests', [str(NODE), '--test', '--test-isolation=none',
                                  '--test-concurrency=1', str(SOURCE / 'managed-admission.test.mjs')])]
        workload += [('syntax-' + name, [str(NODE), '--check', str(SOURCE / name)])
                     for name in ['managed-admission.mjs', 'managed-admission.test.mjs', 'journey.mjs']]
        env = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1', 'TMPDIR': str(directory)}
        for name, command in workload:
            guard.live(True)
            if guardian.poll() is not None:
                raise RuntimeError('independent guard stopped')
            row = {'name': name, 'command': command, 'startedEpoch': time.time()}
            commands.append(row)
            with (directory / (name + '.log')).open('xb') as output:
                child = subprocess.Popen(command, cwd=REPO, env=env, stdout=output,
                                         stderr=subprocess.STDOUT, start_new_session=True)
                while child.poll() is None:
                    guard.live()
                    if guardian.poll() is not None:
                        raise RuntimeError('independent guard stopped')
                    time.sleep(.01)
                row.update(exitCode=child.wait(timeout=1), finishedEpoch=time.time())
            if row['exitCode']:
                raise RuntimeError(name + ' exited nonzero')
            guard.sample()
    except BaseException as error:
        failure = type(error).__name__ + ': ' + str(error)
        if isinstance(error, api.AccountingError):
            write(directory / 'incomplete.json', error.receipt())
    finally:
        # A guardian failure must stop work, but must not interrupt retained-handle
        # cleanup a second time while the primary is reaping its own children.
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=.5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=1)
        (directory / 'stop').write_text('stop\n')
        if guardian is not None:
            try:
                guardian.wait(timeout=min(10, max(.1, full_deadline - time.monotonic())))
            except subprocess.TimeoutExpired:
                os.killpg(guardian.pid, signal.SIGTERM)
                try:
                    guardian.wait(timeout=.5)
                except subprocess.TimeoutExpired:
                    os.killpg(guardian.pid, signal.SIGKILL)
                    guardian.wait(timeout=1)
            if guardian.returncode:
                failure = failure or 'guardian exit ' + str(guardian.returncode)
        closure_verified = False
        remaining = []
        try:
            remaining = processes(os.getpid(), config['processLock'], full_deadline)['pids']
            closure_verified = True
        except BaseException as error:
            failure = failure or 'cleanup process closure unavailable: ' + type(error).__name__
        alive = []
        for pid in remaining:
            if pid == os.getpid():
                continue
            try:
                os.kill(pid, 0)
                alive.append(pid)
            except ProcessLookupError:
                pass
        if alive or time.monotonic() > full_deadline:
            failure = failure or 'cleanup incomplete'
        report = {'offer': args.offer, 'source': args.source, 'failure': failure,
                  'commands': commands, 'actualFullEpoch': time.time(),
                  'workflow': 0 if closure_verified and not alive else 'UNKNOWN',
                  'ownedChildren': alive, 'remainingNewReservation': 0,
                  'processClosureVerified': closure_verified,
                  'withinFullDeadline': time.monotonic() <= full_deadline,
                  'primary': None if guard is None else guard.receipt(failure),
                  'guardianExit': None if guardian is None else guardian.returncode,
                  'allSixActualCases': 'NOT_RUN', 'wholeQa09Accepted': False}
        write(directory / 'FULL.json', report)
        print(json.dumps(report))
    return 1 if failure else 0


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--inventory')
    parser.add_argument('--run-dir')
    parser.add_argument('--offer')
    parser.add_argument('--source')
    parser.add_argument('--retained-new', type=int, default=0)
    parser.add_argument('--prior-whole', type=int, default=0)
    # Small conservative buffer includes final receipts written after guard stop;
    # the allocator separately reserves the full 16 MiB incremental output cap.
    parser.add_argument('--reservation', type=int, default=64 * 1024)
    parser.add_argument('--new-cap', type=int)
    parser.add_argument('--whole-cap', type=int)
    parser.add_argument('--start-free', type=int, default=16 * 1024 ** 3)
    parser.add_argument('--live-free', type=int, default=8 * 1024 ** 3)
    args = parser.parse_args()
    if args.inventory:
        inventory(Path(args.inventory))
    else:
        if not (args.offer and args.source and args.run_dir and args.new_cap and args.whole_cap):
            parser.error('literal offer/source/run directory and exact NEW/WHOLE caps required')
        raise SystemExit(run(args))
