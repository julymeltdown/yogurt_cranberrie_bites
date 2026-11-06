#!/usr/bin/env python3
"""Run the exact Gateway->Profile integration test binary against a new owned native PG16 cluster.

Compile separately under the coordinated shared-target lease. This controller never runs
Cargo, Docker, imported default services or global cleanup. All paths are arguments; the
binary must be inside the declared target directory.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[5]
CODE = [
    'backend/services/api-gateway/tests/concert_profile_owner_integration.rs',
    'backend/services/api-gateway/tests/support/concert_profile_owner_fixture.rs',
    'backend/services/api-gateway/tests/support/concert_profile_owner_native.py',
    'docs/mcr/backend/gateway-profile-owner-integration.md',
]
INPUTS = [
    'backend/services/api-gateway/src/adapters/out/profile_client.rs',
    'backend/services/api-gateway/src/adapters/out/profile_client/concert_identity.rs',
    'backend/services/api-gateway/src/adapters/out/jwt/concert_access.rs',
    'backend/services/profile-service/src/adapters/in/grpc/concert_identity.rs',
    'backend/services/profile-service/src/adapters/in/grpc/method_policy.rs',
    'backend/services/profile-service/src/adapters/in/grpc/server.rs',
    'backend/services/profile-service/src/adapters/out/persistence/concert_identity.rs',
    'backend/services/profile-service/src/config/mod.rs',
    'backend/crates/profile-application/src/concert_identity.rs',
    'backend/proto/profile/v1/profile.proto',
    'backend/crates/platform/src/actor_delegation.rs',
    'backend/crates/platform/src/actor_delegation/read_authority.rs',
    'backend/crates/platform/src/call_deadline.rs',
    'backend/crates/platform/src/grpc_method_policy.rs',
    'backend/crates/platform/src/edge.rs',
    'backend/crates/platform/src/grpc_deadline.rs',
    'backend/crates/platform/src/edge_concurrency.rs',
    'backend/scripts/mcr/migration_smoke.py',
    'backend/Cargo.lock', 'backend/Cargo.toml',
    'backend/services/api-gateway/Cargo.toml',
]

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--build-metadata', required=True, type=Path)
    parser.add_argument('--target-dir', required=True, type=Path, help='exact declared shared Cargo target directory')
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--pg-bin', required=True, type=Path)
    args = parser.parse_args()
    target = args.target_dir.resolve(strict=True)
    binary = args.binary.resolve(strict=True)
    prefix = target / 'debug' / 'deps' / 'concert_profile_owner_integration-'
    if not str(binary).startswith(str(prefix)) or binary.parent != prefix.parent:
        parser.error('binary must be the integration test artifact inside the declared target directory')
    artifacts = [json.loads(line) for line in args.build_metadata.read_text().splitlines() if line.startswith('{')]
    artifact = next((a for a in artifacts if a.get('reason') == 'compiler-artifact'
                     and a.get('executable') == str(binary)
                     and a['target']['name'] == 'concert_profile_owner_integration'), None)
    if artifact is None:
        parser.error('build metadata does not describe the supplied binary')
    if artifact['profile']['debuginfo'] != 0:
        parser.error('test artifact must use test debug level zero')
    migrations = sorted((ROOT / 'backend/services/profile-service/migrations').glob('*.sql'))
    if not migrations:
        parser.error('expected retained Profile migrations')
    attempt = args.output.resolve() / ('attempt-' + str(time.time_ns()) + '-' + uuid.uuid4().hex[:8])
    attempt.mkdir(parents=True, mode=0o700)
    paths = [*CODE, *INPUTS, *(str(p.relative_to(ROOT)) for p in migrations)]
    pins = {p: sha(ROOT / p) for p in paths}
    report = {'result': 'IN_PROGRESS', 'candidate_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'source_sha256': pins, 'binary': str(binary), 'binary_sha256': sha(binary),
              'build_metadata_sha256': sha(args.build_metadata), 'artifact_profile': artifact['profile'], 'target_dir': str(target),
              'transport': 'configured Gateway Profile client to generated Profile server with production guard; plaintext loopback, not TLS, HTTP or Concert acceptance',
              'migration_mode': 'direct exact retained SQL in order, not SQLx ledger or Profile startup qualification',
              'commands': {}, 'attempt': str(attempt)}
    cluster = None
    def save():
        (attempt / 'receipt.json').write_text(json.dumps(report, indent=2) + '\n')
    def run(name, arguments, env, seconds=90):
        if cluster is not None:
            cluster.assert_owned()
        command = [str(binary), *arguments, '--test-threads=1', '--nocapture']
        log = attempt / (name + '.log')
        started = time.monotonic()
        try:
            with log.open('w') as stream:
                code = subprocess.run(command, cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT, timeout=seconds).returncode
        except subprocess.TimeoutExpired:
            code = 'TIMEOUT'
        report['commands'][name] = {'args': command, 'exit_code': code, 'seconds': time.monotonic() - started, 'log_sha256': sha(log)}
        save()
        text = log.read_text()
        if cluster is not None:
            assert cluster.password not in text and cluster.url('mcr_gateway_profile_owner') not in text, 'credential diagnostic leak'
        for canary in ['legacy-token-canary', 'private-delete-canary', 'private-handle-', 'PRIVATE_ROLE_CANARY']:
            assert canary not in text, 'private diagnostic leak'
        assert code == 0, name + ' failed; see preserved private attempt log'
    save()
    try:
        env = {k: v for k, v in os.environ.items() if not k.startswith(('PG', 'MCR_GATEWAY_PROFILE_OWNER_')) and not k.endswith('DATABASE_URL')}
        run('no-database-guard', ['--skip', 'native_'], env)
        sys.path.insert(0, str(ROOT / 'backend/scripts/mcr'))
        import migration_smoke as m
        cluster = m.Cluster({name: args.pg_bin / name for name in m.PG_TOOLS})
        report['owned_root'] = str(cluster.root)
        report['owner_identity'] = list(cluster.identity)
        save()
        cluster.init(); cluster.start()
        db = 'mcr_gateway_profile_owner'
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        cluster.sql('postgres', 'create database mcr_gateway_profile_owner template template0;')
        report['migration_sha256'] = {str(p.relative_to(ROOT)): sha(p) for p in migrations}
        for path in migrations:
            cluster.sql(db, path.read_text())
            report['commands']['migration:' + path.name] = {'exit_code': 0}
            save()
        cluster.sql(db, 'create role mcr_gateway_profile_reader nologin; create role mcr_gateway_profile_noaccess nologin; grant select(user_id,nickname,bio,deleted_at) on profiles to mcr_gateway_profile_reader; grant select(user_id) on profile_user_deletion_tombstones to mcr_gateway_profile_reader;')
        fixture_env = {**cluster.env, 'MCR_GATEWAY_PROFILE_OWNER_ROOT': str(cluster.root),
                       'MCR_GATEWAY_PROFILE_OWNER_IDENTITY': ':'.join(map(str, cluster.identity)),
                       'MCR_GATEWAY_PROFILE_OWNER_TOKEN': cluster.token,
                       'MCR_GATEWAY_PROFILE_OWNER_DATABASE_URL': cluster.url(db)}
        run('native-online', ['native_', '--skip', 'native_stopped_database_is_fixed_unavailable', '--skip', 'native_restarted_database_serves_a_fresh_read'], fixture_env, 120)
        cluster.stop(); report['owned_server_stopped_for_outage'] = True; save()
        run('native-outage', ['--exact', 'native_stopped_database_is_fixed_unavailable'], fixture_env)
        cluster.start()
        run('native-restarted', ['--exact', 'native_restarted_database_serves_a_fresh_read'], fixture_env)
        assert sha(binary) == report['binary_sha256'], 'binary changed during verification'
        assert all(sha(ROOT / p) == d for p, d in pins.items()), 'source changed during verification'
        report['result'] = 'PASS'
    except BaseException as exc:
        report['result'] = 'FAIL'; report['failure_type'] = type(exc).__name__
        if hasattr(exc, 'detail'):
            report['failure_detail'] = exc.detail  # Cluster errors are already redacted.
        print('Integration attempt failed: ' + type(exc).__name__ + '; evidence ' + str(attempt), flush=True)
    finally:
        if cluster is not None:
            owned = cluster.root
            try:
                cluster.assert_owned()
                pg_log = cluster.root / 'postgres.log'
                if pg_log.is_file():
                    # Preserve the owned server's diagnostics before exact-root cleanup.
                    import migration_smoke as m
                    saved_log = attempt / 'postgres.log'
                    saved_log.write_text(m.redact(pg_log.read_text(), (cluster.password,)))
                    report['postgres_log_sha256'] = sha(saved_log)
                cluster.cleanup()
                report['cleanup'] = {'exact_owned_root_absent': not owned.exists(), 'owned_server_stopped': True}
            except BaseException as exc:
                report['cleanup'] = {'result': 'FAIL', 'failure_type': type(exc).__name__}
                report['result'] = 'FAIL'
        else:
            report['cleanup'] = {'no_pg_fixture_started': True}
        save()
    print(json.dumps({'result': report['result'], 'receipt': str(attempt / 'receipt.json'), 'cleanup': report['cleanup']}), flush=True)
    return 0 if report['result'] == 'PASS' else 1

if __name__ == '__main__':
    raise SystemExit(main())
