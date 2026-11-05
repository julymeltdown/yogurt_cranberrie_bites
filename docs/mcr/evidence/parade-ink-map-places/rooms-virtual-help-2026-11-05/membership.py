"""Owner-local receipt discovery; no grants, queue, or peer mutations."""
import ctypes
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import time

OWNER = '01a1a26e-1de2-7b41-a294-c9a79cfded9b'
BASE = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d')
DIRECTORY = BASE / 'owner-resource-receipts'
OWN = DIRECTORY / (OWNER + '.json')
VERIFIED = {}
CONSUMED_RAW = {}


def processes():
    rows = {}
    raw = subprocess.check_output(['ps', '-ww', '-axo', 'pid=,ppid=,lstart=,rss=,stat=,command='], text=True)
    for line in raw.splitlines():
        f = line.strip().split(None, 9)
        if len(f) == 10:
            rows[int(f[0])] = dict(parent=int(f[1]), birth=' '.join(f[2:7]), rss=int(f[7]) * 1024, state=f[8], command=f[9])
    return rows


def identity(pid, script, output=None):
    # Kernel self-observation does not spawn an unregistered helper process.
    assert pid == os.getpid()
    lib = ctypes.CDLL('/usr/lib/libproc.dylib')
    raw = ctypes.create_string_buffer(4096)
    assert lib.proc_pidinfo(pid, 3, 0, raw, 4096) >= 136
    seconds = ctypes.c_uint64.from_buffer(raw, 120).value
    micros = ctypes.c_uint64.from_buffer(raw, 128).value
    executable = ctypes.create_string_buffer(4096)
    assert lib.proc_pidpath(pid, executable, 4096) > 0
    system = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
    mib = (ctypes.c_int * 3)(1, 49, pid)
    length = ctypes.c_size_t()
    assert system.sysctl(mib, 3, None, ctypes.byref(length), None, 0) == 0
    arguments = ctypes.create_string_buffer(length.value)
    assert system.sysctl(mib, 3, arguments, ctypes.byref(length), None, 0) == 0
    raw_args = arguments.raw[:length.value]
    argc = int.from_bytes(raw_args[:4], 'little')
    cursor = raw_args.index(b'\0', 4) + 1
    while raw_args[cursor] == 0:
        cursor += 1
    argv = []
    for _ in range(argc):
        end = raw_args.index(b'\0', cursor)
        argv.append(raw_args[cursor:end].decode())
        cursor = end + 1
    # These owned controller vectors are simple ASCII without PS escaping.
    assert all(arg.isascii() and not any(c.isspace() for c in arg) for arg in argv)
    result = dict(pid=pid, parentPid=os.getppid(),
                  birth=datetime.datetime.fromtimestamp(seconds).strftime('%a %b %e %H:%M:%S %Y'),
                  kernelBirthEpoch=seconds + micros / 1e6, executable=executable.value.decode(),
                  script=str(script) if script is not None else None, cwd=os.getcwd(), argv=argv,
                  argvDigest=hashlib.sha256(json.dumps(argv, separators=(',', ':')).encode()).hexdigest(),
                  commandDigest=hashlib.sha256(' '.join(argv).strip().encode()).hexdigest(),
                  commandDigestEncoding='sha256:ps-ww-args-strip-utf8-v1',
                  commandDigestSource='direct KERN_PROCARGS2 simple ASCII argv before helpers')
    if output is not None:
        result['outputRoot'] = str(output)
    return result


def numeric_birth(control):
    keys = ('birthEpochSeconds', 'birthMicroseconds')
    if not any(key in control for key in keys):
        return None
    assert all(key in control for key in keys), 'incomplete numeric birth tuple'
    seconds, micros = (control[key] for key in keys)
    assert type(seconds) is int and seconds > 0, 'numeric birth seconds must be positive integer seconds'
    assert type(micros) is int and 0 <= micros < 1000000, 'numeric birth microseconds outside exact unit domain'
    return seconds, micros


def birth_text(control):
    numeric = numeric_birth(control)
    if numeric is not None:
        value = ' '.join(datetime.datetime.fromtimestamp(numeric[0]).strftime('%a %b %e %H:%M:%S %Y').split())
        if 'birth' in control:
            assert isinstance(control['birth'], str) and ' '.join(control['birth'].split()) == value, 'contradictory numeric/text birth'
        return value
    assert isinstance(control.get('birth'), str) and control['birth'].strip(), 'missing typed process birth'
    return ' '.join(control['birth'].split())


class KernelIdentityUnavailable(AssertionError):
    def __init__(self, pid, expected, returned):
        self.pid, self.expected, self.returned = pid, expected, returned
        super().__init__('numeric kernel identity unavailable: ' + json.dumps(dict(pid=pid, expectedBirthSeconds=expected[0], expectedBirthMicroseconds=expected[1], returnedBytes=returned)))


def verify_numeric_birth(control, pid):
    expected = numeric_birth(control)
    if expected is None:
        return
    lib = ctypes.CDLL('/usr/lib/libproc.dylib')
    raw = ctypes.create_string_buffer(4096)
    returned = lib.proc_pidinfo(pid, 3, 0, raw, 4096)
    if returned < 136:
        raise KernelIdentityUnavailable(pid, expected, returned)
    actual = (ctypes.c_uint64.from_buffer(raw, 120).value, ctypes.c_uint64.from_buffer(raw, 128).value)
    assert actual == expected, 'numeric kernel birth seconds/microseconds mismatch'


def verify_controller(control, row, claim_key=None, snapshot_sha=None, refresh=True, ordinary=False):
    """Resolve a relative Python script using the process's observed working directory."""
    pid = control['pid']
    descriptor_sha = hashlib.sha256(json.dumps(control, sort_keys=True).encode()).hexdigest()
    key = (tuple(claim_key or ()), pid)
    context = dict(ownerClaim=claim_key, pid=pid, birth=row['birth'], parent=row['parent'], state=row.get('state'), expectedImage=control.get('executable'), expectedScript=control.get('script'), commandSHA256=hashlib.sha256(row['command'].encode()).hexdigest())
    assert row['birth'] == birth_text(control), 'resource PID birth mismatch: ' + json.dumps(context)
    if str(row.get('state', '')).startswith('Z'):
        prior = VERIFIED.get(key, {})
        assert (claim_key and prior.get('descriptorSHA256') == descriptor_sha
                and prior.get('birth') == row['birth'] and prior.get('parent') == row['parent']
                and prior.get('liveVerified') is True and prior.get('snapshotSHA256')), 'unverified terminal controller: ' + json.dumps(context)
        # This establishes identity only. The accepted claim keeps all declared caps.
        return dict(prior, terminal=True)
    verify_numeric_birth(control, pid)
    executable = subprocess.check_output(['ps', '-p', str(pid), '-o', 'comm='], text=True).strip()
    if executable != control['executable'] and refresh:
        fresh = processes()
        current = fresh.get(pid)
        if (current and current['birth'] == row['birth'] and current['parent'] == row['parent']
                and str(current.get('state', '')).startswith('Z')):
            return verify_controller(control, current, claim_key, hashlib.sha256(json.dumps(fresh, sort_keys=True).encode()).hexdigest(), False, ordinary)
    assert executable == control['executable'], 'resource OS executable mismatch: ' + json.dumps(dict(context, observedImage=executable))
    raw = subprocess.check_output(['lsof', '-a', '-p', str(pid), '-d', 'cwd', '-Fn'], text=True)
    cwd = [line[1:] for line in raw.splitlines() if line.startswith('n')]
    assert len(cwd) == 1 and Path(cwd[0]).is_absolute(), 'missing actual controller cwd'
    declared = control.get('cwd', control.get('workingDirectory'))
    # Legacy ordinary inline receipts may omit a declared cwd. Its actual cwd
    # is still observed and bound in the retained live identity proof.
    assert (declared and Path(declared).resolve() == Path(cwd[0]).resolve()) or (ordinary and not declared), 'resource controller cwd mismatch: ' + json.dumps(context)
    arguments = shlex.split(row['command'])
    args = list(arguments[1:])
    while args and args[0] in ('-B', '-u', '-I', '-E', '-s', '-S', '-O', '-OO'):
        args.pop(0)
    program = args[0] if args else None
    expected = control['script']
    observed = None
    if ordinary and expected == '-':
        assert program == '-', 'ordinary inline source token mismatch: ' + json.dumps(dict(context, observedProgram=program))
        observed = '-'
    else:
        assert expected and Path(expected).is_absolute(), 'controller descriptor script must be absolute: ' + json.dumps(context)
        assert program and not program.startswith('-'), 'resource decoded program missing: ' + json.dumps(context)
        observed = str((Path(program) if Path(program).is_absolute() else Path(cwd[0]) / program).resolve())
        assert Path(expected).resolve() == Path(observed), 'resource decoded script/cwd mismatch: ' + json.dumps(dict(context, observedProgram=observed))
    if ordinary and control.get('commandEncoding') == 'json-string-array-utf8':
        verify_typed_vector(control, pid)
    elif ordinary and control.get('commandDigest'):
        assert control.get('commandDigestEncoding') in (None, 'sha256:ps-ww-args-strip-utf8-v1'), 'unsupported ordinary command digest domain'
        assert control['commandDigest'] == hashlib.sha256(row['command'].strip().encode()).hexdigest(), 'ordinary raw PS digest mismatch: ' + json.dumps(context)
    proof = dict(descriptorSHA256=descriptor_sha, birth=row['birth'], parent=row['parent'],
                 actualCwd=cwd[0], actualExecutable=executable, actualProgram=observed,
                 liveVerified=True, snapshotSHA256=snapshot_sha, ownerClaim=claim_key, terminal=False)
    VERIFIED[key] = proof
    return proof


def publish(value):
    value['publishedAtEpoch'] = time.time()
    temporary = OWN.with_name('.' + OWN.name + '.' + str(os.getpid()) + '.tmp')
    encoded = json.dumps(value, separators=(',', ':')) + '\n'
    assert len(encoded.encode()) <= 65536, 'oversized owned resource receipt'
    temporary.write_text(encoded)
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


def released_history(owner, old, record):
    history = record.get('closedHistory', [])
    assert isinstance(history, list) and len(history) <= 128, 'unbounded closed history'
    offer = old.get('offerId', old.get('cohort'))
    accepted = epoch(old.get('acceptedAtEpoch', old.get('actualAccept')))
    if not offer or accepted is None:
        return False
    for item in history:
        if item.get('ownerThreadId') != owner or item.get('offerId') != offer or item.get('state') != 'FULL':
            continue
        if epoch(item.get('acceptedAtEpoch', item.get('acceptedEpoch'))) != accepted:
            continue
        path = Path(item.get('fullPath', ''))
        expected = item.get('fullSha256', item.get('sha256'))
        assert path.is_absolute() and isinstance(expected, str) and re.fullmatch('[a-f0-9]{64}', expected), 'incomplete exact historical FULL binding'
        assert path.is_file() and path.stat().st_size <= 1024 * 1024, 'missing or oversized historical FULL'
        raw = path.read_bytes()
        assert hashlib.sha256(raw).hexdigest() == expected, 'historical FULL hash mismatch'
        full = json.loads(raw)
        assert full.get('state', full.get('type')) == 'FULL', 'historical artifact is not FULL'
        assert full.get('offerId', full.get('lease')) == offer, 'historical FULL offer mismatch'
        assert epoch(full.get('acceptedAtEpoch', full.get('acceptedEpoch'))) == accepted, 'historical FULL accepted epoch mismatch'
        resources = full.get('resources')
        zero = resources == 0 or (isinstance(resources, dict) and resources and all(value == 0 for value in resources.values()))
        assert zero and not full.get('livePids'), 'historical FULL resources not released'
        return True
    return False


def kernel_argv(pid):
    system = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
    mib = (ctypes.c_int * 3)(1, 49, pid)
    length = ctypes.c_size_t()
    assert system.sysctl(mib, 3, None, ctypes.byref(length), None, 0) == 0
    assert 4 < length.value < 1048576
    arguments = ctypes.create_string_buffer(length.value)
    assert system.sysctl(mib, 3, arguments, ctypes.byref(length), None, 0) == 0
    raw = arguments.raw[:length.value];argc = int.from_bytes(raw[:4], 'little')
    assert 0 < argc < 4096
    cursor = raw.index(b'\0', 4) + 1
    while raw[cursor] == 0:cursor += 1
    result = []
    for unused in range(argc):
        end = raw.index(b'\0', cursor);result.append(raw[cursor:end].decode('utf-8'));cursor = end + 1
    return result


def verify_typed_vector(control, pid):
    assert control.get('argvEncoding') == 'json-string-array-utf8'
    assert control.get('commandEncoding') == 'json-string-array-utf8'
    expected = control.get('argv')
    assert isinstance(expected, list) and expected and all(isinstance(x, str) for x in expected)
    actual = kernel_argv(pid)
    assert actual == expected and control.get('actualArgv', expected) == actual, 'ordinary full kernel argv mismatch'
    digest = hashlib.sha256(json.dumps(actual, ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()
    assert control.get('argvSHA256') == digest and control.get('commandDigest') == digest, 'ordinary kernel vector digest mismatch'
    lib = ctypes.CDLL('/usr/lib/libproc.dylib');raw = ctypes.create_string_buffer(4096)
    returned = lib.proc_pidinfo(pid, 3, 0, raw, 4096)
    if returned < 136:raise KernelIdentityUnavailable(pid, numeric_birth(control), returned)
    assert control['parentPid'] == ctypes.c_uint32.from_buffer(raw, 16).value
    assert control['processGroup'] == ctypes.c_uint32.from_buffer(raw, 100).value
    script = Path(control['script'])
    assert script.is_absolute() and hashlib.sha256(script.read_bytes()).hexdigest() == control['scriptSHA256'], 'ordinary script hash mismatch'


def ordinary_controls(record):
    ordinary = []
    control = record.get('controller')
    if record.get('kind') == 'ordinary' and control and control.get('endedAt') is None:ordinary.append(control)
    candidates = []
    observation = record.get('ordinaryObservation')
    if observation:candidates.append(observation)
    observations = record.get('ordinaryObservations', [])
    assert isinstance(observations, list) and len(observations) <= 128
    candidates.extend(observations)
    activity = record.get('ordinaryActivity')
    if activity:
        assert isinstance(activity, dict)
        if activity.get('controller'):candidates.append(activity['controller'])
    for item in candidates:
        assert isinstance(item, dict)
        if item.get('endedAt') is not None:continue
        value = dict(item, pid=item.get('actualpid', item.get('pid')))
        value.setdefault('script', None)
        ordinary.append(value)
    return ordinary


def _sample_once(proposed=None):
    policy = json.loads((BASE / 'resource-receipt-interface.json').read_text())['limits']
    legacy = json.loads((BASE / 'resource-membership-current.json').read_text())['entries']
    files = list(DIRECTORY.glob('*.json'))
    assert len(files) <= 128, 'unbounded resource receipt directory'
    records = {}
    for file in files:
        assert file.stat().st_size <= 65536, 'oversized resource receipt'
        raw_receipt = file.read_bytes()
        if file.stem == '01a19f83-12cc-7f21-8ad3-55749f012d9b':CONSUMED_RAW[file.stem] = raw_receipt
        record = json.loads(raw_receipt)
        if set(record) == {'ordinaryObservation'}:
            record = dict(record, ownerThreadId=file.stem)  # Observation only, no fabricated FULL/caps.
        else:
            assert file.stem == record['ownerThreadId'], 'receipt owner/filename mismatch: ' + file.name
            state = record.get('state')
            assert state in ('ACCEPTED', 'ACCEPTED_RUNNING', 'ACCEPTED_GUARDS_STARTING', 'ACCEPTED_BOOTSTRAP', 'ACTUAL_ACCEPT', 'FULL', 'DECLINED'), 'unsupported receipt state: ' + file.name + ':' + str(state)
            if state in ('ACCEPTED_RUNNING', 'ACCEPTED_GUARDS_STARTING', 'ACCEPTED_BOOTSTRAP', 'ACTUAL_ACCEPT'):
                record = dict(record, state='ACCEPTED')  # Interface aliases retain every declared cap/clock.
        records[record['ownerThreadId']] = record
    if proposed is not None:
        assert proposed['ownerThreadId'] == OWNER
        assert records.get(OWNER, {}).get('state') != 'ACCEPTED', 'cannot hide an accepted own workflow'
        records[OWNER] = proposed
    claims = {owner: record for owner, record in records.items() if record.get('state') == 'ACCEPTED' and record.get('kind') != 'ordinary'}
    for old in legacy:
        if old.get('kind') == 'ordinary' or old['state'] == 'FULL' or old['state'].startswith('LITERAL_FULL') or 'DECLIN' in old['state']:
            continue
        owner = old.get('ownerThreadId', old.get('owner'))
        record = records.get(owner)
        resolved = False
        if record:
            offer, previous_offer = record.get('offerId'), old.get('offerId', old.get('cohort'))
            a, b = epoch(record.get('acceptedAtEpoch')), epoch(old.get('acceptedAtEpoch', old.get('actualAccept')))
            same_claim = bool(offer and previous_offer and offer == previous_offer and a is not None and a == b)
            # A current literal same-claim record deduplicates/ends that claim;
            # a newer ordinary/workflow timestamp alone never closes old history.
            resolved = record.get('kind') != 'ordinary' and same_claim and record.get('state') in ('ACCEPTED', 'FULL')
            if not resolved:
                resolved = released_history(owner, old, record)
        if not resolved:
            assert owner not in claims, 'ambiguous simultaneous legacy/current claims: ' + owner
            rss = old['rssCapBytes'] if 'rssCapBytes' in old else int(old['rssCapGiB'] * 1024**3)
            new = old['remainingNewUpperBytes'] if 'remainingNewUpperBytes' in old else old['remainingNewConservativeUpperBytes']
            claims[owner] = dict(old, ownerThreadId=owner, offerId=old.get('offerId', old.get('cohort')), kind=old['kind'],
                                 processCap=old['processCap'], rssCapBytes=rss,
                                 remainingNewUpperBytes=new, legacyMatchers=old.get('knownRootMatchers', []))
    assert len(claims) <= policy['global'], 'global workflow reservations'
    assert sum(c['kind'] in ('source', 'copied-native') for c in claims.values()) <= policy['source'], 'source workflow reservations'
    assert sum(c['kind'] == 'browser' for c in claims.values()) <= policy['browser'], 'browser workflow reservations'
    assert sum(c['kind'] == 'target' for c in claims.values()) <= policy['warmRustTarget'], 'target workflow reservations'
    assert sum(c['processCap'] for c in claims.values()) <= policy['sumPIDCaps'], 'aggregate workflow PID cap'
    assert sum(c['rssCapBytes'] for c in claims.values()) <= policy['sumRSSBytes'], 'aggregate workflow RSS cap'
    rows, covered = processes(), set()
    snapshot_sha = hashlib.sha256(json.dumps(rows, sort_keys=True).encode()).hexdigest()
    for claim in claims.values():
        roots = set()
        for control in [claim.get('controller')] + claim.get('guardControllers', []):
            if not control:
                continue
            pid = control['pid']
            if pid in rows:
                verify_controller(control, rows[pid], (claim['ownerThreadId'], claim['offerId'], epoch(claim.get('acceptedAtEpoch', claim.get('actualAccept')))), snapshot_sha)
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
        ordinary = ordinary_controls(record)
        for control in ordinary:
            pid = control['pid']
            if pid not in rows:
                continue
            if rows[pid]['birth'] != birth_text(control):
                continue  # A stale observation cannot identify a reused PID.
            if numeric_birth(control) is not None and not str(rows[pid].get('state', '')).startswith('Z'):
                verify_numeric_birth(control, pid)
            if control.get('script'):
                verify_controller(control, rows[pid], (record.get('ownerThreadId'), 'ordinary', record.get('offerId'), epoch(record.get('acceptedAtEpoch'))), snapshot_sha, ordinary=True)
            tree = {pid}
            previous = None
            while previous != tree:
                previous = set(tree)
                tree.update(child for child, row in rows.items() if row['parent'] in tree)
            for child in tree:
                command = rows[child]['command']
                heavy = bool(re.search(r'\b(?:node|cargo|rustc)(?:\s|$)', command)) or ('Google Chrome --' in command and '--headless' in command)
                script_child = child != pid and bool(re.search(r'\b(?:Python|python3(?:\.\d+)?)\s+(?:-B\s+)?/\S+\.py', command))
                assert not heavy and not script_child, 'ordinary role cannot cover runtime payload: ' + json.dumps(dict(owner=record.get('ownerThreadId'), ordinaryPid=pid, childPid=child, childBirth=rows[child]['birth'], commandSHA256=hashlib.sha256(command.encode()).hexdigest()))
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
    assert len(covered) <= policy['sumPIDCaps'], 'aggregate workflow PID cap'
    rss = sum(rows[pid]['rss'] for pid in covered)
    assert rss <= policy['sumRSSBytes'], 'aggregate workflow RSS cap'
    other = policy['ordinaryFuturePositiveUpperBytes'] + sum(c['remainingNewUpperBytes'] for owner, c in claims.items() if owner != OWNER)
    assert int(os.environ['MCR_PI_OTHER_NEW']) >= other, 'OTHER does not cover ordinary and accepted peer growth'
    return dict(observedAtEpoch=time.time(), psSnapshotSHA256=hashlib.sha256(json.dumps(rows, sort_keys=True).encode()).hexdigest(),
                owners=sorted(claims), ordinaryActualPids=sorted(ordinary_pids), reservedProcesses=sum(c['processCap'] for c in claims.values()),
                reservedRSSBytes=sum(c['rssCapBytes'] for c in claims.values()), actualPids=sorted(covered), actualRSSBytes=rss, otherNewUpperBytes=other)


def sample(proposed=None):
    """At most three whole censuses; absence never releases a finite claim."""
    last = None
    for attempt in range(3):
        try:
            return _sample_once(proposed)
        except KernelIdentityUnavailable as error:
            # Only a fresh actual PS absence permits another complete census.
            if error.pid in processes():
                raise
            last = error
    raise AssertionError('numeric kernel identity census exhausted after 3: ' + str(last)) from last
