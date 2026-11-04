#!/usr/bin/env python3
"""Copied Sharing factory test only; requires a fresh bounded native allocation.

Read-only helper provenance: migration_smoke.Cluster and invalidation_clients_smoke.pki,
plus Social native source_manifest, at cf1ca629. No Cargo/cache access or fake owner.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / 'backend/scripts/mcr'))
import migration_smoke as core
from invalidation_clients_smoke import pki
spec = importlib.util.spec_from_file_location('sharing_source', ROOT / 'backend/services/concert-service/tests/social_live_support/native.py')
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)
CASE = 'actual_sharing_worker_incumbent_factory_refusal_cancellation_and_restart'
OWNERS = ('auth', 'concert')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(args):
    meta = json.loads(args.artifacts.read_text())
    if subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip():
        raise RuntimeError('clean frozen source required')
    if subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() != meta['source_commit']:
        raise RuntimeError('source commit changed')
    if source.source_manifest() != meta['source_sha256']:
        raise RuntimeError('source readset changed')
    artifact = meta['artifacts']['sharing']
    binary = Path(artifact['path'])
    if not binary.is_absolute() or binary.is_symlink() or not binary.is_file() or sha(binary) != artifact['sha256']:
        raise RuntimeError('copied Sharing executable changed')
    if meta['cargo_exit_code'] != 0:
        raise RuntimeError('failed build')
    for owner in OWNERS:
        frozen = Path(meta['migration_directories'][owner + '-service'])
        expected = core.migration_manifest(ROOT / 'backend', owner + '-service')
        actual = {int(p.name.split('_', 1)[0]): hashlib.sha384(p.read_bytes()).hexdigest()
                  for p in frozen.glob('*.sql') if not p.name.endswith('.down.sql')}
        if actual != expected:
            raise RuntimeError('full frozen migration readset changed')
    for path, digest in meta['tool_readset'].items():
        if sha(Path(path)) != digest:
            raise RuntimeError('native tool changed')
    return binary, meta


def qualify(args, report):
    binary, meta = verify(args)
    if time.time() >= args.deadline_epoch:
        raise RuntimeError('native allocation deadline expired')
    tools = {n: core.executable(args.pg_bin / n) for n in core.PG_TOOLS}
    cluster = core.Cluster(tools)
    report.update(source_commit=meta['source_commit'], binary_sha256=sha(binary), checks={}, owned_root=str(cluster.root))
    # The independent lease watchdog receives custody before initdb/PG children.
    (args.report.parent / 'runtime-owned-root.json').write_text(json.dumps({'path': str(cluster.root)}))
    custody = args.report.parent / 'runtime-custody.json'
    custody.write_text(json.dumps({'path': str(cluster.root), 'identity': cluster.identity, 'token': cluster.token}))
    custody.chmod(0o600)

    def zero(phase):
        count = int(cluster.sql('postgres', 'select count(*) from pg_prepared_xacts;'))
        report.setdefault('prepared_checks', {})[phase] = count
        if count:
            raise RuntimeError('unsettled prepared transactions')

    def test(phase, env):
        left = args.deadline_epoch - time.time()
        if left <= 0:
            raise RuntimeError('native allocation deadline expired')
        command = [str(binary), '--ignored', '--exact', CASE, '--test-threads=1', '--nocapture']
        child = subprocess.Popen(command, env=env, cwd=ROOT / 'backend', stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, text=True, start_new_session=True)
        began = time.monotonic()
        try:
            out, err = child.communicate(timeout=min(120, left))
        except BaseException:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.communicate(timeout=5)
            raise
        for secret in [cluster.password, cluster.token, *(cluster.url('mcr_canonical_' + o) for o in OWNERS)]:
            out, err = out.replace(secret, '<private-value>'), err.replace(secret, '<private-value>')
        report['checks'][phase] = {'exit_code': child.returncode, 'stdout': out, 'stderr': err,
                                   'elapsed_seconds': round(time.monotonic() - began, 3)}
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
        if child.returncode or 'test result: ok. 1 passed; 0 failed;' not in out:
            raise RuntimeError('Sharing factory case failed; see sanitized report')

    try:
        cluster.init()
        with (cluster.data / 'postgresql.conf').open('a') as f:
            f.write("\nmax_prepared_transactions=256\nmax_connections=48\nshared_buffers=32MB\nwork_mem=2MB\nmax_worker_processes=0\nmax_parallel_workers=0\nmax_wal_senders=0\nautovacuum=off\nlog_line_prefix='MCR_SQLSTATE=%e '\nlog_min_error_statement=panic\n")
        cluster.start()
        pki(cluster)
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        if int(cluster.sql('postgres', 'show server_version_num;')) // 10000 != 16:
            raise RuntimeError('PG16 required')
        env = {**cluster.env, 'CONCERT_SERVICE_CREDENTIAL': 'concert-fixture',
               'MCR_CANONICAL_ROOT': str(cluster.root),
               'MCR_CANONICAL_ROOT_IDENTITY': f'{cluster.identity[0]}:{cluster.identity[1]}',
               'MCR_CANONICAL_ROOT_TOKEN': cluster.token,
               'MCR_CANONICAL_SYSTEM': cluster.sql('postgres', 'select system_identifier from pg_control_system();')}
        for owner in OWNERS:
            db = 'mcr_canonical_' + owner
            core.execute([str(tools['createdb']), '--no-password', '--maintenance-db=postgres', db],
                         env=cluster.client_env('postgres'), cwd=cluster.root, phase='create-' + owner)
            env['MCR_CANONICAL_' + owner.upper() + '_URL'] = cluster.url(db)
        inventory = subprocess.check_output([str(binary), '--list', '--format', 'terse'], env=cluster.env, text=True)
        if inventory.count(CASE + ': test') != 1:
            raise RuntimeError('exact Sharing test inventory required')
        env['MCR_SHARING_RUNTIME_PHASE'] = 'SOURCE'
        test('source-worker-factory-refusal-cancellation', env)
        report['migration_counts'] = {o: core.check_ledger(cluster, 'mcr_canonical_' + o,
            core.migration_manifest(ROOT / 'backend', o + '-service')) for o in OWNERS}
        zero('before-physical-stop')
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise RuntimeError('physical PG restart required')
        report['restart'] = {'before_pid': before, 'after_pid': after}
        env['MCR_SHARING_RUNTIME_PHASE'] = 'RESTARTED'
        test('fresh-worker-factory-replay', env)
        for o in OWNERS:
            core.check_ledger(cluster, 'mcr_canonical_' + o, core.migration_manifest(ROOT / 'backend', o + '-service'))
        zero('after-fresh-replay')
        verify(args)
    finally:
        try:
            if cluster.started:
                zero('before-final-teardown')
        finally:
            # SQLSTATE only: do not retain statements/parameters/private fixture data.
            log = cluster.root / 'postgres.log'
            if log.is_file():
                import re
                report['postgres_sqlstates'] = re.findall(r'MCR_SQLSTATE=([0-9A-Z]{5})\s+ERROR:', log.read_text(errors='replace'))[-100:]
            cluster.cleanup()
            report['cleanup'] = {'owned_root_absent': not cluster.root.exists()}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--artifacts', type=Path, required=True)
    p.add_argument('--pg-bin', type=Path, required=True)
    p.add_argument('--report', type=Path, required=True)
    p.add_argument('--deadline-epoch', type=float, required=True)
    args = p.parse_args()
    if not all(x.is_absolute() for x in (args.artifacts, args.pg_bin, args.report)) or args.report.exists():
        p.error('explicit absolute artifacts/PG path/new report required')
    def stop(_signum, _frame):
        raise KeyboardInterrupt('bounded Sharing native interrupted')
    signal.signal(signal.SIGTERM, stop)
    report = {'status': 'FAIL', 'limitations': [
        'Authenticated SOURCE and Apply use production handlers on test-owned mTLS loopback listeners; main/Gateway mounting is a separate publishing-owner gate.',
        'Foreign-membership disconnect is an explicit storage fixture; real socket delivery/cache removal is not certified.',
        'Only Sharing settles. Seven required saga targets remain pending; aggregate BE42 is not accepted.',
        'Social storage uses AES-GCM opaque fixtures and explicit storage guards, not fabricated age/Intro/admission authority.'
    ]}
    try:
        qualify(args, report)
        report['status'] = 'PASS'
    except (Exception, KeyboardInterrupt) as e:
        report['failure'] = type(e).__name__ + ': ' + str(e)
    finally:
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
        args.report.chmod(0o600)
    print(json.dumps({'status': report['status'], 'report': str(args.report)}))
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    sys.exit(main())
