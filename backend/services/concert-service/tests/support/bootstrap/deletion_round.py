#!/usr/bin/env python3
"""Execute only frozen bootstrap regressions in an owned native PG16.14 cluster."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[6]
sys.path.insert(0, str(ROOT / 'backend/scripts/mcr'))
import migration_smoke as migration


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--test-binary', required=True, type=Path)
    parser.add_argument('--test-sha256', required=True)
    parser.add_argument('--service-binary', required=True, type=Path)
    parser.add_argument('--service-sha256', required=True)
    parser.add_argument('--pg-bin', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    binaries = {'tests': migration.executable(args.test_binary),
                'service': migration.executable(args.service_binary)}
    if digest(binaries['tests']) != args.test_sha256 or digest(binaries['service']) != args.service_sha256:
        raise SystemExit('frozen binary hash mismatch')
    tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
    report = {'state': 'FAIL', 'scope': 'Bootstrap deletion fence, existing native bootstrap and restart; fixture admission/release, not Auth saga/public HTTP.',
              'source_commit': migration.execute(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'],
                  env={'PATH': '/usr/bin:/bin'}, phase='source-pin', cwd=ROOT).stdout.strip(),
              'binary_sha256': {key: digest(value) for key, value in binaries.items()},
              'commands': {}, 'native_tests': 0, 'cleanup': 'NOT_RUN'}
    cluster = migration.Cluster(tools)
    try:
        cluster.init()
        cluster.start()
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        if not report['postgres_version'].startswith('16.14'):
            raise RuntimeError('PostgreSQL16.14 required')
        database = 'mcr_concert_bootstrap'
        cluster.sql('postgres', 'create database mcr_concert_bootstrap template template0;')
        migration_env = {**cluster.env, 'CONCERT_DATABASE_URL': cluster.url(database)}
        manifest = migration.migration_manifest(ROOT / 'backend', 'concert-service')
        for flag in ['--migrate', '--verify-migrations']:
            result = migration.execute([str(binaries['service']), flag], env=migration_env,
                                       phase=flag, cwd=cluster.root)
            report['commands'][flag] = result.returncode
            migration.check_ledger(cluster, database, manifest)
        report['migration_sha384'] = manifest
        release = cluster.root / 'release'
        release.mkdir(mode=0o700)
        document = json.loads((ROOT / 'contracts/fixtures/core/bootstrap-open.json').read_text())
        files = {'bootstrap': document, 'avatar': document['avatar_manifest'], 'map': document['map_manifest']}
        files.update({v['theme_id'].lower(): v for v in document['theme_manifests']})
        for name, value in files.items():
            (release / (name + '.json')).write_text(json.dumps(value))
        env = {**cluster.env, 'MCR_BOOTSTRAP_ROOT': str(cluster.root), 'MCR_BOOTSTRAP_TOKEN': cluster.token,
               'MCR_BOOTSTRAP_INODE': str(cluster.identity[1]), 'MCR_BOOTSTRAP_DATABASE_URL': cluster.url(database)}

        def test(name):
            result = migration.execute([str(binaries['tests']), '--ignored', '--exact', name, '--test-threads=1'],
                                       env=env, phase=name, cwd=cluster.root, timeout=60)
            if not re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;', result.stdout):
                raise RuntimeError('native test missing or skipped')
            report['commands'][name] = result.returncode
            report.setdefault('test_output', {})[name] = result.stdout
            report['native_tests'] += 1

        # The legacy restart case asserts global fixture totals. Complete that
        # pair before the independent deletion-fence case adds its own actor.
        test('native_bootstrap_transactions_and_guarded_response')
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise RuntimeError('postmaster did not restart')
        report['restart'] = {'before_pid': before, 'after_pid': after}
        migration.check_ledger(cluster, database, manifest)
        test('native_bootstrap_restart_persistence')
        test('native_bootstrap_honors_account_deletion_before_defaults')
        report['state'] = 'PASS'
    except Exception as error:
        report['error'] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__
    finally:
        try:
            cluster.cleanup()
            report['cleanup'] = 'PASS' if not cluster.root.exists() else 'FAIL'
        except Exception as error:
            report['state'] = 'FAIL'
            report['cleanup'] = 'FAIL'
            report['cleanup_error'] = type(error).__name__
        args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'state': report['state'], 'native_tests': report['native_tests'], 'cleanup': report['cleanup']}))
    return 0 if report['state'] == 'PASS' and report['cleanup'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
