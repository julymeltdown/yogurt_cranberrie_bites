"""Evidence supervisor; run only after the named slot is explicitly accepted."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument('--slot', required=True)
parser.add_argument('--accepted-epoch', required=True, type=float)
parser.add_argument('--driver', choices=['intrinsic', 'reference'], default='intrinsic')
args = parser.parse_args()
root = Path('/Users/lhs/.codex/worktrees/d51f/mcr')
assert Path.cwd().resolve() == root
evidence = root / 'docs/mcr/evidence/css-parity-feed'
node_bin = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin'
env = os.environ.copy()
env.update(PATH=node_bin + ':' + env['PATH'], MCR_FEED_REPO=str(root), MCR_FEED_SLOT=args.slot)
tmp = Path(tempfile.mkdtemp(prefix='mcr-css-feed-intrinsic-slot-'))
env['TMPDIR'] = str(tmp)
kind = args.driver
driver = 'native-intrinsic.mjs' if kind == 'intrinsic' else 'native-reference-media.mjs'
deadline = args.accepted_epoch + 110
report = {'slot': args.slot, 'acceptedEpoch': args.accepted_epoch,
          'commands': [], 'freeMinimum': shutil.disk_usage(root).free, 'wholePeak': 0}
ledger = evidence / ('supervisor-' + args.slot + '.json')


def total(path):
    return sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink())


def guard():
    free = shutil.disk_usage(root).free
    temporary = total(tmp)
    # Include retained imported derivative files in this checkout's footprint.
    package = root / 'frontend/public/reference-feed-media-v1'
    art_doc = root / 'docs/mcr/art/reference-feed-media-v1.md'
    retained_input = total(package) + (art_doc.stat().st_size if art_doc.exists() else 0)
    source_bytes = sum((root / path).stat().st_size for path in
                       ['frontend/src/features/event-feed/feed.css',
                        'docs/mcr/frontend/css-parity-feed.md'])
    durable = total(evidence) + retained_input + source_bytes
    size = temporary + durable
    report['freeMinimum'] = min(report['freeMinimum'], free)
    report['wholePeak'] = max(report['wholePeak'], size)
    report['temporaryPeak'] = max(report.get('temporaryPeak', 0), temporary)
    report['durablePeak'] = max(report.get('durablePeak', 0), durable)
    report['retainedInputBytes'] = retained_input
    report['ownedSourceBytes'] = source_bytes
    assert free >= 8 * 1024**3 and size < 193273528
    assert temporary < 160 * 1024**2 and durable < 20 * 1024**2
    assert time.time() < deadline


def run(argv, phase=None):
    guard()
    child_env = env.copy()
    if phase:
        child_env['MCR_FEED_PHASE'] = phase
    label = phase or (kind + '-ui-tests')
    log = evidence / (label + '.log')
    assert not log.exists(), 'preserve every previous attempt'
    with log.open('w') as output:
        child = subprocess.Popen(argv, cwd=root if phase else root / 'frontend',
                                 env=child_env, stdout=output, stderr=subprocess.STDOUT,
                                 start_new_session=True)
        try:
            while child.poll() is None:
                guard()
                time.sleep(.25)
        except BaseException:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            raise
    entry = {'argv': argv, 'phase': phase, 'exit': child.returncode,
             'elapsedSinceAccept': time.time() - args.accepted_epoch}
    report['commands'].append(entry)
    print(json.dumps(entry), flush=True)


try:
    number = 1
    while (evidence / (kind + str(number))).exists():
        number += 1
    run([node_bin + '/node', str(evidence / driver)], kind + str(number))
    print('OWNER_FLAGS ' + str(tmp) + ' : finish or retry', flush=True)
    while time.time() < deadline - 15:
        guard()
        if (tmp / 'retry').exists():
            (tmp / 'retry').unlink()
            number += 1
            run([node_bin + '/node', str(evidence / driver)], kind + str(number))
            print('OWNER_FLAGS ' + str(tmp) + ' : finish or retry', flush=True)
        if (tmp / 'finish').exists():
            break
        time.sleep(.25)
    if kind == 'intrinsic' and time.time() < deadline - 10:
        run([node_bin + '/node', str(root / 'frontend/node_modules/vitest/vitest.mjs'),
             'run', '--configLoader', 'runner', '--no-cache', '--maxWorkers', '1',
             'src/features/event-feed/__tests__/presentation.test.tsx',
             'src/features/feed-thread/__tests__/ui.test.tsx'])
finally:
    report['activeEndedEpoch'] = time.time()
    report['elapsedSinceAccept'] = time.time() - args.accepted_epoch
    shutil.rmtree(tmp)
    report['privateTmpRemoved'] = not tmp.exists()
    ledger.write_text(json.dumps(report, indent=2) + '\n')
    print('SUPERVISOR_DONE ' + str(ledger), flush=True)
