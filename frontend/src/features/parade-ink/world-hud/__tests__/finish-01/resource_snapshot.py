"""Consistent observations around the unchanged, strict readonly owner sampler."""
import hashlib
import importlib.util
import json
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[7]
_SOURCE = _ROOT / 'frontend/src/features/gift-discovery/__tests__/parade-ink/membership.py'
_spec = importlib.util.spec_from_file_location('pi08_readonly_membership', _SOURCE)
_reader = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_reader)
publish_census = _reader.publish_census


def receipt_binding(record):
    # All top-level inputs consumed by the readonly sampler, plus explicit
    # lifecycle/custody facts. Queue/history/publication prose is not authority.
    fields = ('ownerThreadId', 'offerId', 'state', 'kind', 'acceptedAtEpoch',
              'workDeadlineEpoch', 'fullDeadlineEpoch', 'actualFullEpoch',
              'processCap', 'rssCapBytes', 'remainingNewUpperBytes', 'resources',
              'controller', 'guardControllers', 'closedOffers', 'ordinaryObservation')
    result = {key: record[key] for key in fields if key in record}
    if isinstance(result.get('ordinaryObservation'), dict):
        # Preserve PID/birth/image/script/cwd/role/book/ended/exit facts. The
        # timestamp of repeating the same observation does not change custody.
        result['ordinaryObservation'] = {key: value for key, value in result['ordinaryObservation'].items()
                                         if key not in ('observedAt', 'publishedAtEpoch', 'lastSeenAt')}
    return result


def receipt_revision(directory=None, extra_files=None):
    directory = _reader.DIRECTORY if directory is None else directory
    files = sorted(directory.glob('*.json'))
    assert len(files) <= 128, 'Unbounded resource receipt set'
    extras = [_reader.BASE / 'resource-receipt-interface.json', _reader.BASE / 'resource-membership-current.json'] if extra_files is None else extra_files
    stamps = []
    for file, owner_record in [(file, True) for file in files] + [(file, False) for file in extras]:
        try:
            raw = file.read_bytes()
        except FileNotFoundError:
            stamps.append((str(file), 'ABSENT_DURING_READ'))
            continue
        try:
            value = json.loads(raw)
        except json.JSONDecodeError:
            # This is only a revision marker. The full strict read must still
            # reject stable malformed records; no invalid record is admitted.
            value = {'invalidJSONSHA256': hashlib.sha256(raw).hexdigest()}
        else:
            if owner_record:
                value = receipt_binding(value)
        stamps.append((str(file), _digest(value)))
    return tuple(stamps)


def _digest(revision):
    return hashlib.sha256(json.dumps(revision, sort_keys=True).encode()).hexdigest()


def consistent_sample(read_sample, revision, attempts=3):
    """Never waive a mismatch: a changed receipt set requires another complete census."""
    assert 1 <= attempts <= 3
    changes = []
    for number in range(1, attempts + 1):
        before = revision()
        failure = None
        try:
            value = read_sample()
        except (AssertionError, json.JSONDecodeError, FileNotFoundError) as error:
            failure = error
        after = revision()
        if before == after:
            if failure is not None:
                raise failure
            return dict(value, receiptReadConsistency={
                'attempts': number, 'stableRevisionSHA256': _digest(after),
                'unstableObservations': changes,
                'boundary': 'Every returned claim/cap/PID/future passed the unchanged strict reader against a stable receipt revision; no reservation released from absence',
            })
        changes.append({'attempt': number, 'beforeSHA256': _digest(before),
                        'afterSHA256': _digest(after), 'strictObservation': str(failure) if failure else 'PASS_BUT_RECEIPT_SET_CHANGED'})
    raise RuntimeError('Resource receipts changed across all bounded full censuses: ' + json.dumps(changes))


def sample(proposed=None):
    return consistent_sample(lambda: _reader.sample(proposed=proposed), receipt_revision)
