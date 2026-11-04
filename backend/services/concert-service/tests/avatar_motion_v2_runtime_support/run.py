#!/usr/bin/env python3
"""Run only exact frozen Rust artifacts, then fresh owned PG16.14 + real restart.

No Cargo, dependency install, external database URL, cache cleanup, or target
access is performed. Auth/Friend scope registration remains a native fixture.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / 'backend/scripts/mcr'))
import migration_smoke as migration
import bootstrap_smoke as bootstrap


def require(value, message):
    if not value:
        raise ValueError(message)


def artifact(metadata, receipt):
    rows = [json.loads(line) for line in metadata.read_text().splitlines()]
    require(any(r.get('reason') == 'build-finished' and r.get('success') is True for r in rows), 'successful Cargo finish required')
    matches = [r for r in rows if r.get('reason') == 'compiler-artifact'
               and r.get('target', {}).get('name') == 'avatar_motion_v2_runtime'
               and 'test' in r.get('target', {}).get('kind', []) and r.get('executable')
               and Path(r['manifest_path']) == ROOT / 'backend/services/concert-service/Cargo.toml']
    require(len(matches) == 1, 'exact unique motion runtime compiler artifact required')
    document = json.loads(receipt.read_text())
    require(document['source_commit'] == subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(), 'source commit mismatch')
    require(document['cargo_metadata_sha256'] == hashlib.sha256(metadata.read_bytes()).hexdigest(), 'Cargo metadata pin mismatch')
    pinned = [r for r in document['artifacts'] if r['package'] == 'concert-service'
              and r['compiler_executable'] == matches[0]['executable']]
    require(len(pinned) == 1, 'exact frozen executable required')
    binary = migration.executable(Path(pinned[0]['snapshot']))
    require(binary.is_absolute() and '/target/' not in str(binary) and '/mcr-target-' not in str(binary), 'copy outside target required')
    require(hashlib.sha256(binary.read_bytes()).hexdigest() == pinned[0]['sha256'], 'frozen binary pin mismatch')
    return binary, document


def run(metadata, receipt, pg_bin):
    binary, pins = artifact(metadata, receipt)
    report = {'state': 'FAIL', 'source_commit': pins['source_commit'], 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'commands': {},
              'limitations': ['Auth/Friend scope registrations are explicit native fixtures.', 'No real public HTTP/WS, operator admission or registry activation is certified.']}
    tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
    cluster = migration.Cluster(tools)
    def interrupted(number, frame):
        raise migration.SmokeError('signal', 'native runner interrupted')
    previous = {number: signal.signal(number, interrupted) for number in (signal.SIGINT, signal.SIGTERM)}
    try:
        cluster.init(); cluster.start()
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        require(report['postgres_version'].startswith('16.14'), 'PostgreSQL16.14 required')
        cluster.sql('postgres', 'create database mcr_concert_bootstrap template template0;')
        report['migration_sha384'] = migration.migration_manifest(ROOT / 'backend', 'concert-service')
        for path in sorted((ROOT / 'backend/services/concert-service/migrations').glob('*.sql')):
            if not path.name.endswith('.down.sql'):
                before = None
                if path.name == '0044_avatar_motion_v2.sql':
                    cluster.sql(bootstrap.DATABASE, """
                        insert into concert_events(event_id,slug,title,phase) values('00000000-0000-4000-8000-000000009410','motion-upgrade','Motion upgrade fixture','OPEN');
                        insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled)
                        select '00000000-0000-4000-8000-000000009410',u::uuid,'REVENGE',false,false,false from unnest(array['00000000-0000-4000-8000-000000009411','00000000-0000-4000-8000-000000009412']) u;
                        insert into concert_avatar_settings(event_id,user_id,contract_version,representation,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id)
                        values('00000000-0000-4000-8000-000000009410','00000000-0000-4000-8000-000000009411','1.0.0','layered','avatar_fixture',1,'body_fixture','eyes_fixture','hair_fixture','outfit_fixture',null,'tone_fixture');
                        insert into concert_avatar_settings(event_id,user_id,contract_version,representation,manifest_id,manifest_version,preset_id)
                        values('00000000-0000-4000-8000-000000009410','00000000-0000-4000-8000-000000009412','1.1.0','whole_character','mcr_selected_static_20261103',1,'photo01_front_01');
                    """)
                    query = "select jsonb_agg(to_jsonb(a) order by user_id) from concert_avatar_settings a where event_id='00000000-0000-4000-8000-000000009410';"
                    before = cluster.sql(bootstrap.DATABASE, query)
                cluster.sql(bootstrap.DATABASE, path.read_text())
                if before is not None:
                    require(cluster.sql(bootstrap.DATABASE, query) == before, '0044 changed existing legacy state/revision')
                    report['legacy_rows_unchanged_by_0044'] = 'PASS'
                report['commands'][path.name] = 0
        bootstrap.run_test(cluster, binary, 'native_motion_v2_transactions_cas_constraints_and_rollback', report)
        report['response_sha256'] = hashlib.sha256((cluster.root / 'motion-v2-state.json').read_bytes()).hexdigest()
        before = cluster.assert_postmaster(); cluster.stop(); cluster.start(); after = cluster.assert_postmaster()
        require(before != after, 'real postmaster restart required')
        report['restart'] = {'before_pid': before, 'after_pid': after}
        bootstrap.run_test(cluster, binary, 'native_motion_v2_restart_persistence', report)
        report['native_tests'] = 2
        report['state'] = 'PASS'
    except (migration.SmokeError, OSError, ValueError, subprocess.SubprocessError) as error:
        report['error'] = error.detail if isinstance(error, migration.SmokeError) else str(error)
    finally:
        try:
            cluster.cleanup(); require(not cluster.root.exists(), 'owned root remains')
            report['cleanup'] = 'PASS'
        except (migration.SmokeError, OSError, ValueError) as error:
            report['cleanup'] = 'FAIL'; report['state'] = 'FAIL'
            report['cleanup_error'] = error.detail if isinstance(error, migration.SmokeError) else str(error)
        for number, handler in previous.items():
            signal.signal(number, handler)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build-metadata', type=Path, required=True)
    parser.add_argument('--pinned-binaries', type=Path, required=True)
    parser.add_argument('--pg-bin', type=Path, required=True)
    args = parser.parse_args()
    try:
        report = run(args.build_metadata, args.pinned_binaries, args.pg_bin)
    except (ValueError, OSError, KeyError) as error:
        report = {'state': 'FAIL', 'preflight_error': str(error), 'cluster_created': False}
    print(json.dumps(report, sort_keys=True))
    return 0 if report['state'] == 'PASS' else 1
if __name__ == '__main__':
    raise SystemExit(main())
