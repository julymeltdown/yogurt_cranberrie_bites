#!/usr/bin/env python3
"""Feed SQL/receipt checks on a new private marker/inode-owned PG16 fixture only."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = 'mcr_concert_feed'
MIGRATIONS = ('0001_concert_core.sql', '0002_command_receipts_outbox.sql', '0010_feed.sql')

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def require(value, label):
    if not value:
        raise RuntimeError(label)

def run(args, report):
    cluster = None
    try:
        tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        report['owned_directory'] = str(cluster.root)
        cluster.init()
        cluster.start()
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        require(report['postgres_version'].startswith('16.'), 'PostgreSQL16 required')
        cluster.sql('postgres', f'create database {DATABASE} template template0;')
        for name in MIGRATIONS:
            path = ROOT / 'backend/services/concert-service/migrations' / name
            cluster.sql(DATABASE, 'begin;\n' + path.read_text() + '\ncommit;')
        require(cluster.sql(DATABASE, 'select count(*) from concert_feed_owner_work;') == '0', 'no seeded production work')
        require(cluster.sql(DATABASE, "select count(*) from pg_indexes where indexname='concert_feed_pending_work';") == '1', 'work index missing')
        report['migration_sql'] = 'PASS'
        if args.repository_test:
            test = migration.executable(args.repository_test)
            report['test_binary_sha256'] = digest(test)
            env = {**cluster.env, 'FEED_TEST_DATABASE_URL': cluster.url(DATABASE),
                   'FEED_TEST_OWNER_ROOT': str(cluster.root), 'FEED_TEST_OWNER_TOKEN': cluster.token,
                   'FEED_TEST_OWNER_IDENTITY': f'{cluster.identity[0]}:{cluster.identity[1]}'}
            result = subprocess.run([str(test), '--test-threads=1'], env=env, cwd=ROOT,
                                    capture_output=True, text=True, timeout=50)
            report['rust_exit_code'] = result.returncode
            report['rust_diagnostics'] = migration.redact(result.stdout + result.stderr,
                                                         (cluster.password, cluster.url(DATABASE), cluster.token))
            require(result.returncode == 0 and '1 passed; 0 failed' in result.stdout, 'native Rust tests did not pass')
            report['native_rust_tests'] = 1
        else:
            report['native_rust_tests'] = 'NOT_RUN'
        counts = cluster.sql(DATABASE, 'select (select count(*) from concert_command_receipts),(select count(*) from concert_feed_owner_work),(select count(*) from concert_outbox);')
        previous = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        require(previous != cluster.assert_postmaster(), 'restart did not create a new postmaster')
        require(cluster.sql(DATABASE, 'select (select count(*) from concert_command_receipts),(select count(*) from concert_feed_owner_work),(select count(*) from concert_outbox);') == counts, 'restart persistence changed')
        report['restart_persistence'] = 'PASS'
        report['row_counts'] = counts
        report['result'] = 'PASS' if args.repository_test else 'SQL_ONLY_PASS'
    except BaseException as error:
        report['result'] = 'FAIL'
        report['failure'] = type(error).__name__
    finally:
        try:
            if cluster:
                cluster.cleanup()
                require(not cluster.root.exists(), 'owned directory remains')
            report['cleanup'] = 'PASS'
        except BaseException as error:
            report.update(result='FAIL', cleanup='FAIL', cleanup_failure=type(error).__name__)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository-test', type=Path)
    parser.add_argument('--sql-only', action='store_true')
    parser.add_argument('--pg-bin', type=Path, default=Path('/opt/homebrew/opt/postgresql@16/bin'))
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if bool(args.repository_test) == args.sql_only:
        parser.error('supply exactly one of --repository-test or --sql-only')
    if not args.report.is_absolute() or (args.repository_test and not args.repository_test.is_absolute()):
        parser.error('report and test paths must be absolute')
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {'scope': 'BE31/BE32 isolated SQL and injected local authority; no live Post/Auth qualification',
              'result': 'FAIL', 'production_authority': 'NOT_QUALIFIED', 'http_browser': 'NOT_RUN'}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        report['revision'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        paths = [Path(__file__), Path(migration.__file__)]
        for prefix in ('backend/crates/concert-domain/src/feed', 'backend/crates/concert-application/src/feed',
                       'backend/services/concert-service/src/feed', 'backend/services/concert-service/tests/support/feed'):
            paths.extend((ROOT / prefix).glob('*.rs'))
        paths.extend(ROOT / 'backend/services/concert-service/migrations' / name for name in MIGRATIONS)
        paths.extend((ROOT / 'backend/services/concert-service/tests').glob('feed*.rs'))
        paths.extend((ROOT / 'backend/crates/concert-application/tests/feed.rs', ROOT / 'backend/Cargo.toml', ROOT / 'backend/Cargo.lock'))
        report['source_sha256'] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}
        report['contract_sha256'] = {name: digest(ROOT / 'contracts/domain/v1' / name) for name in ('feed.schema.json', 'operations.json', 'policy.json')}
        run(args, report)
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, 'w') as stream:
            json.dump(report, stream, indent=2)
            stream.write('\n')
    print(json.dumps({key: report.get(key) for key in ('result', 'failure', 'cleanup', 'native_rust_tests', 'restart_persistence')}))
    return 0 if report['result'] in ('PASS', 'SQL_ONLY_PASS') else 1

if __name__ == '__main__':
    raise SystemExit(main())
