"""Source-only FE16 provenance qualification; no application execution or dependencies."""
import argparse
import hashlib
import json
import os
import re
import resource
import shutil
import signal
import subprocess
import sys
import time
import traceback
from datetime import datetime, timezone
from datetime import timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent
BASE = 'db661820d6af1770839cfda8d0dc6b9ada1ece1b'
OLD = '5863f915eae1526786859876d22a781b48ca5f7c'
CURRENT = '7a94c5e7a4754ee178405be6682cf61218f61bcf'
RUN_SOURCE = '2d7a42e7d9fc645a3d956cd552304539be602be7'
NETWORK = 'frontend/src/world/network/'
DEADLINE = None
GIT_ADMIN = None
WHOLE_LIMIT = 0
LAST_WHOLE_CHECK = float('-inf')


def allocated(path):
    total = path.lstat().st_blocks * 512
    if path.is_dir() and not path.is_symlink():
        for directory, dirs, files in os.walk(path, followlinks=False):
            for name in dirs + files:
                total += (Path(directory) / name).lstat().st_blocks * 512
    return total


def guard_live():
    global LAST_WHOLE_CHECK
    assert datetime.now(timezone.utc) < DEADLINE, 'Work deadline reached'
    assert shutil.disk_usage(ROOT).free - 8 * 1024**2 >= 9 * 1024**3, 'Live/aggregate margin not met'
    if time.monotonic() - LAST_WHOLE_CHECK >= 5:
        assert allocated(ROOT) + allocated(GIT_ADMIN) + 8 * 1024**2 <= WHOLE_LIMIT
        LAST_WHOLE_CHECK = time.monotonic()
    # macOS ru_maxrss is bytes; the sum conservatively combines parent/child peaks.
    assert resource.getrusage(resource.RUSAGE_SELF).ru_maxrss + resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss < 256 * 1024**2


def git(*args):
    guard_live()
    return subprocess.check_output(['git', *args], cwd=ROOT, timeout=10)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    global DEADLINE, GIT_ADMIN, WHOLE_LIMIT
    parser = argparse.ArgumentParser()
    parser.add_argument('--allocation', required=True)
    parser.add_argument('--whole-limit-mib', type=int, required=True)
    args = parser.parse_args()
    attempt_start = datetime.now(timezone.utc)
    previous_guards = [json.loads(path.read_text()) for path in OUT.glob('guard-start*.json')]
    starts = [datetime.fromisoformat(record['utc']) for record in previous_guards
              if record.get('allocation') == args.allocation]
    start = min([attempt_start, *starts])
    started = start.isoformat()
    DEADLINE = start + timedelta(seconds=180)
    signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError('180-second work deadline')))
    signal.alarm(max(1, int((DEADLINE - attempt_start).total_seconds())))
    WHOLE_LIMIT = args.whole_limit_mib * 1024**2
    GIT_ADMIN = Path((ROOT / '.git').read_text().strip().removeprefix('gitdir: ')).resolve()
    common = (GIT_ADMIN / (GIT_ADMIN / 'commondir').read_text().strip()).resolve()
    head = (GIT_ADMIN / 'HEAD').read_text().strip()
    branch = head.removeprefix('ref: ')
    if head.startswith('ref: '):
        ref = common / branch
        if ref.is_file():
            source40 = ref.read_text().strip()
        else:
            source40 = next(line.split()[0] for line in (common / 'packed-refs').read_text().splitlines()
                            if line.endswith(' ' + branch))
    else:
        source40 = head
    assert source40 == BASE
    free_start = shutil.disk_usage(ROOT).free
    assert free_start >= 16 * 1024**3, 'Fresh-start floor not met'
    guard_live()
    guard = {'allocation': args.allocation, 'utc': started, 'source40': source40,
             'attempt_started_at': attempt_start.isoformat(),
             'branch': branch, 'tool_sha256': sha(Path(__file__).read_bytes()),
             'work_deadline_utc': DEADLINE.isoformat(),
             'total_deadline_utc': (start + timedelta(seconds=240)).isoformat(),
             'max_processes_all': 2, 'max_rss_all_bytes': 256 * 1024**2,
             'new_total_limit_bytes': 8 * 1024**2, 'output_limit_bytes': 5 * 1024**2,
             'whole_limit_bytes': WHOLE_LIMIT,
             'checkout_allocated_bytes': allocated(ROOT), 'private_git_allocated_bytes': allocated(GIT_ADMIN),
             'shared_common_git': 'Existing shared repository object storage; not an exclusive job allocation.',
             'fresh_free_bytes': free_start, 'start_floor_bytes': 16 * 1024**3,
             'live_floor_bytes': 8 * 1024**3, 'aggregate_margin_bytes': 9 * 1024**3,
             'manifest': {'read_only_source': str(ROOT),
                          'write_paths': ['docs/mcr/frontend/fe16-original-parity.md', 'evidence/fe16-original-parity/**'],
                          'jobs': 'Sequential standard-library Python and Git only; no tests/build/browser/server/native job.'}}
    (OUT / 'guard-start.json').write_text(json.dumps(guard, indent=2) + '\n')
    assert git('rev-parse', 'HEAD').decode().strip() == BASE
    commits = [OLD, CURRENT,
               '3c9d2f75a0827d29809bc4ccbe55e8879156089d',
               '4b1f8c4fa646eca3b6c93d8c4e673fd820ac69a3']
    for commit in commits:
        guard_live()
        subprocess.run(['git', 'merge-base', '--is-ancestor', commit, BASE], cwd=ROOT, check=True, timeout=10)
    assert git('rev-parse', RUN_SOURCE).decode().strip() == RUN_SOURCE
    owner_source_ancestry = subprocess.run(['git', 'merge-base', '--is-ancestor', RUN_SOURCE, BASE],
                                          cwd=ROOT, timeout=10).returncode
    assert owner_source_ancestry in (0, 1)

    paths = [NETWORK + p for p in ['WorldTransport.ts', 'mock.ts', 'websocket.ts',
             'transport/engine.ts', 'transport/contract.ts', 'transport/wire-types.ts',
             '__tests__/transport.test.ts', '__tests__/generate-types.mjs',
             '__tests__/browser-smoke.mjs', '__tests__/fixtures/browser.ts',
             '__tests__/fixtures/browser.html', '__tests__/social-policy-wire.test.ts',
             '__tests__/social-policy-wire.config.ts', 'replication.ts', '__tests__/replication.test.ts']]
    paths += git('ls-files', 'contracts/realtime', 'contracts/fixtures/realtime',
                 'contracts/assets/v1', 'contracts/assets/v2/reviewed',
                 'contracts/core/v1/core.schema.json', 'frontend/src/shared/api',
                 'frontend/package.json', 'frontend/pnpm-lock.yaml').decode().splitlines()
    bindings = []
    for path in sorted(set(paths)):
        data = (ROOT / path).read_bytes()
        previous = git('show', f'{CURRENT}:{path}')
        run_input = git('show', f'{RUN_SOURCE}:{path}')
        assert run_input == previous or path == NETWORK + 'transport/wire-types.ts'
        same = data == previous
        comment_only = False
        if not same:
            assert path == 'frontend/src/shared/api/domain-operations.generated.ts'
            strip = lambda value: b'\n'.join(line for line in value.splitlines() if not line.lstrip().startswith(b'//'))
            comment_only = strip(data) == strip(previous)
            assert comment_only
        bindings.append({'path': path, 'sha256': sha(data), 'qualified_checkpoint_sha256': sha(previous),
                         'owner_run_input_sha256': sha(run_input),
                         'checkpoint_equal': same, 'comment_only_delta': comment_only})
    original = json.loads((ROOT / 'docs/mcr/evidence/world-transport/hashes.json').read_text())
    for path, expected in original['sha256'].items():
        assert sha(git('show', f'{OLD}:{path}')) == expected
    unchanged_old = [p for p in original['sha256'] if (ROOT / p).read_bytes() == git('show', f'{OLD}:{p}')]
    for name in ['WorldTransport.ts', 'mock.ts', 'websocket.ts', '__tests__/browser-smoke.mjs',
                 '__tests__/fixtures/browser.ts', '__tests__/fixtures/browser.html']:
        assert NETWORK + name in unchanged_old
    trace_bindings = {}
    for name in ['movement-heartbeat', 'resync', 'room-switch', 'epoch-replacement', 'revoke']:
        path = f'contracts/fixtures/realtime/{name}.json'
        data = (ROOT / path).read_bytes()
        assert data == git('show', f'{OLD}:{path}')
        trace_bindings[path] = sha(data)
    release = json.loads((ROOT / 'docs/mcr/evidence/social-policy-wire/source-01/release.json').read_text())
    assert release['source'] == RUN_SOURCE and release['failure'] is None
    assert all(command['exitCode'] == 0 for command in release['commands'])
    assert '195 passed (195)' in (ROOT / 'docs/mcr/evidence/social-policy-wire/source-01/tests.log').read_text()
    config = (ROOT / (NETWORK + '__tests__/social-policy-wire.config.ts')).read_text()
    assert all(name in config for name in ['transport.test.ts', 'replication.test.ts', 'social-policy-wire.test.ts'])
    generated = (ROOT / (NETWORK + 'transport/wire-types.ts')).read_text()
    for path, expected in re.findall(r'^// source_hash (\S+) ([a-f0-9]{64})$', generated, re.M):
        assert sha((ROOT / path).read_bytes()) == expected

    # Conservative lexical closure: includes type-only edges and all literal lazy imports.
    # It is a source/import check, not a substitute for a bundler or native runtime.
    entry = ROOT / 'frontend/src/main.tsx'
    tracked = {}
    for row in git('ls-tree', '-r', BASE, '--', 'frontend/src').decode().splitlines():
        metadata, path = row.split('\t', 1)
        tracked[path] = metadata.split()[2]
    canonical_object_reads = {}
    pending, visited, edges, external = [entry], {}, [], set()
    suffixes = ['', '.ts', '.tsx', '.js', '.jsx', '.mjs', '.json', '/index.ts', '/index.tsx']
    while pending:
        file = pending.pop()
        path = file.relative_to(ROOT).as_posix()
        if path in visited:
            continue
        guard_live()
        if file.is_file():
            data = file.read_bytes()
        else:
            assert path in tracked, path
            data = git('show', f'{BASE}:{path}')
            canonical_object_reads[path] = tracked[path]
        visited[path] = sha(data)
        if file.suffix not in {'.ts', '.tsx', '.js', '.jsx', '.mjs'}:
            continue
        source = data.decode()
        assert not re.search(r'\bimport\.meta\.glob\s*\(', source), path
        assert not re.search(r'\brequire\s*\(', source), path
        dynamic = re.findall(r'\bimport\s*\(\s*([\s\S]*?)\)', source)
        assert all(re.fullmatch(r"['\"][^'\"]+['\"]", x.strip()) for x in dynamic), path
        imports = re.findall(r"\bfrom\s*['\"]([^'\"]+)['\"]", source)
        imports += re.findall(r"\bimport\s*['\"]([^'\"]+)['\"]", source)
        imports += [x.strip()[1:-1] for x in dynamic]
        for spec in sorted(set(imports)):
            if not spec.startswith('.'):
                assert not spec.startswith(('/', '@/')), (path, spec)
                external.add(spec)
                continue
            target = (file.parent / spec.split('?')[0]).resolve()
            candidates = [Path(str(target) + suffix) for suffix in suffixes]
            resolved = next((p for p in candidates if p.is_file() or p.relative_to(ROOT).as_posix() in tracked), None)
            assert resolved, (path, spec)
            edges.append({'from': path, 'specifier': spec, 'to': resolved.relative_to(ROOT).as_posix()})
            pending.append(resolved)
    assert NETWORK + 'websocket.ts' in visited
    assert NETWORK + 'mock.ts' not in visited
    assert not any('/__tests__/' in path or re.search(r'\.(test|spec)\.', path) for path in visited)

    evidence = ['docs/mcr/evidence/world-transport/' + p for p in
                ['hashes.json', 'tests.log', 'browser-results.json', 'browser.log', 'build.log', 'lint.log']]
    evidence += ['docs/mcr/evidence/social-policy-wire/source-01/release.json',
                 'docs/mcr/evidence/social-policy-wire/source-01/tests.log',
                 'docs/mcr/qa/world-async-tests.md',
                 'docs/mcr/frontend/social-policy-production.md',
                 'docs/mcr/frontend/http-transport.md', 'docs/mcr/frontend/world-transport.md']
    free_end = shutil.disk_usage(ROOT).free
    assert free_end >= 8 * 1024**3, 'Running floor not met'
    report = {'assignment': 'FE16-ORIGINAL-PARITY-01', 'qualification': 'SOURCE_ONLY_PASS',
              'command': [sys.executable, *sys.argv],
              'allocation': args.allocation, 'started_at': started,
              'finished_at': datetime.now(timezone.utc).isoformat(), 'source_base': BASE,
              'historical_source': OLD, 'latest_qualified_checkpoint': CURRENT,
              'latest_owner_run_source': RUN_SOURCE, 'ancestor_checks_exit': 0,
              'owner_run_source_ancestry_exit': owner_source_ancestry,
              'owner_run_source_provenance': 'Worker run-source object; canonical publication is 7a94c5e7. Source bytes are compared explicitly, independent of worker ancestry.',
              'free_start_bytes': free_start, 'free_end_bytes': free_end,
              'source_bindings': bindings, 'unchanged_from_original': unchanged_old,
              'five_original_native_trace_sha256': trace_bindings,
              'evidence_sha256': {p: sha((ROOT / p).read_bytes()) for p in evidence},
              'production_import_closure': {'entry': 'frontend/src/main.tsx',
                   'method': 'Conservative lexical closure, including type-only and literal dynamic imports; rejects computed imports/globs/require and unresolved relative imports.',
                   'modules_sha256': dict(sorted(visited.items())), 'edges': edges,
                   'unmaterialized_canonical_object_reads': canonical_object_reads,
                   'external_specifiers': sorted(external), 'mock_reachable': False,
                   'test_module_reachable': False, 'websocket_reachable': True},
              'new_tests_builds_browser_native_runs': 0,
              'elapsed_seconds': (datetime.now(timezone.utc) - start).total_seconds(),
              'conservative_parent_child_peak_rss_bytes': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss + resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
              'checkout_allocated_end_bytes': allocated(ROOT),
              'private_git_allocated_end_bytes': allocated(GIT_ADMIN),
              'current_built_bundle_execution': 'NOT_RUN', 'exit_code': 0}
    payload = json.dumps(report, indent=2) + '\n'
    assert allocated(OUT) + len(payload.encode()) <= 5 * 1024**2
    guard_live()
    (OUT / 'qualification.json').write_text(payload)
    signal.alarm(0)
    print(json.dumps({'qualification': report['qualification'], 'source_bindings': len(bindings),
                      'production_closure_modules': len(visited), 'exit_code': 0}))


if __name__ == '__main__':
    try:
        main()
    except Exception:
        failure = {'status': 'FAILED', 'utc': datetime.now(timezone.utc).isoformat(),
                   'tool_sha256': sha(Path(__file__).read_bytes()), 'traceback': traceback.format_exc()}
        stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
        (OUT / f'failure-{stamp}.json').write_text(json.dumps(failure, indent=2) + '\n')
        if (OUT / 'guard-start.json').is_file():
            (OUT / f'guard-start-failed-{stamp}.json').write_bytes((OUT / 'guard-start.json').read_bytes())
        raise
