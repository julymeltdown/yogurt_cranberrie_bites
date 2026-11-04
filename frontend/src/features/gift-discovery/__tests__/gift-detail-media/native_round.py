"""Bounded native qualification; execute only inside an accepted resource offer."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import time

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-gifts-media-detail')
FRONT = ROOT / 'frontend'
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
BASE = '5da644264ee354f63e609db64d65a7c5e63c9c11'
MIB = 1024 ** 2
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
OLD = [Path('/Users/lhs/Desktop/mcr-worktrees/fe-profile-preferences') / p for p in [
    'docs/mcr/evidence/fe34-runtime-preferences', 'docs/mcr/evidence/runtime-preferences-root',
    'docs/mcr/evidence/world-runtime-preferences', 'docs/mcr/evidence/community-authority',
    'docs/mcr/evidence/community-adoption', 'frontend/src/world/preferences',
    'frontend/src/app/community-authority', 'frontend/src/app/community-adoption', 'frontend/node_modules']]


def footprint(path):
    if not path.exists() or path.is_symlink():
        return 0
    files = [path] if path.is_file() else path.rglob('*')
    return sum(max(p.stat().st_size, p.stat().st_blocks * 512) for p in files if p.is_file() and not p.is_symlink())


def write(path, value):
    temporary = path.with_suffix('.next')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def process_rows():
    result = {}
    table = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,uid=,rss=,lstart=,args='], text=True)
    for line in table.splitlines():
        fields = line.split(None, 10)
        if len(fields) != 11:
            continue
        pid, parent, group, uid, rss = map(int, fields[:5])
        result[pid] = dict(parent=parent, group=group, uid=uid, rss=rss,
                           started=' '.join(fields[5:10]), command=fields[10])
    return result


def stop_owned(observed, excluded, force=False):
    # A formerly observed process group may now contain a system helper or have
    # been reused. Signal only the same owned PID/UID/start-time identity.
    current = process_rows()
    errors = []
    for key, original in observed.items():
        pid = int(key)
        row = current.get(pid)
        if pid in excluded or not row or row['uid'] != os.getuid():
            continue
        if row['uid'] != original['uid'] or row['started'] != original['started']:
            continue
        try:
            os.kill(pid, signal.SIGKILL if force else signal.SIGTERM)
        except ProcessLookupError:
            pass
        except OSError as error:
            errors.append(dict(pid=pid, error=str(error)))
    return errors


def guardian(prefix, owner, work_end, hard_end):
    peaks = dict(pids=0, rss=0, whole=0, temporary=0, durable=0, new_total=0, other=0)
    minimum_free = shutil.disk_usage(ROOT).free
    observed = {}
    reason = None
    stopped = False
    while time.time() < hard_end:
        state = json.loads((prefix / 'state.json').read_text())
        rows = process_rows()
        selected = {owner}
        for pid, row in rows.items():
            old = observed.get(str(pid))
            if old and old['uid'] == row['uid'] and old['started'] == row['started']:
                selected.add(pid)
        for _ in rows:
            fresh = {pid for pid, row in rows.items() if row['parent'] in selected}
            if fresh <= selected:
                break
            selected.update(fresh)
        live = {pid: row for pid, row in rows.items() if pid in selected}
        observed.update({str(pid): row for pid, row in live.items()})
        rss = sum(row['rss'] * 1024 for row in live.values())
        tmp = footprint(prefix / 'work')
        durable = footprint(prefix) - tmp
        source_growth = max(0, footprint(ROOT) - state['source_bytes'])
        git_growth = max(0, footprint(Path(state['git_dir'])) - state['git_dir_bytes']) if state.get('git_dir') else 0
        offset = state.get('new_total_offset', 0)
        other = offset + source_growth + git_growth
        new_total = other + tmp + durable
        whole = state['carry_bytes'] + new_total - offset
        free = shutil.disk_usage(ROOT).free
        minimum_free = min(minimum_free, free)
        for key, value in dict(pids=len(live), rss=rss, whole=whole, temporary=tmp, durable=durable, new_total=new_total, other=other).items():
            peaks[key] = max(peaks[key], value)
        if not stopped:
            if time.time() >= work_end and state['running'] and not state.get('work_done', False):
                reason = 'work deadline'
            elif len(live) > 48 or rss > 8 * 1024 ** 3 or whole > 2560 * MIB or tmp > 200 * MIB or durable > 32 * MIB or new_total > 256 * MIB or other > 32 * MIB:
                reason = 'process/memory/byte bound'
            elif free < 8 * 1024 ** 3:
                reason = 'disk floor'
            if reason:
                stop_owned(observed, {owner, os.getpid()})
                try:
                    os.kill(owner, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                stopped = True
        write(prefix / 'guard.json', dict(peaks=peaks, minimum_free_bytes=minimum_free, reason=reason,
              last_sample_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
              observed_processes=observed, live_pids=list(live)))
        if not state['running']:
            return
        time.sleep(.2)
    stop_owned(observed, {os.getpid()}, force=True)


def run(prefix, job, pin, accepted_at):
    start = datetime.datetime.fromisoformat(accepted_at).timestamp()
    work_end, hard_end = start + 120, start + 180
    assert time.time() < work_end and not prefix.exists(), 'expired offer or existing output'
    assert shutil.disk_usage(ROOT).free >= 12 * 1024 ** 3, 'start disk floor'
    prefix.mkdir()
    (prefix / 'work').mkdir()
    source_bytes = footprint(ROOT)
    state = dict(running=True, groups=[], git_bytes=0, source_bytes=source_bytes,
                 carry_bytes=source_bytes + footprint(DEPS) + sum(footprint(p) for p in OLD))
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        assigned_port = reservation.getsockname()[1]
    readset = [FRONT / 'src/features/gift-discovery/ui/Gifts.tsx',
               FRONT / 'src/features/gift-discovery/ui/gifts.css',
               FRONT / 'src/features/gift-discovery/ui/reference-gifts.css',
               Path(__file__).parent / 'adoption-inputs/CommunityFeatures.tsx.source',
               Path(__file__).parent / 'adoption-inputs/RootMediaView.tsx.source']
    write(prefix / 'control.json', dict(job=job, pin=pin, accepted_at=accepted_at, assigned_port=assigned_port,
          work_deadline_utc=work_end, hard_deadline_utc=hard_end,
          readset_sha256={str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in readset}))
    write(prefix / 'state.json', state)
    watcher = subprocess.Popen([sys.executable, __file__, 'guard', str(prefix), str(os.getpid()), str(work_end), str(hard_end)])
    child = None
    entries = []
    failure = None
    began = time.monotonic()

    def interrupt(_signum, _frame):
        raise RuntimeError('independent guard interrupted the workload')

    signal.signal(signal.SIGTERM, interrupt)
    try:
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()
        assert git('rev-parse', 'HEAD') == pin and not git('status', '--porcelain'), 'source pin or dirty source'
        objects = '\n'.join(line.split()[0] for line in git('rev-list', '--objects', f'{BASE}..HEAD').splitlines()) + '\n'
        sizes = subprocess.check_output(['git', 'cat-file', '--batch-check=%(objectsize:disk)'], input=objects, cwd=ROOT, text=True)
        git_dir = Path(git('rev-parse', '--git-dir'))
        state['git_dir'] = str(git_dir)
        state['git_dir_bytes'] = footprint(git_dir)
        state['git_bytes'] = sum(int(n) for n in sizes.splitlines() if n.isdigit()) + footprint(git_dir)
        state['carry_bytes'] += state['git_bytes']
        state['new_total_offset'] = max(0, state['carry_bytes'] - int(os.environ.get('MCR_GIFTS_ADMISSION_BASELINE_BYTES', state['carry_bytes'])))
        write(prefix / 'state.json', state)
        for path in [FRONT / 'pnpm-lock.yaml', Path('/Users/lhs/Desktop/mcr/frontend/pnpm-lock.yaml')]:
            assert hashlib.sha256(path.read_bytes()).hexdigest() == LOCK, 'lock pin'
        assert not (FRONT / 'node_modules').exists(), 'unexpected existing dependency path'
        (FRONT / 'node_modules').symlink_to(DEPS, target_is_directory=True)
        env = {**os.environ, 'PATH': str(Path(NODE).parent) + ':' + os.environ['PATH'], 'TMPDIR': str(prefix / 'work'),
               'TMP': str(prefix / 'work'), 'TEMP': str(prefix / 'work'), 'XDG_CACHE_HOME': str(prefix / 'work/cache'),
               'MCR_GIFTS_MEDIA_TEMP': str(prefix / 'work'), 'CI': 'true', 'RAYON_NUM_THREADS': '1'}
        version = subprocess.check_output([NODE, '--version'], env=env, text=True).strip()
        assert version == 'v24.22.2', 'runtime version'
        env['MCR_GIFTS_MEDIA_NATIVE_OUTPUT'] = str(prefix / 'work/native')
        env['MCR_GIFTS_NATIVE_PIN'] = pin
        env['MCR_GIFTS_NATIVE_PORT'] = str(assigned_port)
        commands = [[NODE, '--check', 'src/features/gift-discovery/__tests__/gift-detail-media/native/native.mjs'],
                    [NODE, 'src/features/gift-discovery/__tests__/gift-detail-media/native/native.mjs']]
        for i, command in enumerate(commands):
            assert time.time() < work_end, 'work deadline before command'
            at = datetime.datetime.now(datetime.timezone.utc).isoformat()
            with (prefix / f'{i + 1}.log').open('wb') as log:
                child = subprocess.Popen(command, cwd=FRONT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                state['groups'].append(child.pid)
                write(prefix / 'state.json', state)
                code = child.wait(timeout=max(.1, work_end - time.time()))
            entry = dict(command=command, started_at=at, exit_code=code, log=f'{i + 1}.log')
            entries.append(entry)
            print(json.dumps(entry), flush=True)
            if code:
                raise RuntimeError(f'command {i + 1} exited {code}')
    except BaseException as error:
        failure = str(error)
    finally:
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        state['work_done'] = True
        write(prefix / 'state.json', state)
        observed = json.loads((prefix / 'guard.json').read_text()).get('observed_processes', {}) if (prefix / 'guard.json').exists() else {}
        errors = stop_owned(observed, {os.getpid(), watcher.pid})
        if child:
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        errors += stop_owned(observed, {os.getpid(), watcher.pid}, force=True)
        if errors:
            failure = failure or 'Owned-process cleanup signal errors'
            write(prefix / 'cleanup-signal-errors.json', errors)
        native = prefix / 'work/native'
        if native.exists():
            retained = prefix / 'native'; retained.mkdir()
            for artifact in native.iterdir():
                if artifact.is_file() and artifact.suffix in ('.png', '.json'): shutil.move(str(artifact), retained / artifact.name)
        shutil.rmtree(prefix / 'work')
        dependency_link = FRONT / 'node_modules'
        if dependency_link.is_symlink() and dependency_link.resolve() == DEPS: dependency_link.unlink()
        state['running'] = False
        write(prefix / 'state.json', state)
        watcher.wait(timeout=max(1, hard_end - time.time()))
        guard = json.loads((prefix / 'guard.json').read_text()) if (prefix / 'guard.json').exists() else {'reason': 'missing guard evidence'}
        failure = failure or guard.get('reason')
        report = dict(job=job, start_floor_bytes=12 * 1024 ** 3, new_total_limit_bytes=256 * MIB, source_revision=pin, accepted_at=accepted_at, work_deadline_utc=start + 120, hard_deadline_utc=hard_end,
                      commands=entries, failure=failure, elapsed_seconds=time.monotonic() - began, guard=guard,
                      retained_dependencies='ROOT_READ_ONLY_PRESERVED_TEMP_LINK_REMOVED', own_temp_removed=not (prefix / 'work').exists(),
                      charged_carry_paths=[str(ROOT), str(DEPS), *map(str, OLD)], git_bytes=state['git_bytes'], carry_bytes=state['carry_bytes'],
                      finished_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        write(prefix / 'report.json', report)
        print(json.dumps(report), flush=True)
    return 1 if failure else 0


if __name__ == '__main__':
    if sys.argv[1] == 'guard':
        guardian(Path(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4]), float(sys.argv[5]))
    else:
        raise SystemExit(run(Path(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]))
