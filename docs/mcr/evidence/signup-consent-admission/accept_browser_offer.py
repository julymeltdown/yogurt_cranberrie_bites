"""Record one actually authorized, unexpired FE10 browser admission. Starts no workload."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[4]
evidence = root / 'docs/mcr/evidence/signup-consent-admission'
ready = json.loads((evidence / 'browser-ready.json').read_text())
identifier, expiry, other = sys.argv[1], datetime.datetime.fromisoformat(sys.argv[2].replace('Z', '+00:00')), int(sys.argv[3])
assert identifier.startswith('FE10-CONSENT-BROWSER1-')
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == ready['head']
assert hashlib.sha256((evidence / 'guard.py').read_bytes()).hexdigest() == ready['guardSha256']
assert not (root / 'frontend/node_modules').exists() and not (root / 'frontend/node_modules').is_symlink()
scratch = Path(ready['newScratchOnly'])
assert not scratch.exists() and scratch.is_relative_to(root)
assert not (evidence / 'browser-01').exists()
roots = [Path(value) for value in ready['custodyRoots']]
whole, entries = 0, 0
for declared in roots:
    for directory, directories, files in os.walk(declared, followlinks=False):
        for name in directories + files:
            item = Path(directory) / name
            try:
                whole += item.lstat().st_size
                entries += 1
                if item.is_symlink():
                    assert any(item.resolve().is_relative_to(value) for value in roots)
            except FileNotFoundError:
                pass
assert whole <= ready['maxWholeBytes'] and entries <= ready['maxWholeEntries']
disk = os.statvfs(root)
free = disk.f_bavail * disk.f_frsize
own = ready['maxBytes']
assert free >= 12 * 1024**3 and free - other - own >= 9 * 1024**3
now = datetime.datetime.now(datetime.timezone.utc)
assert now < expiry, 'Expired offer: NO acceptance or workload'
lease = {**ready, 'id': identifier, 'accepted': True, 'acceptedAt': now.isoformat(),
         'workDeadline': (now + datetime.timedelta(seconds=300)).isoformat(),
         'hardDeadline': (now + datetime.timedelta(seconds=360)).isoformat(),
         'scratch': str(scratch), 'maxRepairRounds': 3, 'measuredFreeBytes': free,
         'otherReservationsBytes': other, 'ownGrowthReservationBytes': own,
         'remainingAfterReservationsBytes': free - other - own,
         'measuredWholeBytes': max(whole, ready['measuredWholeBytes'], 2881183921),
         'measuredWholeEntries': max(entries, ready['measuredWholeEntries'], 32581),
         'portCustody': 'No prior owned listener; port0 allocated atomically by this job, no borrowed port'}
output = evidence / 'browser-01'
output.mkdir()
(output / 'offer.json').write_text(json.dumps(lease, indent=2) + '\n')
print(json.dumps({key: lease[key] for key in ['id', 'head', 'guardSha256', 'acceptedAt', 'workDeadline', 'hardDeadline',
    'measuredFreeBytes', 'otherReservationsBytes', 'ownGrowthReservationBytes', 'remainingAfterReservationsBytes',
    'measuredWholeBytes', 'measuredWholeEntries']}))
