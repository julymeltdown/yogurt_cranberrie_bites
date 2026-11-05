"""Validate exact owner-index-attested FULL history; no allocation authority."""
import hashlib
import json
import math



def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, 'duplicate JSON key'
        result[key] = value
    return result


def number(value):
    return type(value) in (int, float) and math.isfinite(value)


def zero(value):
    if value is False or type(value) in (int, float) and value == 0:
        return True
    if isinstance(value, (list, dict)):
        values = value.values() if isinstance(value, dict) else value
        return all(zero(item) for item in values)
    return False


def validate_full(closure, owner, offer, accepted, read_text, now):
    """External exact tuple attests absent packet fields; present fields must agree.

    Hash the retained JSON/JSONL bytes. A standalone event timestamp is never
    substituted for its acceptance epoch or matched by temporal proximity.
    """
    assert closure.get('state') == 'FULL', 'closure is not literal FULL'
    assert closure.get('ownerThreadId', owner) == owner, 'closure owner conflict'
    assert closure.get('offerId') == offer, 'closure offer conflict'
    assert number(accepted) and accepted > 0, 'accepted epoch invalid'
    assert closure.get('acceptedAtEpoch') == accepted, 'closure accepted epoch conflict'
    full = closure.get('actualFullEpoch')
    assert number(full) and accepted <= full <= now, 'literal FULL epoch invalid'
    assert closure.get('resourcesReleased') is True, 'explicit resource-zero attestation missing'
    if 'resources' in closure:
        assert zero(closure['resources']), 'closure resource claim nonzero'
    path = closure.get('fullPath', closure.get('immutableFULLpath'))
    sha = next((closure[k] for k in ('sha256', 'fullSHA256', 'fullSHA', 'fullSha256', 'immutableFULLsha256') if closure.get(k)), None)
    assert isinstance(path, str) and path and isinstance(sha, str) and len(sha) == 64, 'immutable FULL path/hash missing'
    raw = read_text(path)
    assert len(raw.encode()) <= 4 * 1024 * 1024, 'FULL packet bound'
    assert hashlib.sha256(raw.encode()).hexdigest() == sha, 'immutable FULL hash changed'
    try:
        packets = [json.loads(raw, object_pairs_hook=unique_pairs)]
    except json.JSONDecodeError:
        packets = [json.loads(line, object_pairs_hook=unique_pairs) for line in raw.splitlines() if line.strip()]
    assert packets and all(isinstance(p, dict) for p in packets), 'FULL packet must be JSON objects or JSONL'
    identity = {
        'ownerThreadId': owner, 'offerId': offer, 'offer': offer, 'allocation': offer,
        'acceptedAtEpoch': accepted, 'accepted_epoch': accepted,
        'actualFullEpoch': full, 'actual_full_epoch': full, 'fullAtEpoch': full,
    }
    resource_keys = ('workflows', 'children', 'guards', 'target_readers', 'remainingNewUpperBytes', 'remaining_new_bytes', 'cleanup_remaining')
    for packet in packets:
        for key, expected in identity.items():
            if key in packet:
                assert packet[key] == expected, 'FULL packet binding conflict: ' + key
        for key in resource_keys:
            if key in packet:
                assert zero(packet[key]), 'FULL packet resource nonzero: ' + key
        if 'resourcesReleased' in packet:
            assert packet['resourcesReleased'] is True, 'FULL packet resource-release conflict'
        if 'resources' in packet:
            assert zero(packet['resources']), 'FULL packet resources nonzero or ambiguous'
    return {'owner': owner, 'offer': offer, 'acceptedAtEpoch': accepted, 'actualFullEpoch': full, 'path': path, 'sha256': sha, 'binding': 'OWNER_INDEX_ATTESTED_WITH_PRESENT_PACKET_FIELDS_CHECKED'}
