#!/usr/bin/env python3
"""Independent primary for the corrected full-carry SOURCE allocation only.
The retained de635bb3 guardian separately enforces the original preparation cap.
No services/browser/install/build or Git mutations are performed here.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = ROOT / 'docs/mcr/evidence/in04-wall-b'
parser = argparse.ArgumentParser()
parser.add_argument('--accepted-offer', required=True, type=Path)
parser.add_argument('--attempt', required=True)
args = parser.parse_args()
offer = json.loads(args.accepted_offer.read_text())
assert offer['accepted'] is True
round_root = EVIDENCE / offer['allocation']
assert round_root.parent == EVIDENCE
round_root.mkdir(exist_ok=True)
trace = round_root / ('primary-' + args.attempt + '.jsonl')
release = round_root / ('primary-' + args.attempt + '-release.json')
assert not trace.exists() and not release.exists()
roots = [Path(p) for p in offer['full_accounting']['deduplicated_roots']]
assert roots[0] == ROOT and all(p.is_absolute() and p.is_dir() for p in roots)
owned = [ROOT / 'frontend/e2e/in04-wall-b', EVIDENCE]
qa = ROOT / 'docs/mcr/qa/in04-wall-b.md'
paths = ['frontend/e2e/in04-wall-b', 'docs/mcr/evidence/in04-wall-b', 'docs/mcr/qa/in04-wall-b.md']
objects = subprocess.check_output(['git', 'rev-list', '--objects', '2ff0aab865a06a5664d2096ba572c766ac0fba85..HEAD', '--', *paths], cwd=ROOT, text=True)
ids = sorted({line.split(' ', 1)[0] for line in objects.splitlines()})
sizes = subprocess.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'], cwd=ROOT, input='\n'.join(ids) + '\n', text=True)
raw_git = sum(map(int, sizes.splitlines()))


def own_files():
    logical = blocks = 0
    seen = set()
    for p in [qa, *(p for base in owned for p in base.rglob('*'))]:
        if p.is_symlink() or not p.is_file():
            continue
        st = p.stat()
        if (st.st_dev, st.st_ino) in seen:
            continue
        seen.add((st.st_dev, st.st_ino))
        logical += st.st_size
        blocks += st.st_blocks * 512
    return logical, blocks


def carry():
    # One du invocation deduplicates shared hard links across every retained root.
    output = subprocess.check_output(['du', '-sk', *map(str, roots)], text=True)
    return sum(int(row.split('\t', 1)[0]) * 1024 for row in output.splitlines())


def processes():
    result = subprocess.Popen(['ps', '-axo', 'pid=,ppid=,rss='], stdout=subprocess.PIPE, text=True)
    rows = [tuple(map(int, row.split())) for row in result.communicate()[0].splitlines()]
    assert result.returncode == 0
    mine = {os.getpid()}
    for _ in rows:
        previous = len(mine)
        mine.update(pid for pid, parent, _ in rows if parent in mine)
        if len(mine) == previous:
            break
    mine.discard(result.pid)
    return mine, sum(rss * 1024 for pid, _, rss in rows if pid in mine)


initial_path = round_root / 'primary-initial.json'
if not initial_path.exists():
    logical, blocks = own_files()
    initial_path.write_text(json.dumps({'logical': logical, 'blocks': blocks, 'rawGitBytes': raw_git}) + '\n')
initial = json.loads(initial_path.read_text())
assert initial['rawGitBytes'] == raw_git, 'No own commits during an accepted source round'
peak = {'fullCarryBytes': offer['full_accounting']['prior_max_bytes'], 'positiveRoundBytes': 0, 'preparationBytes': 0, 'pids': 0, 'rssBytes': 0}
for previous in round_root.glob('primary-*-release.json'):
    old = json.loads(previous.read_text())['peak']
    for key in peak:
        peak[key] = max(peak[key], old[key])
child = None
outcome = 'FAIL'
failure = None


def sample(admission=False):
    current_carry = carry()
    logical, blocks = own_files()
    mine, rss = processes()
    v = os.statvfs(ROOT)
    free = v.f_bavail * v.f_frsize
    data = {'time': time.time(), 'fullCarryBytes': current_carry,
            'positiveRoundBytes': max(0, logical - initial['logical'], blocks - initial['blocks']),
            'preparationBytes': logical + raw_git, 'pids': len(mine) + 2, 'rssBytes': rss, 'freeBytes': free}
    # Two additional positions conservatively reserve transient observers/wrapper.
    for key in peak:
        peak[key] = max(peak[key], data[key])
    with trace.open('a') as stream:
        stream.write(json.dumps(data) + '\n')
    assert data['time'] < offer['work_deadline_epoch'], 'Work cutoff'
    assert peak['fullCarryBytes'] <= offer['max_whole_bytes'], 'Full nonshrinking WHOLE ceiling'
    assert data['positiveRoundBytes'] <= offer['max_new_bytes'], 'Positive logical/allocated round growth ceiling'
    assert data['preparationBytes'] <= offer['max_owned_bytes'], 'Original preparation/history ceiling'
    assert data['pids'] <= offer['max_pids'] and rss <= offer['max_rss_bytes'], 'Aggregate process/RSS ceiling'
    assert free >= offer['live_floor_bytes'], 'Live floor'
    if admission:
        assert free >= offer['start_floor_bytes'] and free - offer['all_new_bytes'] >= 9 * 1024**3
        assert peak['fullCarryBytes'] + offer['max_new_bytes'] <= offer['max_whole_bytes'], 'Carry plus worst-case NEW exceeds WHOLE'


def interrupted(signum, _frame):
    raise RuntimeError('Primary interrupted: ' + str(signum))


signal.signal(signal.SIGTERM, interrupted)
signal.signal(signal.SIGINT, interrupted)
try:
    sample(admission=True)  # Full independent primary precedes the retained guardian/children.
    with (round_root / ('primary-' + args.attempt + '-inner.log')).open('wb') as stream:
        child = subprocess.Popen(['python3', 'frontend/e2e/in04-wall-b/validate.py', '--accepted-offer', str(args.accepted_offer.resolve()), '--attempt', args.attempt, '--profile', 'reconnect'], cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
        while child.poll() is None:
            sample()
            time.sleep(.25)
        assert child.returncode == 0, 'Retain failed guardian/command evidence and repair within this same clock'
        child = None
    sample()
    outcome = 'PASS'
except BaseException as error:
    failure = str(error)
    raise
finally:
    if child and child.poll() is None:
        child.terminate()  # Guardian handles its own child process group and link cleanup.
        try:
            child.wait(timeout=8)
        except subprocess.TimeoutExpired:
            mine, _ = processes()
            for pid in mine - {os.getpid()}:
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            child.wait(timeout=3)
    remaining, rss = processes()
    remaining.discard(os.getpid())
    result = {'status': outcome, 'failure': failure, 'peak': peak, 'finishedEpoch': time.time(),
              'remainingDescendants': sorted(remaining), 'dependencyLinkRemaining': os.path.lexists(ROOT / 'frontend/node_modules'),
              'scratchRemaining': (round_root / args.attempt / 'scratch').exists(), 'rssAtCleanup': rss,
              'fullRoots': list(map(str, roots)), 'originalPreparationRawGitBytes': raw_git, 'realIN04': 'NOT_RUN'}
    release.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
    assert not remaining and not result['dependencyLinkRemaining'] and not result['scratchRemaining']
    assert time.time() < offer['hard_deadline_epoch'], 'Hard FULL cutoff'
