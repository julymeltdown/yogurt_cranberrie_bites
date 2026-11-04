#!/usr/bin/env python3
"""Fresh BE-owned serving fixture. FE owns the actual browser verdict.

No Cargo/target access, external mail, positive account/consent SQL or implicit
production policy. An independently running allocation guardian must cover the
explicit short PG root, output, controller and all descendants before this runs.
"""
from __future__ import annotations
import argparse, hashlib, json, os, signal, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import migration_smoke as pg


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--cluster-root', required=True, type=Path)
    parser.add_argument('--guardian-pid', required=True, type=int)
    parser.add_argument('--absolute-stop', required=True, type=float)
    parser.add_argument('--seconds', type=int, default=180)
    parser.add_argument('--frontend-origin', default='http://localhost:3000')
    parser.add_argument('--pg-bin-dir', type=Path, default=Path('/opt/homebrew/opt/postgresql@16/bin'))
    args = parser.parse_args()
    output = args.output.resolve(strict=True)
    binary = args.binary.resolve(strict=True)
    if args.output.is_symlink() or output.stat().st_uid != os.getuid():
        raise RuntimeError('owned evidence root required')
    if args.binary.is_symlink() or hashlib.sha256(binary.read_bytes()).hexdigest() != args.sha256:
        raise RuntimeError('frozen test binary identity mismatch')
    if not args.cluster_root.is_absolute() or len(str(args.cluster_root / 'socket')) > 90:
        raise RuntimeError('new short absolute PG root required')
    if not 1 <= args.seconds <= 300 or args.frontend_origin != 'http://localhost:3000':
        raise RuntimeError('explicit bounded local browser pairing required')
    os.kill(args.guardian_pid, 0)
    if time.time() >= args.absolute_stop - 25 or __import__('shutil').disk_usage(output).free < 8*1024**3:
        raise RuntimeError('native deadline/free-space preflight failed')
    original = pg.execute
    cleaning = False
    def execute(*a, **kw):
        if not cleaning:
            os.kill(args.guardian_pid, 0)
            if time.time() >= args.absolute_stop - 15:
                raise RuntimeError('original pairing cutoff')
        return original(*a, **kw)
    pg.execute = execute
    cluster = pg.Cluster({n: pg.executable(args.pg_bin_dir/n) for n in pg.PG_TOOLS}, args.cluster_root)
    child = None
    report = {'status': 'FAIL', 'binary_sha256': args.sha256,
              'browser_verdict': 'FE_OWNED_NOT_CLAIMED_BY_SERVING_FIXTURE'}
    def interrupted(_signal, _frame):
        raise KeyboardInterrupt('owned browser pairing stopped')
    signal.signal(signal.SIGTERM, interrupted)
    try:
        cluster.init()
        with (cluster.data/'postgresql.conf').open('a') as stream:
            stream.write('\nmax_connections=48\nshared_buffers=16MB\nmax_worker_processes=2\nmax_prepared_transactions=16\nmax_wal_size=64MB\nmin_wal_size=32MB\n')
        cluster.start()
        cluster.sql('postgres', 'create database mcr_enrollment_controller template template0;')
        seconds = min(args.seconds, int(args.absolute_stop-time.time()-20))
        if seconds < 1:
            raise RuntimeError('pairing budget expired during setup')
        env = {**cluster.env,
               'AUTH_RUNTIME_TEST_DATABASE_URL': cluster.url('mcr_enrollment_controller'),
               'AUTH_RUNTIME_TEST_INITDB_SYSTEM_IDENTIFIER': cluster.sql('postgres', 'select system_identifier::text from pg_control_system();'),
               'MCR_ENROLLMENT_BROWSER_OUTPUT': str(output),
               'MCR_ENROLLMENT_BROWSER_SECONDS': str(seconds),
               'MCR_ENROLLMENT_BROWSER_FRONTEND_ORIGIN': args.frontend_origin}
        with (output/'browser-fixture.log').open('wb') as log:
            child = subprocess.Popen([str(binary), '--ignored', '--exact',
                'paired_browser_fixture_uses_real_auth_rpc_gateway_and_mail', '--nocapture', '--test-threads=1'],
                cwd=Path(__file__).resolve().parents[3], env=env, stdin=subprocess.DEVNULL,
                stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            code = child.wait(timeout=max(1, args.absolute_stop-time.time()-15))
        report['test_exit_code'] = code
        if code != 0:
            raise RuntimeError('serving fixture failed; see owned fixture log')
        report['status'] = 'SERVING_STOPPED'
        report['prepared_transactions'] = int(cluster.sql('postgres','select count(*) from pg_prepared_xacts;'))
        if report['prepared_transactions']:
            raise RuntimeError('prepared transaction leak')
    except BaseException as error:
        report['failure'] = getattr(error, 'detail', type(error).__name__+': '+str(error))
        report['status'] = 'FAIL'
    finally:
        cleaning = True
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL); child.wait(timeout=3)
        cluster.cleanup()
        report['owned_root_absent'] = not cluster.root.exists()
        target = output/'browser-controller-report.json'
        target.write_text(json.dumps(report, indent=2, sort_keys=True)+'\n'); target.chmod(0o600)
    print(json.dumps({'status':report['status'], 'report':str(target), 'browserVerdict':'NOT_CLAIMED'}))
    return 0 if report['status'] == 'SERVING_STOPPED' else 1

if __name__ == '__main__':
    raise SystemExit(main())
