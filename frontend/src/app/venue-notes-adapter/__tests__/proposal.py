"""Ordinary bounded metadata/census only; this does not accept an execution offer."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
ROOT = Path(__file__).resolve().parents[5]
HERE = Path(__file__).resolve().parent
receipt = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a533-50a8-7fb2-b3ed-96b941b96bbf.json')
previous = json.loads(receipt.read_text()); assert previous['state'] == 'FULL'
identity = dict(pid=os.getpid(), birth=' '.join(subprocess.check_output(['/bin/ps', '-p', str(os.getpid()), '-o', 'lstart='], text=True).split()),
                executable=subprocess.check_output(['/bin/ps', '-p', str(os.getpid()), '-o', 'comm='], text=True).strip(),
                script=str(Path(__file__).resolve()), cwd=str(Path.cwd()), outputRoot=str(ROOT / 'docs/mcr/evidence/venue-notes-adapter'))
record = dict(ownerThreadId=previous['ownerThreadId'], offerId='VENU-NOTES-ADAPTER-PREP32-0255', kind='ordinary', state='ACCEPTED',
              acceptedAtEpoch=time.time(), remainingNewUpperBytes=32 * 1024**2, controller=identity, guardControllers=[], previousLiteralFull=previous)
def publish():
    temporary = receipt.with_suffix('.tmp'); temporary.write_text(json.dumps(record, indent=2) + '\n'); os.replace(temporary, receipt)
publish()
sys.path.insert(0, str(ROOT / 'frontend/src/features/auth/__tests__/parade-ink-entry'))
from metrics import CARRY, inventory, free
old = json.loads(Path('/private/tmp/mcr-pi10-native12-0221/offer.json').read_text())
retained = old['retained_roots'] + [old['control']]
carry = max(5558930355, sum(inventory(CARRY + [Path(p) for p in retained] + [receipt]).values()))
cfg = {key: old[key] for key in ('membership_path', 'literal_releases', 'ordinary_git_only', 'factual_control_packet_owners', 'receipt_directory', 'owner_thread', 'owner_receipt', 'node_sha')}
cfg.update(lease='VENU-NOTES-ADAPTER-SOURCE01-PROPOSED', allocator_offer=None, mode='source',
           cwd=str(ROOT), source=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
           control='/private/tmp/mcr-venue-notes-source01', temporary='/private/tmp/mcr-venue-notes-source01/scratch',
           output=str(ROOT / 'docs/mcr/evidence/venue-notes-adapter/source01'),
           readset='docs/mcr/evidence/venue-notes-adapter/readset.json',
           readset_sha=hashlib.sha256((ROOT / 'docs/mcr/evidence/venue-notes-adapter/readset.json').read_bytes()).hexdigest(),
           start_floor=11 * 1024**3, other_new=int(2.5 * 1024**3), prior_whole=carry, whole_cap=8 * 1024**3,
           historical_new_charge=534049536, new_cap=534049536 + 128 * 1024**2,
           remaining_new_reservation=128 * 1024**2, process_cap=16, rss_cap=int(2.5 * 1024**3),
           retained_roots=retained, own_cohort_names=['VENU-NOTES-ADAPTER-SOURCE01-PROPOSED'], additional_cohorts=[],
           accept_by_epoch=0, proposedFreeBytes=free(), pins={name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
                                                            for name in ('round.py', 'vitest.config.mjs')})
path = ROOT / 'docs/mcr/evidence/venue-notes-adapter/source01-proposal.json'
path.write_text(json.dumps(cfg, indent=2) + '\n')
record.update(state='FULL', remainingNewUpperBytes=0, endedAtEpoch=time.time(), resources=0); publish()
print(json.dumps(dict(proposal=str(path), source=cfg['source'], carry=carry, free=free(), pins=cfg['pins'], readset=cfg['readset_sha'], ordinaryFull=record['endedAtEpoch'])))
