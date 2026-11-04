#!/usr/bin/env python3
"""Frozen bridge/retained-kernel local qualification; never public-runtime acceptance."""
from __future__ import annotations
import argparse
import importlib.util
import json
import re
from pathlib import Path
import shutil
import subprocess
import sys
from urllib.parse import urlsplit

ROOT=Path(__file__).resolve().parents[5]
prior=ROOT/'backend/services/concert-service/tests/social_remaining_support/native.py'
spec=importlib.util.spec_from_file_location('social_adapter_native',prior)
adapter=importlib.util.module_from_spec(spec);spec.loader.exec_module(adapter)
Cluster,PG_TOOLS,SmokeError,execute,executable=adapter.Cluster,adapter.PG_TOOLS,adapter.SmokeError,adapter.execute,adapter.executable

def source_manifest():
    result=adapter.source_manifest()
    # Freeze every tracked backend/contract dependency, including the sibling
    # services linked as dev dependencies. Build caches and Python bytecode are
    # not source. This reads only this isolated checkout, never a peer target.
    for name in subprocess.check_output(['git','ls-files','-z','--','backend','contracts'],cwd=ROOT).decode().split('\0'):
        if name and (ROOT/name).is_file():result[name]=adapter.sha(ROOT/name)
    for name in ['social.schema.json','feed.schema.json','common.schema.json','operations.json','policy.json']:
        p=ROOT/'contracts/domain/v1'/name;result[str(p.relative_to(ROOT))]=adapter.sha(p)
    # Pin the implementation, migration and focused test sources as well.
    for name in ['backend/services/concert-service/src/social_live/invalidation.rs','backend/services/concert-service/tests/social_live_support/invalidation.rs','backend/services/concert-service/migrations/0051_social_policy_invalidation.sql']:
        p=ROOT/name;result[name]=adapter.sha(p)
    return result

def run(args,report):
    meta=json.loads(args.artifacts.read_text())
    commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    if commit!=meta['source_commit'] or source_manifest()!=meta['source_sha256']:
        raise SmokeError('source','frozen candidate changed')
    binaries={k:adapter.frozen_binary(Path(v['path']),v['sha256']) for k,v in meta['artifacts'].items()}
    if set(binaries)!={'live','remaining','receipts'}:raise SmokeError('inventory','three exact test executables required')
    if shutil.disk_usage(ROOT).free<8*1024**3:raise SmokeError('preflight','8 GiB free required')
    report.update(source_commit=commit,source_sha256=meta['source_sha256'],artifacts=meta['artifacts'],checks={})
    pure=execute([str(binaries['live']),'--test-threads=1'],env={},phase='pure-input-tests',cwd=ROOT,timeout=30)
    if 'test result: ok. 8 passed; 0 failed; 15 ignored;' not in pure.stdout:raise SmokeError('inventory','exact pure/native inventory required')
    report['checks']['pure-input-tests']={'exit_code':pure.returncode,'stdout':pure.stdout}
    cluster=Cluster({name:executable(args.pg_bin/name) for name in PG_TOOLS})
    report['owned_root']=str(cluster.root)
    def prepared_zero(phase):
        n=int(cluster.sql('postgres','select count(*) from pg_prepared_xacts;'))
        report.setdefault('prepared_checks',{})[phase]={'count':n,'status':'PASS' if n==0 else 'FAIL'}
        if n:raise SmokeError('prepared-cleanup','residual prepared transactions before owned stop')
    try:
        cluster.init()
        # Owned cluster only; exercise actual read-only prepared exclusions.
        with (cluster.data/'postgresql.conf').open('a') as config:config.write("\nmax_prepared_transactions=256\nmax_connections=24\nshared_buffers=32MB\nwork_mem=2MB\nmax_worker_processes=0\nmax_parallel_workers=0\nmax_wal_senders=0\nautovacuum=off\nlog_line_prefix='MCR_SQLSTATE=%e '\nlog_min_error_statement=panic\n")
        cluster.start()
        report['postgres_version']=cluster.sql('postgres','show server_version;')
        if int(cluster.sql('postgres','show server_version_num;'))//10000!=16:raise SmokeError('preflight','PG16 required')
        system=cluster.sql('postgres','select system_identifier from pg_control_system();')
        schema=ROOT/'backend/services/concert-service'
        files=[schema/'migrations'/n for n in ['0001_concert_core.sql','0002_command_receipts_outbox.sql','0003_places_map_releases.sql','0006_relationship_policy.sql','0009_receipt_domain_failures.sql','0011_outbox_maintenance.sql','0028_product_owner_scope_lifecycle.sql','0036_social_effect_witness.sql','0037_gift_private_child_intents.sql','0042_social_read_child_intents.sql']]
        files += [schema/'src/adapters/out/repository/intro_schema_v1.sql',schema/'src/adapters/out/age_provider/schema-v1.sql']
        files += [schema/'migrations/0051_social_policy_invalidation.sql']
        report['isolated_prerequisite_sha256']={str(p.relative_to(ROOT)):adapter.sha(p) for p in files}
        for database in ['mcr_social_concert','mcr_concert_be_commands']:
            execute([str(cluster.tools['createdb']),'--no-password','--maintenance-db=postgres',database],env=cluster.client_env('postgres'),phase='createdb',cwd=cluster.root)
            for p in files:cluster.sql(database,p.read_text())
        cluster.sql('mcr_social_concert',"create table fixture_social_owner(id integer primary key);insert into fixture_social_owner values(1);create table fixture_social_verifier(id integer primary key,verifier text,revision text,available boolean,expires_at timestamptz);insert into fixture_social_verifier values(1,'fixture','v1',true,clock_timestamp()+interval '1 hour');")
        env={**cluster.env,'MCR_SOCIAL_ROOT':str(cluster.root),'MCR_SOCIAL_TOKEN':cluster.token,'MCR_SOCIAL_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}','MCR_SOCIAL_SYSTEM':system,'MCR_SOCIAL_CONCERT_URL':cluster.url('mcr_social_concert'),
             'CONCERT_TEST_DATABASE_URL':cluster.url('mcr_concert_be_commands'),'CONCERT_TEST_OWNER_ROOT':str(cluster.root),'CONCERT_TEST_OWNER_TOKEN':cluster.token,'CONCERT_TEST_OWNER_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}'}
        def tests(phase):
            for name,count in [('live',15),('remaining',8),('receipts',17)]:
                command=[str(binaries[name]),'--test-threads=1','--nocapture']
                if name!='receipts':command.append('--ignored')
                if name=='remaining':command+=['--skip','actual_qa04_http_authorization_matrix']
                r=execute(command,env=env,phase=f'{phase}-{name}',cwd=ROOT/'backend',timeout=120,allowed=(0,101))
                def sanitized(output):
                    for key,value in env.items():
                        if 'URL' in key:
                            output=output.replace(value,'<private-database-url>')
                            password=urlsplit(value).password
                            if password:output=output.replace(password,'<redacted>')
                        if any(part in key for part in ('PASSWORD','SECRET','TOKEN')) and value:output=output.replace(value,'<redacted>')
                    return output
                report['checks'][f'{phase}-{name}']={'exit_code':r.returncode,'stdout':sanitized(r.stdout),'stderr':sanitized(r.stderr)}
                if r.returncode:raise SmokeError(f'{phase}-{name}','native test failure; see sanitized check output',r.returncode)
                if f'test result: ok. {count} passed; 0 failed;' not in r.stdout:raise SmokeError('inventory',f'{name} exact test inventory required')
        tests('before-restart')
        prepared_zero('before-restart-stop')
        persisted={database:cluster.sql(database,'select (select count(*) from concert_command_receipts),(select count(*) from concert_outbox),(select count(*) from concert_social_effect_witnesses),(select count(*) from concert_social_policy_signals),(select coalesce(sum(revision),0) from concert_social_policy_generations);') for database in ['mcr_social_concert','mcr_concert_be_commands']}
        before=cluster.assert_postmaster();cluster.stop();cluster.start();after=cluster.assert_postmaster()
        if before==after:raise SmokeError('restart','postmaster unchanged')
        for database,value in persisted.items():
            if cluster.sql(database,'select (select count(*) from concert_command_receipts),(select count(*) from concert_outbox),(select count(*) from concert_social_effect_witnesses),(select count(*) from concert_social_policy_signals),(select coalesce(sum(revision),0) from concert_social_policy_generations);')!=value:raise SmokeError('restart','persisted evidence changed')
        report['restart']={'status':'PASS','before_pid':before,'after_pid':after,'persisted_counts':persisted}
        tests('after-restart')
        prepared_zero('before-final-stop')
        if source_manifest()!=meta['source_sha256']:raise SmokeError('source','source changed during run')
    finally:
        try:
            # SQL adapters intentionally erase raw database errors. Preserve
            # only bounded SQLSTATE, known identifier-only errors and function
            # context; never statements, parameters, DETAIL rows or credentials.
            log=cluster.root/'postgres.log'
            safe=[]
            if log.is_file():
                for line in log.read_text(errors='replace').splitlines():
                    match=re.search(r'MCR_SQLSTATE=([0-9A-Z]{5})\s+ERROR:\s+(.*)',line)
                    if match:
                        message=match[2]
                        known=re.fullmatch(r'(query returned (no|more than one) rows|record "[a-zA-Z_0-9]+" (has no field "[a-zA-Z_0-9]+"|is not assigned yet)|column reference "[a-zA-Z_0-9]+" is ambiguous|operator does not exist: [a-zA-Z_0-9 =\[\]]+|relationship authority expired(?: during policy generation)?|unbound social policy outcome)',message)
                        safe.append({'sqlstate':match[1],'message':message if known else 'Database error; raw text omitted'})
                    context=re.search(r'CONTEXT:\s+(PL/pgSQL function [a-zA-Z_0-9.]+\([a-zA-Z_0-9, ]*\) line [0-9]+ at [A-Za-z ]+)$',line)
                    if context:safe.append({'context':context[1]})
            report['postgres_diagnostics']=safe[-100:]
            if cluster.started:prepared_zero('before-teardown-finally')
        finally:
            cluster.cleanup();report['cleanup']={'status':'PASS','owned_root_absent':not cluster.root.exists()}

def policy_recovery(args, report):
    """Current full SQLx ledger; focused recovery, not a public WS qualification."""
    from invalidation_clients_smoke import pki
    meta = json.loads(args.artifacts.read_text())
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if commit != meta['source_commit'] or source_manifest() != meta['source_sha256']:
        raise SmokeError('source', 'frozen recovery source changed')
    binaries = {k: adapter.frozen_binary(Path(v['path']), v['sha256']) for k, v in meta['artifacts'].items()}
    if set(binaries) != {'live', 'delivery', 'policy'}:
        raise SmokeError('inventory', 'exact live/delivery/policy copied executables required')
    report.update(source_commit=commit, source_sha256=meta['source_sha256'], artifacts=meta['artifacts'], checks={})
    cluster = Cluster({name: executable(args.pg_bin / name) for name in PG_TOOLS})
    report['owned_root'] = str(cluster.root)
    # The outside lease watchdog learns the cluster before any PG child exists.
    (args.report.parent / 'runtime-owned-root.json').write_text(json.dumps({'path': str(cluster.root)}))
    custody = args.report.parent / 'runtime-custody.json'
    custody.write_text(json.dumps({'path': str(cluster.root), 'identity': cluster.identity, 'token': cluster.token}))
    custody.chmod(0o600)
    env = {}
    def zero(phase):
        n = int(cluster.sql('postgres', 'select count(*) from pg_prepared_xacts;'))
        report.setdefault('prepared_checks', {})[phase] = {'count': n, 'status': 'PASS' if n == 0 else 'FAIL'}
        if n: raise SmokeError('prepared', 'residual prepared transactions')
    def tests(phase, binary, count, extra=(), native=True):
        command = [str(binaries[binary]), '--test-threads=1', '--nocapture']
        if native: command.append('--ignored')
        result = execute(command + list(extra), env=env, cwd=ROOT / 'backend', phase=phase, timeout=120, allowed=(0,101))
        def safe(text):
            for value in [cluster.password, cluster.token, cluster.url('mcr_social_concert')]:
                if value: text = text.replace(value, '<private-value>')
            return text
        report['checks'][phase] = {'exit_code': result.returncode, 'stdout': safe(result.stdout), 'stderr': safe(result.stderr)}
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
        if result.returncode or f'test result: ok. {count} passed; 0 failed;' not in result.stdout:
            raise SmokeError(phase, 'recovery check failed or inventory changed; see sanitized report')
    try:
        tests('pure-live', 'live', 8, native=False)
        tests('pure-policy', 'policy', 1, native=False)
        cluster.init()
        with (cluster.data / 'postgresql.conf').open('a') as config:
            config.write("\nmax_prepared_transactions=256\nmax_connections=24\nshared_buffers=32MB\nwork_mem=2MB\nmax_worker_processes=0\nmax_parallel_workers=0\nmax_wal_senders=0\nautovacuum=off\nlog_line_prefix='MCR_SQLSTATE=%e '\nlog_min_error_statement=panic\n")
        cluster.start(); pki(cluster)
        report['postgres_version'] = cluster.sql('postgres', 'show server_version;')
        if int(cluster.sql('postgres', 'show server_version_num;')) // 10000 != 16:
            raise SmokeError('preflight', 'PG16 required')
        db = 'mcr_social_concert'
        execute([str(cluster.tools['createdb']), '--no-password', '--maintenance-db=postgres', db], env=cluster.client_env('postgres'), phase='createdb', cwd=cluster.root)
        migrations = Path(meta['migration_directory'])
        expected = adapter.migration_manifest(ROOT / 'backend', 'concert-service')
        import hashlib
        frozen = {int(p.name.split('_', 1)[0]): hashlib.sha384(p.read_bytes()).hexdigest() for p in migrations.glob('*.sql') if not p.name.endswith('.down.sql')}
        if frozen != expected: raise SmokeError('source', 'frozen full migration readset differs')
        env = {**cluster.env, 'MCR_SOCIAL_ROOT': str(cluster.root), 'MCR_SOCIAL_TOKEN': cluster.token,
               'MCR_SOCIAL_IDENTITY': f'{cluster.identity[0]}:{cluster.identity[1]}',
               'MCR_SOCIAL_SYSTEM': cluster.sql('postgres', 'select system_identifier from pg_control_system();'),
               'MCR_SOCIAL_CONCERT_URL': cluster.url(db), 'MCR_SOCIAL_MIGRATIONS': str(migrations)}
        tests('actual-sqlx-migrations', 'live', 1, ['--exact', 'invalidation_tests::initialize_current_owned_schema'])
        expected = adapter.migration_manifest(ROOT / 'backend', 'concert-service')
        report['migration_sha384'] = expected
        report['migration_count'] = adapter.check_ledger(cluster, db, expected)
        # These run before the explicit pending restart seed. No fixture SQL
        # settles its signals, advances clocks, or changes its deadline.
        tests('kernel-recovery', 'live', 8, ['invalidation_tests::', '--skip', 'initialize_current_owned_schema', '--skip', 'pending_round_restart_'])
        tests('delivery-recovery', 'delivery', 9)
        tests('factory-recovery', 'policy', 2)
        env['MCR_SOCIAL_RECOVERY_PHASE'] = 'BEFORE_RESTART'
        tests('restart-pending-seed', 'live', 1, ['--exact', 'invalidation_tests::pending_round_restart_seed'])
        zero('before-restart-stop')
        query = "select (select count(*) from concert_command_receipts),(select count(*) from concert_outbox),(select count(*) from concert_social_policy_signals),(select count(*) from concert_social_policy_signals where source_outbox is null and published_at is null),(select coalesce(sum(revision),0) from concert_social_policy_generations);"
        persisted = cluster.sql(db, query)
        if int(persisted.split('|')[3]) != 2:
            raise SmokeError('restart', 'exact two real deadline signals must remain pending')
        env['MCR_SOCIAL_RECOVERY_POSTMASTER_BEFORE'] = cluster.sql(db, 'select pg_postmaster_start_time()::text;')
        before = cluster.assert_postmaster(); cluster.stop(); cluster.start(); after = cluster.assert_postmaster()
        if before == after or cluster.sql(db, query) != persisted:
            raise SmokeError('restart', 'physical restart or persisted source proof failed')
        report['restart'] = {'status': 'PASS', 'before_pid': before, 'after_pid': after, 'persisted_counts': persisted}
        env['MCR_SOCIAL_RECOVERY_PHASE'] = 'AFTER_RESTART'
        tests('restart-pending-recovery', 'live', 1, ['--exact', 'invalidation_tests::pending_round_restart_recovers_unpublished_expiry_with_fresh_owner'])
        adapter.check_ledger(cluster, db, expected)
        zero('before-final-stop')
        if source_manifest() != meta['source_sha256']:
            raise SmokeError('source', 'recovery source changed during execution')
    finally:
        try:
            if cluster.started: zero('before-teardown-finally')
        finally:
            safe = []
            log = cluster.root / 'postgres.log'
            if log.is_file():
                for line in log.read_text(errors='replace').splitlines():
                    match = re.search(r'MCR_SQLSTATE=([0-9A-Z]{5})\s+ERROR:', line)
                    if match: safe.append({'sqlstate': match[1]})
            report['postgres_diagnostics'] = safe[-100:]
            cluster.cleanup()
            report['cleanup'] = {'status': 'PASS', 'owned_root_absent': not cluster.root.exists()}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--policy-recovery',action='store_true');p.add_argument('--artifacts',required=True,type=Path);p.add_argument('--report',required=True,type=Path);p.add_argument('--pg-bin',type=Path,default=Path('/opt/homebrew/opt/postgresql@16/bin'));a=p.parse_args()
    if not a.report.is_absolute() or a.report.exists():p.error('new absolute report required')
    report={'status':'FAIL','limitations':['Synthetic authority/provider/ledger fixtures, no actual acquired OwnerGuardScope, real provider, HTTP/WS/cache or public runtime acceptance.','Selected isolated prerequisite schemas, not current full production migration qualification.']}
    if a.policy_recovery: report['limitations']=['Actual full SQLx migration ledger and private PG recovery; explicit command/authority/membership fixtures. No real Auth/admission, positive Notification, socket/cache or IN03 proof.']
    try:
        (policy_recovery if a.policy_recovery else run)(a,report);report['status']='PASS'
    except SmokeError as e:report['failure']=e.detail
    except Exception as e:report['failure']={'type':type(e).__name__,'detail':str(e)}
    a.report.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');print(json.dumps({'status':report['status'],'report':str(a.report),'failure':report.get('failure')}));return 0 if report['status']=='PASS' else 1
if __name__=='__main__':raise SystemExit(main())
