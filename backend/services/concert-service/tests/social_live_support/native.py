#!/usr/bin/env python3
"""Frozen bridge/retained-kernel local qualification; never public-runtime acceptance."""
from __future__ import annotations
import argparse
import importlib.util
import json
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
    try:
        cluster.init()
        # Owned cluster only; exercise actual read-only prepared exclusions.
        with (cluster.data/'postgresql.conf').open('a') as config:config.write('\nmax_prepared_transactions=256\n')
        cluster.start()
        report['postgres_version']=cluster.sql('postgres','show server_version;')
        if int(cluster.sql('postgres','show server_version_num;'))//10000!=16:raise SmokeError('preflight','PG16 required')
        system=cluster.sql('postgres','select system_identifier from pg_control_system();')
        schema=ROOT/'backend/services/concert-service'
        files=[schema/'migrations'/n for n in ['0001_concert_core.sql','0002_command_receipts_outbox.sql','0003_places_map_releases.sql','0006_relationship_policy.sql','0009_receipt_domain_failures.sql','0011_outbox_maintenance.sql','0028_product_owner_scope_lifecycle.sql','0036_social_effect_witness.sql','0037_gift_private_child_intents.sql','0042_social_read_child_intents.sql']]
        files += [schema/'src/adapters/out/repository/intro_schema_v1.sql',schema/'src/adapters/out/age_provider/schema-v1.sql']
        report['isolated_prerequisite_sha256']={str(p.relative_to(ROOT)):adapter.sha(p) for p in files}
        files += [schema/'migrations/0051_social_policy_invalidation.sql']
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
        persisted={database:cluster.sql(database,'select (select count(*) from concert_command_receipts),(select count(*) from concert_outbox),(select count(*) from concert_social_effect_witnesses),(select count(*) from concert_social_policy_signals),(select coalesce(sum(revision),0) from concert_social_policy_generations);') for database in ['mcr_social_concert','mcr_concert_be_commands']}
        before=cluster.assert_postmaster();cluster.stop();cluster.start();after=cluster.assert_postmaster()
        if before==after:raise SmokeError('restart','postmaster unchanged')
        for database,value in persisted.items():
            if cluster.sql(database,'select (select count(*) from concert_command_receipts),(select count(*) from concert_outbox),(select count(*) from concert_social_effect_witnesses),(select count(*) from concert_social_policy_signals),(select coalesce(sum(revision),0) from concert_social_policy_generations);')!=value:raise SmokeError('restart','persisted evidence changed')
        report['restart']={'status':'PASS','before_pid':before,'after_pid':after,'persisted_counts':persisted}
        tests('after-restart')
        if source_manifest()!=meta['source_sha256']:raise SmokeError('source','source changed during run')
    finally:
        cluster.cleanup();report['cleanup']={'status':'PASS','owned_root_absent':not cluster.root.exists()}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--artifacts',required=True,type=Path);p.add_argument('--report',required=True,type=Path);p.add_argument('--pg-bin',type=Path,default=Path('/opt/homebrew/opt/postgresql@16/bin'));a=p.parse_args()
    if not a.report.is_absolute() or a.report.exists():p.error('new absolute report required')
    report={'status':'FAIL','limitations':['Synthetic authority/provider/ledger fixtures, no actual acquired OwnerGuardScope, real provider, HTTP/WS/cache or public runtime acceptance.','Selected isolated prerequisite schemas, not current full production migration qualification.']}
    try:run(a,report);report['status']='PASS'
    except SmokeError as e:report['failure']=e.detail
    except Exception as e:report['failure']={'type':type(e).__name__,'detail':str(e)}
    a.report.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');print(json.dumps({'status':report['status'],'report':str(a.report),'failure':report.get('failure')}));return 0 if report['status']=='PASS' else 1
if __name__=='__main__':raise SystemExit(main())
