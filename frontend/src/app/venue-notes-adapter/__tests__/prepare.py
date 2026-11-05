"""Factual ordinary preparation receipt; no workload or execution allocation."""
import json
import os
from pathlib import Path
import subprocess
import time

root = Path('/Users/lhs/Desktop/mcr-worktrees/sl-world')
receipt = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a533-50a8-7fb2-b3ed-96b941b96bbf.json')
previous = json.loads(receipt.read_text())
assert previous['state'] == 'FULL'
assert os.statvfs(root).f_bavail * os.statvfs(root).f_frsize >= 11 * 1024**3
identity = dict(pid=os.getpid(), birth=' '.join(subprocess.check_output(['/bin/ps', '-p', str(os.getpid()), '-o', 'lstart='], text=True).split()),
                executable=subprocess.check_output(['/bin/ps', '-p', str(os.getpid()), '-o', 'comm='], text=True).strip(),
                script=str(Path(__file__).resolve()), cwd=str(Path.cwd()), outputRoot=str(root / 'docs/mcr/evidence/venue-notes-adapter'))
record = dict(ownerThreadId=previous['ownerThreadId'], offerId='VENU-NOTES-ADAPTER-PREP32-0255', kind='ordinary', state='ACCEPTED',
              acceptedAtEpoch=time.time(), remainingNewUpperBytes=32 * 1024**2, controller=identity, guardControllers=[],
              previousLiteralFull=previous, phase='Source/helpers/docs/Git only; no Node/tests/browser/materialization')
def publish():
    temporary = receipt.with_suffix('.tmp')
    temporary.write_text(json.dumps(record, indent=2) + '\n'); os.replace(temporary, receipt)
publish()
out = root / 'docs/mcr/evidence/venue-notes-adapter/preparation.json'
out.write_text(json.dumps(dict(book=record['offerId'], acceptedEpoch=record['acceptedAtEpoch'], deadlineUTC='2026-11-05T04:00:00Z',
                              upperBytes=32 * 1024**2, identity=identity, priorPI10='Published8b7/Native12FULL; all MAX/history retained; no reset',
                              scope=['frontend/src/app/venue-notes-adapter/**', 'docs/mcr/frontend/venue-notes-adapter.md', 'docs/mcr/evidence/venue-notes-adapter/**']), indent=2) + '\n')
record.update(state='FULL', endedAtEpoch=time.time(), remainingNewUpperBytes=0, resources=0)
publish()
print(json.dumps(dict(book=record['offerId'], actualOrdinaryFull=record['endedAtEpoch'], resources=0)))
