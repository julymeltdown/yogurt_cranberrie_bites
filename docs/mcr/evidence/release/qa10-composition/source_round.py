#!/usr/bin/env python3
"""Local macOS SOURCE controller; never runs application or receipt commands.

An actual named allocation and current total reservation are required. Evidence
is stored outside Git in the exact output directory. Temporary fixtures are
removed before FULL; retained logs count against NEW and the 2 MiB log cap.
"""
import argparse
import ctypes
import datetime
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

MIB = 1024 ** 2
GIB = 1024 ** 3


def stamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def size(root):
    total = 0
    for base, dirs, files in os.walk(root, followlinks=False):
        for name in files:
            try:
                total += (Path(base) / name).lstat().st_size
            except FileNotFoundError:
                pass
    return total


def write(path, value):
    temporary = path.with_suffix('.pending')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def process_usage(parent):
    library = ctypes.CDLL('/usr/lib/libproc.dylib')
    library.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
    library.proc_listchildpids.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
    pending, seen, rss = [parent], set(), 0
    while pending:
        pid = pending.pop()
        if pid in seen:
            continue
        seen.add(pid)
        info = ctypes.create_string_buffer(96)
        count = library.proc_pidinfo(pid, 4, 0, info, len(info))
        if count == 96:
            rss += int.from_bytes(info.raw[8:16], 'little')
        children = (ctypes.c_int * 1024)()
        library.proc_listchildpids(pid, children, ctypes.sizeof(children))
        pending.extend(value for value in children if value > 0)
    return len(seen), rss


def guardian(config):
    out = Path(config['out'])
    maxima = {'processes': 0, 'rss': 0, 'new_bytes': 0, 'whole_bytes': 0}
    # Establish that the native probe works before any work child is admitted.
    process_usage(config['supervisor'])
    write(out / 'guardian-ready.json', {'pid': os.getpid(), 'at': stamp()})
    breach = None
    while not (out / 'stop').exists():
        count, rss = process_usage(config['supervisor'])
        whole = size(Path(config['root']))
        new = max(0, whole - config['baseline'])
        free = shutil.disk_usage(config['root']).free
        for key, value in [('processes', count), ('rss', rss), ('new_bytes', new), ('whole_bytes', whole)]:
            maxima[key] = max(maxima[key], value)
        reasons = []
        if count > 4: reasons.append('processes')
        if rss > 512 * MIB: reasons.append('rss')
        if new > 32 * MIB: reasons.append('new_bytes')
        if whole > 6 * GIB: reasons.append('whole_bytes')
        if free < 8 * GIB: reasons.append('live_free')
        if time.monotonic() >= config['work_end']: reasons.append('work_deadline')
        if sum(p.stat().st_size for p in out.glob('*.log')) >= 2 * MIB: reasons.append('logs')
        if reasons:
            breach = reasons
            worker = out / 'worker.json'
            if worker.exists():
                try:
                    os.killpg(json.loads(worker.read_text())['pid'], signal.SIGKILL)
                except (OSError, ValueError, KeyError):
                    pass
            break
        time.sleep(0.1)
    write(out / 'guardian-result.json', {'at': stamp(), 'breach': breach, 'sampled_maxima': maxima})
    return 0


def main():
    if len(sys.argv) == 3 and sys.argv[1] == '--guardian':
        return guardian(json.loads(Path(sys.argv[2]).read_text()))
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', required=True)
    parser.add_argument('--offer', required=True)
    parser.add_argument('--all-new-bytes', type=int, required=True)
    args = parser.parse_args()
    root = Path(args.root).resolve()
    free = shutil.disk_usage(root).free
    baseline = size(root)
    if free < 11 * GIB or free - args.all_new_bytes < 9 * GIB or baseline > 6 * GIB:
        print(json.dumps({'status': 'PREFLIGHT_DECLINE', 'at': stamp(), 'free': free, 'whole': baseline}), flush=True)
        return 3
    start = time.monotonic()
    accepted = stamp()
    out = root / 'docs/mcr/evidence/release/qa10-composition' / args.offer
    out.mkdir(exist_ok=False)
    scratch = out / 'temporary'
    scratch.mkdir()
    config = {'root': str(root), 'out': str(out), 'supervisor': os.getpid(), 'baseline': baseline,
              'work_end': start + 180, 'full_end': start + 210}
    write(out / 'config.json', config)
    admission = {'status': 'ACTUAL_ACCEPT', 'offer': args.offer, 'at': accepted, 'free': free,
                 'all_new_bytes': args.all_new_bytes, 'reserved_free': free - args.all_new_bytes,
                 'whole_baseline': baseline, 'work_seconds': 180, 'full_seconds': 210,
                 'new_cap': 32 * MIB, 'whole_cap': 6 * GIB, 'process_cap': 4, 'rss_cap': 512 * MIB}
    write(out / 'admission.json', admission)
    print(json.dumps(admission), flush=True)
    guard = subprocess.Popen([sys.executable, '-B', str(Path(__file__).resolve()), '--guardian', str(out / 'config.json')])
    results = []
    try:
        while not (out / 'guardian-ready.json').exists():
            if guard.poll() is not None or time.monotonic() >= start + 5:
                raise RuntimeError('guardian did not start')
            time.sleep(0.02)
        environment = {**os.environ, 'TMPDIR': str(scratch), 'PYTHONDONTWRITEBYTECODE': '1', 'LC_ALL': 'C',
                       'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_CONFIG_NOSYSTEM': '1',
                       'GIT_CONFIG_COUNT': '2', 'GIT_CONFIG_KEY_0': 'maintenance.auto',
                       'GIT_CONFIG_VALUE_0': 'false', 'GIT_CONFIG_KEY_1': 'gc.auto', 'GIT_CONFIG_VALUE_1': '0'}

        def run(name, command, cwd):
            if guard.poll() is not None or time.monotonic() >= start + 175:
                raise RuntimeError('guardian stopped or work time exhausted')
            with (out / (name + '.log')).open('wb') as log:
                child = subprocess.Popen(command, cwd=cwd, env=environment, stdout=log,
                                         stderr=subprocess.STDOUT, start_new_session=True)
                write(out / 'worker.json', {'pid': child.pid})
                try:
                    result = child.wait(timeout=max(0.1, start + 180 - time.monotonic()))
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
                    raise
                finally:
                    (out / 'worker.json').unlink(missing_ok=True)
            row = {'name': name, 'argv': command, 'cwd': str(cwd), 'exit': result}
            results.append(row)
            print(json.dumps(row), flush=True)

        run('focused-unittest', [sys.executable, '-B', '-m', 'unittest', '-v', 'composition.test_composition',
            'test_check.ReleaseChecks.test_positive_exact_candidate_test_artifacts_production_and_task_link',
            'test_check.ReleaseChecks.test_cli_exit_codes_distinguish_partial_incomplete_and_fail',
            'test_check.ReleaseChecks.test_existing_frontend_schema_is_consumed_and_archived_json_checked'],
            root / 'scripts/mcr/release_checks')
        run('candidate', ['git', 'rev-parse', 'HEAD'], root)
        candidate = (out / 'candidate.log').read_text().strip()
        checker = str(root / 'scripts/mcr/release_checks/check.py')
        for mode in ['source', 'composition']:
            run('cli-' + mode, [sys.executable, '-B', checker, '--root', str(root), '--candidate', candidate,
                               '--mode', mode], root)
        run('cli-stale', [sys.executable, '-B', checker, '--root', str(root), '--candidate', '0' * 40,
                          '--mode', 'composition'], root)
        run('whitespace', ['git', 'diff', '--check', 'ba314a2c96639b05b6c15c1ead57353719004ffa', 'HEAD'], root)
    except Exception as error:
        results.append({'controller_error': type(error).__name__})
    finally:
        shutil.rmtree(scratch)
        (out / 'stop').touch()
        guard.wait(timeout=max(0.1, start + 210 - time.monotonic()))
        final = {'status': 'ACTUAL_FULL', 'offer': args.offer, 'at': stamp(),
                 'elapsed_seconds': time.monotonic() - start, 'results': results,
                 'retained_bytes': size(out), 'temporary_removed': not scratch.exists(),
                 'guardian': json.loads((out / 'guardian-result.json').read_text())}
        write(out / 'result.json', final)
        print(json.dumps(final), flush=True)
    return 0


if __name__ == '__main__':
    sys.exit(main())
