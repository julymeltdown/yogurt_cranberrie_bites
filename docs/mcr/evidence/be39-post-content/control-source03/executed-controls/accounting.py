"""Bounded complete allocation scans. Caller retains custody, readsets and limits."""
from dataclasses import dataclass, field
from pathlib import Path
import os
import errno
import stat
import subprocess
import time


class AccountingError(RuntimeError):
    def __init__(self, reason, attempts):
        self.reason, self.attempts = reason, attempts
        super().__init__(reason)

    def receipt(self):
        return {'reason': self.reason, 'attempts': self.attempts, 'complete': False}


@dataclass(frozen=True)
class RootIdentity:
    path: str
    device: int
    inode: int
    owner: int
    read_only_target: str = None
    link_identity: tuple = None
    link_text: str = None

    @classmethod
    def capture(cls, path, *, read_only_target=None):
        path = Path(os.path.abspath(path))
        if '\n' in str(path) or '\r' in str(path):
            raise ValueError('scan root must fit one diagnostic line')
        info = path.lstat()
        link_identity = link_text = None
        target = None
        if stat.S_ISLNK(info.st_mode):
            if read_only_target is None:
                raise ValueError('root symlink requires explicit read-only target custody')
            target = os.path.abspath(read_only_target)
            target_info = os.lstat(target)
            if not stat.S_ISDIR(target_info.st_mode) or os.path.realpath(path) != target:
                raise ValueError('read-only target must be the declared real directory')
            link_identity = (info.st_dev, info.st_ino, info.st_uid)
            link_text = os.readlink(path)
            info = target_info
        elif read_only_target is not None:
            raise ValueError('read-only target declaration requires a root symlink')
        if not stat.S_ISDIR(info.st_mode):
            raise ValueError('scan root must be a directory')
        return cls(str(path), info.st_dev, info.st_ino, info.st_uid,
                   target, link_identity, link_text)

    @property
    def scan_path(self):
        return self.read_only_target or self.path

    def verify(self):
        current = self.capture(self.path, read_only_target=self.read_only_target)
        if current != self:
            raise ValueError('scan root identity changed')


def _stop(process):
    if process.poll() is None:
        process.terminate()
    try:
        return process.communicate(timeout=.15)
    except subprocess.TimeoutExpired:
        process.kill()
        return process.communicate(timeout=.5)


def complete_allocated(identity, *, deadline, checkpoint, attempts=3,
                       timeout=1.0, retry_delay=.03):
    """Return bytes only after a fresh, successful du of the same root identity.

    deadline is an absolute time.monotonic() deadline, never renewed per retry.
    checkpoint executes before/during/after every child, retaining fast live
    resource/deadline checks. Each owned du child is reaped on every error.
    Failed stdout/stderr are retained in AccountingError or the success receipt.
    No failed subtotal or last-good allocation is returned as a complete scan.
    """
    if not 1 <= attempts <= 5 or not 0 < timeout <= 10 or not 0 <= retry_delay <= .05:
        raise ValueError('invalid bounded scan policy')
    records = []
    def check():
        if time.monotonic() >= deadline:
            raise AccountingError('scan deadline', records)
        checkpoint()
        identity.verify()
    for number in range(1, attempts + 1):
        process = None
        row = {'attempt': number, 'startedMonotonic': time.monotonic()}
        records.append(row)
        try:
            check()
            stop = min(deadline, time.monotonic() + timeout)
            process = subprocess.Popen(['/usr/bin/du', '-sk', identity.scan_path],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                env={**os.environ, 'LC_ALL': 'C'})
            row['pid'] = process.pid
            while True:
                check()
                left = stop - time.monotonic()
                if left <= 0:
                    raise AccountingError('allocation subprocess timeout', records)
                try:
                    stdout, stderr = process.communicate(timeout=min(.025, left))
                    break
                except subprocess.TimeoutExpired as pending:
                    if len(pending.output or b'') + len(pending.stderr or b'') > 65536:
                        raise AccountingError('allocation diagnostics overflow', records)
            row.update(exit=process.returncode, stdout=stdout[:65536].decode('utf-8', 'replace'),
                       stderr=stderr[:max(0, 65536-len(stdout))].decode('utf-8', 'replace'),
                       diagnosticsTruncated=len(stdout) + len(stderr) > 65536)
            if len(stdout) + len(stderr) > 65536:
                raise AccountingError('allocation diagnostics overflow', records)
            check()
            if process.returncode == 0:
                lines = row['stdout'].splitlines()
                fields = lines[0].split('\t', 1) if len(lines) == 1 else []
                if len(fields) != 2 or not fields[0].isdigit() or fields[1] != identity.scan_path or row['stderr']:
                    raise AccountingError('allocation output invalid', records)
                return {'bytes': int(fields[0]) * 1024, 'complete': True,
                        'root': identity.__dict__, 'attempts': records}
            errors = row['stderr'].splitlines()
            retryable = process.returncode == 1 and bool(errors) and all(
                line.startswith('du: ' + identity.scan_path + '/')
                and line.endswith(': No such file or directory') for line in errors)
            if not retryable:
                raise AccountingError('allocation nontransient error', records)
            if number == attempts:
                raise AccountingError('allocation incomplete after bounded retries', records)
            until = min(deadline, time.monotonic() + retry_delay)
            while time.monotonic() < until:
                check()
                time.sleep(min(.005, max(0, until - time.monotonic())))
        except AccountingError as error:
            if error.attempts is not records:
                row['checkpointFailure'] = error.receipt()
                raise AccountingError(error.reason, records) from error
            raise
        except Exception as error:
            row['cause'] = type(error).__name__ + ': ' + str(error)
            raise AccountingError('checkpoint/root/measurement failure', records) from error
        finally:
            if process is not None:
                final_stdout, final_stderr = _stop(process)
                if 'stdout' not in row:
                    row.update(stdout=final_stdout[:65536].decode('utf-8', 'replace'),
                               stderr=final_stderr[:max(0, 65536-len(final_stdout))].decode('utf-8', 'replace'),
                               diagnosticsTruncated=len(final_stdout) + len(final_stderr) > 65536,
                               exit=process.returncode)
                row['reaped'] = process.poll() is not None
            row['finishedMonotonic'] = time.monotonic()
    raise AssertionError('unreachable')


def collect(roots, *, deadline, checkpoint, attempts=3, retry_delay=.03,
            max_entries=1000000, max_depth=128):
    """Complete allocated-block union, with stable first-scope inode attribution.

    Every retry is fresh. Incomplete attempts retain their observed subtotal and
    named races, but cannot authorize work. Only child ENOENT is retryable.
    Directory descriptors and O_NOFOLLOW prevent following replaced symlinks.
    The caller supplies all roots, including explicit retained external targets.
    """
    if not roots or not 1 <= attempts <= 5 or not 0 <= retry_delay <= .05:
        raise ValueError('invalid bounded collection policy')
    if max_entries < 1 or not 1 <= max_depth <= 128:
        raise ValueError('invalid traversal bounds')
    records = []
    for number in range(1, attempts + 1):
        row = {'attempt': number, 'complete': False, 'knownBytes': 0,
               'scopes': {key: 0 for key in roots}, 'races': [], 'entries': 0}
        records.append(row)
        seen = set()
        inodes = []

        def check():
            if time.monotonic() >= deadline:
                raise AccountingError('scan deadline', records)
            checkpoint()

        def count(info, scope):
            row['entries'] += 1
            if row['entries'] > max_entries:
                raise AccountingError('scan entry bound', records)
            key = (info.st_dev, info.st_ino)
            if key in seen:
                return False
            seen.add(key)
            size = info.st_blocks * 512
            row['knownBytes'] += size
            row['scopes'][scope] += size
            inodes.append((scope, info.st_dev, info.st_ino, size))
            return True

        def race(path, error):
            if error.errno != errno.ENOENT:
                raise error
            if len(row['races']) >= 64:
                raise AccountingError('scan race evidence bound', records)
            row['races'].append({'path': str(path), 'errno': error.errno})

        def walk(fd, path, scope, depth):
            if depth > max_depth:
                raise AccountingError('scan depth bound', records)
            with os.scandir(fd) as entries:
                for entry in entries:
                    check()
                    child_path = os.path.join(path, entry.name)
                    try:
                        info = os.stat(entry.name, dir_fd=fd, follow_symlinks=False)
                        if stat.S_ISDIR(info.st_mode):
                            child = os.open(entry.name, os.O_RDONLY | os.O_DIRECTORY |
                                            os.O_NOFOLLOW, dir_fd=fd)
                            try:
                                opened = os.fstat(child)
                                if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
                                    raise ValueError('directory identity changed during scan')
                                if count(opened, scope):
                                    walk(child, child_path, scope, depth + 1)
                            finally:
                                os.close(child)
                        else:
                            count(info, scope)
                    except OSError as error:
                        race(child_path, error)
        try:
            for scope, identity in roots.items():
                check()
                identity.verify()
                fd = os.open(identity.scan_path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                try:
                    info = os.fstat(fd)
                    if (info.st_dev, info.st_ino, info.st_uid) != (identity.device, identity.inode, identity.owner):
                        raise ValueError('opened root identity changed')
                    if count(info, scope):
                        walk(fd, identity.scan_path, scope, 0)
                finally:
                    os.close(fd)
                identity.verify()
            for identity in roots.values():
                identity.verify()
            check()
            if not row['races']:
                row['complete'] = True
                return {'complete': True, 'bytes': row['knownBytes'], 'inodes': inodes,
                        'scopes': {key: {'complete': True, 'bytes': size}
                                   for key, size in row['scopes'].items()},
                        'attempts': records}
            if number == attempts:
                raise AccountingError('allocation incomplete after bounded retries', records)
            until = min(deadline, time.monotonic() + retry_delay)
            while time.monotonic() < until:
                check()
                time.sleep(min(.005, max(0, until - time.monotonic())))
        except AccountingError as error:
            if error.attempts is not records:
                row['checkpointFailure'] = error.receipt()
                raise AccountingError(error.reason, records) from error
            raise
        except Exception as error:
            row['cause'] = {'type': type(error).__name__, 'message': str(error),
                            'errno': getattr(error, 'errno', None)}
            raise AccountingError('checkpoint/root/measurement failure', records) from error
        finally:
            row['finishedMonotonic'] = time.monotonic()


@dataclass
class Carry:
    """Conservative high water ledger; pass collect()['scopes'] in fixed order."""
    baseline: dict
    retained_new: int
    prior_whole: int
    prior_new: int
    reservation: int = 0
    growth: dict = field(default_factory=dict)

    def __post_init__(self):
        amounts = [*self.baseline.values(), self.retained_new, self.prior_whole,
                   self.prior_new, self.reservation, *self.growth.values()]
        if not self.baseline or any(type(value) is not int or value < 0 for value in amounts):
            raise ValueError('ledger requires nonnegative integer byte amounts')
        if not set(self.growth).issubset(self.baseline):
            raise ValueError('growth scope not declared')

    def observe(self, complete_samples):
        if tuple(complete_samples) != tuple(self.baseline) or any(v.get('complete') is not True for v in complete_samples.values()):
            raise ValueError('every declared root requires a complete sample')
        for key, sample in complete_samples.items():
            if type(sample['bytes']) is not int or sample['bytes'] < 0:
                raise ValueError('negative allocation')
            self.growth[key] = max(self.growth.get(key, 0), sample['bytes'] - self.baseline[key], 0)
        self.prior_new = max(self.prior_new, self.retained_new + self.reservation + sum(self.growth.values()))
        self.prior_whole = max(self.prior_whole, sum(v['bytes'] for v in complete_samples.values()) + self.reservation)
        return {'new': self.prior_new, 'whole': self.prior_whole, 'positiveGrowth': dict(self.growth)}


def limit_reason(sample, caps, *, now, deadline):
    """Small adoption example; owner retains its existing limits and extra checks."""
    if now >= deadline:
        return 'deadline'
    if sample['free'] < caps['free_floor']:
        return 'free'
    for key in ('new', 'whole', 'processes', 'rss'):
        if sample[key] > caps[key]:
            return key
    return None
