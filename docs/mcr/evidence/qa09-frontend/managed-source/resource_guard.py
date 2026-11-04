"""QA09 adoption of qualified TOOLS-FS-01; no product/runtime control."""
from pathlib import Path
import fcntl
import hashlib
import importlib.util
import json
import os
import signal
import subprocess
import sys
import time

HELPER = Path('/Users/lhs/Desktop/mcr-worktrees/qa07-input-delta-332b/scripts/mcr/resource-accounting/accounting.py')
HELPER_SHA = '1a2ed019be0af4d673786532c8b46eda81cd6137a7ac27c9f97b83a34387483c'
if hashlib.sha256(HELPER.read_bytes()).hexdigest() != HELPER_SHA:
    raise RuntimeError('qualified accounting helper changed')
spec = importlib.util.spec_from_file_location('qa09_accounting', HELPER)
api = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = api
spec.loader.exec_module(api)


def write(path, value):
    path = Path(path)
    temporary = path.with_suffix('.next')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify_source_ref(config):
    # Git's files backend can pack a branch and remove its loose file. Observe
    # that storage without rewriting refs, invoking Git/GC or adding a child.
    name = config['branchRefName']
    if Path(config['headPath']).read_text().strip() != 'ref: ' + name:
        raise RuntimeError('source worktree HEAD changed')
    loose = Path(config['branchRef'])
    def loose_value():
        try:
            return loose.read_text().strip()
        except FileNotFoundError:
            return None
    value = loose_value()
    if value is None:
        with (Path(config['gitCommon']) / 'packed-refs').open('r') as stream:
            packed = stream.read(4 * 1024 ** 2 + 1)
        if len(packed) > 4 * 1024 ** 2:
            raise RuntimeError('packed refs size bound')
        matches = [line.split(' ', 1)[0] for line in packed.splitlines()
                   if line.endswith(' ' + name) and not line.startswith(('#', '^'))]
        if len(matches) != 1:
            raise RuntimeError('source packed ref unavailable or ambiguous')
        # A new loose update supersedes the packed baseline, including during
        # a concurrent ordinary pack-refs transition by the repository owner.
        latest = loose_value()
        value = matches[0] if latest is None else latest
    if value != config['source']:
        raise RuntimeError('source branch changed')


def processes(parent, lock_path, deadline):
    # Both independent ledgers remain separate. Serialize ps itself so their
    # measurement children cannot exceed the four-ALL-process proposal.
    with open(lock_path, 'a') as lock:
        while True:
            if time.monotonic() >= deadline:
                raise RuntimeError('deadline acquiring process measurement')
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                time.sleep(.005)
        raw = subprocess.check_output(['/bin/ps', '-axo', 'pid=,ppid=,rss='], text=True,
                                      timeout=min(1, max(.001, deadline - time.monotonic())))
    rows = [tuple(map(int, line.split())) for line in raw.splitlines()]
    owned = {parent}
    while True:
        expanded = owned | {pid for pid, ppid, _ in rows if ppid in owned}
        if expanded == owned:
            break
        owned = expanded
    return {'pids': sorted(pid for pid, _, _ in rows if pid in owned),
            'processes': sum(pid in owned for pid, _, _ in rows),
            'rss': sum(rss * 1024 for pid, _, rss in rows if pid in owned)}


def collect_whole(roots, *, deadline, checkpoint):
    allocated = api.collect(roots, deadline=deadline, checkpoint=checkpoint, attempts=3)
    # Root's intake also retains logical usage when it exceeds allocated blocks.
    # This independent no-follow, inode-deduplicated metadata pass fails closed
    # on ANY error; it never turns a partial apparent-size subtotal into credit.
    seen, logical = set(), {name: 0 for name in roots}
    entries = 0
    def check():
        if time.monotonic() >= deadline:
            raise RuntimeError('logical scan deadline')
        checkpoint()
    def count(info, scope):
        nonlocal entries
        check()
        entries += 1
        if entries > 1000000:
            raise RuntimeError('logical scan entry bound')
        key = (info.st_dev, info.st_ino)
        if key not in seen:
            seen.add(key)
            logical[scope] += info.st_size
    def fail(error):
        raise error
    for name, identity in roots.items():
        identity.verify()
        for path, directories, files, fd in os.fwalk(identity.scan_path, follow_symlinks=False, onerror=fail):
            check()
            if len(Path(path).relative_to(identity.scan_path).parts) > 128:
                raise RuntimeError('logical scan depth bound')
            count(os.fstat(fd), name)
            for child in directories + files:
                count(os.stat(child, dir_fd=fd, follow_symlinks=False), name)
        identity.verify()
    for identity in roots.values():
        identity.verify()
    return {'complete': True, 'bytes': sum(max(allocated['scopes'][name]['bytes'], logical[name]) for name in roots),
            'allocatedBytes': allocated['bytes'], 'logicalBytes': sum(logical.values()),
            'scopes': {name: {'complete': True, 'bytes': max(allocated['scopes'][name]['bytes'], logical[name]),
                              'allocatedBytes': allocated['scopes'][name]['bytes'], 'logicalBytes': logical[name]}
                       for name in roots}, 'allocatedAttempts': allocated['attempts']}


class Guard:
    def __init__(self, config):
        self.config = config
        self.roots = {key: api.RootIdentity(**value) for key, value in config['roots'].items()}
        self.carry = api.Carry(config['baseline'], config['retainedNew'], config['priorWhole'],
                              config['retainedNew'], config['reservation'])
        self.peak = {key: 0 for key in ('new', 'whole', 'processes', 'rss')}
        self.minimum_free = None
        self.last = 0
        self.samples = 0

    def live(self, force=False):
        now = time.monotonic()
        if now >= self.config['deadline']:
            raise RuntimeError('work deadline')
        if not force and now - self.last < .1:
            return
        for item in self.config['pins']:
            if digest(item['path']) != item['sha256']:
                raise RuntimeError('frozen source/controller readset changed')
        verify_source_ref(self.config)
        sample = processes(self.config['parent'], self.config['processLock'], self.config['deadline'])
        # Keep the qualified completion-time throttle fix; no ps per file entry.
        self.last = time.monotonic()
        space = os.statvfs(self.config['checkout'])
        sample.update(free=space.f_bavail * space.f_frsize,
                      new=self.carry.prior_new, whole=self.carry.prior_whole)
        self.minimum_free = sample['free'] if self.minimum_free is None else min(self.minimum_free, sample['free'])
        for key in self.peak:
            self.peak[key] = max(self.peak[key], sample[key])
        reason = api.limit_reason(sample, self.config['caps'], now=time.monotonic(),
                                  deadline=self.config['deadline'])
        if reason:
            raise RuntimeError(reason)

    def sample(self):
        self.live(True)
        snapshot = collect_whole(self.roots, deadline=min(self.config['deadline'], time.monotonic() + 10),
                                 checkpoint=self.live)
        self.carry.observe(snapshot['scopes'])
        self.samples += 1
        self.live(True)
        return snapshot

    def receipt(self, failure):
        return {'failure': failure, 'samples': self.samples, 'peak': self.peak,
                'minimumFree': self.minimum_free}


def independent(config_path):
    config_path = Path(config_path)
    config = json.loads(config_path.read_text())
    run = config_path.parent
    guard = Guard(config)
    failure = None
    try:
        guard.sample()
        write(run / 'guardian-ready.json', guard.receipt(None))
        while not (run / 'stop').exists():
            guard.sample()
            time.sleep(.05)
    except BaseException as error:
        failure = type(error).__name__ + ': ' + str(error)
        write(run / 'guardian-failure.json', {'failure': failure})
        if isinstance(error, api.AccountingError):
            write(run / 'guardian-incomplete.json', error.receipt())
        try:
            os.kill(config['parent'], signal.SIGTERM)
        except ProcessLookupError:
            pass
    finally:
        write(run / 'guardian-full.json', guard.receipt(failure))
    return 1 if failure else 0


if __name__ == '__main__':
    raise SystemExit(independent(sys.argv[1]))
