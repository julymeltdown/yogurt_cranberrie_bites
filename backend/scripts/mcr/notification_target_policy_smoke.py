#!/usr/bin/env python3
"""Frozen NB03-L SQL component, one disposable PG16; never inbox acceptance."""
from __future__ import annotations
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'backend/scripts/mcr'))
from migration_smoke import Cluster, PG_TOOLS, SmokeError, check_ledger, executable, execute, migration_manifest
from auth_invalidation_smoke import frozen_binary

CASES = (
    'local_targets_use_actual_current_owner_rows',
    'missing_foreign_and_unsupported_targets_fail_unavailable',
    'explicit_local_restrictions_hide_without_dependency_downgrade',
    'terminal_metadata_never_creates_a_private_action_grant',
    'retained_row_exclusions_block_actual_writers',
    'original_deadline_and_dependency_failures_remain_errors',
)


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_manifest() -> dict[str, str]:
    # Use the index, not a recursive target walk. Never read a shared target here.
    names = subprocess.check_output(['git', 'ls-files', '-z', '--', 'backend', 'contracts'], cwd=ROOT).decode().split('\0')
    paths = {ROOT / name for name in names if name and (Path(name).suffix in {'.rs', '.sql', '.toml', '.lock', '.proto', '.py', '.json'} or Path(name).name == 'Cargo.lock')}
    paths.add(Path(__file__).resolve())
    for name in ('.cargo/config.toml', 'backend/.cargo/config.toml', 'rust-toolchain.toml', 'backend/rust-toolchain.toml'):
        if (ROOT / name).is_file():
            paths.add(ROOT / name)
    return {str(path.relative_to(ROOT)): sha(path) for path in sorted(paths)}


def qualify(checkpoint: dict, pg: Path, report: dict) -> None:
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if checkpoint['source_commit'] != head or checkpoint['source_sha256'] != source_manifest():
        raise SmokeError('preflight', 'source checkpoint mismatch')
    if subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip():
        raise SmokeError('preflight', 'clean committed source required')
    if not checkpoint.get('target_released'):
        raise SmokeError('preflight', 'explicit target release required before PG')
    binary = frozen_binary(Path(checkpoint['binary']), checkpoint['binary_sha256'])
    if shutil.disk_usage(ROOT).free < 8 * 1024**3:
        raise SmokeError('preflight', '8 GiB disk floor required')
    tools = {name: executable(pg / name) for name in PG_TOOLS}
    cluster = Cluster(tools)
    report.update(source_commit=head, source_sha256=checkpoint['source_sha256'], binary_sha256=checkpoint['binary_sha256'])
    try:
        cluster.init()
        cluster.start()
        if int(cluster.sql('postgres', 'show server_version_num;')) // 10000 != 16:
            raise SmokeError('preflight', 'PG16 required')
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        database = 'mcr_notification_target_policy'
        execute([str(tools['createdb']), '--no-password', '--maintenance-db=postgres', database], env=cluster.client_env('postgres'), phase='create-owned-database', cwd=cluster.root)
        env = {**cluster.env,
               'NB03_LOCAL_TEST_OWNER_ROOT': str(cluster.root),
               'NB03_LOCAL_TEST_OWNER_IDENTITY': f'{cluster.identity[0]}:{cluster.identity[1]}',
               'NB03_LOCAL_TEST_OWNER_TOKEN': cluster.token,
               'NB03_LOCAL_TEST_DATABASE_URL': cluster.url(database)}
        inventory = execute([str(binary), '--list'], env=env, phase='inventory', cwd=ROOT/'backend')
        actual = {line.removesuffix(': test') for line in inventory.stdout.splitlines() if line.endswith(': test')}
        if actual != set(CASES):
            raise SmokeError('inventory', 'exact six owned component cases required')
        result = execute([str(binary), '--ignored', '--test-threads=1', '--nocapture'], env=env, phase='local-policy', cwd=ROOT/'backend', timeout=120)
        if 'test result: ok. 6 passed; 0 failed; 0 ignored;' not in result.stdout:
            raise SmokeError('inventory', 'all six cases must execute')
        report['tests'] = {'exit_code': result.returncode, 'stdout': result.stdout, 'cases': list(CASES)}
        report['migration_receipts'] = check_ledger(cluster, database, migration_manifest(ROOT/'backend', 'concert-service'))
        if int(cluster.sql('postgres', 'select count(*) from pg_prepared_xacts;')):
            raise SmokeError('cleanup', 'unexpected prepared transaction')
        # No protected fixture write was allowed to become an owning effect.
        protected = cluster.sql(database, 'select (select count(*) from public.concert_event_settings)+(select count(*) from public.concert_operation_notices)+(select count(*) from public.concert_social_requests)+(select count(*) from public.concert_relationship_pairs);')
        if int(protected):
            raise SmokeError('fixture', 'protected fixture metadata committed')
        report['protected_fixture_rows_committed'] = 0
        report['prepared_after'] = 0
        if source_manifest() != checkpoint['source_sha256'] or sha(binary) != checkpoint['binary_sha256']:
            raise SmokeError('provenance', 'source or copied binary drift')
    finally:
        cluster.cleanup()
        report['cleanup'] = {'status': 'PASS', 'owned_root_absent': not cluster.root.exists()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--checkpoint', required=True, type=Path)
    parser.add_argument('--pg-bin-dir', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    if not args.pg_bin_dir.is_absolute() or not args.report.is_absolute() or args.report.exists():
        parser.error('explicit absolute PG directory and new absolute report required')
    report = {'status': 'FAIL', 'qualification': 'NB03-L local SQL component only', 'limitations': [
        'Context and SQL metadata fixtures are not canonical Auth/Friend grants or owning command acceptance.',
        'Protected settings/notice/social/pair rows stay uncommitted and roll back; their committed-owner revocation interleavings are NOT_RUN.',
        'Observed writer exclusions cover only committed event, party, giveaway, application, reservation, admission and participant rows.',
        'Report/Post/None remain unavailable. RootReadView binding, canonical owner fences, native registration/recovery, retained child completion/Root EOF and actual inbox/public HTTP are NOT_RUN.']}
    try:
        qualify(json.loads(args.checkpoint.read_text()), args.pg_bin_dir, report)
        report['status'] = 'PASS'
    except SmokeError as error:
        report['failure'] = error.detail
    except Exception as error:
        report['failure'] = {'phase': 'runner', 'diagnostic': type(error).__name__}
    finally:
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True)+'\n')
    print(json.dumps({'status': report['status'], 'report': str(args.report)}))
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    sys.exit(main())
