"""Publish the actual bootstrap identity before expensive native census."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

proposal = Path(sys.argv[1]).resolve()
cfg = json.loads(proposal.read_text())
folder = Path(cfg['control'])
assert Path.cwd() == Path(cfg['cwd'])
assert not (folder / 'offer.json').exists(), 'Finite round cannot restart'
assert time.time() < cfg['accept_by_epoch']
GIB = 1024**3
# Bounded, cheap declaration/floor checks precede actual acceptance. Complete
# current process/carry sampling follows publication and precedes workload.
management = json.loads(Path(cfg['membership_path']).read_text())
latest = {}
for entry in management['entries']:
    owner = entry.get('ownerThreadId', entry.get('owner'))
    latest[owner] = entry
files = sorted(Path(cfg['receipt_directory']).glob('*.json'))
assert len(files) <= 128
for file in files:
    assert file.stat().st_size <= 128 * 1024
    entry = json.loads(file.read_text())
    if 'ownerThreadId' not in entry:
        observation = entry.get('ordinaryObservation') or {}
        assert observation.get('ordinaryCommandsComplete') is True and observation.get('commandExit') == 0, 'Unresolved noncanonical owner metadata'
        # Completed ordinary observation cannot release an accepted legacy cohort.
        # Legacy reservations and actual unknown-root detection remain active.
        continue
    owner = entry['ownerThreadId']
    old = latest.get(owner, {})
    old_clock = old.get('acceptedAtEpoch', old.get('actualAccept', 0)) or 0
    new_clock = entry.get('acceptedAtEpoch') or 0
    if isinstance(old_clock, (int, float)) and new_clock >= old_clock:
        latest[owner] = entry
active = [entry for owner, entry in latest.items() if owner != cfg['owner_thread']
          and entry.get('kind') != 'ordinary' and entry['state'] not in ('FULL', 'DECLINED') and not entry['state'].startswith('LITERAL_FULL')]
pids = cfg['process_cap'] + sum(entry['processCap'] for entry in active)
rss = cfg['rss_cap'] + sum(entry.get('rssCapBytes', entry.get('rssCapGiB', 0) * GIB) for entry in active)
browsers = 1 + sum(entry.get('kind') == 'browser' for entry in active)
targets = sum(entry.get('kind') == 'target' for entry in active)
sources = sum(entry.get('kind') in ('source', 'copied-native') for entry in active)
assert len(active) + 1 <= 4 and browsers <= 2 and targets <= 1 and sources <= 3
assert pids <= 96 and rss <= 18 * GIB
other = int(2.5 * GIB) + sum(entry.get('remainingNewUpperBytes', entry.get('remainingNewConservativeUpperBytes', 0)) for entry in active)
cfg['other_new'] = max(cfg['other_new'], other)
space = os.statvfs(cfg['cwd']); available = space.f_bavail * space.f_frsize
assert available >= cfg['start_floor']
assert available - cfg['other_new'] - cfg['remaining_new_reservation'] >= 9 * GIB
accepted = time.time()
cfg.update(status='ACTUAL_ACCEPTED', accept_epoch=accepted, work_deadline=accepted + 300, full_deadline=accepted + 360, fresh_free=available)
(folder / 'offer.json').write_text(json.dumps(cfg, indent=2) + '\n')
pid = os.getpid()
birth = ' '.join(subprocess.check_output(['/bin/ps', '-p', str(pid), '-o', 'lstart='], text=True).split())
executable = subprocess.check_output(['/bin/ps', '-p', str(pid), '-o', 'comm='], text=True).strip()
record = dict(ownerThreadId=cfg['owner_thread'], offerId=cfg['lease'], state='ACCEPTED', acceptedAtEpoch=accepted,
              workDeadlineEpoch=cfg['work_deadline'], fullDeadlineEpoch=cfg['full_deadline'], kind='browser',
              processCap=cfg['process_cap'], rssCapBytes=cfg['rss_cap'], remainingNewUpperBytes=cfg['remaining_new_reservation'],
              controller=dict(pid=pid, birth=birth, executable=executable, script=str(Path(__file__).resolve()), cwd=os.getcwd(), outputRoot=cfg['output']),
              guardControllers=[], receiptPaths=dict(accept=str(folder / 'offer.json'), full=str(folder / 'outer-full.json')),
              observedAtEpoch=time.time(), phase='Actual bootstrap, not a future controller identity')
receipt = Path(cfg['owner_receipt'])
temporary = receipt.with_suffix('.tmp')
temporary.write_text(json.dumps(record, indent=2) + '\n'); os.replace(temporary, receipt)
print(json.dumps(dict(actualACCEPT=accepted, source=cfg['source'], readset=cfg['readset_sha'], work=cfg['work_deadline'], full=cfg['full_deadline'], free=available, bootstrap=record['controller'])), flush=True)
try:
    # Import performs Git discovery; it intentionally follows factual publication.
    sys.path.insert(0, str(Path(__file__).parent))
    import metrics
    sample = metrics.global_sample(cfg, {pid: None})
    carry = max(cfg['prior_whole'], sum(metrics.inventory(metrics.CARRY + [folder, receipt] + [Path(root) for root in cfg['retained_roots']]).values()))
    assert carry + cfg['remaining_new_reservation'] <= cfg['whole_cap']
    metrics.write(folder / 'bootstrap-sample.json', dict(epoch=time.time(), carry=carry, global_sample=sample))
    assert time.time() < cfg['work_deadline']
    os.environ['PI10_CONFIG'] = str(folder / 'offer.json')
    # The PID and birth remain the same. The new script publishes its actual
    # identity immediately on entry; no future executable is advertised here.
    os.execv(sys.executable, [sys.executable, '-B', str(Path(__file__).with_name('controller.py').resolve())])
except BaseException as error:
    full = dict(lease=cfg['lease'], result='FAIL', exit=1, failure='Bootstrap before payload: ' + repr(error),
                full_epoch=time.time(), before_hard=time.time() < cfg['full_deadline'], remaining=[], execution_resources=0)
    (folder / 'outer-full.json').write_text(json.dumps(full, indent=2) + '\n')
    record.update(state='FULL', remainingNewUpperBytes=0, observedAtEpoch=full['full_epoch'])
    temporary.write_text(json.dumps(record, indent=2) + '\n'); os.replace(temporary, receipt)
    print(json.dumps(full), flush=True)
    sys.exit(1)
