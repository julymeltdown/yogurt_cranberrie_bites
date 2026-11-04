"""Record actual finite native feature allocation after fresh census; starts no validation."""
import datetime, hashlib, json, os, subprocess, sys
from pathlib import Path
root = Path(__file__).resolve().parents[4]
evidence = root / 'docs/mcr/evidence/signup-consent-admission'
ready = json.loads((evidence / 'native-feature-ready.json').read_text())
identifier, expires, other = sys.argv[1], datetime.datetime.fromisoformat(sys.argv[2].replace('Z', '+00:00')), int(sys.argv[3])
assert identifier.startswith('FE10-NATIVE-FEATURE-')
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == ready['head']
assert hashlib.sha256((evidence / 'browser-guard.py').read_bytes()).hexdigest() == ready['guardSha256']
assert not (root / 'frontend/node_modules').exists() and not (root / 'frontend/node_modules').is_symlink()
assert not Path(ready['newScratchOnly']).exists() and not (evidence / 'browser-02').exists()
roots = [Path(p).resolve() for p in ready['custodyRoots']]
size = 208; entries = 4
for declared in roots:
    for base, directories, files in os.walk(declared, followlinks=False):
        for name in directories + files:
            p = Path(base) / name
            try:
                size += p.lstat().st_size; entries += 1
                if p.is_symlink(): assert any(p.resolve().is_relative_to(r) for r in roots), str(p)
            except FileNotFoundError: pass
assert size <= ready['maxWholeBytes'] and entries <= ready['maxWholeEntries']
v = os.statvfs(root); free = v.f_bavail * v.f_frsize
assert free >= 11*1024**3 and free - other - ready['maxBytes'] >= 9*1024**3
now = datetime.datetime.now(datetime.timezone.utc)
assert now < expires, 'Expired offer; not accepted'
lease = {**ready, 'id': identifier, 'accepted': True, 'acceptedAt': now.isoformat(),
    'workDeadline': (now+datetime.timedelta(seconds=300)).isoformat(), 'hardDeadline': (now+datetime.timedelta(seconds=360)).isoformat(),
    'scratch': ready['newScratchOnly'], 'maxRepairRounds': 3, 'measuredFreeBytes': free, 'otherReservationsBytes': other,
    'remainingAfterReservationsBytes': free-other-ready['maxBytes'], 'measuredWholeBytes': max(size,ready['measuredWholeBytes']),
    'measuredWholeEntries': max(entries,ready['measuredWholeEntries'])}
output = evidence / 'browser-02'; output.mkdir(); (output/'offer.json').write_text(json.dumps(lease,indent=2)+'\n')
print(json.dumps({k:lease[k] for k in ['id','head','acceptedAt','workDeadline','hardDeadline','measuredFreeBytes','remainingAfterReservationsBytes','measuredWholeBytes','measuredWholeEntries']}))
