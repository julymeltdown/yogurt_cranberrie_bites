#!/usr/bin/env python3
"""One explicitly accepted functional SOURCE allocation; no install/build/browser."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[6]
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
EVIDENCE = ROOT / 'docs/mcr/evidence/media-natural-geometry'
GIB = 1024 ** 3
CAPS = {'wholeBytes': 256 * 1024 ** 2, 'temporaryBytes': 192 * 1024 ** 2,
        'durableBytes': 48 * 1024 ** 2, 'rssBytes': 4 * GIB, 'processes': 32}


def deadline(name):
    return datetime.datetime.fromisoformat(os.environ[name].replace('Z', '+00:00')).timestamp()


def free():
    value = os.statvfs(ROOT)
    return value.f_bavail * value.f_frsize


def size(path):
    if not path.exists():
        return 0
    return sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink()) if path.is_dir() else path.stat().st_size


def descendants():
    rows = [list(map(int, row.split())) for row in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss='], text=True).splitlines()]
    selected = {os.getpid()}
    while True:
        found = selected | {pid for pid, parent, _ in rows if parent in selected}
        if found == selected:
            break
        selected = found
    alive = []
    for pid, _, rss in rows:
        if pid in selected:
            try: os.kill(pid, 0)
            except ProcessLookupError: continue
            alive.append((pid, rss * 1024))
    return alive


def main():
    allocation = os.environ['C06_ALLOCATION']
    work, hard = deadline('C06_WORK_DEADLINE_UTC'), deadline('C06_HARD_DEADLINE_UTC')
    accepted = deadline('C06_ACCEPTED_AT_UTC')
    assert accepted <= time.time() < work < hard and hard - work >= 20
    assert int(os.environ['C06_ACCEPT_FREE_BYTES']) >= 16 * GIB
    assert free() >= 8 * GIB
    run = os.environ['C06_RUN']
    assert run.startswith('source-') and all(c.islower() or c.isdigit() or c == '-' for c in run)
    assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT).strip()
    link = ROOT / 'frontend/node_modules'
    assert not link.exists() and not link.is_symlink()
    assert (DEPS / '.pnpm/lock.yaml').read_bytes() == (ROOT / 'frontend/pnpm-lock.yaml').read_bytes()
    output = EVIDENCE / run
    output.mkdir(parents=True, exist_ok=False)
    temporary = Path(tempfile.mkdtemp(prefix='mcr-c06-media-source-', dir='/private/tmp'))
    report = {'allocation': allocation, 'candidate': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'actualAcceptedAtUtc': os.environ['C06_ACCEPTED_AT_UTC'], 'actualAcceptanceFreeBytes': int(os.environ['C06_ACCEPT_FREE_BYTES']),
              'workDeadlineUtc': os.environ['C06_WORK_DEADLINE_UTC'], 'hardDeadlineUtc': os.environ['C06_HARD_DEADLINE_UTC'],
              'commands': [], 'caps': CAPS, 'peaks': {key: 0 for key in CAPS}, 'status': 'FAIL', 'failure': None}
    source_paths = ['frontend/src/app/media-entry/RootMediaView.tsx',
                    'frontend/src/app/media-entry/__tests__/root-binding/provider.test.tsx',
                    'frontend/src/app/media-entry/__tests__/natural-geometry/vitest.config.mjs',
                    'frontend/src/app/media-entry/__tests__/natural-geometry/allocated-source.py',
                    'frontend/src/app/media-entry/provider.tsx', 'frontend/src/app/media-entry/index.ts',
                    'frontend/src/shared/query/privacy.ts', 'frontend/src/shared/media/read-media.ts',
                    'frontend/src/app/media-entry/root-media.css', 'frontend/tsconfig.json', 'frontend/vite.config.ts', 'frontend/pnpm-lock.yaml']
    pins = {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in source_paths}
    report['sourceSha256'] = pins
    started = time.monotonic()
    child = None
    groups = []
    try:
        link.symlink_to(DEPS, target_is_directory=True)
        env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ['PATH'], 'TMPDIR': str(temporary),
               'TMP': str(temporary), 'TEMP': str(temporary), 'C06_MEDIA_SOURCE_TMP': str(temporary),
               'XDG_CACHE_HOME': str(temporary / 'cache'), 'PYTHONDONTWRITEBYTECODE': '1'}
        tests = ['src/app/media-entry/__tests__/owner.test.ts', 'src/app/media-entry/__tests__/root-binding/provider.test.tsx',
                 'src/app/media-entry/__tests__/root-binding/rows.test.tsx', 'src/app/media-entry/__tests__/profile-composition/layered-fallback.test.tsx']
        commands = [[str(NODE), str(DEPS / 'vitest/vitest.mjs'), 'run', *tests,
                     '--config', 'src/app/media-entry/__tests__/natural-geometry/vitest.config.mjs', '--configLoader', 'runner',
                     '--no-cache', '--maxWorkers=1', '--reporter=json', '--outputFile=' + str(output / 'vitest.json')],
                    [str(NODE), str(DEPS / 'typescript/bin/tsc'), '-p', 'tsconfig.json', '--noEmit', '--incremental', 'false', '--pretty', 'false', '--allowImportingTsExtensions'],
                    [str(DEPS / '.bin/oxlint'), '--deny-warnings', 'src/app/media-entry/RootMediaView.tsx',
                     'src/app/media-entry/__tests__/root-binding/provider.test.tsx', 'src/app/media-entry/__tests__/natural-geometry/vitest.config.mjs']]
        for index, argv in enumerate(commands):
            assert time.time() < work and free() >= 8 * GIB
            began = time.monotonic()
            record = {'argv': argv, 'cwd': 'frontend', 'exitCode': None}
            report['commands'].append(record)
            with (output / f'{index:02}.log').open('wb') as log:
                child = subprocess.Popen(argv, cwd=ROOT / 'frontend', env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                groups.append(child.pid)
                while child.poll() is None:
                    current = descendants()
                    temporary_bytes, durable_bytes = size(temporary), size(EVIDENCE)
                    owned = sum(size(ROOT / p) for p in source_paths[:4]) + size(ROOT / 'docs/mcr/frontend/media-natural-geometry.md')
                    sample = {'wholeBytes': temporary_bytes + durable_bytes + owned, 'temporaryBytes': temporary_bytes,
                              'durableBytes': durable_bytes, 'rssBytes': sum(rss for _, rss in current), 'processes': len(current)}
                    for key, value in sample.items():
                        report['peaks'][key] = max(report['peaks'][key], value)
                        assert value <= CAPS[key], 'Resource cap: ' + key
                    assert free() >= 8 * GIB and time.time() < work, 'Continuous free-space floor/work deadline'
                    time.sleep(0.25)
            record.update(exitCode=child.returncode, elapsedSeconds=time.monotonic() - began)
            assert child.returncode == 0, f'Command {index} failed; exact log retained'
        assert pins == {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in source_paths}
        assert not subprocess.check_output(['git', 'diff', '--name-only', 'HEAD'], cwd=ROOT).strip()
        report['status'] = 'PASS'
    except Exception as error:
        report['failure'] = str(error)
    finally:
        cleanup = time.monotonic()
        for group in groups:
            try: os.killpg(group, signal.SIGTERM)
            except ProcessLookupError: pass
        if child and child.poll() is None:
            try: child.wait(timeout=2)
            except subprocess.TimeoutExpired: os.killpg(child.pid, signal.SIGKILL); child.wait(timeout=2)
        for pid, _ in descendants():
            if pid != os.getpid():
                try: os.kill(pid, signal.SIGKILL)
                except ProcessLookupError: pass
        if link.is_symlink() and link.readlink() == DEPS:
            link.unlink()
        import shutil
        shutil.rmtree(temporary)
        report.update(elapsedSeconds=time.monotonic() - started, cleanupSeconds=time.monotonic() - cleanup,
                      dependencyLinkAbsent=not link.exists() and not link.is_symlink(), temporaryAbsent=not temporary.exists(),
                      remainingProcesses=[pid for pid, _ in descendants() if pid != os.getpid()], freeAfterBytes=free(),
                      finishedBeforeHardDeadline=time.time() < hard)
        report['released'] = report['dependencyLinkAbsent'] and report['temporaryAbsent'] and not report['remainingProcesses']
        if not report['released'] or not report['finishedBeforeHardDeadline']:
            report['status'] = 'FAIL'
        report['logSha256'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in output.glob('*.log')}
        (output / 'release.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2), flush=True)
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
