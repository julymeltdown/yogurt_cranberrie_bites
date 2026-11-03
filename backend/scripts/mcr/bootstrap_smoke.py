#!/usr/bin/env python3
"""BE06 native proof in a fresh marker/inode-owned PG16.14 cluster."""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import venv
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = 'mcr_concert_bootstrap'


def require(condition, diagnostic):
    if not condition:
        raise migration.SmokeError('bootstrap', diagnostic)


def build_artifacts(path):
    """Use Cargo's exact compiler artifacts, never glob an arbitrary old binary."""
    selected = {}
    finished = False
    for line in path.read_text().splitlines():
        row = json.loads(line)
        if row.get('reason') == 'build-finished':
            require(row.get('success') is True, 'Cargo build did not succeed')
            finished = True
        if row.get('reason') != 'compiler-artifact' or not row.get('executable'):
            continue
        target = row['target']
        if target['name'] == 'concert-service' and 'bin' in target['kind'] and not row['profile']['test']:
            key = 'service'
        elif target['name'] == 'bootstrap' and 'test' in target['kind'] and Path(row['manifest_path']).resolve() == ROOT / 'backend/services/concert-service/Cargo.toml':
            key = 'tests'
        else:
            continue
        value = Path(row['executable']).resolve()
        require(key not in selected or selected[key] == value, 'ambiguous build artifact')
        selected[key] = migration.executable(value)
    require(finished, 'Cargo build-finished receipt missing')
    require(set(selected) == {'service', 'tests'}, 'missing service/bootstrap Cargo artifacts')
    return selected


def run_test(cluster, binary, name, report):
    env = {**cluster.env, 'MCR_BOOTSTRAP_ROOT': str(cluster.root),
           'MCR_BOOTSTRAP_TOKEN': cluster.token, 'MCR_BOOTSTRAP_INODE': str(cluster.identity[1]),
           'MCR_BOOTSTRAP_DATABASE_URL': cluster.url(DATABASE)}
    result = migration.execute([str(binary), '--ignored', '--exact', name, '--test-threads=1'],
                               env=env, phase=name, cwd=cluster.root, timeout=60)
    require(re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;', result.stdout), 'native test missing/skipped')
    report['commands'][name] = result.returncode


def schema_gate(cluster, report):
    # A private venv makes required date-time/URI format checkers mandatory.
    validation = cluster.root / 'validation'
    venv.EnvBuilder(with_pip=True).create(validation)
    python = validation / 'bin/python'
    migration.execute([str(python), '-m', 'pip', 'install', '--disable-pip-version-check',
                       '-r', str(ROOT / 'scripts/mcr/requirements-contracts.txt')],
                      env=cluster.env, phase='validation-dependencies', cwd=cluster.root, timeout=60)
    source = """import importlib.util,json,sys
from pathlib import Path
root=Path(sys.argv[1]); response=Path(sys.argv[2])
spec=importlib.util.spec_from_file_location('bootstrap_core_validator',root/'scripts/mcr/validate-core.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
value=m.assets.read_json(response)
m.validate(value,'Bootstrap',json.loads((root/'backend/docs/api/mcr-core.openapi.json').read_text()),actor_id=value['participant']['user_id'],event_id=value['event']['event_id'])
assert value['participant']['profile']['display_name']=='Current public name'
assert value['participant']['profile']['bio']=='Current public bio'
assert not any(k in response.read_text() for k in ['private-canary','provider_subject','refresh_token','avatar_url','password'])
"""
    result = migration.execute([str(python), '-c', source, str(ROOT), str(cluster.root / 'response.json')],
                               env=cluster.env, phase='actual-rpc-c03-validator', cwd=cluster.root)
    report['commands']['actual-rpc-c03-validator'] = result.returncode


def exercise(cluster, artifacts, report):
    cluster.init(); cluster.start()
    report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
    require(report['postgres_version'].startswith('16.14'), 'PostgreSQL16.14 required')
    cluster.sql('postgres', 'create database mcr_concert_bootstrap template template0;')
    env = {**cluster.env, 'CONCERT_DATABASE_URL': cluster.url(DATABASE)}
    manifest = migration.migration_manifest(ROOT / 'backend', 'concert-service')
    for name, flag in [('migrate', '--migrate'), ('verify', '--verify-migrations'), ('reapply', '--migrate')]:
        migration.execute([str(artifacts['service']), flag], env=env, phase=name, cwd=cluster.root)
        report['commands'][name] = 0
        migration.check_ledger(cluster, DATABASE, manifest)
    report['migration_sha384'] = manifest
    release = cluster.root / 'release'; release.mkdir(mode=0o700)
    document = json.loads((ROOT / 'contracts/fixtures/core/bootstrap-open.json').read_text())
    files = {'bootstrap': document, 'avatar': document['avatar_manifest'], 'map': document['map_manifest']}
    files.update({theme['theme_id'].lower(): theme for theme in document['theme_manifests']})
    for name, value in files.items():
        (release / (name + '.json')).write_text(json.dumps(value))
    # Exercise the real binary's startup boundary too: local mode cannot select
    # the synthetic constructor used by the explicit native RPC test below.
    serving = {**env, 'CONCERT_RUNTIME_MODE': 'local',
               'CONCERT_METRICS_ADDR': '127.0.0.1:0', 'CONCERT_GRPC_ADDR': '127.0.0.1:0',
               'CONCERT_BOOTSTRAP_FILE': str(release / 'bootstrap.json'),
               'CONCERT_AVATAR_MANIFEST_FILE': str(release / 'avatar.json'),
               'CONCERT_MAP_MANIFEST_FILE': str(release / 'map.json'),
               'CONCERT_REVENGE_MANIFEST_FILE': str(release / 'revenge.json'),
               'CONCERT_PARADE_MANIFEST_FILE': str(release / 'parade.json'),
               'CONCERT_DANGER_MANIFEST_FILE': str(release / 'danger.json')}
    rejected = migration.execute([str(artifacts['service'])], env=serving,
                                 phase='synthetic-product-startup-refused', cwd=cluster.root,
                                 allowed=(1,))
    require('Concert product release is unavailable' in rejected.stderr,
            'startup must reject synthetic product release')
    require('CONCERT_HEALTH_LISTENER=' not in rejected.stdout,
            'invalid product release opened a listener')
    require(str(cluster.root) not in rejected.stderr and cluster.password not in rejected.stderr,
            'startup error leaked private configuration')
    report['commands']['synthetic-product-startup-refused'] = rejected.returncode
    run_test(cluster, artifacts['tests'], 'native_bootstrap_transactions_and_guarded_response', report)
    schema_gate(cluster, report)
    report['response_sha256'] = hashlib.sha256((cluster.root / 'response.json').read_bytes()).hexdigest()
    before = cluster.assert_postmaster(); cluster.stop(); cluster.start()
    after = cluster.assert_postmaster(); require(before != after, 'postmaster did not restart')
    report['restart'] = {'before_pid': before, 'after_pid': after}
    migration.check_ledger(cluster, DATABASE, manifest)
    run_test(cluster, artifacts['tests'], 'native_bootstrap_restart_persistence', report)
    report['native_tests'] = 2
    report['state'] = 'PASS'


def run(metadata, pg_bin):
    artifacts = build_artifacts(metadata)
    tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
    report = {'state': 'FAIL', 'commands': {}, 'binary_sha256': {
        key: hashlib.sha256(path.read_bytes()).hexdigest() for key,path in artifacts.items()}}
    cluster = migration.Cluster(tools)
    try:
        exercise(cluster, artifacts, report)
    except (migration.SmokeError, OSError, ValueError, subprocess.SubprocessError) as error:
        report['error'] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__
    finally:
        try:
            cluster.cleanup(); report['cleanup'] = 'PASS'
            require(not cluster.root.exists(), 'owned root remains')
        except (migration.SmokeError, OSError) as error:
            report['state'] = 'FAIL'; report['cleanup'] = 'FAIL'
            report['cleanup_error'] = error.detail if isinstance(error,migration.SmokeError) else type(error).__name__
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
