import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

root = Path('<FEED_WORKTREE>')
carry = Path(__file__).parent
run, expected = sys.argv[1:3]
accept = json.loads((carry / 'lifetime-source-accept.json').read_text())
active = datetime.datetime.fromisoformat(accept['activeUntil']).timestamp()
hard = datetime.datetime.fromisoformat(accept['hardUntil']).timestamp()
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == expected
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=root)
node = '<RETAINED_NODE24>/bin/node'
env = os.environ.copy()
for key in list(env):
    if key.startswith('MCR_FEED_') or key in ('MCR_IN03_SOURCE', 'MCR_IN03_SERVE_SOURCE', 'MCR_SOCIAL_LIFETIME_SOURCE', 'NODE_OPTIONS'):
        env.pop(key, None)
env.update(MCR_FEED_CARRY=str(carry), MCR_FEED_NODE=node,
           MCR_FEED_DEPENDENCIES='<READ_ONLY_DEPENDENCIES>',
           MCR_FEED_ALLOCATION=accept['allocation'], MCR_FEED_RUN=run,
           MCR_FEED_ACTIVE_UNTIL=accept['activeUntil'], MCR_FEED_MAX_FILES='1400',
           MCR_FEED_MAX_BYTES=str(128 * 1024**2), MCR_FEED_MAX_PROCESSES='24',
           MCR_FEED_MAX_RSS_BYTES=str(4 * 1024**3), MCR_SOCIAL_LIFETIME_SOURCE='1')
paths = ['frontend/src/app/feed-entry', 'frontend/src/app/in03-entry', 'frontend/src/app/social-entry', 'frontend/src/app/world-panels/social.tsx',
         'docs/mcr/frontend/feed-root-composition.md', 'docs/mcr/evidence/feed-root-composition',
         'docs/mcr/frontend/in03-production-availability.md', 'docs/mcr/evidence/in03-production-availability',
         'frontend/src/features/feed-thread/FeedThread.tsx', 'frontend/src/features/event-feed/__tests__/thread-author',
         'frontend/e2e/real/social', 'frontend/e2e/real/social.spec.ts',
         'frontend/src/shared/api/__tests__/c07-social-client', 'docs/mcr/frontend/c07-social-client-in03.md',
         'docs/mcr/frontend/in03-public-journey.md', 'docs/mcr/evidence/c07-social-client-in03',
         'docs/mcr/evidence/in03-public-journey']
gitdir = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=root, text=True).strip())
ids = set(line.split()[0] for line in subprocess.check_output(
    ['git', 'rev-list', '--objects', '654974128a27c948a449fa9e58f466b8e612361d..HEAD',
     'codex/mcr-qa-IN03-stack-runtime', 'codex/mcr-fe-IN03-public-journey'], cwd=root, text=True).splitlines())
sizes = [int(x) for x in subprocess.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'],
         cwd=root, input='\n'.join(ids) + '\n', text=True).splitlines()]
cache, temporary = (carry / ('feed-' + run + suffix) for suffix in ('-cache', '-tmp'))

def usage():
    files, total, temp_bytes = len(sizes), sum(sizes), 0
    def walk(path):
        nonlocal files, total, temp_bytes
        try:
            stat = path.lstat()
            if path.is_dir() and not path.is_symlink():
                for child in path.iterdir():
                    walk(child)
            else:
                files += 1
                total += stat.st_size
                if str(path).startswith(str(cache) + '/') or str(path).startswith(str(temporary) + '/'):
                    temp_bytes += stat.st_size
        except FileNotFoundError:
            return
    for path in [carry, *[root / p for p in paths], root / '.git', gitdir, root / 'frontend/node_modules']:
        walk(path)
    disk = os.statvfs(carry)
    return dict(files=files, bytes=total, temporaryBytes=temp_bytes, freeBytes=disk.f_bavail * disk.f_frsize)

def whole_usage():
    total = sum(sizes)
    def walk(path):
        nonlocal total
        try:
            stat = path.lstat()
            if path.is_dir() and not path.is_symlink():
                for child in path.iterdir(): walk(child)
            else: total += stat.st_size
        except FileNotFoundError: pass
    for path in [root, carry, gitdir, Path('<READ_ONLY_DEPENDENCIES>')]: walk(path)
    return total

import hashlib
preflight = json.loads((carry / 'lifetime-source-preflight.json').read_text())
assert hashlib.sha256(Path(node).read_bytes()).hexdigest() == 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a'
for path in [root / 'frontend/pnpm-lock.yaml', Path('<PUBLISHING_FRONTEND>/pnpm-lock.yaml')]:
    assert hashlib.sha256(path.read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
for path, pin in preflight['pins'].items(): assert hashlib.sha256((root/path).read_bytes()).hexdigest() == pin
initial_whole = whole_usage()
assert initial_whole + 128*1024**2 <= 3*1024**3
assert os.statvfs(carry).f_bavail*os.statvfs(carry).f_frsize - 2*1024**3 - 3*1024**3 >= 9*1024**3
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == accept['guardianSha256']
initial = usage()
assert initial['freeBytes'] >= 16 * 1024**3 and initial['files'] < 1400 and initial['bytes'] < 128 * 1024**2
assert time.time() < active - 30
owned = {os.getpid()}
peak_count = peak_rss = 0
violations = []
child = None
code = 1
def sample():
    rows = [tuple(map(int, line.split())) for line in subprocess.check_output(
        ['/bin/ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines()]
    changed = True
    while changed:
        previous = len(owned)
        owned.update(pid for pid, parent, group, rss in rows if parent in owned)
        changed = previous != len(owned)
    return [row for row in rows if row[0] in owned]

try:
    # The independent guardian starts before the Node supervisor and all FE children.
    sample()
    with (carry / ('feed-' + run + '-external.log')).open('x') as log:
        child = subprocess.Popen([node, 'frontend/src/app/feed-entry/__tests__/qualify.mjs'],
                                 cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        owned.add(child.pid)
        while True:
            live = sample()
            count, rss, state = len(live), sum(row[3] * 1024 for row in live), usage()
            peak_count, peak_rss = max(peak_count, count), max(peak_rss, rss)
            whole = whole_usage()
            if not (whole <= 3*1024**3 and whole-initial_whole <= 128*1024**2 and time.time() < active and state['freeBytes'] >= 8 * 1024**3 and state['files'] <= 1400
                    and state['bytes'] <= 128 * 1024**2 and state['temporaryBytes'] <= 100 * 1024**2
                    and state['bytes'] - state['temporaryBytes'] - initial['bytes'] <= 8 * 1024**2
                    and count <= 24 and rss <= 4 * 1024**3):
                violations.append(dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(), usage=state, processes=count, rss=rss))
                raise RuntimeError('Independent source guardian limit')
            if child.poll() is not None:
                code = child.returncode
                break
            time.sleep(.25)
except Exception as error:
    violations.append(dict(error=type(error).__name__))
finally:
    if child is not None and child.poll() is None:
        for pid, parent, group, rss in sample():
            if pid != os.getpid():
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        child.wait(timeout=max(1, hard - time.time()))
    record = dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(), allocation=accept['allocation'],
                  source=expected, exit=code, initialWholeBytes=initial_whole, finalWholeBytes=whole_usage(), initial=initial, final=usage(), peakAllProcesses=peak_count,
                  peakAllRssBytes=peak_rss, violations=violations, ownedPidsObserved=sorted(owned), independentGuardianIntervalMs=250)
    with (carry / ('feed-' + run + '-guardian.json')).open('x') as output:
        json.dump(record, output, indent=2)
        output.write('\n')
    print(json.dumps(record))
sys.exit(code if code else bool(violations))
