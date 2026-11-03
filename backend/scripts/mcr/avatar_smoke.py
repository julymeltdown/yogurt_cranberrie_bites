#!/usr/bin/env python3
"""BE07 avatar races/rollback/restart in a fresh marker-owned native PG cluster.

Reuses the audited migration-smoke Cluster and bootstrap fixture execution;
never attaches an existing database, Docker volume or externally supplied URL.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import signal
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


def write_selected_release(root, document):
    selected = json.loads((ROOT / 'contracts/fixtures/assets/selected-static/avatar-manifest.json').read_text())
    selected_document = json.loads(json.dumps(document))
    selected_document['avatar_manifest'] = selected
    selected_document['avatar']['config'] = selected['presets'][0]['config']
    selected_document['participant']['profile']['avatar'] = selected_document['avatar']
    for theme in selected_document['theme_manifests']:
        theme['avatar_manifest_id'] = selected['manifest_id']
        theme['avatar_manifest_version'] = selected['manifest_version']
        theme['avatar_content_hash'] = selected['content_hash']
        theme.pop('content_hash')
        theme['content_hash'] = 'sha256:' + hashlib.sha256(json.dumps(
            theme, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode()).hexdigest()
    selected_dir = root / 'release-selected'; selected_dir.mkdir(mode=0o700)
    selected_files = {'bootstrap': selected_document, 'avatar': selected,
                      'map': selected_document['map_manifest']}
    selected_files.update({theme['theme_id'].lower(): theme for theme in selected_document['theme_manifests']})
    for name, value in selected_files.items():
        (selected_dir / (name + '.json')).write_text(json.dumps(value))
    return {key: selected[key] for key in ('manifest_id', 'manifest_version', 'content_hash')}


def run(metadata, pg_bin, pinned_binaries=None):
    binary = artifacts(metadata)
    source_commit = None
    if pinned_binaries is not None:
        # Explicit copies are tied to an exact compiler artifact and checksum;
        # a later checkout build may overwrite the shared executable path.
        receipt = json.loads(pinned_binaries.read_text())
        matches = [row for row in receipt['artifacts']
                   if row['package'] == 'concert-service'
                   and Path(row['manifest_path']).resolve() == ROOT / 'backend/services/concert-service/Cargo.toml'
                   and Path(row['compiler_executable']).resolve() == binary]
        bootstrap.require(len(matches) == 1, 'exact pinned compiler artifact required')
        row = matches[0]
        snapshot = migration.executable(Path(row['snapshot']))
        bootstrap.require(hashlib.sha256(snapshot.read_bytes()).hexdigest() == row['sha256'], 'pinned binary checksum mismatch')
        binary = snapshot
        source_commit = receipt['source_commit']
    tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
    report = {'state': 'FAIL', 'commands': {}, 'source_commit': source_commit, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}
    cluster = migration.Cluster(tools)
    def interrupted(number, _frame):
        raise migration.SmokeError('signal', f'interrupted by signal {number}')
    previous = {number: signal.signal(number, interrupted) for number in (signal.SIGINT, signal.SIGTERM)}
    try:
        cluster.init(); cluster.start()
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        bootstrap.require(report['postgres_version'].startswith('16.14'), 'PostgreSQL16.14 required')
        cluster.sql('postgres', 'create database mcr_concert_bootstrap template template0;')
        # Exercise the real service schema without changing production ledgers.
        report['migration_sha384'] = migration.migration_manifest(ROOT / 'backend', 'concert-service')
        upgrade_event = '00000000-0000-4000-8000-000000000719'
        upgrade_user = '00000000-0000-4000-8000-000000000720'
        upgrade_before = None
        for path in sorted((ROOT / 'backend/services/concert-service/migrations').glob('*.sql')):
            if not path.name.endswith('.down.sql'):
                if path.name == '0025_whole_character_avatar.sql':
                    cluster.sql(DATABASE, f"""
                        insert into concert_events(event_id,slug,title,phase)
                        values('{upgrade_event}','avatar-upgrade-proof','Avatar upgrade proof','OPEN');
                        insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled)
                        values('{upgrade_event}','{upgrade_user}','REVENGE',false,false,false);
                        insert into concert_avatar_settings(event_id,user_id,contract_version,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id)
                        values('{upgrade_event}','{upgrade_user}','1.0.0','avatar_fixture',1,'body_fixture','eyes_fixture','hair_fixture','outfit_fixture','accessory_fixture','tone_fixture');
                    """)
                    upgrade_before = json.loads(cluster.sql(DATABASE, f"select to_jsonb(a) from concert_avatar_settings a where event_id='{upgrade_event}';"))
                cluster.sql(DATABASE, path.read_text())
                report['commands'][path.name] = 0
                if path.name == '0025_whole_character_avatar.sql':
                    upgrade_after = json.loads(cluster.sql(DATABASE, f"select to_jsonb(a) from concert_avatar_settings a where event_id='{upgrade_event}';"))
                    bootstrap.require(upgrade_after.pop('representation') == 'layered'
                                      and upgrade_after.pop('preset_id') is None
                                      and upgrade_after == upgrade_before,
                                      '0025 changed existing layered state or revision')
                    report['existing_layered_upgrade'] = 'PASS'
                    cluster.sql(DATABASE, f"delete from concert_events where event_id='{upgrade_event}';")
        release = cluster.root / 'release'; release.mkdir(mode=0o700)
        document = json.loads((ROOT / 'contracts/fixtures/core/bootstrap-open.json').read_text())
        files = {'bootstrap': document, 'avatar': document['avatar_manifest'], 'map': document['map_manifest']}
        files.update({theme['theme_id'].lower(): theme for theme in document['theme_manifests']})
        for name, value in files.items():
            (release / (name + '.json')).write_text(json.dumps(value))
        report['selected_manifest_pin'] = write_selected_release(cluster.root, document)
        bootstrap.run_test(cluster, binary, 'native_avatar_transactions', report)
        bootstrap.run_test(cluster, binary, 'native_selected_avatar_transactions', report)
        report['response_sha256'] = hashlib.sha256((cluster.root / 'avatar-state.json').read_bytes()).hexdigest()
        before = cluster.assert_postmaster(); cluster.stop(); cluster.start()
        after = cluster.assert_postmaster(); bootstrap.require(before != after, 'postmaster did not restart')
        report['restart'] = {'before_pid': before, 'after_pid': after}
        bootstrap.run_test(cluster, binary, 'native_avatar_restart_persistence', report)
        bootstrap.run_test(cluster, binary, 'native_selected_avatar_restart_persistence', report)
        report['native_tests'] = 4
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
    for number, handler in previous.items():
        signal.signal(number, handler)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build-metadata', type=Path, required=True)
    parser.add_argument('--pg-bin', type=Path, required=True)
    parser.add_argument('--pinned-binaries', type=Path)
    args = parser.parse_args()
    report = run(args.build_metadata, args.pg_bin, args.pinned_binaries)
    print(json.dumps(report, sort_keys=True))
    return 0 if report['state'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
