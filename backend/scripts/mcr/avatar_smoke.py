#!/usr/bin/env python3
"""BE07 avatar races/rollback/restart in a fresh marker-owned native PG cluster.

Reuses the audited migration-smoke Cluster and bootstrap fixture execution;
never attaches an existing database, Docker volume or externally supplied URL.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import bootstrap_smoke as bootstrap
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = bootstrap.DATABASE


def artifacts(path):
    selected = []
    finished = False
    for line in path.read_text().splitlines():
        row = json.loads(line)
        if row.get('reason') == 'build-finished':
            bootstrap.require(row.get('success') is True, 'Cargo build failed')
            finished = True
        if (row.get('reason') == 'compiler-artifact' and row.get('executable')
                and row['target']['name'] == 'avatar' and 'test' in row['target']['kind']
                and Path(row['manifest_path']).resolve() == ROOT / 'backend/services/concert-service/Cargo.toml'):
            selected.append(migration.executable(Path(row['executable'])))
    bootstrap.require(finished and len(set(selected)) == 1, 'exact service avatar compiler artifact required')
    return selected[0]


def run(metadata, pg_bin):
    binary = artifacts(metadata)
    tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
    report = {'state': 'FAIL', 'commands': {}, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}
    cluster = migration.Cluster(tools)
    try:
        cluster.init(); cluster.start()
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        bootstrap.require(report['postgres_version'].startswith('16.14'), 'PostgreSQL16.14 required')
        cluster.sql('postgres', 'create database mcr_concert_bootstrap template template0;')
        # Exercise the real service schema without changing production ledgers.
        report['migration_sha384'] = migration.migration_manifest(ROOT / 'backend', 'concert-service')
        for path in sorted((ROOT / 'backend/services/concert-service/migrations').glob('*.sql')):
            if not path.name.endswith('.down.sql'):
                cluster.sql(DATABASE, path.read_text())
                report['commands'][path.name] = 0
        release = cluster.root / 'release'; release.mkdir(mode=0o700)
        document = json.loads((ROOT / 'contracts/fixtures/core/bootstrap-open.json').read_text())
        files = {'bootstrap': document, 'avatar': document['avatar_manifest'], 'map': document['map_manifest']}
        files.update({theme['theme_id'].lower(): theme for theme in document['theme_manifests']})
        for name, value in files.items():
            (release / (name + '.json')).write_text(json.dumps(value))
        bootstrap.run_test(cluster, binary, 'native_avatar_transactions', report)
        report['response_sha256'] = hashlib.sha256((cluster.root / 'avatar-state.json').read_bytes()).hexdigest()
        before = cluster.assert_postmaster(); cluster.stop(); cluster.start()
        after = cluster.assert_postmaster(); bootstrap.require(before != after, 'postmaster did not restart')
        report['restart'] = {'before_pid': before, 'after_pid': after}
        bootstrap.run_test(cluster, binary, 'native_avatar_restart_persistence', report)
        report['native_tests'] = 2
        report['state'] = 'PASS'
    except (migration.SmokeError, OSError, ValueError, subprocess.SubprocessError) as error:
        report['error'] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__
    finally:
        try:
            cluster.cleanup(); report['cleanup'] = 'PASS'
            bootstrap.require(not cluster.root.exists(), 'owned cluster remains')
        except (migration.SmokeError, OSError) as error:
            report['state'] = 'FAIL'; report['cleanup'] = 'FAIL'
            report['cleanup_error'] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build-metadata', type=Path, required=True)
    parser.add_argument('--pg-bin', type=Path, required=True)
    args = parser.parse_args()
    report = run(args.build_metadata, args.pg_bin)
    print(json.dumps(report, sort_keys=True))
    return 0 if report['state'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
