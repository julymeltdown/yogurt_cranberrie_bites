"""Owner-local receipt discovery; no grants, queue, or peer mutations."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

OWNER = '01a1a213-61d6-7a20-a8e7-476649312f8c'
BASE = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d')
DIRECTORY = BASE / 'owner-resource-receipts'
OWN = DIRECTORY / (OWNER + '.json')


def processes():
    rows = {}
    raw = subprocess.check_output(['ps', '-ww', '-axo', 'pid=,ppid=,lstart=,rss=,command='], text=True)
    for line in raw.splitlines():
        f = line.strip().split(None, 8)
        if len(f) == 9:
            rows[int(f[0])] = dict(parent=int(f[1]), birth=' '.join(f[2:7]), rss=int(f[7]) * 1024, command=f[8])
    return rows


def identity(pid, script, output=None):
    row = processes()[pid]
    executable = subprocess.check_output(['ps', '-p', str(pid), '-o', 'comm='], text=True).strip()
    result = dict(pid=pid, birth=row['birth'], executable=executable, script=str(script), cwd=os.getcwd())
    if output is not None:
        result['outputRoot'] = str(output)
    return result


def publish(value):
    value['publishedAtEpoch'] = time.time()
    temporary = OWN.with_name('.' + OWN.name + '.' + str(os.getpid()) + '.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    os.replace(temporary, OWN)


def publish_census(path, value):
    temporary = path.with_name('.' + path.name + '.' + str(os.getpid()) + '.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    os.replace(temporary, path)


def epoch(value):
    try:
        return float(value)
    except (TypeError, ValueError):
        try:
            return datetime.datetime.fromisoformat(value.replace('Z', '+00:00')).timestamp()
        except (AttributeError, ValueError):
            return None


def _sample(proposed, receipt_bytes):
    policy = json.loads((BASE / 'resource-receipt-interface.json').read_text())['limits']
    legacy = json.loads((BASE / 'resource-membership-current.json').read_text())['entries']
    records = {}
    for name, data in receipt_bytes.items():
        file = DIRECTORY / name
        record = json.loads(data)
        if file.stem == '01a19e5c-e9c6-7973-a53b-a6d19141620f' and set(record) == {'ordinaryObservation'}:
            # Exact observed Main-only ordinary format, resolved factually before
            # admission. Never relabel absence/completion as execution FULL.
            value = record['ordinaryObservation']
            assert value['role'] == 'ordinary Main management authoring and Git only'
            assert value['booking']['id'] == 'MAIN-VENUE-AMENDMENT-DOC64-0111'
            assert value['ordinaryCommandsComplete'] is True and value['commandExit'] == 0
            assert value['processChildren'] == 'All synchronous Git/metadata subprocesses exited; no application or background process launched.'
            record = {'ownerThreadId': file.stem, 'offerId': value['booking']['id'], 'state': 'ACCEPTED',
                'acceptedAtEpoch': epoch(value['observedAt']), 'kind': 'ordinary', 'processCap': None, 'rssCapBytes': None,
                'remainingNewUpperBytes': value['booking']['additionalUpperBytes'],
                'controller': {'pid': value['actualpid'], 'birth': value['birth'], 'executable': value['executable'],
                    'script': None, 'cwd': value['cwd']}, 'guardControllers': [],
                'ordinaryObservation': value, 'identityBasis': 'Exact known ordinaryObservation format; ordinary reservation retained, not inferred FULL'}
        assert file.stem == record['ownerThreadId']
        assert record['state'] in ('ACCEPTED', 'FULL', 'DECLINED', 'UNREADY_YIELD')
        if record['state'] == 'UNREADY_YIELD':
            assert record.get('acceptedAtEpoch') is None and record.get('controller') is None and not record.get('guardControllers')
            assert record.get('remainingNewUpperBytes') == 0
            previous = record.get('previousActualExecution')
            if previous:
                assert previous['state'] == 'FULL' and previous.get('actualFullEpoch') and previous.get('resourcesReleased') is True
        records[record['ownerThreadId']] = record
    if proposed is not None:
        assert proposed['ownerThreadId'] == OWNER
        assert records.get(OWNER, {}).get('state') != 'ACCEPTED', 'cannot hide an accepted own workflow'
        records[OWNER] = proposed
    claims = {owner: record for owner, record in records.items() if record['state'] == 'ACCEPTED' and record.get('kind') != 'ordinary'}
    for old in legacy:
        if old.get('kind') == 'ordinary' or old['state'] == 'FULL' or old['state'].startswith('LITERAL_FULL') or 'DECLIN' in old['state']:
            continue
        owner = old.get('ownerThreadId', old.get('owner'))
        record = records.get(owner)
        resolved = False
        if record:
            a, b = epoch(record.get('acceptedAtEpoch')), epoch(old.get('acceptedAtEpoch', old.get('actualAccept')))
            resolved = record.get('offerId') == old.get('offerId', old.get('cohort')) or record.get('offerId') in old.get('knownRootMatchers', []) or (a is not None and b is not None and a >= b)
        if not resolved:
            assert owner not in claims, 'ambiguous simultaneous legacy/current claims: ' + owner
            rss = old['rssCapBytes'] if 'rssCapBytes' in old else int(old['rssCapGiB'] * 1024**3)
            new = old['remainingNewUpperBytes'] if 'remainingNewUpperBytes' in old else old['remainingNewConservativeUpperBytes']
            claims[owner] = dict(old, ownerThreadId=owner, offerId=old.get('offerId', old.get('cohort')), kind=old['kind'],
                                 processCap=old['processCap'], rssCapBytes=rss,
                                 remainingNewUpperBytes=new, legacyMatchers=old.get('knownRootMatchers', []))
    assert len(claims) <= policy['global'], 'global workflow reservations: '+json.dumps([{'owner': owner, 'offer': c.get('offerId'), 'kind': c.get('kind')} for owner,c in claims.items()])
    assert sum(c['kind'] in ('source', 'copied-native') for c in claims.values()) <= policy['source'], 'aggregate reservation cap: '+json.dumps({'claims': [{k: c.get(k) for k in ['ownerThreadId', 'offerId', 'kind', 'processCap', 'rssCapBytes']} for c in claims.values()], 'limits': policy})
    assert sum(c['kind'] == 'browser' for c in claims.values()) <= policy['browser'], 'aggregate reservation cap: '+json.dumps({'claims': [{k: c.get(k) for k in ['ownerThreadId', 'offerId', 'kind', 'processCap', 'rssCapBytes']} for c in claims.values()], 'limits': policy})
    assert sum(c['kind'] == 'target' for c in claims.values()) <= policy['warmRustTarget'], 'aggregate reservation cap: '+json.dumps({'claims': [{k: c.get(k) for k in ['ownerThreadId', 'offerId', 'kind', 'processCap', 'rssCapBytes']} for c in claims.values()], 'limits': policy})
    assert sum(c['processCap'] for c in claims.values()) <= policy['sumPIDCaps'], 'aggregate reservation cap: '+json.dumps({'claims': [{k: c.get(k) for k in ['ownerThreadId', 'offerId', 'kind', 'processCap', 'rssCapBytes']} for c in claims.values()], 'limits': policy})
    assert sum(c['rssCapBytes'] for c in claims.values()) <= policy['sumRSSBytes'], 'aggregate reservation cap: '+json.dumps({'claims': [{k: c.get(k) for k in ['ownerThreadId', 'offerId', 'kind', 'processCap', 'rssCapBytes']} for c in claims.values()], 'limits': policy})
    rows, covered = processes(), set()
    dead_controllers = []
    for claim in claims.values():
        roots = set()
        for control in [claim.get('controller')] + claim.get('guardControllers', []):
            if not control:
                continue
            pid = control['pid']
            if pid in rows:
                assert rows[pid]['birth'] == ' '.join(control['birth'].split()), 'resource PID birth mismatch'
                state = subprocess.check_output(['ps', '-p', str(pid), '-o', 'stat='], text=True).strip()
                if state.startswith('Z'):
                    # A factual same-birth zombie has no live argv/executable.
                    # Retain its accepted full caps and actual tree; never infer
                    # FULL or waive independently unknown runtime descendants.
                    roots.add(pid)
                    dead_controllers.append(dict(owner=claim['ownerThreadId'], pid=pid, birth=rows[pid]['birth'], state=state))
                    continue
                assert control['script'] in rows[pid]['command'], 'resource controller identity mismatch: ' + json.dumps(dict(owner=claim['ownerThreadId'], pid=pid, birth=rows[pid]['birth'], expectedScript=control['script'], observedExecutable=rows[pid]['command'].split()[0], observedCommandSHA256=hashlib.sha256(rows[pid]['command'].encode()).hexdigest(), commandLength=len(rows[pid]['command'])))
                executable = subprocess.check_output(['ps', '-p', str(pid), '-o', 'comm='], text=True).strip()
                assert executable == control['executable'], 'actual controller executable mismatch: ' + claim['ownerThreadId']
                roots.add(pid)
        for matcher in claim.get('legacyMatchers', []):
            roots.update(pid for pid, row in rows.items() if matcher and matcher in row['command'])
        tree = set(roots)
        previous = None
        while previous != tree:
            previous = set(tree)
            tree.update(pid for pid, row in rows.items() if row['parent'] in tree)
        assert len(tree) <= claim['processCap'], 'actual peer workflow PID cap'
        assert sum(rows[pid]['rss'] for pid in tree) <= claim['rssCapBytes'], 'actual peer workflow RSS cap'
        covered.update(tree)
    # An ordinary publisher may publish FULL while its final atomic write still
    # runs. Exact factual identity remains usable; this never reserves a workflow.
    ordinary_pids = set()
    for record in records.values():
        ordinary = []
        if record.get('kind') == 'ordinary' and record.get('controller'):
            ordinary.append(record['controller'])
        observation = record.get('ordinaryObservation')
        if observation:
            ordinary.append(dict(pid=observation.get('actualpid', observation.get('pid')),
                                 birth=observation['birth'], script=None))
        for control in ordinary:
            pid = control['pid']
            if pid not in rows:
                continue
            if rows[pid]['birth'] != ' '.join(control['birth'].split()):
                continue  # A stale observation cannot identify a reused PID.
            if control.get('script') and control['script'] not in rows[pid]['command']:
                if record['state'] == 'FULL':
                    # An ended ordinary observation cannot identify the current
                    # command. Do not cover it: any real scoped runtime below
                    # still fails the independent unknown-root classification.
                    continue
                raise AssertionError('ordinary script identity mismatch: ' + json.dumps({'owner': record['ownerThreadId'], 'pid': pid, 'birth': rows[pid]['birth'], 'expectedScript': control['script'], 'observedCommandSHA256': hashlib.sha256(rows[pid]['command'].encode()).hexdigest()}))
            tree = {pid}
            previous = None
            while previous != tree:
                previous = set(tree)
                tree.update(child for child, row in rows.items() if row['parent'] in tree)
            # Strict accepted workflow identities/caps were checked first.
            # Classify only the remaining nodes as ordinary; a live ordinary
            # observation may be an ancestor of a separately allocated runtime.
            for child in tree - covered:
                command = rows[child]['command']
                heavy = bool(re.search(r'\b(?:node|cargo|rustc)(?:\s|$)', command)) or ('Google Chrome --' in command and '--headless' in command)
                script_child = child != pid and bool(re.search(r'\b(?:Python|python3(?:\.\d+)?)\s+(?:-B\s+)?/\S+\.py', command))
                assert not heavy and not script_child, 'ordinary role cannot cover runtime payload: '+json.dumps(dict(owner=record['ownerThreadId'], state=record['state'], rootPid=pid, childPid=child, birth=rows[child]['birth'], commandSHA256=hashlib.sha256(command.encode()).hexdigest(), declaredScript=control.get('script')))
            ordinary_pids.update(tree - covered)
    ordinary_extra = ordinary_pids - covered
    reserved_processes = sum(c['processCap'] for c in claims.values()) + len(ordinary_extra)
    reserved_rss = sum(c['rssCapBytes'] for c in claims.values()) + sum(rows[pid]['rss'] for pid in ordinary_extra)
    assert reserved_processes <= policy['sumPIDCaps'], 'reserved workflow caps plus ordinary actual PIDs: '+str(reserved_processes)
    assert reserved_rss <= policy['sumRSSBytes'], 'reserved workflow caps plus ordinary actual RSS: '+json.dumps({'bytes': reserved_rss, 'cap': policy['sumRSSBytes'], 'ordinaryPids': sorted(ordinary_extra)})
    covered.update(ordinary_pids)
    unknown = []
    for pid, row in rows.items():
        command = row['command']
        if command.startswith(('/bin/zsh', '/bin/bash', '/bin/sh')):
            continue
        scoped = any(s in command for s in ('/mcr-worktrees/', '/mcr-build/', '/private/tmp/mcr-', '/tmp/mcr-', '/T/mcr-', '/mcr-evidence/'))
        # Inline Python file/metadata preparation is not an execution cohort.
        # Actual script controllers and all scoped Node/Rust/browser roots are.
        browser = 'Google Chrome --' in command and '--headless' in command
        runtime = bool(re.search(r'\b(?:node|cargo|rustc)(?:\s|$)', command)) or bool(re.search(r'\b(?:Python|python3(?:\.\d+)?)\s+(?:-B\s+)?/\S+\.py', command)) or browser
        if (scoped or browser) and runtime and pid not in covered:
            unknown.append(dict(pid=pid, parentPid=row['parent'], birth=row['birth'], executable=command.split()[0], scriptCandidates=re.findall(r'/[^\s]+\.py', command), commandSHA256=hashlib.sha256(command.encode()).hexdigest()))
    assert not unknown, 'unresolved workflow roots: ' + json.dumps(unknown)
    assert len(covered) <= policy['sumPIDCaps']
    rss = sum(rows[pid]['rss'] for pid in covered)
    assert rss <= policy['sumRSSBytes']
    other = policy['ordinaryFuturePositiveUpperBytes'] + sum(c['remainingNewUpperBytes'] for owner, c in claims.items() if owner != OWNER)
    assert int(os.environ['MCR_PI_PRIVATE_OTHER_NEW']) >= other, 'OTHER does not cover ordinary and accepted peer growth'
    return dict(observedAtEpoch=time.time(), psSnapshotSHA256=hashlib.sha256(json.dumps(rows, sort_keys=True).encode()).hexdigest(),
                owners=sorted(claims), deadControllerPids=dead_controllers, ordinaryActualPids=sorted(ordinary_pids), reservedProcesses=reserved_processes,
                reservedRSSBytes=reserved_rss, actualPids=sorted(covered), actualRSSBytes=rss, otherNewUpperBytes=other)


def receipt_snapshot():
    files = sorted(DIRECTORY.glob('*.json'))
    assert len(files) <= 128, 'unbounded resource receipt directory'
    result = {}
    for file in files:
        data = file.read_bytes()
        assert len(data) <= 65536, 'oversized resource receipt'
        result[file.name] = data
    return result


def sample(proposed=None):
    # Atomic records and PS are separate observations. Retry only after actual
    # bytes change, never guess a future script or accept a stable mismatch.
    transitions = []
    before = receipt_snapshot()
    for attempt in range(3):
        try:
            result = _sample(proposed, before)
        except AssertionError as error:
            retryable = any(label in str(error) for label in (
                'resource controller identity mismatch', 'actual controller executable mismatch',
                'resource PID birth mismatch', 'ordinary script identity mismatch',
                'unresolved workflow roots'))
            if not retryable or attempt == 2:
                raise
            after = receipt_snapshot()
            if before == after:
                # One bounded fresh read catches a publisher finishing the
                # factual same-PID exec publication; unchanged bytes fail closed.
                time.sleep(.025)
                after = receipt_snapshot()
            if before == after:
                raise
            transitions.append({'reason': str(error), 'changedOwnerFiles': sorted(
                name for name in before.keys() | after.keys() if before.get(name) != after.get(name))})
            before = after
            continue
        after = receipt_snapshot()
        if before == after:
            result['receiptSnapshotAttempts'] = attempt + 1
            result['observedRecordTransitions'] = transitions
            return result
        transitions.append({'reason': 'Receipt bytes changed across PS census',
            'changedOwnerFiles': sorted(name for name in before.keys() | after.keys() if before.get(name) != after.get(name))})
        before = after
    raise AssertionError('Resource metadata did not stabilize within three factual snapshots')
