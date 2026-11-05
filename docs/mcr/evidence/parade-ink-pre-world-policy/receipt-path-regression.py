"""Adversarial literal-history decoder checks; run only inside an accepted lease."""
import copy
import json
import os
from pathlib import Path
import runpy

scratch = Path(os.environ['MCR_PI12_PREWORLD_TEMP'])
assert scratch.is_dir() and str(scratch).startswith('/private/tmp/mcr-pi12-preworld-')
receipts = scratch / 'decoder-fixtures'
receipts.mkdir()
owner = '11111111-1111-1111-1111-111111111111'
literal = receipts / 'literal-FULL'
record_path = receipts / (owner + '.json')
payload = dict(state='ACTUAL_FULL', remainingOwned=[], dependencyLinkAbsent=True,
    privateTempAbsent=True, browserServerTargetResources=0,
    offer=dict(id='OLD-EXACT', acceptedUTC='2026-11-05T01:00:00+00:00'))
record = dict(ownerThreadId=owner, state='FULL', offerId='LATEST', acceptedAtEpoch=1793844000,
    receiptPaths=dict(full=str(literal)), closedHistory=[str(literal)])
record_path.write_text(json.dumps(record))
metadata = dict(asOfUTC='2026-11-05T02:00:00Z', entries=[
    dict(owner=owner, offerId='OLD-EXACT', acceptedAtEpoch=1793840400),
    dict(owner=owner, offerId='DIFFERENT-ROUND', acceptedAtEpoch=1793840401),
    dict(owner='22222222-2222-2222-2222-222222222222', offerId='OLD-EXACT', acceptedAtEpoch=1793840400)])
reader = runpy.run_path(str(Path(__file__).with_name('guardian.py')))['membership_entries']
reader.__globals__['RECEIPTS'] = receipts
literal.write_text(json.dumps(payload))
entries, pins, ordinary = reader(metadata)
assert len(entries) == 2 and entries[0]['offerId'] == 'DIFFERENT-ROUND'
assert entries[1]['owner'] != owner and len(pins) == 1 and ordinary == []
for patch in [dict(remainingOwned=[123]), dict(dependencyLinkAbsent=False),
              dict(browserServerTargetResources=1), dict(state='ACCEPTED'),
              dict(ownerThreadId='22222222-2222-2222-2222-222222222222')]:
    rejected = copy.deepcopy(payload)
    rejected.update(patch)
    literal.write_text(json.dumps(rejected))
    try:
        reader(metadata)
    except AssertionError:
        pass
    else:
        raise AssertionError(('unsafe literal accepted', patch))
literal.write_text(json.dumps(payload))
print(json.dumps(dict(result='PASS', cases=6,
    behavior='Exact owner/offer closure only; unrelated round/owner retained; five unsafe literal receipts rejected')))
