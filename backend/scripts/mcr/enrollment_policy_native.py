#!/usr/bin/env python3
"""Run a frozen enrollment EXE on a newly owned PG16 cluster, then restart PG.

This never invokes Cargo or re-enters a target. The allocator's independent
process/byte/RSS guardian must already cover this controller and its descendants.
"""
from __future__ import annotations
import argparse, hashlib, json, os, signal, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import migration_smoke as pg


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--absolute-stop', type=float, required=True)
    parser.add_argument('--guardian-pid', type=int, required=True)
    parser.add_argument('--pg-bin-dir', type=Path, default=Path('/opt/homebrew/opt/postgresql@16/bin'))
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve(strict=True)
    if args.binary.is_symlink() or hashlib.sha256(binary.read_bytes()).hexdigest() != args.sha256:
        raise RuntimeError('frozen binary identity mismatch')
    if args.output.is_symlink() or output.stat().st_uid != os.getuid():
        raise RuntimeError('foreign evidence root')
    report = {'status': 'FAIL', 'binary_sha256': args.sha256, 'phases': {}}
    cleaning = False
    child = None

    def guard():
        if not cleaning:
            if time.time() >= args.absolute_stop - 15:
                raise RuntimeError('native work cutoff')
            os.kill(args.guardian_pid, 0)
            if __import__('shutil').disk_usage(output).free < 8 * 1024**3:
                raise RuntimeError('8GiB continuous floor')

    original = pg.execute
    def execute(*a, **kw):
        guard()
        return original(*a, **kw)
    pg.execute = execute
    guard()
    cluster = pg.Cluster({name: pg.executable(args.pg_bin_dir / name) for name in pg.PG_TOOLS}, output / 'owned-enrollment-pg')
    marker = cluster.root / 'private-restart.json'

    def stop_child():
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=3)
    def interrupted(_signal, _frame):
        raise KeyboardInterrupt('owned native stop')
    signal.signal(signal.SIGTERM, interrupted)
    try:
        guard(); cluster.init()
        with (cluster.data / 'postgresql.conf').open('a') as stream:
            stream.write('\nmax_connections=48\nshared_buffers=16MB\nmax_worker_processes=2\nmax_prepared_transactions=16\nmax_wal_size=64MB\nmin_wal_size=32MB\n')
        cluster.start()
        system = cluster.sql('postgres', 'select system_identifier::text from pg_control_system();')
        report['system_identifier'] = system
        original_postmaster = cluster.assert_postmaster()
        report['original_postmaster'] = original_postmaster
        for phase in ('source', 'restart'):
            if phase == 'restart':
                cluster.stop(); guard(); cluster.start()
                report['restart_postmaster'] = cluster.assert_postmaster()
                if report['restart_postmaster'] == original_postmaster:
                    raise RuntimeError('physical postmaster did not restart')
            guard()
            env = {**cluster.env, 'AUTH_RUNTIME_TEST_DATABASE_URL': cluster.url('postgres'),
                   'AUTH_RUNTIME_TEST_INITDB_SYSTEM_IDENTIFIER': system,
                   'MCR_ENROLLMENT_PHASE': phase, 'MCR_ENROLLMENT_PRIVATE_MARKER': str(marker)}
            started = time.monotonic()
            with (output / f'enrollment-{phase}.log').open('wb') as log:
                child = subprocess.Popen([str(binary), '--ignored', '--exact',
                    'native_policy_signup_atomicity_guarded_rpc_gateway_and_restart', '--nocapture', '--test-threads=1'],
                    env=env, cwd=ROOT, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                code = child.wait(timeout=max(1, min(65, args.absolute_stop-time.time()-15)))
            report['phases'][phase] = {'exit_code': code, 'seconds': time.monotonic()-started, 'pid': child.pid}
            if code != 0:
                raise RuntimeError(f'{phase} failed; see owned phase log')
            if not marker.exists():
                raise RuntimeError('private phase state absent')
            marker.chmod(0o600)
        report['prepared_transactions'] = int(cluster.sql('postgres', 'select count(*) from pg_prepared_xacts;'))
        if report['prepared_transactions'] != 0:
            raise RuntimeError('prepared transaction leak')
        report['status'] = 'PASS'
    except BaseException as error:
        report['failure'] = type(error).__name__ + ': ' + str(error)
    finally:
        cleaning = True
        stop_child()
        cluster.cleanup()
        report['owned_root_absent'] = not cluster.root.exists()
        report_path = output / 'enrollment-native-report.json'
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
        report_path.chmod(0o600)
    print(json.dumps({'status': report['status'], 'report': str(report_path)}))
    return 0 if report['status'] == 'PASS' else 1

if __name__ == '__main__':
    raise SystemExit(main())
