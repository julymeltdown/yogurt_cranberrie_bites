"""Bounded source qualification; execute only inside an accepted resource offer."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-profile-preferences')
FRONT = ROOT / 'frontend'
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
BASE = '32fed3eb41e2b57783cebb78e4aff11d3fa087ba'
MIB = 1024 ** 2
OWNED = [ROOT / 'frontend/src/app/community-adoption', ROOT / 'frontend/src/app/WorldShell.tsx',
         ROOT / 'frontend/src/app/CommunityFeatures.tsx', ROOT / 'frontend/src/app/ports.ts', ROOT / 'frontend/src/app/router.tsx',
         ROOT / 'frontend/src/app/community-authority', ROOT / 'docs/mcr/frontend/community-authority.md',
         ROOT / 'docs/mcr/evidence/community-authority']
# Conservatively charge retained evidence from the prior completed assignment too.
OLD = [ROOT / 'docs/mcr/evidence/fe34-runtime-preferences', ROOT / 'docs/mcr/evidence/runtime-preferences-root',
       ROOT / 'docs/mcr/evidence/world-runtime-preferences', ROOT / 'frontend/src/world/preferences']


def footprint(path):
    if not path.exists() or path.is_symlink():
        return 0
    files = [path] if path.is_file() else path.rglob('*')
    return sum(max(p.stat().st_size, p.stat().st_blocks * 512) for p in files if p.is_file() and not p.is_symlink())


def write(path, value):
    temporary = path.with_suffix('.next')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def guardian(prefix, owner, work_end, hard_end):
    peaks = dict(pids=0, rss=0, whole=0, temporary=0, durable=0)
    minimum_free = shutil.disk_usage(ROOT).free
    groups = set()
    reason = None
    stopped = False
    while time.time() < hard_end:
        try:
            state = json.loads((prefix / 'state.json').read_text())
            groups.update(state.get('groups', []))
            table = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True)
            rows = [tuple(map(int, line.split())) for line in table.splitlines() if line.strip()]
            selected = {owner}
            for _ in range(len(rows)):
                fresh = {p for p, parent, group, _ in rows if parent in selected or group in groups}
                if fresh <= selected:
                    break
                selected.update(fresh)
            live = [(p, rss) for p, _, _, rss in rows if p in selected]
            rss = sum(value * 1024 for _, value in live)
            tmp = footprint(prefix / 'work')
            durable = footprint(prefix) - tmp
            carry = sum(footprint(p) for p in OWNED + OLD) + state['git_bytes']
            whole = carry + tmp + durable
            free = shutil.disk_usage(ROOT).free
            minimum_free = min(minimum_free, free)
            for key, value in dict(pids=len(live), rss=rss, whole=whole, temporary=tmp, durable=durable).items():
                peaks[key] = max(peaks[key], value)
            if not stopped:
                if time.time() >= work_end and state['running']:
                    reason = 'work deadline'
                elif len(live) > 4 or rss > 2.5 * 1024 ** 3 or whole > 128 * MIB or tmp > 100 * MIB or durable > 8 * MIB:
                    reason = 'process/memory/byte bound'
                elif free < 8 * 1024 ** 3:
                    reason = 'disk floor'
                if reason:
                    for group in groups:
                        try:
                            os.killpg(group, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                    os.kill(owner, signal.SIGTERM)
                    stopped = True
            write(prefix / 'guard.json', dict(peaks=peaks, minimum_free_bytes=minimum_free, reason=reason,
                                               last_sample_utc=datetime.datetime.now(datetime.timezone.utc).isoformat()))
            if not state['running']:
                return
        except ProcessLookupError:
            break
        time.sleep(.2)
    for group in groups:
        try:
            os.killpg(group, signal.SIGKILL)
        except ProcessLookupError:
            pass


def run(prefix, job, pin, accepted_at):
    start = datetime.datetime.fromisoformat(accepted_at).timestamp()
    work_end, hard_end = start + 120, start + 180
    assert time.time() < work_end and not prefix.exists(), 'expired offer or existing output'
    assert shutil.disk_usage(ROOT).free >= 16 * 1024 ** 3, 'start disk floor'
    prefix.mkdir()
    (prefix / 'work').mkdir()
    state = dict(running=True, groups=[], git_bytes=0)
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
        state['git_bytes'] = sum(int(n) for n in sizes.splitlines()) + footprint(git_dir)
        write(prefix / 'state.json', state)
        for path in [FRONT / 'pnpm-lock.yaml', Path('/Users/lhs/Desktop/mcr/frontend/pnpm-lock.yaml')]:
            assert hashlib.sha256(path.read_bytes()).hexdigest() == LOCK, 'lock pin'
        assert (FRONT / 'node_modules').is_dir() and not (FRONT / 'node_modules').is_symlink(), 'preserve pre-existing physical dependencies'
        env = {**os.environ, 'PATH': str(Path(NODE).parent) + ':' + os.environ['PATH'], 'TMPDIR': str(prefix / 'work'),
               'TMP': str(prefix / 'work'), 'TEMP': str(prefix / 'work'), 'XDG_CACHE_HOME': str(prefix / 'work/cache'),
               'MCR_COMMUNITY_AUTHORITY_TEMP': str(prefix / 'work'), 'CI': 'true', 'RAYON_NUM_THREADS': '1'}
        version = subprocess.check_output([NODE, '--version'], env=env, text=True).strip()
        assert version == 'v24.22.2', 'runtime version'
        commands = [
            [NODE, 'node_modules/vitest/vitest.mjs', 'run', '--config', 'src/app/community-adoption/__tests__/vitest.config.ts',
             '--configLoader', 'runner', '--pool=threads', '--cache=false', '--no-file-parallelism', '--maxWorkers=1'],
            [NODE, 'node_modules/typescript/bin/tsc', '--project', 'tsconfig.json', '--noEmit', '--pretty', 'false', '--incremental', 'false'],
            [NODE, 'node_modules/oxlint/bin/oxlint', 'src/app/community-authority', 'src/app/community-adoption', 'src/app/WorldShell.tsx', 'src/app/CommunityFeatures.tsx', 'src/app/ports.ts', 'src/app/router.tsx'],
            [NODE, '--check', 'src/app/community-adoption/__tests__/native/native.mjs'],
        ]
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
        for group in state['groups']:
            try:
                os.killpg(group, signal.SIGTERM)
            except ProcessLookupError:
                pass
        if child:
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
        shutil.rmtree(prefix / 'work')
        state['running'] = False
        write(prefix / 'state.json', state)
        watcher.wait(timeout=max(1, hard_end - time.time()))
        guard = json.loads((prefix / 'guard.json').read_text()) if (prefix / 'guard.json').exists() else {'reason': 'missing guard evidence'}
        failure = failure or guard.get('reason')
        report = dict(job=job, source_revision=pin, accepted_at=accepted_at, work_deadline_utc=start + 120, hard_deadline_utc=hard_end,
                      commands=entries, failure=failure, elapsed_seconds=time.monotonic() - began, guard=guard,
                      retained_dependencies='PREEXISTING_READ_ONLY_PRESERVED', own_temp_removed=not (prefix / 'work').exists(),
                      charged_carry_paths=[str(p.relative_to(ROOT)) for p in OWNED + OLD], git_bytes=state['git_bytes'],
                      finished_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        write(prefix / 'report.json', report)
        print(json.dumps(report), flush=True)
    return 1 if failure else 0


if __name__ == '__main__':
    if sys.argv[1] == 'guard':
        guardian(Path(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4]), float(sys.argv[5]))
    else:
        raise SystemExit(run(Path(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]))
