"""PI12 finite native runner. Requires actual named predecessor release and explicit ACCEPT.

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
PEAK = {'processes': 0, 'rssBytes': 0, 'wholeBytes': LEASE['priorWholeMax'], 'newBytes': LEASE.get('priorNewMax', 0)}
# Publish each factual guard identity before any broad PID/filesystem census.
_owner_path = Path(LEASE['ownerReceiptPath'])
_owner = json.loads(_owner_path.read_text())
_identity = {'pid': os.getpid(), 'birth': subprocess.check_output(['ps', '-p', str(os.getpid()), '-o', 'lstart='], text=True).strip(),
             'executable': subprocess.check_output(['ps', '-p', str(os.getpid()), '-o', 'comm='], text=True).strip(), 'script': str(Path(__file__).resolve()), 'cwd': str(ROOT), 'outputRoot': str(OUT)}
if SECONDARY:
    _owner['guardControllers'] = [_identity]
else:
    _owner['controller'] = _identity
_owner['publishedAtEpoch'] = time.time()
_temporary = _owner_path.with_suffix('.next')
_temporary.write_text(json.dumps(_owner, indent=2) + '\n')
_temporary.replace(_owner_path)



def save(path, value):
    temporary = path.with_suffix('.next')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def table():
    probe = subprocess.Popen(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], stdout=subprocess.PIPE, text=True)
    rows, _ = probe.communicate()
    assert probe.returncode == 0
    return {int(v[0]): tuple(map(int, v[1:])) for line in rows.splitlines()
            if len(v := line.split()) == 4 and int(v[0]) != probe.pid}


def tree(rows):
    ids = {ROOT_PID}
    while True:
        expanded = ids | {pid for pid, (parent, _group, _rss) in rows.items() if parent in ids}
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
        for group in groups:
            if any(row[1] == group for row in rows.values()):
                try:
                    os.killpg(group, sig)
                except ProcessLookupError:
                    pass
        if sig == signal.SIGTERM:
            time.sleep(.4)


def global_sample(rows, ids):
    membership = json.loads(Path(LEASE['membershipPath']).read_text())
    # Distinct rounds remain charged until an exact literal FULL closes that offer.
    by_offer = {entry.get('offerId'): entry for entry in membership['entries'] if entry.get('offerId')}
    for path in Path(LEASE['ownerReceiptPath']).parent.glob('*.json'):
        latest = json.loads(path.read_text())
        history = latest.get('closedHistory', [])
        if isinstance(history, dict):
            history = history.values()
        for closed in history:
            if not isinstance(closed, dict):
                continue  # Unstructured history cannot prove release; retain any accepted offer.
            if closed.get('state') == 'FULL' and closed.get('fullPath'):
                prior = by_offer.get(closed.get('offerId'))
                if prior is None or prior.get('state') == 'FULL':
                    continue
                full = json.loads(Path(closed['fullPath']).read_text())
                if full.get('id') == closed['offerId'] and full.get('status') == 'FULL':
                    by_offer[closed['offerId']] = {**closed, 'state': 'FULL'}
                # Other receipt formats confer no release; any accepted offer stays charged.
        if latest.get('offerId'):
            by_offer[latest['offerId']] = latest
    entries = [entry for offer, entry in by_offer.items()
               if offer != LEASE['id'] and entry.get('kind') != 'ordinary'
               and entry.get('state') not in ['FULL', 'RETURNED', 'DECLINED', 'OFFERED']
               and offer not in LEASE.get('literalReleasedOffers', [])]
    known = {entry.get('offerId') for entry in entries}
    entries += [entry for entry in LEASE.get('additionalAcceptedCohorts', []) if entry.get('offerId') not in known
                and entry.get('offerId') not in LEASE.get('literalReleasedOffers', [])]
    assert sum(entry['kind'] == 'browser' for entry in entries) + 1 <= 2, 'Accepted browser workflow cap'
    assert len(entries) + 1 <= 4, 'Accepted global workflow cap'
    assert sum('source' in entry['kind'] for entry in entries) <= 3, 'Accepted source workflow cap'
    assert sum(entry['processCap'] for entry in entries) + 24 <= 96, 'Declared global process cap'
    assert sum(entry['rssCapBytes'] for entry in entries) + 4 * GIB <= 18 * GIB, 'Declared global RSS cap'
    global_ids = set(ids)
    for entry in entries:
        controllers = [entry.get('controller') or {}, *entry.get('guardControllers', [])]
        matched = {controller['pid'] for controller in controllers if controller.get('pid') in rows}
        while True:
            expanded = matched | {pid for pid, (parent, _group, _rss) in rows.items() if parent in matched}
            if expanded == matched:
                break
            matched = expanded
        global_ids |= matched & rows.keys()
    assert len(global_ids) + 1 <= 96, 'Actual observed global process cap'
    assert sum(rows[pid][2] * 1024 for pid in global_ids) <= 18 * GIB, 'Actual observed global RSS cap'
    return {'acceptedOffers': [entry.get('offerId') for entry in entries], 'observedPids': sorted(global_ids),
            'boundary': 'Fresh allocator membership plus exact accepted owner controller/guard receipts; unknown membership stays charged.'}


def sample():
    rows = table()
    ids = tree(rows)
    rss = sum(rows[pid][2] * 1024 for pid in ids)
    global_state = global_sample(rows, ids)
    root_sizes = {path: allocated([path]) for path in LEASE['roots']}
    whole = sum(root_sizes.values())
    growth = LEASE['includedPriorDebtBytes'] + sum(max(0, size - LEASE['baselineByRoot'].get(path, 0)) for path, size in root_sizes.items())
    PEAK['wholeBytes'] = max(PEAK['wholeBytes'], whole)
    PEAK['newBytes'] = max(PEAK['newBytes'], growth)
    PEAK['processes'] = max(PEAK['processes'], len(ids))
    PEAK['rssBytes'] = max(PEAK['rssBytes'], rss)
    space = os.statvfs(ROOT)
    free = space.f_bavail * space.f_frsize
    assert time.time() < LEASE['acceptedEpoch'] + LEASE['workSeconds'], 'Fixed work deadline'
    assert free >= 8 * GIB, 'Continuous disk floor'
    assert PEAK['wholeBytes'] <= 8 * GIB, 'Complete WHOLE cap'
    assert PEAK['newBytes'] <= LEASE.get('newCapBytes', 256 * 1024 ** 2), 'All-phase positive NEW cap'
    assert len(ids) + 1 <= 24, 'Process cap includes the transient measurement helper'
    assert rss <= 4 * GIB, 'Actual complete process-tree RSS cap'
    assert sum(p.stat().st_size for p in OUT.glob('*.log')) <= 2 * 1024 ** 2, 'Aggregate log cap'
    pngs = list(OUT.parent.glob('run*/native/*.png'))
    assert len(pngs) <= LEASE.get('pngCountCap', 40), 'All-attempt PNG count cap'
    assert sum(p.stat().st_size for p in pngs) <= LEASE.get('pngBytesCap', 8 * 1024 ** 2), 'All-attempt PNG byte cap'
    return {'epoch': time.time(), 'pid': os.getpid(), 'ownedPids': sorted(ids),
            'global': global_state, 'freeBytes': free, 'wholeBytes': whole, 'positiveNewBytes': growth, 'rssBytes': rss, 'peak': dict(PEAK)}


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

assert os.environ.get('PI12_NATIVE_ACCEPTED') == '1'
assert LEASE['id'].startswith(('PI12-OPS-APP-NATIVE', 'PI12-OPS-NATIVE'))
assert 0 < LEASE.get('newCapBytes', 256 * 1024 ** 2) <= 256 * 1024 ** 2
assert 0 < LEASE.get('pngCountCap', 40) <= 40
assert 0 < LEASE.get('pngBytesCap', 8 * 1024 ** 2) <= 8 * 1024 ** 2
assert LEASE['namedPredecessorReleased'] is True
assert LEASE['workSeconds'] == 300 and LEASE['fullSeconds'] == 360
assert not OUT.exists(), 'Private execution output must be fresh'
assert ROOT == Path('/Users/lhs/Desktop/mcr-worktrees/fe-ref-customize')
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == LEASE['head']
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip()
space = os.statvfs(ROOT)
free = space.f_bavail * space.f_frsize
assert free >= LEASE['freshStartBytes'] and free - LEASE.get('newCapBytes', 256 * 1024 ** 2) - LEASE['otherGrowthBytes'] >= LEASE['reservationFloorBytes']
assert LEASE['otherGrowthBytes'] >= 0 and LEASE['reservationFloorBytes'] >= 8 * GIB
assert allocated(LEASE['roots']) + LEASE.get('newCapBytes', 256 * 1024 ** 2) <= 8 * GIB
OUT.mkdir(parents=True, mode=0o700)
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
    env = dict(os.environ, **LEASE['environment'], TMPDIR=str(OUT), TMP=str(OUT), TEMP=str(OUT),
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
    for name in ['native/vite-cache-canonical', 'native/vite-cache-fe37', 'cache']:
        shutil.rmtree(OUT / name, ignore_errors=True)
    rows = table()
    remaining = (tree(rows) - {ROOT_PID}) | {pid for pid, row in rows.items() if row[1] in state['groups']}
    save(OUT / 'release-pending.json', {'status': status, 'epoch': time.time(), 'primaryPid': ROOT_PID,
         'secondaryPid': secondary.pid, 'remainingChildren': sorted(remaining), 'peak': PEAK,
         'wholeBytes': allocated(LEASE['roots']), 'commands': commands})
    assert not remaining
    assert time.time() <= LEASE['acceptedEpoch'] + 360
sys.exit(0 if status == 'PASS' else 1)
