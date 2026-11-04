"""Accept a fresh explicitly allocated paired browser turn; starts no validation/server."""
import datetime, hashlib, json, os, subprocess, sys
from pathlib import Path
root = Path(__file__).resolve().parents[4]
evidence = root / 'docs/mcr/evidence/signup-consent-admission'
ready = json.loads((evidence / 'main-browser-ready.json').read_text())
identifier, expires, other = sys.argv[1], datetime.datetime.fromisoformat(sys.argv[2].replace('Z', '+00:00')), int(sys.argv[3])
assert identifier.startswith('FE10-ACTUAL-MAIN-') and expires.tzinfo is not None
assert other >= 0
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == ready['head']
for relative, digest in ready['controllerAndSourceSha256'].items():
    assert hashlib.sha256((root / relative).read_bytes()).hexdigest() == digest, relative
assert not (root / 'frontend/node_modules').exists() and not (root / 'frontend/node_modules').is_symlink()
assert not Path(ready['newScratchOnly']).exists() and not (evidence / 'browser-main-01').exists()
backend = Path(ready['pairedBackendOutput'])
assert backend == Path('/private/tmp/mcr-enrollment207-pair-4mnomt5v')
serving = json.loads((backend / 'browser-ready.json').read_text())
assert serving['sourceRevision'] == ready['backendSource'] and serving['binarySha256'] == ready['backendBinarySha256']
assert serving['authOnly'] is True and serving['syntheticPolicyOnly'] is True
assert serving['frontendOrigin'] == 'http://localhost:3000'
assert serving['verdict'] == 'SERVING_FIXTURE_ONLY_BROWSER_ACCEPTANCE_REMAINS_FE_OWNED'
assert not any((backend / name).exists() for name in ['browser-expire.json', 'browser-rotate.json', 'browser-stop'])
roots = [Path(p).resolve() for p in ready['custodyRoots']]
size = 208; entries = 4
for declared in roots:
    for base, directories, files in os.walk(declared, followlinks=False):
        for name in directories + files:
            item = Path(base) / name
            try:
                size += item.lstat().st_size; entries += 1
                if item.is_symlink(): assert any(item.resolve().is_relative_to(r) for r in roots), str(item)
            except FileNotFoundError: pass
assert size <= ready['maxWholeBytes'] and entries <= ready['maxWholeEntries']
v = os.statvfs(root); free = v.f_bavail * v.f_frsize
assert free >= 11 * 1024**3 and free - other - ready['maxBytes'] >= 9 * 1024**3
now = datetime.datetime.now(datetime.timezone.utc)
assert now < expires, 'Expired offer; not accepted'
lease = {**ready, 'id': identifier, 'accepted': True, 'acceptedAt': now.isoformat(),
    'workDeadline': (now + datetime.timedelta(seconds=300)).isoformat(), 'hardDeadline': (now + datetime.timedelta(seconds=360)).isoformat(),
    'scratch': ready['newScratchOnly'], 'maxRepairRounds': 2, 'measuredFreeBytes': free, 'otherReservationsBytes': other,
    'remainingAfterReservationsBytes': free - other - ready['maxBytes'],
    'measuredWholeBytes': max(size, ready['measuredWholeBytes']), 'measuredWholeEntries': max(entries, ready['measuredWholeEntries'])}
output = evidence / 'browser-main-01'; output.mkdir(); (output / 'offer.json').write_text(json.dumps(lease, indent=2) + '\n')
print(json.dumps({k: lease[k] for k in ['id','head','acceptedAt','workDeadline','hardDeadline','measuredFreeBytes','remainingAfterReservationsBytes','measuredWholeBytes','measuredWholeEntries']}))
