"""Read-only receipt compatibility helpers. Callers retain admission/cleanup policy.

No process discovery, receipt publication, allocation, clock reset or inference of
release from elapsed time, a newer ordinary record, or missing processes.
"""
import copy
import datetime
import hashlib
import json
import math
import posixpath
from pathlib import Path

ACTIVE = frozenset(('ACCEPTED', 'ACCEPTED_RUNNING', 'ACCEPTED_GUARDS_STARTING',
                    'ACTUAL_ACCEPT_NO_FULL', 'ACCEPTED_BOOTSTRAP', 'ACCEPTED_INNER_CENSUS', 'ACTUAL_ACCEPT'))
KINDS = frozenset(('ordinary', 'source', 'browser', 'target', 'copied-native'))


class ReceiptError(ValueError):
    """Safe diagnostic: never include raw command arguments or packet contents."""


def fail(code, **context):
    allowed = ('owner', 'offer', 'pid', 'birth', 'expectedScript',
               'observedExecutable', 'commandSHA256')
    safe = {k: v for k, v in context.items() if k in allowed}
    digest = safe.get('commandSHA256')
    if not isinstance(digest, str) or len(digest) != 64 or any(ch not in '0123456789abcdef' for ch in digest):
        safe.pop('commandSHA256', None)
    raise ReceiptError(json.dumps(dict(code=code, **safe), sort_keys=True))


def number(value, field, *, integer=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        fail('invalid-' + field)
    if not math.isfinite(value) or value < 0 or (integer and int(value) != value):
        fail('invalid-' + field)
    return value


def epoch(value):
    return number(value, 'epoch')


def key(record):
    """Exact nonempty owner/offer/acceptance tuple; None never matches None."""
    owner = record.get('ownerThreadId', record.get('owner'))
    offer = record.get('offerId', record.get('cohort'))
    accepted = record.get('acceptedAtEpoch', record.get('actualAccept'))
    if not isinstance(owner, str) or not owner or not isinstance(offer, str) or not offer:
        fail('missing-claim-key')
    return owner, offer, epoch(accepted)


def normalize(raw):
    """Preserve producer facts; classify ordinary before execution arithmetic."""
    if not isinstance(raw, dict):
        fail('record-not-object')
    r = copy.deepcopy(raw)
    owner = r.get('ownerThreadId', r.get('owner'))
    if not isinstance(owner, str) or not owner:
        fail('missing-owner')
    r['ownerThreadId'] = owner
    state, kind = r.get('state'), r.get('kind')
    if state is None and isinstance(r.get('ordinaryObservation'), dict):
        if kind not in (None, 'ordinary'):
            fail('observation-without-workload-state', owner=owner)
        r['_observationOnly'] = True
        r['_active'] = False
        return r
    if state not in ACTIVE | {'FULL', 'DECLINED'}:
        fail('unknown-state', owner=owner)
    if kind not in KINDS:
        fail('unknown-kind', owner=owner)
    r['_active'] = state in ACTIVE and kind != 'ordinary'
    if r['_active']:
        _, r['offerId'], r['acceptedAtEpoch'] = key(r)
        if 'rssCapBytes' not in r and 'rssCapGiB' in r:
            r['rssCapBytes'] = number(r['rssCapGiB'], 'rssCapGiB') * 1024**3
        if 'remainingNewUpperBytes' not in r:
            r['remainingNewUpperBytes'] = r.get('remainingNewConservativeUpperBytes')
        for field in ('processCap', 'rssCapBytes', 'remainingNewUpperBytes'):
            number(r.get(field), field, integer=True)
        for field in ('workDeadlineEpoch', 'fullDeadlineEpoch'):
            if field in r and r[field] is not None:
                if epoch(r[field]) < r['acceptedAtEpoch']:
                    fail('deadline-before-acceptance', owner=owner)
    return r


def load_receipts(directory, *, max_files=128, max_bytes=65536):
    """Bound actual reads, reject duplicate JSON keys and owner/file mismatch."""
    files = sorted(Path(directory).glob('*.json'))
    if len(files) > max_files:
        fail('too-many-receipts')
    result = []
    for path in files:
        with path.open('rb') as stream:
            data = stream.read(max_bytes + 1)
        if len(data) > max_bytes:
            fail('oversized-receipt')
        r = decode(data)
        if not isinstance(r, dict) or r.get('ownerThreadId') != path.stem:
            fail('owner-file-mismatch')
        result.append(normalize(r))
    return result


def decode(data):
    def pairs(items):
        value = {}
        for name, item in items:
            if name in value:
                fail('duplicate-json-key')
            value[name] = item
        return value
    try:
        return json.loads(data, object_pairs_hook=pairs,
                          parse_constant=lambda _: fail('nonfinite-json'))
    except (ValueError, UnicodeError) as error:
        if isinstance(error, ReceiptError):
            raise
        fail('malformed-json')


def resources_zero(value):
    """Explicit zero resources, including allocator's boolean release form."""
    if value is True:
        return True
    if value is False or value is None:
        return False
    if isinstance(value, (int, float)):
        return math.isfinite(value) and value == 0
    if isinstance(value, list):
        return all(x is not True and resources_zero(x) for x in value)
    if isinstance(value, dict):
        return all(x is not True and resources_zero(x) for x in value.values())
    return False


def read_packet(path, maximum):
    with Path(path).open('rb') as stream:
        return stream.read(maximum + 1)


def verify_full(binding, *, reader=read_packet, max_bytes=4 * 1024**2,
                adapter=None):
    """Verify one exact immutable closure. No claim is released on read failure.

    Default JSON requires literal owner/offer/acceptance/FULL/resource-zero facts.
    JSONL or producer-specific envelopes require a caller's explicit pure adapter:
    adapter(decoded_records, binding) -> normalized closure dictionary. It must
    document where each exact identity/epoch/resource fact originates. This API
    never substitutes event time for actual bootstrap acceptance time.
    """
    wanted = key(binding)
    for aliases in (('fullPath', 'immutableFULLpath'), ('fullSHA', 'fullSha256', 'immutableFULLsha256')):
        values = [binding[name] for name in aliases if name in binding]
        if values and any(value != values[0] for value in values):
            fail('conflicting-full-aliases')
    path = binding.get('fullPath', binding.get('immutableFULLpath'))
    sha = binding.get('fullSHA', binding.get('fullSha256', binding.get('immutableFULLsha256')))
    if not isinstance(path, str) or not posixpath.isabs(path):
        fail('invalid-full-path')
    if not isinstance(sha, str) or len(sha) != 64 or any(c not in '0123456789abcdef' for c in sha):
        fail('missing-full-sha')
    try:
        data = reader(path, max_bytes)
    except (OSError, ValueError):
        fail('unreadable-full-packet')
    if not isinstance(data, bytes) or len(data) > max_bytes:
        fail('oversized-full-packet')
    if hashlib.sha256(data).hexdigest() != sha:
        fail('full-sha-mismatch')
    if path.endswith('.jsonl'):
        records = [decode(line) for line in data.splitlines() if line.strip()]
        if not records or not all(isinstance(r, dict) for r in records):
            fail('invalid-jsonl-packet')
        if adapter is None:
            fail('jsonl-binding-adapter-required')
    else:
        records = decode(data)
    closure = adapter(records, copy.deepcopy(binding)) if adapter else records
    if not isinstance(closure, dict) or key(closure) != wanted:
        fail('full-binding-mismatch')
    if closure.get('state') != 'FULL':
        fail('not-literal-full')
    resource_fields = [closure[f] for f in ('resourcesReleased', 'resources') if f in closure]
    if not resource_fields or not all(resources_zero(value) for value in resource_fields):
        fail('resources-not-zero')
    actual = epoch(closure.get('actualFullEpoch'))
    if actual < wanted[2] or actual != epoch(binding.get('actualFullEpoch')):
        fail('full-epoch-mismatch')
    return dict(ownerThreadId=wanted[0], offerId=wanted[1], acceptedAtEpoch=wanted[2],
                actualFullEpoch=actual, fullPath=path, fullSHA=sha,
                proofKind='PACKET_INDEPENDENT')



def verify_attested_full(binding, owner_index_bytes, *, reader=read_packet,
                         max_bytes=4 * 1024**2, adapter=None):
    """Allocator-authorized sole-owner index attestation, explicitly distinguished.

    Immutable packet hash + exact owner-index history tuple authorize omitted
    fields. Never override conflicting packet fields. Noncanonical formats need
    a pure adapter returning semantic facts, not fabricated producer identifiers.
    """
    wanted = key(binding)
    if not isinstance(owner_index_bytes, bytes) or len(owner_index_bytes) > 65536:
        fail('bounded-owner-index-required')
    index = decode(owner_index_bytes)
    if not isinstance(index, dict) or index.get('ownerThreadId') != wanted[0]:
        fail('attestation-owner-mismatch')
    histories = []
    for name in ('closedReceipts', 'closedOffers', 'closedHistory'):
        value = index.get(name, [])
        if not isinstance(value, list):
            fail('invalid-closed-history')
        histories.extend(value)
    if binding not in histories:
        fail('attestation-tuple-not-in-index')
    if not resources_zero(binding.get('resourcesReleased', binding.get('resources'))):
        fail('attestation-resources-not-zero')

    def map_packet(records, exact_binding):
        if adapter:
            facts = adapter(records, exact_binding)
        elif isinstance(records, list):
            # Documented ART event-envelope format only. ACCEPT.at is logging
            # time, not bootstrap acceptance. No proximity comparison is made.
            accepts = [r for r in records if r.get('event') == 'ACCEPT']
            fulls = [r for r in records if r.get('event') == 'FULL']
            if len(accepts) != 1 or len(fulls) != 1:
                fail('ambiguous-jsonl-events')
            if accepts[0].get('offer') != wanted[1]:
                fail('jsonl-offer-mismatch')
            f = fulls[0]
            if not all(f.get(k) is True for k in ('allChildrenExited', 'resourceGuardianExited', 'tempAbsent')):
                fail('jsonl-cleanup-not-literal')
            try:
                actual = datetime.datetime.fromisoformat(f['at'].replace('Z', '+00:00')).timestamp()
            except (KeyError, ValueError, AttributeError):
                fail('invalid-jsonl-full-time')
            facts = dict(f, state='FULL', actualFullEpoch=actual)
            # Explicit semantic fields in ANY event must agree, even if the
            # default event envelope usually omits them.
            for event in records:
                for field, expected in zip(('ownerThreadId', 'offerId', 'acceptedAtEpoch'), wanted):
                    if field in event and event[field] != expected:
                        fail('attested-packet-field-conflict')
        else:
            facts = records
        if not isinstance(facts, dict) or facts.get('state') != 'FULL':
            fail('attested-format-adapter-required')
        for field, expected in zip(('ownerThreadId', 'offerId', 'acceptedAtEpoch'), wanted):
            if field in facts and facts[field] != expected:
                fail('attested-packet-field-conflict')
        if 'actualFullEpoch' in facts and facts['actualFullEpoch'] != exact_binding['actualFullEpoch']:
            fail('attested-packet-full-conflict')
        for field in ('resourcesReleased', 'resources'):
            if field in facts and not resources_zero(facts[field]):
                fail('attested-packet-resource-conflict')
        return dict(facts, ownerThreadId=wanted[0], offerId=wanted[1],
                    acceptedAtEpoch=wanted[2], actualFullEpoch=exact_binding['actualFullEpoch'],
                    resourcesReleased=True)

    proof = verify_full(binding, reader=reader, max_bytes=max_bytes, adapter=map_packet)
    proof['proofKind'] = 'OWNER_INDEX_ATTESTED'
    proof['ownerIndexSHA256'] = hashlib.sha256(owner_index_bytes).hexdigest()
    return proof


def reconcile(current, legacy=(), *, previous=(), reader=read_packet, adapter=None,
              owner_index_bytes=None):
    """Return all unclosed reservations, ordinary observations and verified FULLs.

    Keep distinct same-owner claims (charge both). Never use ordering, current
    ordinary/FULL alone, absent PIDs, or equal missing offers to retire legacy.
    Caller retains previous observations across samples to detect clock resets.
    """
    records = [normalize(r) for r in current]
    if len({r['ownerThreadId'] for r in records}) != len(records):
        fail('duplicate-owner')
    prior = {key(r): normalize(r) for r in previous if normalize(r)['_active']}
    claims = {}
    for r in list(prior.values()) + [normalize(x) for x in legacy] + records:
        if not r['_active']:
            continue
        k = key(r)
        for oldkey in prior:
            if oldkey[:2] == k[:2] and oldkey[2] != k[2]:
                fail('same-offer-acceptance-reset', owner=k[0], offer=k[1])
        if k in prior:
            for clock in ('workDeadlineEpoch', 'fullDeadlineEpoch'):
                if r.get(clock) != prior[k].get(clock):
                    fail('same-offer-deadline-reset', owner=k[0], offer=k[1])
        if any(old[:2] == k[:2] and old[2] != k[2] for old in claims):
            fail('same-offer-acceptance-reset', owner=k[0], offer=k[1])
        if k in claims:
            if r['kind'] != claims[k]['kind']:
                fail('same-offer-kind-reset', owner=k[0], offer=k[1])
            # Current descriptors may improve identity, never reduce reservations.
            r = dict(r, **{f: max(r[f], claims[k][f]) for f in
                          ('processCap', 'rssCapBytes', 'remainingNewUpperBytes')})
        claims[k] = r
    verified = []
    for r in records:
        histories = []
        for name in ('closedReceipts', 'closedOffers', 'closedHistory'):
            items = r.get(name, [])
            if not isinstance(items, list):
                fail('invalid-closed-history', owner=r['ownerThreadId'])
            histories.extend(items)
        # Validate only history relevant to retained execution claims. Ordinary
        # history with unknown acceptance cannot release an execution workload.
        for h in histories:
            if not isinstance(h, dict):
                fail('invalid-closed-history', owner=r['ownerThreadId'])
            try:
                k = key(h)
            except ReceiptError:
                continue
            if k not in claims:
                continue
            if k[0] != r['ownerThreadId']:
                fail('cross-owner-closure')
            if owner_index_bytes is not None:
                snapshot = owner_index_bytes.get(k[0])
                proof = verify_attested_full(h, snapshot, reader=reader, adapter=adapter)
            else:
                proof = verify_full(h, reader=reader, adapter=adapter)
            verified.append(proof)
            del claims[k]
    return dict(claims=list(claims.values()), records=records, verifiedFull=verified)


def descendants(rows, roots):
    tree = set(roots)
    while True:
        expanded = tree | {pid for pid, row in rows.items() if row['parent'] in tree}
        if expanded == tree:
            return tree
        tree = expanded


def birth(value):
    return ' '.join(str(value).split())


def identity(control, row, owner, *, prior_verified=None, claim_key=None,
             snapshot_sha=None, ordinary=False):
    """Validate measured comm, birth, cwd and argv; never resolve launcher aliases."""
    context = dict(owner=owner, pid=control.get('pid', control.get('actualpid')),
                   birth=row.get('birth'), expectedScript=control.get('script'),
                   observedExecutable=row.get('executable'),
                   commandSHA256=row.get('commandDigest'))
    if not control.get('birth') or birth(control['birth']) != birth(row.get('birth')):
        fail('pid-birth-mismatch', **context)
    descriptor_sha = hashlib.sha256(json.dumps(control, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    if str(row.get('state', '')).startswith('Z'):
        prior = prior_verified or {}
        if (prior.get('descriptorSHA256') != descriptor_sha or
                prior.get('pid') != context['pid'] or
                prior.get('birth') != birth(row.get('birth')) or
                prior.get('parent') != row.get('parent') or
                prior.get('liveVerified') is not True or
                tuple(prior.get('ownerClaim') or ()) != tuple(claim_key or ()) or not prior.get('snapshotSHA256')):
            fail('unverified-terminal-identity', **context)
        return dict(prior, terminal=True)
    if not isinstance(row.get('executable'), str) or not posixpath.isabs(row['executable']):
        fail('measured-executable-required', **context)
    executable = control.get('executable')
    digest_bound_ordinary = ordinary and not executable and control.get('commandDigest') and control['commandDigest'] == row.get('commandDigest')
    if not digest_bound_ordinary and (not executable or not posixpath.isabs(executable) or executable != row.get('executable')):
        fail('actual-executable-mismatch', **context)
    cwd = control.get('cwd', control.get('workingDirectory'))
    if not cwd or cwd != row.get('cwd'):
        fail('actual-cwd-mismatch', **context)
    argv = row.get('argv')
    if not isinstance(argv, list) or not argv or not all(isinstance(a, str) for a in argv):
        fail('actual-argv-required', **context)
    digest = hashlib.sha256(json.dumps(argv, separators=(',', ':')).encode()).hexdigest()
    # commandDigest can instead be compared by the adapter using its exact raw
    # PS bytes. argvDigest is this library's explicit unambiguous convention.
    if control.get('argvDigest') and control['argvDigest'] != digest:
        fail('argv-digest-mismatch', **context)
    script = control.get('script')
    if script:
        if not posixpath.isabs(script):
            fail('expected-script-not-absolute', **context)
        args = list(argv[1:])
        while args and args[0] in ('-B', '-u', '-I', '-E', '-s', '-S', '-O', '-OO'):
            args.pop(0)
        program = args[0] if args and not args[0].startswith('-') else None
        actual_script = posixpath.normpath(program if posixpath.isabs(program) else posixpath.join(cwd, program)) if program else None
        if posixpath.normpath(script) != actual_script:
            fail('actual-script-mismatch', **context)
    elif not control.get('argvDigest'):
        if not control.get('commandDigest') or control['commandDigest'] != row.get('commandDigest'):
            fail('exact-command-digest-required', **context)
    return dict(pid=context['pid'], birth=birth(row['birth']), parent=row['parent'],
                descriptorSHA256=descriptor_sha, terminal=False, liveVerified=True,
                ownerClaim=claim_key, snapshotSHA256=snapshot_sha)


def project(reconciled, rows, *, unknown_candidates, own_owner=None,
            ordinary_future_bytes=2684354560, infrastructure=(), previous_identities=None):
    """One caller-supplied fresh snapshot; candidates are mandatory, never guessed.

    Rows: integer PID -> parent,birth,rss,actual executable,cwd,argv,commandDigest.
    The platform adapter must supply ALL relevant unknown roots (including shells),
    not merely runtimes matching a basename. This is not a universal OS classifier.
    """
    for pid, row in rows.items():
        number(pid, 'pid', integer=True)
        number(row.get('rss'), 'rss', integer=True)
    covered, ordinary, owners = set(), set(), {}
    snapshot_sha = hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    verified = {}
    previous_identities = previous_identities or {}
    totals = dict(globalWorkflows=0, source=0, browser=0, target=0,
                  reservedProcesses=0, reservedRSSBytes=0,
                  otherNewUpperBytes=number(ordinary_future_bytes, 'ordinary-future', integer=True))
    for claim in reconciled['claims']:
        owner = claim['ownerThreadId']
        totals['globalWorkflows'] += 1
        lane = 'source' if claim['kind'] == 'copied-native' else claim['kind']
        totals[lane] += 1
        totals['reservedProcesses'] += claim['processCap']
        totals['reservedRSSBytes'] += claim['rssCapBytes']
        if owner != own_owner:
            totals['otherNewUpperBytes'] += claim['remainingNewUpperBytes']
        roots = set()
        controls = [claim.get('controller')] + claim.get('guardControllers', [])
        for control in controls:
            if not control:
                continue
            pid = control.get('pid')
            if pid in rows:
                verified[pid] = identity(control, rows[pid], owner, prior_verified=previous_identities.get(pid, previous_identities.get(str(pid))), claim_key=key(claim), snapshot_sha=snapshot_sha)
                roots.add(pid)
        tree = descendants(rows, roots)
        if len(tree) > claim['processCap'] or sum(rows[p]['rss'] for p in tree) > claim['rssCapBytes']:
            fail('actual-workflow-cap', owner=owner, offer=claim['offerId'])
        for pid in tree:
            if pid in owners and owners[pid] != key(claim):
                fail('ambiguous-process-ownership', owner=owner, pid=pid)
            owners[pid] = key(claim)
        covered.update(tree)
    for record in reconciled['records']:
        controls = []
        if record.get('kind') == 'ordinary' and record.get('controller'):
            controls.append(record['controller'])
        if record.get('ordinaryObservation'):
            observation = record['ordinaryObservation']
            # A finished observation can still identify its factual final writer;
            # PID+birth/digest must be live exact, never a new/reused process.
            controls.append(dict(observation, pid=observation.get('actualpid', observation.get('pid'))))
        for control in controls:
            pid = control.get('pid')
            if pid not in rows or birth(control.get('birth')) != birth(rows[pid].get('birth')):
                continue
            if not control.get('role'):
                fail('ordinary-role-required', owner=record['ownerThreadId'], pid=pid)
            verified[pid] = identity(control, rows[pid], record['ownerThreadId'], prior_verified=previous_identities.get(pid, previous_identities.get(str(pid))), claim_key=(record['ownerThreadId'], 'ordinary', control['role']), snapshot_sha=snapshot_sha, ordinary=True)
            tree = descendants(rows, {pid})
            for child in tree:
                row = rows[child]
                # Adapter factual roles are required for EVERY ordinary process.
                # No cwd, basename, null-controller or parent-shell exemption.
                exe = posixpath.basename(row.get('executable', '')).lower()
                argv = row.get('argv', [])
                heavy = exe in ('node', 'cargo', 'rustc', 'pytest', 'jest', 'vitest', 'cc', 'gcc', 'clang', 'clang++', 'swift', 'swiftc', 'make', 'cmake') or any(a in ('--headless', '--headless=new', '-m') for a in argv[1:])
                script_child = child != pid and any(a.endswith(('.py', '.js', '.mjs', '.cjs')) for a in argv[1:])
                if heavy or script_child or row.get('resourceRole') != 'ordinary':
                    fail('ordinary-covers-unclassified-payload', pid=child)
                if child in owners:
                    fail('ordinary-execution-overlap', pid=child)
            ordinary.update(tree)
    covered.update(ordinary)
    for control in infrastructure:
        pid = control.get('pid')
        if pid in rows:
            verified[pid] = identity(control, rows[pid], 'infrastructure', prior_verified=previous_identities.get(pid, previous_identities.get(str(pid))), claim_key=('infrastructure', control.get('role')), snapshot_sha=snapshot_sha)
            # Only exact factual infrastructure descriptors are exempt. Its
            # unregistered children remain candidates, never an inherited pass.
            covered.add(pid)
    unresolved = set(unknown_candidates) - covered
    if unresolved:
        pid = min(unresolved)
        fail('unresolved-process', pid=pid, birth=rows.get(pid, {}).get('birth'),
             observedExecutable=rows.get(pid, {}).get('executable'),
             commandSHA256=rows.get(pid, {}).get('commandDigest'))
    return dict(totals, psSnapshotSHA256=snapshot_sha, verifiedIdentities=verified, actualPids=sorted(covered), ordinaryActualPids=sorted(ordinary),
                actualRSSBytes=sum(rows[p]['rss'] for p in covered),
                owners=sorted({c['ownerThreadId'] for c in reconciled['claims']}))


def check_limits(projection, limits):
    """Optional arithmetic only; use current offer/policy, never historical caps."""
    for field, ceiling in limits.items():
        number(ceiling, 'limit', integer=True)
        value = len(projection[field]) if isinstance(projection[field], list) else projection[field]
        if value > ceiling:
            fail('limit-' + field)
    return projection
