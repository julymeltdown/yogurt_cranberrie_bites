#!/usr/bin/env python3
"""Independent bounded FE37 guardian; execute only under a fresh direct named offer."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
BASE = ROOT / 'docs/mcr/evidence/operations-fe37-fe38-current'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
GIB = 1024 ** 3


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def deadline(name):
    return datetime.datetime.fromisoformat(os.environ[name].replace('Z', '+00:00')).timestamp()


def free():
    v = os.statvfs(ROOT)
    return v.f_bavail * v.f_frsize


def size(path):
    if not path.exists(): return 0
    if path.is_file(): return path.stat().st_size
    total = 0
    for p in path.rglob('*'):
        try:
            if p.is_file() and not p.is_symlink(): total += p.stat().st_size
        except FileNotFoundError: pass
    return total


def allocated(path):
    return int(subprocess.check_output(['/usr/bin/du', '-sk', str(path)], text=True).split()[0]) * 1024


def processes(groups):
    rows = [list(map(int, line.split())) for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines()]
    selected = {os.getpid()}
    while True:
        found = selected | {pid for pid, parent, group, _ in rows if parent in selected or group in groups}
        if found == selected: break
        selected = found
    live = []
    for pid, _, group, rss in rows:
        if pid not in selected: continue
        try: os.kill(pid, 0)
        except ProcessLookupError: continue
        live.append((pid, group, rss * 1024))
    return live


def main():
    mode = os.environ['MCR_OPS_CLASS']; assert mode in ['source', 'native']
    run = os.environ['MCR_OPS_RUN']; assert run.startswith(mode + '-') and all(c.islower() or c.isdigit() or c == '-' for c in run)
    work, hard = deadline('MCR_OPS_WORK_UTC'), deadline('MCR_OPS_HARD_UTC')
    accepted = deadline('MCR_OPS_ACCEPTED_UTC')
    assert accepted <= time.time() < work < hard and hard - work >= 20
    caps = {'wholeBytes': 2 * GIB, 'newBytes': (128 if mode == 'source' else 200) * 1024 ** 2,
            'temporaryBytes': (96 if mode == 'source' else 160) * 1024 ** 2,
            'durableBytes': (32 if mode == 'source' else 40) * 1024 ** 2,
            'rssBytes': 4 * GIB, 'processes': 32}
    admitted = int(os.environ['MCR_OPS_ACCEPT_FREE_BYTES']); reserved = int(os.environ['MCR_OPS_OTHER_RESERVE_BYTES'])
    assert admitted >= 12 * GIB and reserved >= 0 and admitted - reserved - caps['newBytes'] >= 9 * GIB
    assert free() >= 8 * GIB
    assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT).strip()
    link = ROOT / 'frontend/node_modules'; assert not os.path.lexists(link)
    assert (DEPS / '.pnpm/lock.yaml').read_bytes() == (ROOT / 'frontend/pnpm-lock.yaml').read_bytes()
    sources = []
    for lane in ['reports', 'members', 'content']:
        sources += [p for p in (ROOT / 'frontend/src/features/admin' / lane).rglob('*') if p.suffix in ['.ts', '.tsx', '.css']]
    sources += [BASE / 'native.mjs', Path(__file__).resolve(), ROOT / 'frontend/pnpm-lock.yaml', ROOT / 'frontend/tsconfig.json',
                ROOT / 'frontend/src/app/AdminBoundary.tsx', ROOT / 'frontend/src/app/ports.ts', ROOT / 'frontend/src/app/router.tsx',
                ROOT / 'contracts/domain/v1/admin.schema.json', ROOT / 'contracts/domain/v1/operations.json', ROOT / 'contracts/generated/mcr-domain.openapi.json']
    pins = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}
    git_metadata = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
    retained = {'checkoutBytes': allocated(ROOT), 'gitMetadataBytes': allocated(git_metadata),
                'sharedGitCommonDirReadOnly': subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT, text=True).strip(),
                'copiedGitCarryBytes': 0}
    baseline = retained['checkoutBytes'] + retained['gitMetadataBytes']
    assert baseline < caps['wholeBytes']
    output = BASE / 'runs' / run; output.mkdir(parents=True, exist_ok=False)
    temporary = Path(tempfile.mkdtemp(prefix='mcr-operations-current-', dir='/private/tmp'))
    report = {'allocation': os.environ['MCR_OPS_ALLOCATION'], 'class': mode, 'candidate': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'acceptedUtc': os.environ['MCR_OPS_ACCEPTED_UTC'], 'acceptedFreeBytes': admitted, 'otherReserveBytes': reserved,
              'workUtc': os.environ['MCR_OPS_WORK_UTC'], 'hardUtc': os.environ['MCR_OPS_HARD_UTC'], 'sourceSha256': pins,
              'guardianPid': os.getpid(), 'standingBeforeChildrenUtc': utc(), 'retained': retained, 'caps': caps, 'peaks': {k: 0 for k in caps}, 'commands': [], 'status': 'FAIL'}
    groups = set(); child = None
    try:
        link.symlink_to(DEPS, target_is_directory=True)
        env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ['PATH'], 'MCR_OPS_SOURCE_TMP': str(temporary),
               'MCR_OPS_OUTPUT': str(output), 'TMPDIR': str(temporary), 'TMP': str(temporary), 'TEMP': str(temporary), 'XDG_CACHE_HOME': str(temporary / 'cache')}
        commands = [[str(NODE), str(BASE / 'native.mjs')]] if mode == 'native' else [
            [str(NODE), str(DEPS / 'vitest/vitest.mjs'), 'run', '--config', 'src/features/admin/reports/__tests__/current/runner.ts', '--configLoader', 'runner', '--no-cache', '--maxWorkers=1', '--reporter=json', '--outputFile=' + str(output / 'vitest.json')],
            [str(NODE), str(DEPS / 'typescript/bin/tsc'), '-p', 'tsconfig.json', '--noEmit', '--incremental', 'false', '--allowImportingTsExtensions', '--pretty', 'false'],
            [str(DEPS / '.bin/oxlint'), '--deny-warnings', 'src/features/admin/reports/model/controller.ts', 'src/features/admin/reports/__tests__/current', str(BASE / 'native.mjs')],
            [str(NODE), '--check', str(BASE / 'native.mjs')]]
        for index, argv in enumerate(commands):
            assert time.time() < work and free() >= 8 * GIB
            began = time.monotonic(); item = {'argv': argv, 'cwd': '.' if mode == 'native' else 'frontend'}; report['commands'].append(item)
            with (output / f'{index:02}.log').open('wb') as log:
                child = subprocess.Popen(argv, cwd=ROOT if mode == 'native' else ROOT / 'frontend', env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                groups.add(child.pid)
                while child.poll() is None:
                    rows = processes(groups)
                    for pid, group, _ in rows:
                        if pid != os.getpid() and group != os.getpgrp(): groups.add(group)
                    temporary_bytes, durable_bytes = allocated(temporary), allocated(BASE)
                    whole = allocated(ROOT) + allocated(git_metadata) + temporary_bytes
                    sample = {'wholeBytes': whole, 'newBytes': max(0, whole - baseline), 'temporaryBytes': temporary_bytes, 'durableBytes': durable_bytes,
                              'rssBytes': sum(rss for _, _, rss in rows), 'processes': len(rows)}
                    for key, value in sample.items():
                        report['peaks'][key] = max(report['peaks'][key], value)
                        assert value <= caps[key], 'Resource cap: ' + key
                    assert free() >= 8 * GIB and time.time() < work, 'Live free-space floor/work deadline'
                    time.sleep(.25)
            item.update(exitCode=child.returncode, elapsedSeconds=time.monotonic() - began)
            assert child.returncode == 0, 'Command failed; original log retained'
        assert pins == {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}
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
            try: child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                for group in groups:
                    try: os.killpg(group, signal.SIGKILL)
                    except ProcessLookupError: pass
                child.wait(timeout=2)
        for pid, _, _ in processes(groups):
            if pid != os.getpid():
                try: os.kill(pid, signal.SIGKILL)
                except ProcessLookupError: pass
        if link.is_symlink() and link.readlink() == DEPS: link.unlink()
        shutil.rmtree(temporary)
        remaining = [pid for pid, _, _ in processes(groups) if pid != os.getpid()]
        report.update(fullUtc=utc(), cleanupSeconds=time.monotonic() - cleanup, remainingProcesses=remaining,
                      dependencyLinkAbsent=not os.path.lexists(link), temporaryAbsent=not temporary.exists(), freeAfterBytes=free(),
                      finishedBeforeHardDeadline=time.time() < hard)
        report['released'] = not remaining and report['dependencyLinkAbsent'] and report['temporaryAbsent'] and report['finishedBeforeHardDeadline']
        if not report['released']: report['status'] = 'FAIL'
        report['logSha256'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in output.glob('*.log')}
        (output / 'release.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2), flush=True)
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
