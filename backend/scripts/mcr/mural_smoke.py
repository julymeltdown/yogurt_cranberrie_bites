#!/usr/bin/env python3
"""Qualify mural adapters in a new marker-owned PostgreSQL fixture, never a live database."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = 'mcr_mural'
MIGRATIONS = tuple(path.name for path in sorted((ROOT/'backend/services/concert-service/migrations').glob('*.sql')))

def source_paths():
    paths = [ROOT/'backend/crates/concert-domain/src/mural/mod.rs',
             ROOT/'backend/crates/concert-application/src/mural/mod.rs',
             ROOT/'backend/services/concert-service/src/adapters/out/repository/mural.rs',
             ROOT/'backend/services/concert-service/src/world/mural_subscription.rs',
             ROOT/'backend/services/concert-service/tests/mural.rs',
             ROOT/'backend/services/concert-service/tests/mural_support/mod.rs',
             ROOT/'backend/crates/migration-runtime/src/lib.rs',
             ROOT/'backend/services/concert-service/tests/mural_subscription.rs',
             ROOT/'backend/services/concert-service/src/adapters/out/repository/receipt.rs',
             ROOT/'backend/crates/concert-application/src/commands/mod.rs',
             Path(__file__).resolve()]
    paths.extend(ROOT/path for path in [
        'backend/crates/concert-domain/src/lib.rs','backend/crates/concert-application/src/lib.rs',
        'backend/services/concert-service/src/lib.rs','backend/services/concert-service/src/world/mod.rs',
        'backend/services/concert-service/src/adapters/out/repository.rs',
        'backend/Cargo.toml','backend/Cargo.lock',
        'backend/scripts/mcr/migration_smoke.py','backend/scripts/mcr/test_mural_smoke.py',
        'backend/crates/concert-domain/Cargo.toml','backend/crates/concert-application/Cargo.toml',
        'backend/services/concert-service/Cargo.toml',
        'contracts/domain/v1/mural.schema.json','contracts/domain/v1/operations.json','contracts/domain/v1/policy.json'])
    paths.extend(ROOT/'backend/services/concert-service/migrations'/name for name in MIGRATIONS)
    return {str(path.relative_to(ROOT)):hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}

def verify_build_receipt(receipt, sources, binaries):
    if receipt.get('exit_code')!=0 or receipt.get('source_sha256')!=sources:
        raise RuntimeError('missing-or-stale-build-source-receipt')
    for binary in binaries:
        if receipt.get('binaries',{}).get(str(binary))!=hashlib.sha256(binary.read_bytes()).hexdigest():
            raise RuntimeError('binary-not-bound-to-build-receipt')

def sql_guards(cluster):
    event='dddddddd-dddd-4ddd-8ddd-dddddddddddd'
    snapshot='eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee'
    cluster.sql(DATABASE,f"insert into concert_events(event_id,slug,title,phase) values('{event}','mural-sql-fixture','SQL fixture','OPEN');")
    base=f"insert into concert_mural_state(event_id,snapshot_id,state,pixels,palette,cooldown_ms) values('{event}','{snapshot}','EDITABLE',"
    def refused(sql,label):
        try: cluster.sql(DATABASE,sql)
        except migration.SmokeError: return
        raise RuntimeError(label)
    refused(base+"decode(repeat('00',16383),'hex'),array_fill('#123ABC'::text,array[16]),0);",'grid-length')
    refused(base+"decode(repeat('10',16384),'hex'),array_fill('#123ABC'::text,array[16]),0);",'grid-colors')
    refused(base+"decode(repeat('00',16384),'hex'),array_fill('bad'::text,array[16]),0);",'palette-colors')
    cluster.sql(DATABASE,base+"decode(repeat('00',16384),'hex'),array_fill('#123ABC'::text,array[16]),0);")
    refused(f"update concert_mural_state set revision=1 where event_id='{event}';",'revision-not-enforced')
    refused(f"update concert_mural_state set revision=2,reset_revision=3 where event_id='{event}';",'future-reset')
    cluster.sql(DATABASE,f"update concert_mural_state set state='FROZEN',revision=2,reset_revision=2,snapshot_id=gen_random_uuid() where event_id='{event}';")
    refused(f"update concert_mural_state set state='EDITABLE',revision=3,snapshot_id=gen_random_uuid() where event_id='{event}';",'freeze-terminal')


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,action='append',default=[])
    parser.add_argument('--build-receipt',type=Path)
    parser.add_argument('--sql-only',action='store_true')
    parser.add_argument('--report',required=True,type=Path)
    args=parser.parse_args()
    if args.binary and (args.build_receipt is None or not args.build_receipt.is_absolute()): parser.error('--binary requires absolute --build-receipt')
    if not args.binary and not args.sql_only: parser.error("--binary or --sql-only required")
    if not args.report.is_absolute() or any(not path.is_absolute() for path in args.binary):
        parser.error('explicit absolute paths required')
    fd=os.open(args.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    report={'scope':'BE34/BE35 synthetic native mural adapters','result':'FAIL','cleanup':'NOT_RUN',
            'production_authority':'NOT_RUN','public_routes':'NOT_EXPOSED','IN04':'NOT_RUN',
            'source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
            'migration_ledger':'NOT_APPLIED','tests':[]}
    report['source_sha256']=source_paths()
    cluster=None
    previous=signal.signal(signal.SIGTERM,lambda *_:(_ for _ in ()).throw(KeyboardInterrupt()))
    try:
        tools={name:migration.executable(Path('/opt/homebrew/bin')/name) for name in migration.PG_TOOLS}
        binaries=[migration.executable(path) for path in args.binary]
        if binaries: verify_build_receipt(json.loads(args.build_receipt.read_text()),report['source_sha256'],binaries)
        report['build_receipt_sha256']=hashlib.sha256(args.build_receipt.read_bytes()).hexdigest() if binaries else None
        cluster=migration.Cluster(tools)
        report['owned_directory']=str(cluster.root)
        cluster.init();cluster.start()
        cluster.sql('postgres',f'create database {DATABASE} template template0;')
        report['migration_sha256']={}
        for name in MIGRATIONS:
            path=ROOT/'backend/services/concert-service/migrations'/name
            report['migration_sha256'][name]=hashlib.sha256(path.read_bytes()).hexdigest()
            if args.sql_only: cluster.sql(DATABASE,path.read_text())
        report['postgres_version']=cluster.sql(DATABASE,'show server_version;')
        report['native_adapters']='NOT_RUN' if not binaries else 'RUN'
        environment={**cluster.env,'MURAL_TEST_DATABASE_URL':cluster.url(DATABASE),
            'MURAL_TEST_OWNER_ROOT':str(cluster.root),'MURAL_TEST_OWNER_TOKEN':cluster.token,
            'MURAL_TEST_OWNER_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}'}
        def native(binary:Path,phase:str):
            result=subprocess.run([str(binary),'--test-threads=1'],cwd=cluster.root,env=environment,capture_output=True,text=True,timeout=60)
            output=result.stdout+result.stderr
            expected=2 if binary.name.startswith('mural_subscription-') else 8 if binary.name.startswith('mural-') else None
            if result.returncode==0 and (expected is None or not re.search(rf'test result: ok\. {expected} passed; 0 failed; 0 ignored;',output)):
                raise RuntimeError('native-suite-empty-or-incomplete')
            if cluster.password in output or cluster.url(DATABASE) in output:
                raise RuntimeError('secret-leak')
            row={'phase':phase,'binary':binary.name,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'exit_code':result.returncode,'passed_tests':expected if result.returncode==0 else None,'output_sha256':hashlib.sha256(output.encode()).hexdigest()}
            if result.returncode: row['tail']=output[-4000:]
            report['tests'].append(row)
            if result.returncode: raise RuntimeError('native-mural-test')
        for binary in binaries: native(binary,'before-restart')
        sql_guards(cluster)
        report['direct_sql_guards']='PASS'
        def ledgers():
            values={}
            for database in (DATABASE,'mcr_mural_fresh','mcr_mural_upgrade'):
                rows=json.loads(cluster.sql(database,"select json_agg(json_build_object('version',version,'checksum',encode(checksum,'hex'),'success',success) order by version) from public._sqlx_migrations;"))
                expected={int(name.split('_')[0]):hashlib.sha384((ROOT/'backend/services/concert-service/migrations'/name).read_bytes()).hexdigest() for name in MIGRATIONS}
                if {row['version']:row['checksum'] for row in rows}!=expected or not all(row['success'] for row in rows): raise RuntimeError('migration-ledger-mismatch')
                values[database]=rows
            return values
        ledger_before=ledgers() if binaries else None
        if binaries:
            report['migration_ledger']='PASS'
            report['migration_receipts']=ledger_before
            report['forward_upgrade_preserved_event_receipt']='PASS'
            report['reapply']='PASS'
        query="select md5(string_agg(row_to_json(b)::text,',' order by event_id)) from concert_mural_state b;"
        before=cluster.sql(DATABASE,query)
        if not before: raise RuntimeError('missing-persisted-mural')
        pid=cluster.assert_postmaster();cluster.stop();cluster.start()
        if cluster.assert_postmaster()==pid or cluster.sql(DATABASE,query)!=before:
            raise RuntimeError('restart-persistence')
        report['restart_persistence']='PASS'
        if binaries and ledgers()!=ledger_before: raise RuntimeError('restart-ledger-mismatch')
        # Rerun adapters on independent event fixtures after an actual process restart.
        environment['MURAL_RESTART_VERIFY']='1'
        for binary in binaries: native(binary,'after-restart')
        if source_paths()!=report['source_sha256']: raise RuntimeError('source-changed-during-qualification')
        report['result']='PASS'
    except BaseException as error:
        report['failure']=getattr(error,'detail',type(error).__name__+': '+str(error))
    finally:
        signal.signal(signal.SIGTERM,previous)
        try:
            if cluster is not None: cluster.cleanup()
            report['cleanup']='PASS'
        except BaseException:
            report['cleanup']='FAIL';report['result']='FAIL'
        with os.fdopen(fd,'w') as output: json.dump(report,output,indent=2);output.write('\n')
    print(json.dumps({key:report.get(key) for key in ('result','cleanup','restart_persistence','failure')}))
    return 0 if report['result']=='PASS' else 1
if __name__=='__main__': raise SystemExit(main())
