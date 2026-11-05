"""Owner-local receipt discovery; no grants, queue, or peer mutations."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import time

OWNER = '01a1a5d9-332b-7822-a8f7-40fde6c81bb4'
BASE = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d')
DIRECTORY = BASE / 'owner-resource-receipts'
OWN = DIRECTORY / (OWNER + '.json')
_verified = {}


def python_script_command(command):
    arguments = command.split()
    if not arguments or not re.fullmatch(r'Python|python3(?:\.\d+)?', Path(arguments[0]).name): return False
    arguments = arguments[1:]
    while arguments and arguments[0] in ('-B', '-u', '-I', '-s', '-E'): arguments = arguments[1:]
    return bool(arguments and (arguments[0] == '-m' or not arguments[0].startswith('-') and arguments[0].endswith('.py')))


def verify_controller(claim, control, row):
    key = (claim['ownerThreadId'], claim.get('offerId'), epoch(claim.get('acceptedAtEpoch')),
           control['pid'], row['birth'], control.get('script'), control.get('executable'), control.get('cwd'))
    assert row['birth'] == ' '.join(control['birth'].split()), 'resource PID birth mismatch'
    unavailable = row['command'] in ('(Python)', '<defunct>')
    if unavailable:
        prior = _verified.get(key)
        assert row.get('status', '').startswith('Z') and prior and prior['parent'] == row['parent'], 'unavailable argv lacks independently verified exact terminal identity'
        return
    assert script_matches(control, row), 'resource controller identity mismatch: ' + json.dumps(dict(owner=claim['ownerThreadId'], pid=control['pid'], birth=row['birth'], expectedScript=control['script'], observedExecutable=row['command'].split()[0], observedCommandSHA256=hashlib.sha256(row['command'].encode()).hexdigest(), commandLength=len(row['command'])))
    actual_comm = None
    if control.get('executable'):
        actual_comm = subprocess.check_output(['ps', '-p', str(control['pid']), '-o', 'comm='], text=True).strip()
        assert actual_comm == control['executable'], 'resource actual executable mismatch: ' + json.dumps(dict(owner=claim['ownerThreadId'], offer=claim.get('offerId'), pid=control['pid'], birth=row['birth'], expectedComm=control['executable'], actualComm=actual_comm, expectedScript=control.get('script')))
    _verified[key] = dict(parent=row['parent'], actualComm=actual_comm)


def script_matches(control, row):
    script = control.get('script')
    if not isinstance(script, str):
        return False
    arguments = shlex.split(row['command'])
    expected = Path(script).resolve()
    if any(Path(arg).is_absolute() and Path(arg).resolve() == expected for arg in arguments[1:]):
        return True
    # Verify actual process cwd, never a basename-only or advertised-cwd exemption.
    result = subprocess.run(['lsof', '-a', '-p', str(control['pid']), '-d', 'cwd', '-Fn'], text=True, capture_output=True)
    directories = [line[1:] for line in result.stdout.splitlines() if line.startswith('n')]
    return len(directories) == 1 and any(not arg.startswith('-') and not Path(arg).is_absolute()
        and (Path(directories[0]) / arg).resolve() == expected for arg in arguments[1:])


def processes():
    rows = {}
    raw = subprocess.check_output(['ps', '-ww', '-axo', 'pid=,ppid=,lstart=,rss=,stat=,command='], text=True)
    for line in raw.splitlines():
        f = line.strip().split(None, 9)
        if len(f) == 10:
            rows[int(f[0])] = dict(parent=int(f[1]), birth=' '.join(f[2:7]), rss=int(f[7]) * 1024, status=f[8], command=f[9])
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


def sample(proposed=None):
    policy = json.loads((BASE / 'resource-receipt-interface.json').read_text())['limits']
    legacy = json.loads((BASE / 'resource-membership-current.json').read_text())['entries']
    files = list(DIRECTORY.glob('*.json'))
    assert len(files) <= 128, 'unbounded resource receipt directory'
    records = {}
    for file in files:
        assert file.stat().st_size <= 65536, 'oversized resource receipt'
        record = json.loads(file.read_text())
        assert file.stem == record['ownerThreadId']
        assert record['state'] in ('ACCEPTED', 'FULL', 'DECLINED')
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
    assert len(claims) <= policy['global'], 'global workflow reservations'
    assert sum(c['kind'] in ('source', 'copied-native') for c in claims.values()) <= policy['source']
    assert sum(c['kind'] == 'browser' for c in claims.values()) <= policy['browser']
    assert sum(c['kind'] == 'target' for c in claims.values()) <= policy['warmRustTarget']
    assert sum(c['processCap'] for c in claims.values()) <= policy['sumPIDCaps']
    assert sum(c['rssCapBytes'] for c in claims.values()) <= policy['sumRSSBytes']
    rows, covered = processes(), set()
    for key in list(_verified):
        if key[3] not in rows or rows[key[3]]['birth'] != key[4]: del _verified[key]
    for claim in claims.values():
        roots = set()
        for control in [claim.get('controller')] + claim.get('guardControllers', []):
            if not control:
                continue
            pid = control['pid']
            if pid in rows:
                verify_controller(claim, control, rows[pid])
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
            if control.get('script'):
                assert script_matches(control, rows[pid]), 'ordinary script identity mismatch: ' + json.dumps(dict(
                    owner=record.get('ownerThreadId'), offer=record.get('offerId'), pid=pid, birth=rows[pid]['birth'],
                    expectedScript=control['script'], observedExecutable=rows[pid]['command'].split()[0],
                    status=rows[pid].get('status'), commandSHA256=hashlib.sha256(rows[pid]['command'].encode()).hexdigest()))
            tree = {pid}
            previous = None
            while previous != tree:
                previous = set(tree)
                tree.update(child for child, row in rows.items() if row['parent'] in tree)
            for child in tree:
                command = rows[child]['command']
                heavy = bool(re.search(r'\b(?:node|cargo|rustc)(?:\s|$)', command)) or ('Google Chrome --' in command and '--headless' in command)
                script_child = child != pid and python_script_command(command)
                assert not heavy and not script_child, 'ordinary role cannot cover runtime payload'
            ordinary_pids.update(tree)
            # Count the observed shell supervising a factual ordinary publisher.
            # Its other children are NOT covered by this metadata observation.
            parent = rows[pid]['parent']
            if parent in rows and rows[parent]['command'].startswith(('/bin/zsh ', '/bin/bash ', '/bin/sh ')):
                ordinary_pids.add(parent)
    covered.update(ordinary_pids)
    unknown = []
    for pid, row in rows.items():
        command = row['command']
        shell = command.startswith(('/bin/zsh', '/bin/bash', '/bin/sh'))
        scoped = any(s in command for s in ('/mcr-worktrees/', '/mcr-build/', '/private/tmp/mcr-', '/tmp/mcr-', '/T/mcr-', '/mcr-evidence/'))
        # Inline Python file/metadata preparation is not an execution cohort.
        # Actual script controllers and all scoped Node/Rust/browser roots are.
        browser = 'Google Chrome --' in command and '--headless' in command
        runtime = bool(re.search(r'\b(?:node|cargo|rustc)(?:\s|$)', command)) or python_script_command(command) or browser
        shell_body = command.split(None, 2)[-1] if shell else ''
        # Inline Python metadata editing was never a runtime cohort. A .py name
        # inside its heredoc is data; actual script/module children remain strict.
        opaque_shell = shell and scoped and (bool(re.search(r'\b(?:node|cargo|rustc)\b|\.mjs\b', command)) or python_script_command(shell_body))
        if (scoped or browser) and (runtime or opaque_shell) and pid not in covered:
            unknown.append(dict(pid=pid, parentPid=row['parent'], birth=row['birth'], executable=command.split()[0], scriptCandidates=re.findall(r'/[^\s]+\.py', command), commandSHA256=hashlib.sha256(command.encode()).hexdigest()))
    assert not unknown, 'unresolved workflow roots: ' + json.dumps(unknown)
    assert len(covered) <= policy['sumPIDCaps']
    rss = sum(rows[pid]['rss'] for pid in covered)
    assert rss <= policy['sumRSSBytes']
    other = policy['ordinaryFuturePositiveUpperBytes'] + sum(c['remainingNewUpperBytes'] for owner, c in claims.items() if owner != OWNER)
    assert int(os.environ['MCR_VENU_OTHER_NEW']) >= other, 'OTHER does not cover ordinary and accepted peer growth'
    return dict(observedAtEpoch=time.time(), psSnapshotSHA256=hashlib.sha256(json.dumps(rows, sort_keys=True).encode()).hexdigest(),
                owners=sorted(claims), ordinaryActualPids=sorted(ordinary_pids), reservedProcesses=sum(c['processCap'] for c in claims.values()),
                reservedRSSBytes=sum(c['rssCapBytes'] for c in claims.values()), actualPids=sorted(covered), actualRSSBytes=rss, otherNewUpperBytes=other)
