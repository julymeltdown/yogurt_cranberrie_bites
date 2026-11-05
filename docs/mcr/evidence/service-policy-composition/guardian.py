"""Independent guard for one explicitly accepted FE38 composition allocation; adapted from continuity guardian at bd087fde."""
import json
import os
from pathlib import Path
import signal
import subprocess as sp
import sys
import time
import re
import hashlib
import shlex

MEMBERSHIP = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/resource-membership-current.json')
RECEIPTS = MEMBERSHIP.parent / 'owner-resource-receipts'
OWNER = '01a1a1fe-1eed-7d12-a247-879cd521e0ec'


def atomic(path, value):
    path = Path(path)
    temporary = path.with_name(path.name + '.' + str(os.getpid()) + '.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def process_rows():
    result = {}
    executables = {}
    for line in sp.check_output(['ps', '-axo', 'pid=,comm='], text=True).splitlines():
        fields = line.strip().split(None, 1)
        if len(fields) == 2:
            executables[int(fields[0])] = fields[1]
    for line in sp.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,lstart=,args='], text=True).splitlines():
        fields = line.split(None, 9)
        if len(fields) == 10:
            pid, parent, group, rss = map(int, fields[:4])
            result[pid] = {'pid': pid, 'parent': parent, 'group': group, 'rss': rss * 1024,
                           'birth': ' '.join(fields[4:9]), 'command': fields[9],
                           'executable': executables.get(pid)}
    # A process can be born between the comm and args snapshots. Re-observe
    # only missing scoped identities, retaining their birth match. No missing
    # executable is treated as an ordinary-program exemption.
    missing = [pid for pid, row in result.items() if not row['executable'] and
               any(marker in row['command'] for marker in ['/mcr/', '/mcr-', '/mcr-worktrees/'])]
    assert len(missing) <= 16, 'Unbounded missing process identities'
    if missing:
        probe = sp.run(['ps', '-p', ','.join(map(str, missing)), '-o', 'pid=,lstart=,comm='], capture_output=True, text=True)
        assert probe.returncode in (0, 1), 'Process identity re-observation failed'
        observed = set()
        for line in probe.stdout.splitlines():
            fields = line.split(None, 6)
            if len(fields) != 7:
                continue
            pid = int(fields[0]); observed.add(pid)
            if pid in result and result[pid]['birth'].split() == fields[1:6]:
                result[pid]['executable'] = fields[6]
        for pid in missing:
            if pid not in observed:
                result.pop(pid, None)  # Actually absent now; reservations remain separately charged.
    return result


def matches(row, identity):
    if not row or not identity or row['pid'] != identity.get('pid'):
        return False
    if row['birth'].split() != str(identity.get('birth', '')).split():
        return False
    executable = identity.get('executable')
    if not executable or not row.get('executable') or Path(executable).resolve() != Path(row['executable']).resolve():
        return False
    script = identity.get('script')
    if not script:
        return True
    arguments = shlex.split(row['command'])
    if script in arguments:
        return True
    # A factual receipt may declare the absolute script while Python was
    # launched with that same script relative to its declared actual cwd.
    # Resolve only the executable's script position, never arbitrary arguments.
    position = 1
    while position < len(arguments) and arguments[position] in ('-B', '-u', '-I', '-E', '-s'):
        position += 1
    cwd = identity.get('cwd') or identity.get('workingDirectory')
    return bool(cwd and position < len(arguments) and not arguments[position].startswith('-')
                and (Path(cwd) / arguments[position]).resolve() == Path(script).resolve())


def signal_owned(identities, sig):
    """Never signal a reused PID or a process group containing unowned processes."""
    errors = []
    rows = process_rows()
    for pid, identity in identities.items():
        if pid == os.getpid() or not matches(rows.get(pid), identity):
            continue
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass
        except PermissionError as exc:
            errors.append({'pid': pid, 'birth': identity['birth'], 'signal': int(sig), 'error': str(exc)})
    return errors


def descendants(rows, roots):
    ids = set(roots)
    while True:
        more = ids | {pid for pid, row in rows.items() if row['parent'] in ids}
        if more == ids:
            return ids & rows.keys()
        ids = more


def global_sample(config, rows, owned):
    metadata = json.loads(MEMBERSHIP.read_text())
    members = []
    claimed = set(owned)
    entries = {}
    def key(entry):
        return (entry.get('ownerThreadId', entry.get('owner')), entry.get('offerId', entry.get('cohort')),
                entry.get('acceptedAtEpoch', entry.get('actualAccept')))
    for entry in metadata['entries']:
        entries[key(entry)] = entry
    discovered = []
    receipt_paths = sorted(RECEIPTS.glob('*.json'))
    assert len(receipt_paths) <= 128, 'Unbounded receipt directory'
    for path in receipt_paths:
        assert path.stat().st_size <= 65536, 'Oversize owner receipt'
        entry = json.loads(path.read_text())
        if 'ordinaryObservation' in entry and 'ownerThreadId' not in entry:
            observation = entry['ordinaryObservation']
            assert isinstance(observation, dict) and str(observation.get('role', '')).startswith('ordinary '), 'Ambiguous observation role'
            complete = observation.get('ordinaryCommandsComplete') is True and observation.get('commandExit') == 0
            entry = {'ownerThreadId': path.stem, 'offerId': observation.get('booking', {}).get('id'),
                     'state': 'FULL' if complete else 'ACCEPTED', 'kind': 'ordinary',
                     'processCap': None, 'rssCapBytes': None, 'executionWorkflow': False,
                     'remainingNewUpperBytes': 0 if complete else observation.get('booking', {}).get('additionalUpperBytes', 0),
                     'controller': {'pid': observation.get('actualpid'), 'birth': observation.get('birth'),
                                    'executable': observation.get('executable')}, 'ordinaryObservation': observation}
        assert path.stem == entry['ownerThreadId'], 'Ambiguous receipt owner'
        # Exact round identity replaces a legacy snapshot; a missing file or a
        # different/newer round never erases an old accepted reservation.
        entries[key(entry)] = {**entries.get(key(entry), {}), **entry}
        discovered.append(str(path))
    # A newer per-owner file may overwrite an older release. Resolve only
    # exact old rounds with an immutable, responsible-owner literal receipt;
    # never infer old FULL from the newer round or from process absence.
    for release in config.get('peerFullReceipts', []):
        path = Path(release['path'])
        raw = path.read_bytes()
        assert hashlib.sha256(raw).hexdigest() == release['sha256'], 'Peer FULL receipt changed'
        value = json.loads(raw)
        assert (value.get('resourcesReleased') is True or
                (value.get('state') == 'ACTUAL_FULL' and value.get('remainingOwned') == []) or
                (value.get('status') == 'FULL' and value.get('resources') and all(v == 0 for v in value['resources'].values())))
        exact = (release['ownerThreadId'], release['offerId'], release['acceptedAtEpoch'])
        if exact in entries:
            entries[exact] = {**entries[exact], 'state': 'FULL', 'literalFullPath': str(path)}
    ordinary = []
    for entry in entries.values():
        own = entry.get('ownerThreadId', entry.get('owner')) == OWNER
        if own and entry.get('offerId') == config['offer']['id']:
            continue  # The current owned cohort is appended exactly once below.
        state = entry['state']
        closed = state in ('FULL', 'DECLINED') or state.startswith(('LITERAL_FULL_RECEIVED', 'ACTUAL_FULL', 'UNACCEPTED_DECLINE', 'LITERAL_DECLINE'))
        kind = entry.get('kind')
        cohort = entry.get('cohort') or entry.get('offerId') or entry.get('ownerThreadId')
        if not kind:
            kind = 'target' if 'Compile' in cohort or 'Target' in cohort else 'browser' if 'Native' in cohort else 'source'
        if closed and kind != 'ordinary':
            continue
        controllers = [entry.get('controller') or {}, *(entry.get('guardControllers') or [])]
        controllers = [identity for identity in controllers if isinstance(identity, dict)]
        roots = {identity['pid'] for identity in controllers if matches(rows.get(identity.get('pid')), identity)}
        # Legacy unstructured entries retain conservative charges. A marker
        # cannot replace the birth identity when a structured controller exists.
        if not entry.get('controller'):
            roots |= {pid for pid, row in rows.items() if any(marker in row['command'] for marker in (entry.get('knownRootMatchers') or []))}
        ids = descendants(rows, roots)
        if kind == 'ordinary':
            # This exact publisher program was read as Git/capacity/metadata-only;
            # its immutable hash plus actual owner/role/birth resolves identity.
            # No arbitrary publisher prefix or Python process is exempted.
            controller = entry.get('controller') or {}
            program = config.get('ordinaryPrograms', {}).get(controller.get('script'))
            explicit_source_role = any(
                key(entry) == (pin['ownerThreadId'], pin['offerId'], pin['acceptedAtEpoch'])
                and entry.get('phase') == pin['phase']
                and entry.get('ordinaryObservation', {}).get('role') == pin['role']
                and controller.get('script') == pin['script']
                for pin in config.get('ordinaryRolePins', []))
            verified = (program and roots and (entry.get('executionWorkflow') is False or explicit_source_role) and
                        entry.get('processCap') is None and entry.get('rssCapBytes') is None and
                        hashlib.sha256(Path(controller['script']).read_bytes()).hexdigest() == program)
            if verified:
                # Only the identified ordinary controllers are exempted. A
                # Node/Python/compiler/browser child would contradict the
                # recorded Git-only role and must remain an unknown workflow.
                claimed.update(roots)
            ordinary.append({'owner': entry.get('ownerThreadId'), 'state': state, 'verified': bool(verified),
                             'pids': sorted(ids), 'remainingNewBytes': entry.get('remainingNewUpperBytes', 0)})
            continue
        if closed:
            continue
        claimed.update(ids)
        process_cap = entry.get('processCap')
        rss_cap = entry.get('rssCapBytes')
        if rss_cap is None and entry.get('rssCapGiB') is not None:
            rss_cap = int(entry['rssCapGiB'] * 1024**3)
        assert isinstance(process_cap, int) and isinstance(rss_cap, int), 'Unresolved execution caps'
        members.append({'cohort': cohort, 'kind': kind, 'pids': sorted(ids),
                        'processCap': max(process_cap, len(ids)),
                        'rssCapBytes': max(rss_cap, sum(rows[pid]['rss'] for pid in ids)),
                        'remainingNewBytes': entry.get('remainingNewUpperBytes', entry.get('remainingNewConservativeUpperBytes', 0)),
                        'state': state})
    # A PID's absence is not a release. Accepted entries above remain charged.
    # New jobs not yet represented in allocator membership fail closed rather than
    # borrowing guessed caps or representing this metadata file as a runtime guard.
    candidates = set()
    for pid, row in rows.items():
        command = row['command']
        executable = (row.get('executable') or command.split(' ', 1)[0]).lower()
        workflow = any(marker in command for marker in ['/mcr/', '/mcr-', '/mcr-worktrees/'])
        if workflow and re.search(r'(python[0-9.]*|node|cargo|rustc|chrome(?: helper(?: \([a-z]+\))?)?)$', executable) and pid not in claimed:
            candidates.add(pid)
    unknown = sorted(candidates - claimed)
    own_caps = config['offer']
    members.append({'cohort': 'FE38Composition', 'kind': 'source' if config['mode'] == 'source' else 'browser',
                    'pids': sorted(owned), 'processCap': max(own_caps['processes'], len(owned)),
                    'rssCapBytes': max(own_caps['rssBytes'], sum(rows[pid]['rss'] for pid in owned)),
                    'remainingNewBytes': own_caps['newBytes'], 'state': 'ACTUAL_ACCEPT'})
    workflow_union = set().union(*(set(member['pids']) for member in members))
    ordinary_union = set().union(*(set(item['pids']) for item in ordinary)) if ordinary else set()
    ordinary_only = ordinary_union - workflow_union
    union = workflow_union | ordinary_union
    targets = sum(member['kind'] == 'target' for member in members)
    higher_growth = sum(member['remainingNewBytes'] for member in members if member['cohort'] != 'FE38Composition')
    higher_growth += sum(item['remainingNewBytes'] for item in ordinary if item['state'] != 'FULL')
    return {'metadataAsOf': metadata['asOfUTC'], 'members': members, 'unresolvedPids': unknown,
            'workflows': len(members), 'sources': sum(member['kind'] == 'source' for member in members),
            'browsers': sum(member['kind'] == 'browser' for member in members), 'targets': targets,
            'distinctPids': sorted(union), 'actualRSS': sum(rows[pid]['rss'] for pid in union),
            'chargedPIDs': sum(member['processCap'] for member in members) + len(ordinary_only),
            'chargedRSS': sum(member['rssCapBytes'] for member in members) + sum(rows[pid]['rss'] for pid in ordinary_only),
            'otherUpper': own_caps['otherUpperBytes'] + higher_growth,
            'receiptFiles': discovered, 'ordinary': ordinary,
            'processes': [rows[pid] for pid in sorted(union | set(unknown))]}


def census(paths):
    seen = set(); total = 0; counts = []
    for root in paths:
        subtotal = 0; count = 0
        root = Path(root)
        assert root.exists() or root.is_symlink(), 'Required census root missing: ' + str(root)
        if root.is_symlink() or root.is_file():
            entries = [(root.parent, [], [root.name])]
        else:
            entries = __import__('os').walk(root, followlinks=False)
        for here, directories, files in entries:
            for name in ['.'] + directories + files:
                path = Path(here) if name == '.' else Path(here) / name
                try: stat = path.lstat()
                except FileNotFoundError: continue
                key = (stat.st_dev, stat.st_ino)
                if key in seen: continue
                seen.add(key); subtotal += max(stat.st_size, stat.st_blocks * 512); count += 1
        total += subtotal; counts.append({'root': str(root), 'bytes': subtotal, 'inodes': count})
    return {'total': total, 'roots': counts}


def size(path):
    return census([path])['total'] if path.exists() else 0


def fresh_bytes(config):
    root = Path(config['root'])
    objects = {row.split()[0] for row in sp.check_output(['git', 'rev-list', '--objects', config['base'] + '..HEAD'], cwd=root, text=True).splitlines()}
    git_bytes = sum(int(row) for row in sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'], cwd=root,
        input='\n'.join(sorted(objects)) + '\n', text=True).splitlines()) if objects else 0
    git_bytes += size(Path(sp.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=root, text=True).strip()))
    return git_bytes + sum(size(root / path) for path in config['freshPaths'])


def sample(config):
    rows = process_rows()
    assert matches(rows.get(config['supervisor']), config['supervisorIdentity']), 'Supervisor identity mismatch'
    ids = descendants(rows, {config['supervisor']})
    global_state = global_sample(config, rows, ids)
    temporary = size(Path(config['temporary']))
    fresh = fresh_bytes(config) + temporary + config.get('spentTemporaryBytes', 0) + config.get('ordinaryReservedBytes', 0)
    pngs = list(Path(config['pngDirectory']).glob('*.png')) if config.get('pngDirectory') else []
    inventory = census(config['wholeRoots'])
    return {'pids': sorted(ids), 'processes': len(ids), 'rss': sum(rows[pid]['rss'] for pid in ids),
            'temporary': temporary, 'durable': size(Path(config['evidence'])), 'new': fresh,
            'whole': max(config['fixedCarry'], inventory['total']) + fresh, 'wholeInventory': inventory,
            'free': __import__('shutil').disk_usage(config['root']).free, 'global': global_state,
            'ownedIdentities': [rows[pid] for pid in sorted(ids)],
            'pngCount': config.get('priorPngCount', 0) + len(pngs),
            'pngBytes': config.get('priorPngBytes', 0) + sum(path.stat().st_size for path in pngs)}


def breach(config, values):
    caps = config['offer']
    shared = values['global']
    return ('unresolved workflow claims' if shared['unresolvedPids'] else
            'PNG cap' if values.get('pngCount', 0) > 40 or values.get('pngBytes', 0) > 16 * 1024**2 else
            'global workflow cap' if shared['workflows'] > 4 or shared['sources'] > 3 or shared['browsers'] > 2 or shared['targets'] > 1 else
            'global PID/RSS cap' if shared['chargedPIDs'] > 96 or shared['chargedRSS'] > 18 * 1024**3 else
            'deadline' if time.time() >= config['workDeadline'] else
            'live free' if values['free'] < caps['continuousFreeBytes'] else
            'after ALL free' if values['free'] - caps['newBytes'] - shared['otherUpper'] < caps['afterAllFreeBytes'] else
            next((key for key, field in [('processes', 'processes'), ('rss', 'rssBytes'), ('temporary', 'temporaryBytes'),
                  ('durable', 'durableBytes'), ('new', 'newBytes'), ('whole', 'wholeBytes')] if values[key] > caps[field]), None))


def main():
    path = Path(sys.argv[1]); config = json.loads(path.read_text())
    report = {'breach': None, 'samples': 0, 'peaks': {}}
    terminate_supervisor = False
    try:
        # Publish the actual guard identity before either observer starts census.
        while config.get('start') and not Path(config['start']).exists():
            assert time.time() < config['workDeadline'], 'Unpublished guard identity'
            time.sleep(.05)
        while not Path(config['stop']).exists():
            values = sample(config); report['samples'] += 1
            if config.get('independentCensus'):
                atomic(config['independentCensus'], values)
            report.setdefault('initial', values)
            report['last'] = values
            for key in ['processes', 'rss', 'temporary', 'durable', 'new', 'whole']:
                report['peaks'][key] = max(report['peaks'].get(key, 0), values[key])
            report['minimumFree'] = min(report.get('minimumFree', values['free']), values['free'])
            failed = breach(config, values)
            if failed:
                report['breach'] = failed
                active = Path(config['activeChild'])
                if active.exists():
                    identities = json.loads(active.read_text())
                    report['cleanupErrors'] = signal_owned({int(pid): item for pid, item in identities.items()}, signal.SIGTERM)
                terminate_supervisor = True
                break
            if not Path(config['ready']).exists():
                atomic(config['ready'], values)
            time.sleep(.4)
    except BaseException as exc:
        report['breach'] = report['breach'] or type(exc).__name__ + ': ' + str(exc)
        terminate_supervisor = True
    finally:
        atomic(config['report'], report)
        if terminate_supervisor:
            signal_owned({config['supervisor']: config['supervisorIdentity']}, signal.SIGTERM)
    return 1 if report['breach'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
