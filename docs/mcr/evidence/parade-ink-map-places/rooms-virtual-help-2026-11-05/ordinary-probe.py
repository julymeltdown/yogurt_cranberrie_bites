"""Real ordinary stdin identity, exact digest, and terminal rejection boundaries."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from membership import OWNER, identity, ordinary_controls, processes, verify_controller

# Activity completion only selects ordinary observations; it never releases an
# accepted execution claim or asserts kernel death. Source claims are unchanged.
ended = dict(kind='ordinary', controller=dict(pid=1, endedAt=1), ordinaryObservation=dict(pid=2, birth='retained', endedAt=1))
unchanged = json.dumps(ended, sort_keys=True)
assert ordinary_controls(ended) == [] and json.dumps(ended, sort_keys=True) == unchanged
active = dict(kind='ordinary', controller=dict(pid=1), ordinaryObservation=dict(pid=2, birth='retained'))
assert len(ordinary_controls(active)) == 2
source = dict(kind='source', state='ACCEPTED', controller=dict(pid=1, endedAt=1), processCap=8, rssCapBytes=3*1024**3)
unchanged = json.dumps(source, sort_keys=True)
assert ordinary_controls(source) == [] and json.dumps(source, sort_keys=True) == unchanged
print('PASS: 3 new ordinary activity-selection cases; no kernel-exit or execution-release inference')

temporary = Path(os.environ['MCR_PI_TMP'])
claim = (OWNER, 'ordinary', 'ISOLATED_SOURCE07_REGRESSION', time.time())
child = subprocess.Popen([sys.executable, '-B', '-'], stdin=subprocess.PIPE, cwd=temporary)
child.stdin.write(b'import time\ntime.sleep(30)\n')
child.stdin.close()
passed = 0


def snapshot():
    rows = processes()
    return rows, hashlib.sha256(json.dumps(rows, sort_keys=True).encode()).hexdigest()


def reject(control, row, key, sha):
    global passed
    try:
        verify_controller(control, row, key, sha, ordinary=True)
    except AssertionError:
        passed += 1
    else:
        raise AssertionError('A changed or unverified ordinary identity must fail closed')


try:
    time.sleep(.05)
    rows, sha = snapshot()
    row = rows[child.pid]
    control = dict(pid=child.pid, birth=row['birth'], script='-', cwd=str(temporary),
                   executable=subprocess.check_output(['ps', '-p', str(child.pid), '-o', 'comm='], text=True).strip(),
                   commandDigest=hashlib.sha256(row['command'].strip().encode()).hexdigest())
    proof = verify_controller(control, row, claim, sha, ordinary=True)
    assert proof['actualProgram'] == '-' and Path(proof['actualCwd']).resolve() == temporary.resolve()
    passed += 1
    reject(dict(control, commandDigest='0' * 64), row, claim, sha)
    reject(dict(control, script=str(temporary / 'not-the-source.py')), row, claim, sha)
    reject(dict(control, cwd=str(temporary.parent)), row, claim, sha)
    reject(dict(control, executable='/usr/bin/false'), row, claim, sha)
    child.terminate()
    deadline = time.monotonic() + 3
    while True:
        rows, sha = snapshot()
        terminal = rows.get(child.pid)
        if terminal and terminal['state'].startswith('Z'):
            break
        assert time.monotonic() < deadline, 'A real unreaped Z state is required'
        time.sleep(.025)
    terminal_proof = verify_controller(control, terminal, claim, sha, ordinary=True)
    assert terminal_proof['terminal'] and terminal_proof['snapshotSHA256'] == proof['snapshotSHA256']
    passed += 1
    reject(control, terminal, ('different-owner', *claim[1:]), sha)
    reject(control, terminal, (claim[0], claim[1], 'unseen-offer', claim[3]), sha)
    reject(dict(control, script='different-token'), terminal, claim, sha)
    reject(control, dict(terminal, parent=terminal['parent'] + 1), claim, sha)
    assert not any(key in terminal_proof for key in ['state', 'resources', 'remainingNewUpperBytes', 'actualFullEpoch'])
finally:
    if child.returncode is None:
        try:
            child.kill()
        except ProcessLookupError:
            pass
    child.wait(timeout=2)

# Independently compare this same PID's kernel-derived producer digest with PS.
rows, sha = snapshot()
self_control = identity(os.getpid(), Path(__file__).resolve())
self_row = rows[os.getpid()]
assert self_control['commandDigest'] == hashlib.sha256(self_row['command'].strip().encode()).hexdigest()
assert self_control['argvDigest'] != self_control['commandDigest']
verify_controller(self_control, self_row, claim, sha, ordinary=True)
passed += 1
reject(dict(self_control, commandDigest=self_control['argvDigest']), self_row, claim, sha)

# A real -c process must never be mistaken for the truthful stdin '-' token.
child = subprocess.Popen([sys.executable, '-B', '-c', 'import time; time.sleep(30)'], cwd=temporary)
try:
    time.sleep(.05)
    rows, sha = snapshot()
    row = rows[child.pid]
    control = dict(pid=child.pid, birth=row['birth'], script='-', cwd=str(temporary),
                   executable=subprocess.check_output(['ps', '-p', str(child.pid), '-o', 'comm='], text=True).strip())
    reject(control, row, claim, sha)
finally:
    child.terminate()
    child.wait(timeout=2)
print(f'PASS: {passed} new real-OS ordinary inline/digest/terminal boundary cases; no cap release inferred')
