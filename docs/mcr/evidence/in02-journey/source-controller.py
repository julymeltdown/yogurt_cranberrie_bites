"""IN02 finite SOURCE supervisor, adapted from the retained FE10 source controller.

This entry point needs a fresh accepted21d grant and matching environment ID.
Preparation metadata alone never grants execution. No browser/build/database job.
"""
import datetime, hashlib, json, os, signal, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
E = ROOT / 'docs/mcr/evidence/in02-journey'
os.umask(0o077)
r = json.loads((E / 'source-ready.json').read_text())
grant_path = Path(sys.argv[1]).resolve()
assert grant_path.is_relative_to(E)
grant = json.loads(grant_path.read_text())
assert grant['accepted'] is True and grant['task'] == 'IN02'
assert os.environ.get('MCR_IN02_ACCEPTED') == grant['id']
assert grant['resourceClass'] == r['resourceClass'] == 'SOURCE_ONLY_NO_BROWSER_NO_DB_NO_BUILD'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == r['head']
assert hashlib.sha256((ROOT / r['guard']).read_bytes()).hexdigest() == r['guardSha256']
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == r['supervisorSha256']
assert hashlib.sha256(Path(r['node']).read_bytes()).hexdigest() == r['nodeSha256']
for path, digest in r['readSet'].items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest, 'Changed frozen source/read input'
assert not (ROOT / 'frontend/node_modules').exists() and not (ROOT / 'frontend/node_modules').is_symlink()
assert not Path(r['newScratchOnly']).exists() and not (ROOT / r['retainedOutput']).exists()
for key in ['maxBytes', 'maxRssBytes', 'maxProcesses', 'maxWholeBytes', 'maxWholeEntries']:
    assert grant[key] <= r[key], 'A larger workload needs a new bounded intake'
lease = {**r, **grant, 'scratch': r['newScratchOnly']}
assert lease['commands'] == r['commands'] and lease['files'] == r['files']
assert lease['maxRepairRounds'] <= r['maxRepairRounds']
start = datetime.datetime.fromisoformat(grant['acceptedAt']).timestamp()
stop = datetime.datetime.fromisoformat(grant['workDeadline']).timestamp()
hard = datetime.datetime.fromisoformat(grant['hardDeadline']).timestamp()
assert start <= time.time() < stop and stop - start <= r['workSeconds'] and hard - stop >= r['cleanupSeconds']
assert hard - start <= r['workSeconds'] + r['cleanupSeconds']
assert abs(hard - stop - r['cleanupSeconds']) < .001
roots = [Path(path).resolve() for path in r['custodyRoots']]

def census():
    pending = list(roots); seen = set(); size, entries = 208, 4
    while pending:
        path = pending.pop()
        try:
            stat = path.lstat(); inode = (stat.st_dev, stat.st_ino)
            if inode in seen: continue
            seen.add(inode); size += stat.st_size; entries += 1
            if path.is_symlink(): assert any(path.resolve().is_relative_to(root) for root in roots), 'Undeclared custody target'
            elif path.is_dir(): pending.extend(path.iterdir())
        except FileNotFoundError: pass
    return size, entries

def free():
    stat = os.statvfs(ROOT)
    return stat.f_bavail * stat.f_frsize

def table():
    result = {}
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines():
        values = line.split()
        if len(values) == 4:
            pid, parent, group, rss = map(int, values); result[pid] = (parent, group, rss * 1024)
    return result

size, entries = census()
assert size <= lease['maxWholeBytes'] and entries <= lease['maxWholeEntries']
assert free() >= lease['startFloorBytes']
assert free() - lease['otherReservationsBytes'] - lease['maxBytes'] >= lease['reserveMarginBytes']
lease.update(measuredWholeBytes=max(size, r['measuredWholeBytes']), measuredWholeEntries=max(entries, r['measuredWholeEntries']),
             measuredFreeBytes=free(), remainingAfterReservationsBytes=free()-lease['otherReservationsBytes']-lease['maxBytes'])
out = ROOT / r['retainedOutput']; out.mkdir()
(out / 'offer.json').write_text(json.dumps(lease, indent=2) + '\n')
seen = set(); peak = {'bytes': 0, 'rss': 0, 'processes': 0}; whole = [lease['measuredWholeBytes'], lease['measuredWholeEntries']]
last_census = 0; guardian = None; failure = None

def sample():
    global last_census
    rows = table(); ids = {os.getpid()}
    if guardian: ids.add(guardian.pid)
    while True:
        more = {pid for pid, row in rows.items() if row[0] in ids}
        if more <= ids: break
        ids |= more
    seen.update(ids); live = ids & rows.keys(); rss = sum(rows[pid][2] for pid in live)
    paths = set()
    for name in lease['taskCarryRoots'] + [lease['scratch']]:
        path = Path(name)
        if path.exists():
            paths.add(path)
            if path.is_dir() and not path.is_symlink(): paths.update(path.rglob('*'))
    total = lease['gitCarryBytes'] + sum(path.lstat().st_size for path in paths if path.exists() or path.is_symlink())
    assert total <= lease['maxBytes'] and rss <= lease['maxRssBytes'] and len(live) <= lease['maxProcesses']
    assert free() >= lease['liveFloorBytes'] and time.time() < hard
    if time.monotonic() - last_census >= 2:
        a, b = census(); whole[0] = max(whole[0], a); whole[1] = max(whole[1], b); last_census = time.monotonic()
    assert whole[0] <= lease['maxWholeBytes'] and whole[1] <= lease['maxWholeEntries']
    peak['bytes'] = max(peak['bytes'], total); peak['rss'] = max(peak['rss'], rss); peak['processes'] = max(peak['processes'], len(live))
    with (out / 'primary-supervisor.jsonl').open('a') as stream:
        stream.write(json.dumps({'epoch': time.time(), 'pids': sorted(live), 'newBytes': total, 'rss': rss, 'freeBytes': free(), 'whole': whole}) + '\n')

try:
    sample()
    guardian = subprocess.Popen(['python3', str(ROOT / r['guard']), str(out / 'offer.json')], cwd=ROOT,
                                env={**os.environ, 'MCR_IN02_ACCEPTED': lease['id']}, start_new_session=True)
    while guardian.poll() is None:
        sample(); assert time.time() < stop, 'Original WORK deadline'; time.sleep(.1)
    sample(); assert guardian.returncode == 0, ('Guardian failed', guardian.returncode)
except BaseException as error:
    failure = repr(error)
    if guardian and guardian.poll() is None:
        guardian.send_signal(signal.SIGINT)
        try: guardian.wait(timeout=10)
        except subprocess.TimeoutExpired: os.killpg(guardian.pid, signal.SIGKILL); guardian.wait()
finally:
    remaining = sorted((seen - {os.getpid()}) & table().keys())
    link = ROOT / 'frontend/node_modules'; scratch = Path(lease['scratch'])
    released = not remaining and not link.exists() and not link.is_symlink() and not scratch.exists()
    receipt = {'state': 'FULL' if released else 'NOT_FULL', 'actualFullAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
               'allocation': lease['id'], 'head': lease['head'], 'commands': len(lease['commands']), 'guardianExit': guardian.returncode if guardian else None,
               'failure': failure, 'ownedProcessesRemaining': remaining, 'dependencyLinkAbsent': not link.exists() and not link.is_symlink(),
               'scratchCacheAbsent': not scratch.exists(), 'peakNew': peak, 'peakWhole': {'bytes': whole[0], 'entries': whole[1]},
               'priorUnconfirmedExternalCarry': '208 bytes/four entries untouched; no external Chrome started',
               'workDeadline': lease['workDeadline'], 'hardDeadline': lease['hardDeadline'], 'noBrowserDatabaseBuildTarget': True}
    (out / 'release.json').write_text(json.dumps(receipt, indent=2) + '\n'); print(json.dumps(receipt), flush=True)
    if not released or failure: raise SystemExit(1)
