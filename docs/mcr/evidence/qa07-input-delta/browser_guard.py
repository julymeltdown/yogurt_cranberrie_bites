"""Independent outer guard; counts itself, the inner guard and all observed children."""
from pathlib import Path
import hashlib, json, os, shutil, signal, subprocess, sys, time
sys.dont_write_bytecode = True
from guard_disk import measured_du

repo = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
round_name = sys.argv[1]
assert round_name.replace('-', '').isalnum()
assert os.environ['QA07_KIND'] == 'native'
deadline = float(os.environ['QA07_DEADLINE'])
assert 0 < deadline - time.time() <= 480
full_deadline = deadline + 60
inner = out / 'supervise.py'
assert hashlib.sha256(inner.read_bytes()).hexdigest() == '4a877cbf8b4d63df0a70de098066be512bf8cdbf72895293c8e6d77c144282a2'
temp = Path('/private/tmp/mcr-qa07-delta-' + round_name)
link = repo / 'frontend/node_modules'
deps = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
assert not temp.exists() and not link.exists() and not link.is_symlink()

measurement_races = []
def du(path):
    return measured_du(path, temp, measurement_races)

gitdir = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=repo, text=True).strip())
ids = subprocess.check_output(['git', 'rev-list', '--objects', 'a10e4f409f5727cb25b6cc37d75f499a73e9bb70..HEAD'], cwd=repo, text=True)
objects = subprocess.check_output(['git', 'cat-file', '--batch-check=%(objectsize:disk)'], cwd=repo, input='\n'.join(x.split()[0] for x in ids.splitlines()) + '\n', text=True)
git_bytes = sum(int(x) for x in objects.splitlines())
base = {'checkout': du(repo), 'deps': du(deps), 'gitmeta': du(gitdir)}
carry = du(repo / 'frontend/e2e/qa07-input-delta') + du(out) + git_bytes
prior = [json.loads(p.read_text()) for pattern in ('supervisor-*.json', 'outer-*.json') for p in out.glob(pattern)]
prior_whole = max([x.get('peaks', {}).get('whole', 0) for x in prior] + [0])
prior_new = max([x.get('peaks', {}).get('new', 0) for x in prior] + [0])
free = shutil.disk_usage(repo).free
assert free >= 11 * 1024**3
assert free - int(os.environ['QA07_OUTSTANDING']) - 128 * 1024**2 >= 9 * 1024**3
assert max(prior_whole, sum(base.values()) + git_bytes) + 128 * 1024**2 < 2 * 1024**3
record = {'job': os.environ['QA07_JOB'], 'round': round_name, 'guardReadyEpoch': time.time(), 'guardianPid': os.getpid(), 'deadline': deadline, 'fullDeadline': full_deadline, 'base': base, 'carry': carry, 'priorWholeMax': prior_whole, 'priorNewMax': prior_new, 'gitObjects': git_bytes, 'peaks': {}, 'minFree': free}
path = out / ('outer-' + round_name + '.json')
path.write_text(json.dumps(record, indent=2) + '\n')
seen = {os.getpid()}
child = None

def snapshot():
    rows = [x.split() for x in subprocess.check_output(['ps', '-axo', 'pid,ppid,rss'], text=True).splitlines()[1:]]
    owned = {os.getpid()}
    for _ in range(24):
        new = {int(x[0]) for x in rows if len(x) == 3 and int(x[1]) in owned}
        if new.issubset(owned):
            break
        owned.update(new)
    seen.update(owned)
    group = [x for x in rows if len(x) == 3 and int(x[0]) in owned]
    return owned, len(group), sum(int(x[2]) * 1024 for x in group)

def stop_owned(owned):
    for pid in sorted(owned - {os.getpid(), child.pid}, reverse=True):
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    try:
        child.wait(timeout=3)
    except subprocess.TimeoutExpired:
        child.terminate()
        try:
            child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()

try:
    child = subprocess.Popen([sys.executable, str(inner), round_name], cwd=repo)
    record['innerPid'] = child.pid
    while child.poll() is None:
        owned, pids, rss = snapshot()
        current = {'checkout': du(repo), 'deps': du(deps), 'gitmeta': du(gitdir)}
        tmp = du(temp)
        values = {'pids': pids, 'rss': rss, 'tmp': tmp, 'durable': du(out), 'whole': max(prior_whole, sum(current.values()) + git_bytes + tmp + 8 * 1024**2), 'new': max(prior_new, carry + sum(max(0, current[k] - base[k]) for k in base) + tmp + 8 * 1024**2)}
        caps = {'pids': 24, 'rss': 4 * 1024**3, 'tmp': 96 * 1024**2, 'durable': 24 * 1024**2, 'whole': 2 * 1024**3, 'new': 128 * 1024**2}
        for k, v in values.items():
            record['peaks'][k] = max(record['peaks'].get(k, 0), v)
        record['minFree'] = min(record['minFree'], shutil.disk_usage(repo).free)
        reason = next((k + ' cap' for k, v in values.items() if v > caps[k]), None)
        if record['minFree'] < 8 * 1024**3:
            reason = '8GiB live floor'
        if time.time() >= deadline - 3:
            reason = 'absolute work deadline'
        if reason:
            record['stopReason'] = reason
            stop_owned(owned)
            break
        time.sleep(.5)
    record['exitCode'] = child.wait()
except BaseException as error:
    record['guardError'] = repr(error)
    raise
finally:
    if child and child.poll() is None:
        stop_owned(snapshot()[0])
    record.update(finishedEpoch=time.time(), allSeenPids=sorted(seen), linkAbsent=not link.exists() and not link.is_symlink(), tempAbsent=not temp.exists(), withinFullDeadline=time.time() < full_deadline, transientDiskRaces=measurement_races)
    path.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record))
sys.exit(record.get('exitCode', 1) if 'stopReason' not in record else 1)
