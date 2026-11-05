"""Independent finite source guardian. Pin this file before launching any test child."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import sys
import time

import re

# Adapted from 132a81ade88368d361872a003f1e63152fff2f22 FE38 guardian.
# Positive terminal prefixes preserve NO_FULL_RECEIPT_YET membership.
MEMBERSHIP = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/resource-membership-current.json')
RECEIPTS = MEMBERSHIP.parent / 'owner-resource-receipts'
OWN_ID = '01a1a21e-ab80-7012-b9fa-cdb69eed097b'
GIT_PUBLISHER = Path('/private/tmp/mcr-publisher-412e-capacity-composition-delivery.py')
GIT_PUBLISHER_SHA256 = '8bd689d14ffd14ef04b2209cf890eb113a52a261c350e9cc2809d60971293622'


def terminal(state):
    return state in ('FULL', 'DECLINED') or state.startswith(('LITERAL_FULL_RECEIVED', 'ACTUAL_FULL', 'ACTUAL_PARTIAL_FULL', 'ACTUAL_PASS_FULL', 'ACTUAL_FAIL_FULL', 'ACTUAL_FAILED_FULL', 'LITERAL_DECLINE', 'UNACCEPTED_DECLINE', 'UNACCEPTED_LITERAL_DECLINE'))


def accepted_epoch(entry, day):
    from datetime import datetime
    for value in (entry.get('acceptedAtEpoch'), entry.get('actualAccept'), entry.get('state')):
        if isinstance(value, (int, float)):
            return float(value)
        match = re.search(r'\d{10}(?:\.\d+)?', str(value))
        if match:
            return float(match.group())
        match = re.search(r'\d\d:\d\d:\d\d(?:\.\d+)?', str(value))
        if match:
            return datetime.fromisoformat(day + 'T' + match.group() + '+00:00').timestamp()
    return None


def membership_entries(metadata, rows=None):
    entries = list(metadata['entries'])
    paths = sorted(RECEIPTS.glob('*.json'))
    assert len(paths) <= 256, 'bounded owner receipt directory exceeded'
    pins = []
    ordinary = []
    for path in paths:
        assert re.fullmatch(r'[0-9a-f-]{36}\.json', path.name) and not path.is_symlink()
        data = path.read_bytes()
        assert len(data) <= 65536, ('oversized owner receipt', str(path))
        record = json.loads(data)
        pins.append({'path': str(path), 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
        histories = record.get('closedReceipts', record.get('closedOffers', record.get('closedHistory', [])))
        assert isinstance(histories, list), ('closed receipt history shape', str(path))
        for closed in histories:
            if isinstance(closed, str):
                # The sole-owner interface also permits literal historical FULL
                # paths. The owner file supplies identity; the referenced record
                # must itself prove terminal cleanup and its exact offer.
                literal = Path(closed)
                assert literal.is_absolute() and literal.is_file() and not literal.is_symlink(), ('historical FULL path', closed)
                literal_bytes = literal.read_bytes()
                assert len(literal_bytes) <= 65536, ('oversized historical FULL', closed)
                payload = json.loads(literal_bytes)
                assert payload.get('ownerThreadId', path.stem) == path.stem, ('historical FULL owner', closed)
                assert terminal(payload.get('state', payload.get('status', ''))), ('historical FULL state', closed)
                assert payload.get('remainingOwned') == [] and payload.get('dependencyLinkAbsent') is True and payload.get('browserServerTargetResources') == 0 and (payload.get('privateTempAbsent') is True or payload.get('privateTemporaryAbsent') is True), ('historical FULL cleanup', closed)
                offer = payload.get('offer')
                assert isinstance(offer, dict) and isinstance(offer.get('id'), str) and offer['id'], ('historical FULL offer', closed)
                from datetime import datetime
                historical_accepted = datetime.fromisoformat(offer['acceptedUTC']).timestamp()
                closed = dict(ownerThreadId=path.stem, offerId=offer['id'], fullPath=str(literal), resourcesReleased=True,
                    acceptedAtEpoch=historical_accepted, fullSHA256=hashlib.sha256(literal_bytes).hexdigest())
            assert isinstance(closed, dict), ('closed receipt entry shape', str(path), type(closed).__name__)
            full_path = closed.get('fullPath')
            released = closed.get('resourcesReleased') is True
            if not released or not full_path:
                continue
            full = Path(full_path)
            assert full.is_file(), ('missing historical literal FULL', str(path), full_path)
            digest = closed.get('fullSHA', closed.get('fullSha256', closed.get('fullSHA256')))
            if digest:
                assert hashlib.sha256(full.read_bytes()).hexdigest() == digest, ('historical FULL pin', full_path)
            closed_owner = closed.get('ownerThreadId', path.stem)
            assert closed_owner == path.stem
            closed_offer = closed.get('offerId')
            closed_epoch = closed.get('acceptedAtEpoch')
            def same_closed(entry):
                if entry.get('owner', entry.get('ownerThreadId')) != closed_owner:
                    return False
                if closed_offer and entry.get('offerId', entry.get('cohort')) == closed_offer:
                    return True
                previous = accepted_epoch(entry, metadata['asOfUTC'][:10])
                return isinstance(closed_epoch, (int, float)) and previous is not None and abs(previous - closed_epoch) < 1
            entries = [entry for entry in entries if not same_closed(entry)]
        observation = record.get('ordinaryObservation')
        if observation is not None:
            assert isinstance(observation, dict)
            ordinary.append({'ownerThreadId': path.stem, 'state': 'ORDINARY_OBSERVATION', 'ordinaryObservation': observation})
        if 'state' not in record:
            assert observation is not None and set(record) <= {'ordinaryObservation', 'ownerThreadId', 'closedReceipts', 'closedOffers'}, ('unknown observation-only shape', str(path))
            assert record.get('ownerThreadId', path.stem) == path.stem
            continue
        assert record['ownerThreadId'] + '.json' == path.name
        raw_state = record['state']
        if raw_state.startswith('ACCEPTED_'):
            record = {**record, 'state': 'ACCEPTED', 'originalState': raw_state}
        assert record['state'] in ('ACCEPTED', 'FULL', 'DECLINED'), ('unsupported owner state', str(path), raw_state)
        owner = record['ownerThreadId']
        is_ordinary = record.get('kind') == 'ordinary'
        if is_ordinary:
            remaining = record.get('remainingNewUpperBytes')
            assert remaining is None or isinstance(remaining, (int, float)) and remaining >= 0, ('ordinary remaining shape', str(path))
        # Remove only a positively identified copy of this same accepted round.
        # An old FULL file cannot erase a different/new legacy acceptance.
        epoch = record.get('acceptedAtEpoch')
        def same_round(entry):
            if entry.get('owner', entry.get('ownerThreadId')) != owner:
                return False
            if record.get('offerId') and entry.get('offerId') == record['offerId']:
                return True
            previous = accepted_epoch(entry, metadata['asOfUTC'][:10])
            return epoch is not None and previous is not None and abs(previous - epoch) < 1
        entries = [entry for entry in entries if not same_round(entry)]
        if is_ordinary:
            ordinary.append(record)
            continue
        if record['state'] != 'ACCEPTED':
            if record['state'] == 'FULL':
                full_path = Path(record['receiptPaths']['full'])
                assert full_path.is_file(), ('missing literal FULL receipt', owner)
                identity = record.get('controller') or {}
                pid, script = identity.get('pid'), identity.get('script')
                if rows and pid in rows and script and rows[pid]['birth'] == ' '.join(identity.get('birth', '').split()) and script in rows[pid]['command'].split():
                    raw_full = full_path.read_bytes()
                    assert len(raw_full) <= 65536, ('oversized live finisher FULL', str(full_path))
                    literal = json.loads(raw_full)
                    literal_offer = literal.get('offerId', literal.get('allocation', literal.get('offer')))
                    if isinstance(literal_offer, dict):
                        literal_offer = literal_offer.get('id')
                    resources = literal.get('resources')
                    zero_resources = isinstance(resources, dict) and all(resources.get(key) == 0 for key in ('children', 'link', 'temporary', 'reservations'))
                    explicit_cleanup = literal.get('ownedProcessesRemaining') == [] and literal.get('dependencyLinkAbsent') is True and literal.get('scratchCacheAbsent') is True
                    if literal_offer == record.get('offerId') and terminal(literal.get('state', '')) and (zero_resources or explicit_cleanup):
                        ordinary.append(dict(ownerThreadId=owner, state='FULL', completedControllerFinisher=identity,
                            fullProofPath=str(full_path), fullProofSHA256=hashlib.sha256(raw_full).hexdigest()))
            continue
        assert Path(record['receiptPaths']['accept']).is_file(), ('missing ACCEPT receipt', owner)
        assert record['processCap'] > 0 and record['rssCapBytes'] > 0 and record['remainingNewUpperBytes'] >= 0
        entries.append({'owner': owner, 'cohort': record['offerId'], 'kind': record['kind'],
            'state': 'ACCEPTED_OWNER_RESOURCE_RECEIPT', 'actualAccept': record['acceptedAtEpoch'],
            'processCap': record['processCap'], 'rssCapGiB': record['rssCapBytes'] / 1024**3,
            'remainingNewConservativeUpperBytes': record['remainingNewUpperBytes'],
            'identities': [record['controller'], *record.get('guardControllers', [])],
            'knownRootMatchers': record.get('knownRootMatchers', [])})
    return entries, pins, ordinary


def publish_accepted(config, supervisor, rows, output_root, accept_path, scratch):
    def identity(pid, script):
        row = rows[pid]
        return {'pid': pid, 'birth': row['birth'], 'executable': row['command'].split(' ', 1)[0],
                'script': str(script), 'cwd': str(Path.cwd()), 'outputRoot': str(output_root), 'actualArgs': row['command']}
    ids = descendants(rows, {supervisor})
    record = {'ownerThreadId': OWN_ID, 'offerId': config['offerId'], 'state': 'ACCEPTED',
        'publishedAtEpoch': time.time(), 'acceptedAtEpoch': config['acceptedAtEpoch'],
        'workDeadlineEpoch': config['workDeadlineEpoch'], 'fullDeadlineEpoch': config['fullDeadlineEpoch'],
        'kind': config['mode'], 'processCap': config['offer']['processes'],
        'rssCapBytes': config['offer']['rssBytes'], 'remainingNewUpperBytes': config['offer']['newBytes'],
        'controller': identity(supervisor, output_root / 'lease.py'),
        'guardControllers': [identity(pid, output_root / 'guardian.py') for pid in ids
            if pid != supervisor and '/guardian.py ' in rows[pid]['command']],
        'knownRootMatchers': [str(scratch)], 'receiptPaths': {'accept': str(accept_path), 'full': None}}
    path = RECEIPTS / (OWN_ID + '.json')
    previous = json.loads(path.read_text()) if path.exists() else {}
    assert previous.get('state') != 'ACCEPTED' or previous.get('offerId') == config['offerId'], 'different own round remains accepted'
    record['closedReceipts'] = previous.get('closedReceipts', [])
    staging = path.with_name(path.name + '.' + str(os.getpid()) + '.tmp')
    assert not staging.exists()
    staging.write_text(json.dumps(record, indent=2) + '\n')
    staging.replace(path)


def process_rows():
    result = {}
    for line in sp.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,lstart=,args='], text=True).splitlines():
        fields = line.split(None, 9)
        if len(fields) == 10:
            pid, parent, group, rss = map(int, fields[:4])
            result[pid] = {'pid': pid, 'parent': parent, 'group': group, 'rss': rss * 1024,
                           'birth': ' '.join(fields[4:9]), 'command': fields[9]}
    return result


def descendants(rows, roots):
    ids = set(roots)
    while True:
        more = ids | {pid for pid, row in rows.items() if row['parent'] in ids}
        if more == ids:
            return ids & rows.keys()
        ids = more


def global_sample(config, rows, owned):
    owned = descendants(rows, set(owned))
    metadata = json.loads(MEMBERSHIP.read_text())
    entries, receipt_pins, ordinary_records = membership_entries(metadata, rows)
    members = []
    claimed = set(owned)
    ordinary_roots = {pid for pid, row in rows.items() if str(GIT_PUBLISHER) in row['command'].split()}
    if ordinary_roots:
        assert hashlib.sha256(GIT_PUBLISHER.read_bytes()).hexdigest() == GIT_PUBLISHER_SHA256, 'ordinary publisher script changed'
    # Current sole-owner ordinary metadata identifies exact PID + birth + script;
    # a previous FULL cannot identify a reused PID or a different invocation.
    ordinary_identity_pins = []
    finishing_controllers = set()
    for record in ordinary_records:
        finisher = record.get('completedControllerFinisher')
        if finisher:
            finishing_controllers.add(finisher['pid'])
            ordinary_identity_pins.append(dict(owner=record['ownerThreadId'], role='exact completed controller finisher',
                pid=finisher['pid'], birth=finisher['birth'], script=finisher['script'],
                fullProofPath=record['fullProofPath'], fullProofSHA256=record['fullProofSHA256']))
        observation = record.get('ordinaryObservation')
        if observation and not any(observation.get(key) for key in ('endedAt', 'completedAt', 'ordinaryCommandsComplete')):
            pid = observation.get('actualpid')
            if pid in rows and rows[pid]['birth'] == ' '.join(observation.get('birth', '').split()) and hashlib.sha256(rows[pid]['command'].encode()).hexdigest() == observation.get('commandDigest'):
                ordinary_roots.add(pid)
                ordinary_identity_pins.append({'owner': record['ownerThreadId'], 'pid': pid, 'birth': rows[pid]['birth'], 'role': observation.get('role'), 'commandDigest': observation['commandDigest']})
        identity = record.get('controller') or {}
        pid, script = identity.get('pid'), identity.get('script')
        if pid in rows and script and rows[pid]['birth'] == ' '.join(identity.get('birth', '').split()) and script in rows[pid]['command'].split():
            ordinary_roots.add(pid)
            ordinary_identity_pins.append({'owner': record.get('ownerThreadId'), 'pid': pid, 'birth': rows[pid]['birth'], 'script': script})
    interface = json.loads((MEMBERSHIP.parent / 'resource-receipt-interface.json').read_text())
    infrastructure = interface.get('verifiedInfrastructureObservation', {})
    for pid in infrastructure.get('pids', []):
        if pid in rows and rows[pid]['birth'] == ' '.join(infrastructure.get('birth', '').split()) and rows[pid]['parent'] == infrastructure.get('ppid') and rows[pid]['group'] == infrastructure.get('pgid'):
            command = rows[pid]['command'].split()
            if command and command[0] == infrastructure.get('executable') and any(program in command for program in infrastructure.get('actualPrograms', [])):
                ordinary_roots.add(pid)
                ordinary_identity_pins.append({'kind': 'verified desktop tool infrastructure', 'pid': pid, 'birth': rows[pid]['birth'], 'actualArgs': rows[pid]['command']})
    ordinary = descendants(rows, ordinary_roots) | finishing_controllers
    claimed.update(ordinary)
    for entry in entries:
        if entry.get('owner', entry.get('ownerThreadId')) == OWN_ID or terminal(entry['state']):
            continue
        if entry.get('kind') == 'ordinary':
            ordinary_records.append({'state': 'ACCEPTED', 'remainingNewUpperBytes': entry['remainingNewUpperBytes']})
            continue
        matches = {pid for pid, row in rows.items() if pid not in owned and
                   any(marker in row['command'] for marker in entry.get('knownRootMatchers', []))}
        identities = entry.get('identities', [entry.get('controller'), *entry.get('guardControllers', [])])
        matches.update(identity['pid'] for identity in identities
            if identity and identity['pid'] in rows and identity.get('birth')
            and rows[identity['pid']]['birth'] == ' '.join(identity['birth'].split()))
        ids = descendants(rows, matches)
        claimed.update(ids)
        cohort = entry.get('cohort') or entry['offerId']
        kind = entry.get('kind') or ('target' if 'Compile' in cohort or 'Target' in cohort else 'browser' if 'Native' in cohort else 'source')
        if kind == 'copied-native':
            kind = 'browser'
        members.append({'cohort': cohort, 'kind': kind, 'pids': sorted(ids),
                        'processCap': max(entry['processCap'], len(ids)),
                        'rssCapBytes': max(entry['rssCapBytes'] if 'rssCapBytes' in entry else int(entry['rssCapGiB'] * 1024**3), sum(rows[pid]['rss'] for pid in ids)),
                        'remainingNewBytes': entry.get('remainingNewUpperBytes', entry.get('remainingNewConservativeUpperBytes')),
                        'state': entry['state']})
    # A PID's absence is not a release. Accepted entries above remain charged.
    # New jobs not yet represented in allocator membership fail closed rather than
    # borrowing guessed caps or representing this metadata file as a runtime guard.
    candidates = set()
    for pid, row in rows.items():
        command = row['command']
        executable = command.split(' ', 1)[0].lower()
        workflow = any(marker in command for marker in ['/mcr/', '/mcr-', '/mcr-worktrees/'])
        if workflow and re.search(r'(python[0-9.]*|node|cargo|rustc|chrome)$', executable) and pid not in claimed:
            candidates.add(pid)
    # Ordinary/full finishers identify only their exact PID. A new Node/Chrome
    # child after FULL must not inherit a completed controller's exemption.
    unknown = sorted(candidates)
    own_caps = config['offer']
    members.append({'cohort': 'PI12PreWorld', 'kind': 'source' if config['mode'] == 'source' else 'browser',
                    'pids': sorted(owned), 'processCap': max(own_caps['processes'], len(owned)),
                    'rssCapBytes': max(own_caps['rssBytes'], sum(rows[pid]['rss'] for pid in owned)),
                    'remainingNewBytes': own_caps['newBytes'], 'state': 'ACTUAL_ACCEPT'})
    union = set().union(*(set(member['pids']) for member in members)) | ordinary
    targets = sum(member['kind'] == 'target' for member in members)
    dynamic_other = max(3 + min(targets, 2), 0) * 1024**3
    higher_growth = sum(member['remainingNewBytes'] for member in members if member['cohort'] != 'PI12PreWorld')
    ordinary_upper = max(int(2.5 * 1024**3), sum(record['remainingNewUpperBytes'] for record in ordinary_records if record['state'] == 'ACCEPTED' and isinstance(record.get('remainingNewUpperBytes'), (int, float))))
    higher_growth += ordinary_upper
    result = {'metadataAsOf': metadata['asOfUTC'], 'receiptPins': receipt_pins, 'members': members, 'unresolvedPids': unknown,
            'workflows': len(members), 'sources': sum(member['kind'] == 'source' for member in members),
            'browsers': sum(member['kind'] == 'browser' for member in members), 'targets': targets,
            'distinctPids': sorted(union), 'actualRSS': sum(rows[pid]['rss'] for pid in union),
            'ordinaryIdentityPins': ordinary_identity_pins,
            'ordinaryGitOnly': {'pids': sorted(ordinary), 'script': str(GIT_PUBLISHER), 'sha256': GIT_PUBLISHER_SHA256, 'executionWorkflow': False},
            'chargedPIDs': sum(member['processCap'] for member in members) + len(ordinary),
            'chargedRSS': sum(member['rssCapBytes'] for member in members) + sum(rows[pid]['rss'] for pid in ordinary),
            'otherUpper': max(own_caps['otherUpperBytes'], dynamic_other, higher_growth),
            'ordinaryFutureUpperBytes': ordinary_upper,
            'processes': [rows[pid] for pid in sorted(union | set(unknown))]}
    if config.get('censusPath'):
        target = Path(config['censusPath'])
        staging = target.with_name(target.name + '.' + str(os.getpid()) + '.tmp')
        staging.write_text(json.dumps({'sampled': time.time(), 'samplerPID': os.getpid(), **result}, indent=2) + '\n')
        staging.replace(target)
    return result



def global_check(config, state, free):
    assert not state['unresolvedPids'], ('unresolved workflow claims', state['unresolvedPids'])
    assert state['workflows'] <= 4 and state['sources'] <= 3 and state['browsers'] <= 2 and state['targets'] <= 1, ('global membership cap', state)
    assert state['chargedPIDs'] <= 96 and state['chargedRSS'] <= 18 * 1024**3, ('global PID/RSS cap', state)
    assert free - state['otherUpper'] - config['offer']['newBytes'] >= 9 * 1024**3, ('after ALL free floor', free, state['otherUpper'])

def inventory(root, exclude=()):
    assert Path(root).is_dir(), ('required custody root missing', str(root))
    count = total = 0
    for here, directories, files in os.walk(root, followlinks=False):
        directories[:] = [name for name in directories if name not in exclude and not (Path(here) / name).is_symlink()]
        for name in files:
            path = Path(here) / name
            try:
                if not path.is_symlink():
                    count += 1
                    total += path.stat().st_size
            except FileNotFoundError:
                pass
    return count, total

def main():
    config_path = Path(sys.argv[1])
    config = json.loads(config_path.read_text())
    assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == config['guardianSha256']
    assert hashlib.sha256(Path(config['supervisorFile']).read_bytes()).hexdigest() == config['supervisorSha256']
    root, temporary, output = (Path(config[key]) for key in ['root', 'scratch', 'output'])
    supervisor = config['supervisor']
    ready, stop, receipt = (Path(str(output) + suffix) for suffix in ['.ready', '.stop', '.json'])
    assert not any(path.exists() for path in [ready, stop, receipt])
    known, births = {supervisor}, {}
    peaks = {'processes': 0, 'rss': 0, 'temporary': 0}
    failure = None

    def processes():
        rows = process_rows()
        known.intersection_update({pid for pid, row in rows.items() if pid in known and (pid not in births or births[pid] == row['birth'])})
        while True:
            children = {pid for pid, row in rows.items() if row['parent'] in known or str(temporary) in row['command']}
            if children <= known:
                break
            known.update(children)
        births.update({pid: row['birth'] for pid, row in rows.items() if pid in known})
        return [(pid, row['rss']) for pid, row in rows.items() if pid in known]

    try:
        initial = None
        # Publish this real guard before its first expensive inventory, rather
        # than waiting for the supervisor to observe the .ready file.
        bootstrap = Path(config['admission']['bootstrapReceipt'])
        assert bootstrap.is_file(), ('missing own bootstrap receipt', str(bootstrap))
        publish_accepted(config, supervisor, process_rows(), Path(config['evidence']), bootstrap, temporary)
        registration = json.loads((RECEIPTS / (OWN_ID + '.json')).read_text())
        assert any(identity['pid'] == os.getpid() for identity in registration['guardControllers']), 'actual guard not registered'
        Path(str(output) + '.bootstrap.json').write_text(json.dumps(dict(publishedBeforeInventoryAt=time.time(),
            guardPid=os.getpid(), actualRegistration=registration), indent=2) + '\n')
        while not stop.exists():
            rows = processes()
            assert supervisor in known, 'supervisor exited before FULL cleanup'
            usage = 0
            for directory, _, files in os.walk(temporary, followlinks=False):
                for name in files:
                    try:
                        path = Path(directory) / name
                        if not path.is_symlink(): usage += path.stat().st_size
                    except FileNotFoundError:
                        pass
            values = {'processes': len(rows), 'rss': sum(rss for _, rss in rows), 'temporary': usage}
            baseline = config['admission']
            sf, sb = inventory(root, ('.git', 'node_modules'))
            gf, gb = inventory(Path(config['commonGit']))
            mf, mb = inventory(Path(config['worktreeGit']))
            tf, tb = inventory(temporary)
            type_config = Path(config['typeConfig']) if config.get('typeConfig') else None
            if type_config and type_config.is_file():
                tf += 1; tb += type_config.stat().st_size
                values['temporary'] = tb
            _, db = inventory(Path(config['evidence']))
            positive = max(0, sb - baseline['sourceBytes']) + max(0, gb - baseline['gitBytes']) + max(0, mb - baseline['metadataBytes'])
            owner_record = RECEIPTS / (OWN_ID + '.json')
            positive += max(0, owner_record.stat().st_size - baseline.get('ownerReceiptBytes', 0))
            positive_files = max(0, sf - baseline['sourceFiles']) + max(0, gf - baseline['gitFiles']) + max(0, mf - baseline['metadataFiles'])
            values.update(whole=config['carryBytes'] + tb + positive, growth=baseline.get('retainedGrowthBytes', 0) + tb + positive,
                          durable=max(0, db - baseline['outBytes']), files=config['carryFiles'] + tf + positive_files)
            for key, value in values.items():
                peaks[key] = max(peaks.get(key, 0), value)
                assert value <= config[key], (key, value)
            assert shutil.disk_usage(root).free >= 8 * 1024**3, 'live disk floor'
            assert time.time() < config['cutoff'], 'work cutoff'
            global_state = global_sample(config, process_rows(), set(known))
            global_check(config, global_state, shutil.disk_usage(root).free)
            if initial is None:
                initial = dict(sampled=time.time(), values=values, free=shutil.disk_usage(root).free, globalState=global_state)
                staging = Path(str(ready) + '.tmp')
                staging.write_text(json.dumps(initial))
                staging.replace(ready)
            time.sleep(.25)
    except BaseException as error:
        failure = str(error)
        for sig in [signal.SIGTERM, signal.SIGKILL]:
            processes()
            for pid in known - {supervisor, os.getpid()}:
                try: os.kill(pid, sig)
                except ProcessLookupError: pass
    finally:
        receipt.write_text(json.dumps({'failure': failure, 'peaks': peaks, 'finished': time.time(),
            'guardianSha256': config['guardianSha256'], 'initial': initial, 'lastGlobal': locals().get('global_state')}, indent=2) + '\n')
    raise SystemExit(1 if failure else 0)


if __name__ == '__main__':
    main()
