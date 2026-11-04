#!/usr/bin/env python3
"""Qualify frozen social increment executables on a private socket-only PG16."""
from __future__ import annotations
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / 'backend/scripts/mcr'))
from migration_smoke import Cluster, PG_TOOLS, SmokeError, executable, execute, check_ledger, migration_manifest
from auth_invalidation_smoke import frozen_binary

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def source_manifest():
    paths = {ROOT / 'backend/Cargo.toml', ROOT / 'backend/Cargo.lock', Path(__file__).resolve()}
    for prefix in ['backend/crates', 'backend/proto', 'backend/services/friend-service', 'backend/services/concert-service']:
        paths.update(p for p in (ROOT / prefix).rglob('*') if p.is_file() and p.suffix in {'.rs', '.sql', '.toml', '.proto', '.py'})
    for name in ['migration_smoke.py', 'auth_invalidation_smoke.py']:
        paths.add(ROOT / 'backend/scripts/mcr' / name)
    return {str(p.relative_to(ROOT)): sha(p) for p in sorted(paths)}

def run(args, report):
    meta = json.loads(args.artifacts.read_text())
    current = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if current != meta['source_commit'] or source_manifest() != meta['source_sha256']:
        raise SmokeError('preflight', 'source checkpoint changed')
    binaries = {k: frozen_binary(Path(v['path']), v['sha256']) for k, v in meta['artifacts'].items()}
    if set(binaries) != {'friend', 'concert'}:
        raise SmokeError('preflight', 'exact two test executables required')
    if shutil.disk_usage(ROOT).free < 8 * 1024**3:
        raise SmokeError('preflight', '8 GiB free required')
    cluster = Cluster({name: executable(args.pg_bin / name) for name in PG_TOOLS})
    report.update(source_commit=current, source_sha256=meta['source_sha256'], artifacts=meta['artifacts'], owned_root=str(cluster.root), checks={})
    try:
        cluster.init(); cluster.start()
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        if int(cluster.sql('postgres', 'show server_version_num;')) // 10000 != 16:
            raise SmokeError('preflight', 'PG16 required')
        system = cluster.sql('postgres', 'select system_identifier from pg_control_system();')
        for name in ['friend', 'concert']:
            execute([str(cluster.tools['createdb']), '--no-password', '--maintenance-db=postgres', f'mcr_social_{name}'], env=cluster.client_env('postgres'), phase='createdb', cwd=cluster.root)
        concert = ROOT / 'backend/services/concert-service'
        prerequisites = [concert / 'migrations' / name for name in ['0001_concert_core.sql', '0002_command_receipts_outbox.sql', '0003_places_map_releases.sql', '0006_relationship_policy.sql']]
        prerequisites += [concert / 'src/adapters/out/repository/intro_schema_v1.sql', concert / 'src/adapters/out/age_provider/schema-v1.sql']
        report['isolated_prerequisite_schema_sha256'] = {str(p.relative_to(ROOT)): sha(p) for p in prerequisites}
        for p in prerequisites:
            cluster.sql('mcr_social_concert', p.read_text())
        cluster.sql('mcr_social_concert', "create table fixture_social_owner(id integer primary key);insert into fixture_social_owner values(1);create table fixture_social_verifier(id integer primary key,verifier text,revision text,available boolean,expires_at timestamptz);insert into fixture_social_verifier values(1,'fixture','v1',true,clock_timestamp()+interval '1 hour');")
        env = {**cluster.env, 'MCR_SOCIAL_ROOT': str(cluster.root), 'MCR_SOCIAL_TOKEN': cluster.token,
               'MCR_SOCIAL_IDENTITY': f'{cluster.identity[0]}:{cluster.identity[1]}', 'MCR_SOCIAL_SYSTEM': system,
               'MCR_SOCIAL_FRIEND_URL': cluster.url('mcr_social_friend'), 'MCR_SOCIAL_CONCERT_URL': cluster.url('mcr_social_concert')}
        def tests(phase):
            for name, binary in binaries.items():
                result = execute([str(binary), '--ignored', '--test-threads=1', '--nocapture'], env=env, phase=f'{phase}-{name}', cwd=ROOT / 'backend', timeout=90)
                expected = 5 if name == 'friend' else 6
                if f'test result: ok. {expected} passed; 0 failed; 0 ignored;' not in result.stdout:
                    raise SmokeError('inventory', 'exact native test inventory required')
                report['checks'][f'{phase}-{name}'] = {'exit_code': result.returncode, 'stdout': result.stdout}
            check_ledger(cluster, 'mcr_social_friend', migration_manifest(ROOT / 'backend', 'friend-service'))
        tests('before-restart')
        counts = {name: cluster.sql(f'mcr_social_{name}', sql) for name, sql in {
            'friend': 'select (select count(*) from user_blocks),(select count(*) from follows),(select count(*) from friend_world_invalidation_journal);',
            'concert': 'select (select count(*) from concert_intros),(select count(*) from concert_age_claims);'}.items()}
        before = cluster.assert_postmaster(); cluster.stop(); cluster.start(); after = cluster.assert_postmaster()
        if before == after:
            raise SmokeError('restart', 'postmaster did not change')
        for name, sql in {'friend': 'select (select count(*) from user_blocks),(select count(*) from follows),(select count(*) from friend_world_invalidation_journal);', 'concert': 'select (select count(*) from concert_intros),(select count(*) from concert_age_claims);'}.items():
            if cluster.sql(f'mcr_social_{name}', sql) != counts[name]:
                raise SmokeError('restart', 'persisted row counts changed')
        report['restart'] = {'status': 'PASS', 'before_pid': before, 'after_pid': after, 'persisted_counts': counts}
        tests('after-restart')
        if source_manifest() != meta['source_sha256']:
            raise SmokeError('source', 'source changed during qualification')
    finally:
        cluster.cleanup()
        report['cleanup'] = {'status': 'PASS', 'owned_root_absent': not cluster.root.exists()}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--pg-bin', type=Path, default=Path('/opt/homebrew/opt/postgresql@16/bin'))
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if not args.report.is_absolute() or args.report.exists():
        parser.error('new absolute report required')
    report = {'status': 'FAIL', 'limitations': ['Synthetic fenced owner/provider fixtures only; no real provider, registered claim/intro runtime schema, canonical root join, HTTP/WS/browser acceptance.']}
    try:
        run(args, report); report['status'] = 'PASS'
    except SmokeError as e:
        report['failure'] = e.detail
    except Exception as e:
        report['failure'] = {'type': type(e).__name__, 'detail': str(e)}
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(json.dumps({'status': report['status'], 'report': str(args.report), 'failure': report.get('failure')}))
    return 0 if report['status'] == 'PASS' else 1

if __name__ == '__main__':
    raise SystemExit(main())
