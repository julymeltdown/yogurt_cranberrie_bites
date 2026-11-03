#!/usr/bin/env python3
"""Run the exact generated-RPC test binary with a newly owned native PG16 fixture.

Compile separately under the coordinated shared-target lease; this controller never
runs Cargo, Docker, source-default services or a global database cleanup.
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
DEFAULT_OUT = Path('/Users/lhs/Desktop/mcr-build/profile-self-rpc')
CODE = [
    'backend/services/profile-service/src/adapters/in/grpc/concert_identity.rs',
    'backend/services/profile-service/tests/concert_identity_rpc.rs',
    'backend/services/profile-service/tests/support/concert_identity_fixture.rs',
    'backend/services/profile-service/tests/support/concert_identity_native.py',
]
INPUTS = [
    'backend/proto/profile/v1/profile.proto',
    'backend/services/profile-service/src/adapters/in/grpc/method_policy.rs',
    'backend/services/profile-service/src/adapters/in/grpc/server.rs',
    'backend/services/profile-service/src/adapters/in/grpc/mod.rs',
    'backend/services/profile-service/src/config/mod.rs',
    'backend/services/profile-service/src/adapters/out/persistence/concert_identity.rs',
    'backend/crates/profile-application/src/concert_identity.rs',
    'backend/crates/platform/src/actor_delegation.rs',
    'backend/crates/platform/src/actor_delegation/read_authority.rs',
    'backend/crates/platform/src/actor_delegation/authority_deadline.rs',
    'backend/crates/platform/src/grpc_method_policy.rs',
    'backend/crates/platform/src/grpc_auth.rs',
    'backend/crates/platform/src/call_deadline.rs',
    'backend/crates/platform/src/edge.rs',
    'backend/crates/platform/src/grpc_deadline.rs',
    'backend/scripts/mcr/migration_smoke.py',
    'backend/Cargo.lock', 'backend/Cargo.toml',
    'backend/services/profile-service/Cargo.toml',
    'docs/mcr/backend/profile-rpc-prerequisite.md',
    'MCR_IMPLEMENTATION_PLAN_V2_EN.md', 'docs/mcr/project-assessment.md',
    'docs/mcr/execution-plan.md',
]

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--build-metadata', required=True, type=Path)
    parser.add_argument('--output', type=Path, default=DEFAULT_OUT)
    parser.add_argument('--pg-bin', type=Path, default=Path('/opt/homebrew/opt/postgresql@16/bin'))
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if not str(binary).startswith('/Users/lhs/Desktop/mcr-build/backend/debug/deps/concert_identity_rpc-'):
        parser.error('the coordinated shared-target RPC binary is required')
    artifacts = [json.loads(line) for line in args.build_metadata.read_text().splitlines() if line.startswith('{')]
    artifact = next(a for a in artifacts if a.get('reason') == 'compiler-artifact'
                    and a.get('executable') == str(binary) and a['target']['name'] == 'concert_identity_rpc')
    if artifact['profile']['debuginfo'] != 0:
        parser.error('RPC test artifact must use test debug level zero')
    migrations = sorted((ROOT / 'backend/services/profile-service/migrations').glob('*.sql'))
    if len(migrations) != 7:
        parser.error('expected exact seven retained Profile migrations')
    attempt = args.output.resolve() / ('attempt-' + str(time.time_ns()) + '-' + uuid.uuid4().hex[:8])
    attempt.mkdir(parents=True, mode=0o700)
    paths = [*CODE, *INPUTS, *(str(p.relative_to(ROOT)) for p in migrations)]
    pins = {p: sha(ROOT / p) for p in paths}
    report = {'result': 'IN_PROGRESS', 'candidate_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'source_sha256': pins, 'binary': str(binary), 'binary_sha256': sha(binary),
              'build_metadata_sha256': sha(args.build_metadata), 'artifact_profile': artifact['profile'],
              'transport': 'generated service plus production caller/actor guard; plaintext controlled loopback, not TLS qualification',
              'migration_mode': 'direct exact retained SQL in order, not SQLx ledger or Profile startup qualification',
              'commands': {}, 'attempt': str(attempt)}
    cluster = None
    def save():
        (attempt / 'receipt.json').write_text(json.dumps(report, indent=2) + '\n')
    def run(name, arguments, env, seconds=60):
        if cluster is not None:
            cluster.assert_owned()
        command = [str(binary), *arguments, '--test-threads=1']
        log = attempt / (name + '.log')
        started = time.monotonic()
        try:
            with log.open('w') as stream:
                result = subprocess.run(command, cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT, timeout=seconds)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code = 'TIMEOUT'
        report['commands'][name] = {'args': command, 'exit_code': code, 'seconds': time.monotonic()-started, 'log_sha256': sha(log)}
        save()
        text = log.read_text()
        if cluster is not None:
            assert cluster.password not in text and cluster.url('mcr_profile_self_rpc') not in text, 'credential diagnostic leak'
        for canary in ['legacy-token-canary', 'private-delete-canary', 'private-handle-']:
            assert canary not in text, 'private diagnostic leak'
        assert code == 0, name + ' failed; see preserved private attempt log'
    save()
    try:
        # Deterministic signed transport never opens a database connection.
        env = {k:v for k,v in os.environ.items() if not k.startswith(('PG', 'MCR_PROFILE_RPC_')) and not k.endswith('DATABASE_URL')}
        run('signed-transport', ['--skip', 'native_'], env)
        sys.path.insert(0, str(ROOT / 'backend/scripts/mcr'))
        import migration_smoke as m
        cluster = m.Cluster({name: args.pg_bin / name for name in m.PG_TOOLS})
        report['owned_root'] = str(cluster.root)
        report['owner_identity'] = list(cluster.identity)
        save()
        cluster.init(); cluster.start()
        db = 'mcr_profile_self_rpc'
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        cluster.sql('postgres', 'create database mcr_profile_self_rpc template template0;')
        report['migration_sha256'] = {str(p.relative_to(ROOT)): sha(p) for p in migrations}
        for path in migrations:
            cluster.sql(db, path.read_text())
            report['commands']['migration:' + path.name] = {'exit_code': 0}
            save()
        cluster.sql(db, 'create role mcr_profile_rpc_reader nologin; create role mcr_profile_rpc_noaccess nologin; grant select(user_id,nickname,bio,deleted_at) on profiles to mcr_profile_rpc_reader; grant select(user_id) on profile_user_deletion_tombstones to mcr_profile_rpc_reader;')
        fixture_env = {**cluster.env, 'MCR_PROFILE_RPC_OWNER_ROOT': str(cluster.root),
                       'MCR_PROFILE_RPC_OWNER_IDENTITY': ':'.join(map(str, cluster.identity)),
                       'MCR_PROFILE_RPC_OWNER_TOKEN': cluster.token,
                       'MCR_PROFILE_RPC_DATABASE_URL': cluster.url(db)}
        run('native-online', ['native_', '--skip', 'native_rpc_stopped_owned_database_is_fixed_unavailable'], fixture_env)
        cluster.stop(); report['owned_server_stopped_for_outage'] = True; save()
        run('native-outage', ['--exact', 'native_rpc_stopped_owned_database_is_fixed_unavailable'], fixture_env)
        cluster.start()
        run('native-recovered', ['--exact', 'native_rpc_minimal_private_projection_unicode_and_fresh_updates'], fixture_env)
        assert sha(binary) == report['binary_sha256'], 'binary changed during verification'
        assert all(sha(ROOT / path) == digest for path, digest in pins.items()), 'source changed during verification'
        report['result'] = 'PASS'
    except BaseException as exc:
        report['result'] = 'FAIL'; report['failure_type'] = type(exc).__name__
        # Avoid exception text/tracebacks containing database options or fixture contents.
        print('RPC attempt failed: ' + type(exc).__name__ + '; evidence ' + str(attempt), flush=True)
    finally:
        if cluster is not None:
            owned = cluster.root
            try:
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
