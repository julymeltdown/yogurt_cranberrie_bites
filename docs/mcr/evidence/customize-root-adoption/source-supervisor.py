"""Run only inside a fresh explicitly accepted Root character source allocation."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import tempfile
import time

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish')
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'


def git(*args):
    return sp.check_output(['git', *args], cwd=ROOT, text=True).strip()


def size(root):
    return sum(p.lstat().st_size for p in root.rglob('*') if p.is_file() or p.is_symlink()) if root.exists() else 0


def process_tree(pid):
    rows = [tuple(map(int, r.split())) for r in sp.check_output(['ps', '-axo', 'pid=,ppid=,rss='], text=True).splitlines() if r.strip()]
    ids = {pid}
    while True:
        more = ids | {p for p, parent, _ in rows if parent in ids}
        if more == ids:
            break
        ids = more
    return ids, sum(rss * 1024 for p, _, rss in rows if p in ids)


def stop(child):
    if child.poll() is None:
        os.killpg(child.pid, signal.SIGTERM)
        try:
            child.wait(timeout=3)
        except sp.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=3)


def main():
    slot = os.environ['MCR_ROOT_CHAR_ACCEPTED_SLOT']
    pin = os.environ['MCR_ROOT_CHAR_PIN']
    deadline = dt.datetime.fromisoformat(os.environ['MCR_ROOT_CHAR_DEADLINE']).timestamp()
    carry = int(os.environ['MCR_ROOT_CHAR_CARRY_BYTES'])
    output = (ROOT / os.environ['MCR_ROOT_CHAR_OUTPUT']).resolve()
    assert slot.startswith('ROOT-CHAR-SOURCE-') and pin == git('rev-parse', 'HEAD') and not git('status', '--porcelain')
    assert output.is_relative_to(ROOT / 'docs/mcr/evidence/customize-root-adoption') and not output.exists()
    assert 90 < deadline - time.time() <= 420
    assert hashlib.sha256((ROOT / 'frontend/pnpm-lock.yaml').read_bytes()).hexdigest() == LOCK
    assert shutil.disk_usage(ROOT).free >= 16 * 1024**3
    link = ROOT / 'frontend/node_modules'
    assert not link.exists() and not link.is_symlink()
    output.mkdir(parents=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-root-char-source-', dir='/private/tmp'))
    env = os.environ.copy()
    env.update(TMPDIR=str(tmp), XDG_CACHE_HOME=str(tmp / 'cache'), MCR_MOTION_CACHE=str(tmp / 'vite'),
               MCR_FRONTEND_DEPENDENCIES=str(DEPS.parent), PATH=str(NODE.parent) + ':' + env.get('PATH', ''))
    commands = [
        ('tests', [str(NODE), str(DEPS / 'vitest/vitest.mjs'), 'run', '--configLoader', 'runner', '--config',
                   'src/app/character-entry/__tests__/reviewed-production/vitest.config.mjs', '--no-cache', '--reporter', 'dot']),
        ('types', [str(NODE), '--max-old-space-size=2048', str(DEPS / 'typescript/bin/tsc'), '--noEmit', '--incremental', 'false', '-p', 'tsconfig.json']),
        ('lint', [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings',
                  'src/app/character-entry', 'src/app/account-entry', 'src/app/CommunityFeatures.tsx',
                  'src/app/world-entry/production.ts', 'src/app/world-entry/resolver.ts',
                  'src/features/avatar/motion-adoption', 'src/world/avatar/motion-adoption/renderer.ts',
                  'src/app/errors', 'src/features/connection-status', 'src/main.tsx',
                  'src/app/WorldShell.tsx', 'src/app/router.tsx', 'src/app/ports.ts']),
    ]
    report = {'slot': slot, 'source': pin, 'qualification': 'SOURCE_ONLY_NOT_NATIVE_OR_ACTIVATION', 'carryBytes': carry,
              'commands': [], 'wholePeak': carry, 'temporaryPeak': 0, 'durablePeak': 0, 'rssPeak': 0,
              'freeMinimum': shutil.disk_usage(ROOT).free, 'started': dt.datetime.now(dt.timezone.utc).isoformat()}
    child = None
    owned_pids = set()
    status = 'FAIL'
    try:
        link.symlink_to(DEPS, target_is_directory=True)
        for label, argv in commands:
            if time.time() >= deadline - 60:
                raise RuntimeError('Cleanup reserve reached before next child')
            log = output / (label + '.log')
            with log.open('wb') as stream:
                started = time.time()
                child = sp.Popen(argv, cwd=ROOT / 'frontend', env=env, stdout=stream, stderr=sp.STDOUT, start_new_session=True)
                row = {'label': label, 'argv': argv, 'pid': child.pid, 'startedEpoch': started}
                report['commands'].append(row)
                owned_pids.add(child.pid)
                while child.poll() is None:
                    ids, rss = process_tree(child.pid)
                    owned_pids.update(ids)
                    temporary, durable = size(tmp), size(output)
                    free = shutil.disk_usage(ROOT).free
                    report['wholePeak'] = max(report['wholePeak'], carry + temporary + durable)
                    report['temporaryPeak'] = max(report['temporaryPeak'], temporary)
                    report['durablePeak'] = max(report['durablePeak'], durable)
                    report['rssPeak'] = max(report['rssPeak'], rss)
                    report['freeMinimum'] = min(report['freeMinimum'], free)
                    if (time.time() >= deadline - 60 or time.time() - started >= 180
                            or carry + temporary + durable > 224 * 1024**2 or temporary > 48 * 1024**2
                            or durable > 16 * 1024**2 or rss > int(2.5 * 1024**3) or len(ids) > 24 or free < 8 * 1024**3):
                        stop(child)
                        raise RuntimeError('Time/whole/TMP/durable/RSS/process/free floor bound')
                    time.sleep(.2)
                row.update(exit=child.returncode, endedEpoch=time.time())
                if child.returncode:
                    raise RuntimeError(label + ' exited ' + str(child.returncode))
        status = 'PASS'
    except BaseException as exc:
        report['failure'] = type(exc).__name__ + ': ' + str(exc)
    finally:
        if child is not None:
            stop(child)
        if link.is_symlink() and link.resolve() == DEPS:
            link.unlink()
        shutil.rmtree(tmp)
        live_pids = {int(row.strip()) for row in sp.check_output(['ps', '-axo', 'pid='], text=True).splitlines() if row.strip()}
        report['ownedPids'] = sorted(owned_pids)
        report['aliveOwnedPids'] = sorted(owned_pids & live_pids)
        if report['aliveOwnedPids']:
            status = 'FAIL'
        report.update(status=status, ended=dt.datetime.now(dt.timezone.utc).isoformat(), linkAbsent=not link.is_symlink() and not link.exists(),
                      privateTempAbsent=not tmp.exists(), finalFree=shutil.disk_usage(ROOT).free)
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))
    return 0 if status == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
