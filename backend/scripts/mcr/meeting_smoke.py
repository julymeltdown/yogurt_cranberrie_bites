#!/usr/bin/env python3
"""Qualify meeting adapters in a new marker-owned PostgreSQL fixture, never a live database."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = 'mcr_meeting'
MIGRATIONS = ('0001_concert_core.sql','0002_command_receipts_outbox.sql','0003_places_map_releases.sql','0008_meeting_beacons.sql')

def sql_guards(cluster):
    event='dddddddd-dddd-4ddd-8ddd-dddddddddddd'
    beacon='eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee'
    actor='ffffffff-ffff-4fff-8fff-ffffffffffff'
    cluster.sql(DATABASE,f"insert into concert_events(event_id,slug,title,phase) values('{event}','meeting-sql-fixture','SQL fixture','OPEN');")
    cluster.sql(DATABASE,f"insert into concert_meeting_settings(event_id,event_cutoff) values('{event}',clock_timestamp()+interval '1 day');")
    fields="beacon_id,event_id,owner_id,place_id,map_revision,place_revision,selected_catalog_revision,instructions,identification,audience,expires_at"
    audience=json.dumps({'kind':'EXPLICIT_USERS','user_ids':[actor]},separators=(',',':'))
    base=f"insert into concert_meeting_beacons({fields}) values('{beacon}','{event}','{actor}','{actor}',1,1,1,'Synthetic only','Synthetic only','{audience}',"
    def refused(sql,label):
        try: cluster.sql(DATABASE,sql)
        except migration.SmokeError: return
        raise RuntimeError(label)
    refused(base+"clock_timestamp()+interval '901 seconds');",'ttl-not-enforced')
    refused(base+"clock_timestamp()-interval '1 second');",'past-expiry-not-enforced')
    cluster.sql(DATABASE,base+"clock_timestamp()+interval '2 seconds');")
    refused(f"update concert_meeting_beacons set instructions='wrong revision' where beacon_id='{beacon}';",'revision-not-enforced')
    refused(f"update concert_meeting_beacons set owner_id='{event}',revision=revision+1 where beacon_id='{beacon}';",'scope-not-enforced')
    cluster.sql(DATABASE,f"update concert_meeting_beacons set state='REVOKED',revision=revision+1 where beacon_id='{beacon}';")
    refused(f"update concert_meeting_beacons set state='ACTIVE',revision=revision+1 where beacon_id='{beacon}';",'terminal-revoke-not-enforced')


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,action='append',default=[])
    parser.add_argument('--sql-only',action='store_true')
    parser.add_argument('--report',required=True,type=Path)
    args=parser.parse_args()
    if not args.binary and not args.sql_only: parser.error("--binary or --sql-only required")
    if not args.report.is_absolute() or any(not path.is_absolute() for path in args.binary):
        parser.error('explicit absolute paths required')
    fd=os.open(args.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    report={'scope':'BE09/BE19 synthetic native meeting adapters','result':'FAIL','cleanup':'NOT_RUN',
            'production_authority':'NOT_RUN','public_routes':'NOT_EXPOSED','IN02':'NOT_RUN',
            'source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
            'migration_ledger':'NOT_APPLIED','tests':[]}
    source_paths = [*sorted((ROOT/'backend/services/concert-service/src/meeting').glob('*.rs')),
        *sorted((ROOT/'backend/services/concert-service/tests').glob('meeting*.rs')),
        ROOT/'backend/services/concert-service/tests/support/meeting/mod.rs',
        ROOT/'backend/crates/concert-domain/src/beacon/mod.rs', ROOT/'backend/crates/concert-domain/src/rally/mod.rs',
        ROOT/'backend/crates/concert-application/src/beacon/mod.rs', ROOT/'backend/crates/concert-application/src/rally/mod.rs',Path(__file__).resolve()]
    report['source_sha256']={str(path.relative_to(ROOT)):hashlib.sha256(path.read_bytes()).hexdigest() for path in source_paths}
    cluster=None
    previous=signal.signal(signal.SIGTERM,lambda *_:(_ for _ in ()).throw(KeyboardInterrupt()))
    try:
        tools={name:migration.executable(Path('/opt/homebrew/bin')/name) for name in migration.PG_TOOLS}
        binaries=[migration.executable(path) for path in args.binary]
        cluster=migration.Cluster(tools)
        report['owned_directory']=str(cluster.root)
        cluster.init();cluster.start()
        cluster.sql('postgres',f'create database {DATABASE} template template0;')
        report['migration_sha256']={}
        for name in MIGRATIONS:
            path=ROOT/'backend/services/concert-service/migrations'/name
            report['migration_sha256'][name]=hashlib.sha256(path.read_bytes()).hexdigest()
            cluster.sql(DATABASE,path.read_text())
        report['postgres_version']=cluster.sql(DATABASE,'show server_version;')
        sql_guards(cluster)
        report['direct_sql_guards']='PASS'
        report['native_adapters']='NOT_RUN' if not binaries else 'RUN'
        environment={**cluster.env,'MEETING_TEST_DATABASE_URL':cluster.url(DATABASE),
            'MEETING_TEST_OWNER_ROOT':str(cluster.root),'MEETING_TEST_OWNER_TOKEN':cluster.token,
            'MEETING_TEST_OWNER_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}'}
        def native(binary:Path,phase:str):
            result=subprocess.run([str(binary),'--test-threads=1'],cwd=cluster.root,env=environment,capture_output=True,text=True,timeout=60)
            output=result.stdout+result.stderr
            if cluster.password in output or cluster.url(DATABASE) in output:
                raise RuntimeError('secret-leak')
            row={'phase':phase,'binary':binary.name,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'exit_code':result.returncode,'output_sha256':hashlib.sha256(output.encode()).hexdigest()}
            if result.returncode: row['tail']=output[-4000:]
            report['tests'].append(row)
            if result.returncode: raise RuntimeError('native-meeting-test')
        for binary in binaries: native(binary,'before-restart')
        query="select md5(string_agg(row_to_json(b)::text,',' order by beacon_id)) from concert_meeting_beacons b;"
        before=cluster.sql(DATABASE,query)
        if not before: raise RuntimeError('missing-persisted-beacons')
        pid=cluster.assert_postmaster();cluster.stop();cluster.start()
        if cluster.assert_postmaster()==pid or cluster.sql(DATABASE,query)!=before:
            raise RuntimeError('restart-persistence')
        report['restart_persistence']='PASS'
        # Rerun adapters on independent event fixtures after an actual process restart.
        environment['MEETING_RESTART_VERIFY']='1'
        for binary in binaries: native(binary,'after-restart')
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
