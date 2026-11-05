"""PI05 finite browser runner. Requires actual PI04 native release and explicit ACCEPT.

Two independent live guards sample the complete resource roots, process tree,
disk floors, fixed deadlines, and total positive growth. No dependency mutation.
"""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

GIB = 1024 ** 3
CONFIG = Path(sys.argv[1]).resolve()
LEASE = json.loads(CONFIG.read_text())
OUT = Path(LEASE['output']).resolve()
ROOT = Path(LEASE['checkout']).resolve()
SECONDARY = len(sys.argv) > 2 and sys.argv[2] == 'secondary'
ROOT_PID = int(sys.argv[3]) if SECONDARY else os.getpid()
STATE = OUT / 'state.json'
HEARTBEAT = OUT / 'secondary.json'
PEAK = {'processes': 0, 'rssBytes': 0, 'wholeBytes': LEASE['priorWholeMax'], 'newBytes': 0}


def save(path, value):
    temporary = path.with_suffix('.next')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


COMMANDS = {}

def table():
    global COMMANDS
    probe = subprocess.Popen(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,args='], stdout=subprocess.PIPE, text=True)
    rows, _ = probe.communicate()
    assert probe.returncode == 0
    parsed = [line.split(maxsplit=4) for line in rows.splitlines()]
    COMMANDS = {int(v[0]): v[4] for v in parsed if len(v) == 5}
    return {int(v[0]): tuple(map(int, v[1:4])) for v in parsed if len(v) >= 4 and int(v[0]) != probe.pid}


def tree(rows):
    ids = {ROOT_PID}
    while True:
        expanded = ids | {pid for pid, (parent, _group, _rss) in rows.items() if parent in ids}
        expanded |= {pid for pid, command in COMMANDS.items() if str(OUT / 'tmp') in command}
        if expanded == ids:
            return ids & rows.keys()
        ids = expanded


def allocated(paths):
    rows = subprocess.check_output(['du', '-sk', *paths], text=True).splitlines()
    return sum(int(row.split()[0]) * 1024 for row in rows)


def stop_groups(groups):
    # These groups are created exclusively by this runner within this fixed lease.
    for sig in (signal.SIGTERM, signal.SIGKILL):
        rows = table()
        for pid, row in rows.items():
            if row[1] in groups or str(OUT / 'tmp') in COMMANDS.get(pid, ''):
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
        if sig == signal.SIGTERM:
            time.sleep(.4)


def sample():
    rows = table()
    ids = tree(rows)
    rss = sum(rows[pid][2] * 1024 for pid in ids)
    whole = allocated(LEASE['roots'])
    growth = max(0, whole - LEASE['baselineWholeBytes'])
    PEAK['wholeBytes'] = max(PEAK['wholeBytes'], whole)
    PEAK['newBytes'] = max(PEAK['newBytes'], growth)
    PEAK['processes'] = max(PEAK['processes'], len(ids))
    PEAK['rssBytes'] = max(PEAK['rssBytes'], rss)
    space = os.statvfs(ROOT)
    free = space.f_bavail * space.f_frsize
    membership = json.loads(Path(LEASE['membershipPath']).read_text())
    cohorts = [entry for entry in membership['entries'] if entry['cohort'] not in LEASE['literalReleasedCohorts']]
    assert len(cohorts) + 1 <= 4, 'Actual accepted global workflow envelope'
    assert sum(entry['processCap'] for entry in cohorts) + 24 <= 96
    assert sum(entry['rssCapGiB'] * GIB for entry in cohorts) + 4 * GIB <= 18 * GIB
    global_ids = set(ids)
    for entry in cohorts:
        matched = {pid for pid, command in COMMANDS.items() if any(root in command for root in entry['knownRootMatchers'])}
        while True:
            expanded = matched | {pid for pid, (parent, _group, _rss) in rows.items() if parent in matched}
            if expanded == matched:
                break
            matched = expanded
        global_ids |= matched & rows.keys()
    assert len(global_ids) + 1 <= 96 and sum(rows[pid][2] * 1024 for pid in global_ids) <= 18 * GIB
    assert time.time() < LEASE['acceptedEpoch'] + LEASE['workSeconds'], 'Fixed work deadline'
    assert free >= 8 * GIB, 'Continuous disk floor'
    assert PEAK['wholeBytes'] <= 8 * GIB, 'Complete WHOLE cap'
    assert PEAK['newBytes'] <= 256 * 1024 ** 2, 'All-phase positive NEW cap'
    assert len(ids) + 1 <= 24, 'Process cap includes the transient measurement helper'
    assert rss <= 4 * GIB, 'Actual complete process-tree RSS cap'
    assert sum(p.stat().st_size for p in OUT.glob('*.log')) <= 2 * 1024 ** 2, 'Aggregate log cap'
    temporary = allocated([str(OUT / 'tmp')]) if (OUT / 'tmp').exists() else 0
    cache = allocated([str(OUT / 'native/vite-cache')]) if (OUT / 'native/vite-cache').exists() else 0
    total_output = allocated([str(OUT)])
    assert temporary + cache <= 128 * 1024 ** 2, 'Aggregate private TMP/cache cap'
    assert total_output - temporary - cache <= 16 * 1024 ** 2, 'Durable proof cap'
    assert sum(p.stat().st_size for p in OUT.parent.glob('run*/native/*.png')) <= 8 * 1024 ** 2, 'All-attempt PNG cap'
    return {'epoch': time.time(), 'pid': os.getpid(), 'ownedPids': sorted(ids),
            'globalObservedPids': sorted(global_ids), 'globalDeclaredCohorts': [e['cohort'] for e in cohorts],
            'globalBoundary': 'Allocator membership plus actual root/descendant census; literal releases override stale membership. No shared external guard claimed.',
            'freeBytes': free, 'wholeBytes': whole, 'positiveNewBytes': growth, 'rssBytes': rss, 'peak': dict(PEAK)}


if SECONDARY:
    try:
        while True:
            current = json.loads(STATE.read_text())
            if current.get('stop'):
                break
            assert ROOT_PID in table(), 'Primary guard exited'
            assert time.time() - current['heartbeat'] < 8, 'Primary guard heartbeat stalled'
            save(HEARTBEAT, sample())
            time.sleep(.5)
    except BaseException as error:
        try:
            current = json.loads(STATE.read_text())
            stop_groups(current.get('groups', []))
            save(OUT / 'secondary-failure.json', {'epoch': time.time(), 'error': str(error), 'peak': PEAK})
        finally:
            sys.exit(1)
    sys.exit(0)

assert os.environ.get('PI05_NATIVE_ACCEPTED') == '1'
assert LEASE['id'] == 'PI05-MEET-APP-NATIVE01-20261105T0023Z'
assert LEASE['pi04NativePredecessorReleased'] is True
assert LEASE['workSeconds'] == 300 and LEASE['fullSeconds'] == 360
assert not OUT.exists(), 'Private execution output must be fresh'
assert ROOT == Path('/Users/lhs/Desktop/mcr-worktrees/fe-ref-customize')
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == LEASE['head']
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip()
space = os.statvfs(ROOT)
free = space.f_bavail * space.f_frsize
assert free >= 12 * GIB and free - 256 * 1024 ** 2 - LEASE['otherGrowthBytes'] >= 9 * GIB
assert LEASE['otherGrowthBytes'] >= 3 * GIB
assert allocated(LEASE['roots']) + 256 * 1024 ** 2 <= 8 * GIB
OUT.mkdir(parents=True, mode=0o700)
(OUT / 'tmp').mkdir(mode=0o700)
state = {'heartbeat': time.time(), 'groups': [], 'stop': False}
save(STATE, state)
secondary = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), str(CONFIG), 'secondary', str(ROOT_PID)])
commands = []
status = 'FAIL'
try:
    start = time.monotonic()
    while not HEARTBEAT.exists():
        assert secondary.poll() is None and time.monotonic() - start < 8
        state['heartbeat'] = time.time(); save(STATE, state); time.sleep(.1)
    # Both initial samples and both live guard identities precede any Node workload.
    initial = sample()
    assert time.time() - json.loads(HEARTBEAT.read_text())['epoch'] < 4
    save(OUT / 'initial.json', {'primary': initial, 'secondary': json.loads(HEARTBEAT.read_text())})
    env = dict(os.environ, **LEASE['environment'], TMPDIR=str(OUT / 'tmp'), TMP=str(OUT / 'tmp'), TEMP=str(OUT / 'tmp'),
               XDG_CACHE_HOME=str(OUT / 'cache'), NODE_OPTIONS='--max-old-space-size=2048')
    for index, argv in enumerate(LEASE['commands'], 1):
        state['heartbeat'] = time.time(); save(STATE, state)
        assert secondary.poll() is None
        sample()
        with (OUT / f'{index:02d}.log').open('w') as log:
            child = subprocess.Popen(argv, cwd=ROOT / 'frontend', env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            state['groups'].append(child.pid); state['heartbeat'] = time.time(); save(STATE, state)
            while child.poll() is None:
                state['heartbeat'] = time.time(); save(STATE, state)
                assert secondary.poll() is None, 'Secondary guard stopped'
                assert time.time() - json.loads(HEARTBEAT.read_text())['epoch'] < 8, 'Secondary heartbeat stalled'
                save(OUT / 'primary.json', sample()); time.sleep(.3)
            commands.append({'argv': argv, 'exitCode': child.returncode})
            save(OUT / 'commands.json', commands)
            assert child.returncode == 0, f'Stage {index} failed; repair and rerun requires remaining original time/caps'
    status = 'PASS'
except BaseException as error:
    save(OUT / 'failure.json', {'error': str(error), 'epoch': time.time(), 'peak': PEAK})
finally:
    stop_groups(state['groups'])
    state['stop'] = True; state['heartbeat'] = time.time(); save(STATE, state)
    try:
        secondary.wait(timeout=5)
    except subprocess.TimeoutExpired:
        secondary.kill(); secondary.wait()
    if HEARTBEAT.exists():
        secondary_peak = json.loads(HEARTBEAT.read_text()).get('peak', {})
        for key in PEAK:
            PEAK[key] = max(PEAK[key], secondary_peak.get(key, 0))
    for name in ['native/vite-cache', 'cache', 'tmp']:
        shutil.rmtree(OUT / name, ignore_errors=True)
    rows = table()
    remaining = (tree(rows) - {ROOT_PID}) | {pid for pid, row in rows.items() if row[1] in state['groups']}
    save(OUT / 'release-pending.json', {'status': status, 'epoch': time.time(), 'primaryPid': ROOT_PID,
         'secondaryPid': secondary.pid, 'remainingChildren': sorted(remaining), 'peak': PEAK,
         'wholeBytes': allocated(LEASE['roots']), 'commands': commands})
    assert not remaining
    assert time.time() <= LEASE['acceptedEpoch'] + 360
sys.exit(0 if status == 'PASS' else 1)
