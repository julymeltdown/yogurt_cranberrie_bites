"""Owner-local receipt discovery; no grants, queue, or peer mutations."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

OWNER = '01a1a26e-1de2-7b41-a294-c9a79cfded9b'
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
    for claim in claims.values():
        roots = set()
        for control in [claim.get('controller')] + claim.get('guardControllers', []):
            if not control:
                continue
            pid = control['pid']
            if pid in rows:
                assert rows[pid]['birth'] == ' '.join(control['birth'].split()), 'resource PID birth mismatch'
                assert control['script'] in rows[pid]['command'], 'resource controller identity mismatch: ' + json.dumps(dict(owner=claim['ownerThreadId'], pid=pid, birth=rows[pid]['birth'], expectedScript=control['script'], observedExecutable=rows[pid]['command'].split()[0], observedCommandSHA256=hashlib.sha256(rows[pid]['command'].encode()).hexdigest(), commandLength=len(rows[pid]['command'])))
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
                assert control['script'] in rows[pid]['command'], 'ordinary script identity mismatch'
            tree = {pid}
            previous = None
            while previous != tree:
                previous = set(tree)
                tree.update(child for child, row in rows.items() if row['parent'] in tree)
            for child in tree:
                command = rows[child]['command']
                heavy = bool(re.search(r'\b(?:node|cargo|rustc)(?:\s|$)', command)) or ('Google Chrome --' in command and '--headless' in command)
                script_child = child != pid and bool(re.search(r'\b(?:Python|python3(?:\.\d+)?)\s+(?:-B\s+)?/\S+\.py', command))
                assert not heavy and not script_child, 'ordinary role cannot cover runtime payload'
            ordinary_pids.update(tree)
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
    assert int(os.environ['MCR_PI_OTHER_NEW']) >= other, 'OTHER does not cover ordinary and accepted peer growth'
    return dict(observedAtEpoch=time.time(), psSnapshotSHA256=hashlib.sha256(json.dumps(rows, sort_keys=True).encode()).hexdigest(),
                owners=sorted(claims), ordinaryActualPids=sorted(ordinary_pids), reservedProcesses=sum(c['processCap'] for c in claims.values()),
                reservedRSSBytes=sum(c['rssCapBytes'] for c in claims.values()), actualPids=sorted(covered), actualRSSBytes=rss, otherNewUpperBytes=other)
