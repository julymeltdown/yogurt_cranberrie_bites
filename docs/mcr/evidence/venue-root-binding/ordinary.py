"""Bounded ordinary source/Git observation; never starts application workloads."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
OWNER = '01a1a1fe-1eed-7d12-a247-879cd521e0ec'
RECEIPT = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts') / (OWNER + '.json')
DEADLINE = datetime.datetime.fromisoformat('2026-11-05T04:00:00+00:00').timestamp()
assert time.time() < DEADLINE
previous = json.loads(RECEIPT.read_text())
assert previous['state'] == 'FULL' or previous.get('kind') == 'ordinary'
free = __import__('shutil').disk_usage(ROOT).free
assert free - 2684354560 - 16777216 >= 9663676416
row = subprocess.check_output(['ps', '-p', str(os.getpid()), '-o', 'lstart=,comm='], text=True).strip().split(None, 5)
paths = ['frontend/src/app/venue-root', 'frontend/e2e/venue-root-binding', 'docs/mcr/frontend/venue-root-binding.md', 'docs/mcr/evidence/venue-root-binding']
size = sum(p.stat().st_size for name in paths for p in ([ROOT / name] if (ROOT / name).is_file() else (ROOT / name).rglob('*')) if p.is_file())
assert size < 16777216
value = {'ownerThreadId': OWNER, 'offerId': 'VENU-FE-ROOT-BINDING-PREP16-0249', 'state': 'ACCEPTED',
         'kind': 'ordinary', 'executionWorkflow': False, 'acceptedAtEpoch': previous.get('acceptedAtEpoch') if previous.get('offerId') == 'VENU-FE-ROOT-BINDING-PREP16-0249' else time.time(),
         'workDeadlineEpoch': DEADLINE, 'fullDeadlineEpoch': DEADLINE, 'processCap': None, 'rssCapBytes': None,
         'remainingNewUpperBytes': 16777216, 'controller': {'pid': os.getpid(), 'birth': ' '.join(row[:5]),
         'executable': row[5], 'script': str(Path(__file__).resolve()), 'cwd': str(ROOT)},
         'phase': 'SOURCE_AUTHORING', 'role': 'Bounded venue root source preparation; no executable qualification',
         'ordinaryCommandsComplete': True, 'commandExit': 0, 'observedAtEpoch': time.time(),
         'ordinaryBytesObserved': size, 'wholeHistoricalMaximumBytes': 7367959993, 'previousFamilyNewBytes': 224274655,
         'historicalFullPaths': list(dict.fromkeys(previous.get('historicalFullPaths', []) + [str(ROOT / 'docs/mcr/evidence/service-policy-composition/native07/FULL.json')]))}
# Keep every exact immutable execution closure discoverable when returning to
# ordinary authoring; the ordinary observer is never an execution release.
closed = []
for full_path in sorted((ROOT / 'docs/mcr/evidence/venue-root-binding').glob('*/FULL.json')):
    raw = full_path.read_bytes(); record = json.loads(raw)
    assert record['state'] == 'ACTUAL_FULL' and record['remainingOwned'] == []
    offer = record['offer']
    closed.append({'ownerThreadId': OWNER, 'offerId': offer['id'],
        'acceptedAtEpoch': datetime.datetime.fromisoformat(offer['acceptedUTC']).timestamp(),
        'actualFullEpoch': record['fullAtEpoch'], 'fullPath': str(full_path),
        'fullSHA': hashlib.sha256(raw).hexdigest(), 'resourcesReleased': True})
value['closedOffers'] = closed
value['historicalFullPaths'] = list(dict.fromkeys(value['historicalFullPaths'] + [row['fullPath'] for row in closed]))
value['lastExecutionReceipt'] = previous if previous.get('kind') != 'ordinary' else previous.get('lastExecutionReceipt')
temporary = RECEIPT.with_suffix('.tmp')
temporary.write_text(json.dumps(value, indent=2) + '\n'); temporary.replace(RECEIPT)
print(json.dumps({'ordinary': value['offerId'], 'bytes': size, 'free': free, 'applicationWorkloads': 0}))
