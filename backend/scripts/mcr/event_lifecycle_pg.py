#!/usr/bin/env python3
"""Owned BE41 SQL/restart and optional native adapter fixture; not full acceptance."""
import argparse
import hashlib
import json
import os
import re
import time
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[3]
SERVICE = ROOT / 'backend/services/concert-service'
EVENT = '41414141-4141-4141-8141-414141414141'
ACTOR = '41414141-4141-4141-8141-414141414142'
JOB = '41414141-4141-4141-8141-414141414143'
KEY = '41414141-4141-4141-8141-414141414144'
STEPS = ['READ_ONLY','FREEZE_MURAL','SNAPSHOT','EXPIRE_GRANTS','EXPIRE_BEACONS','EXPORT','CLEANUP']

def run(args, **kw):
    return subprocess.run(args, check=True, capture_output=True, text=True, timeout=40, **kw)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--native-binary', type=Path, action='append', default=[])
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    for binary in ['initdb','pg_ctl','psql']:
        if not shutil.which(binary): raise SystemExit(f'{binary} is required')
    with tempfile.TemporaryDirectory(prefix='mcr-be41-owned-') as temp:
        home = Path(temp); data = home/'data'; sock = home/'socket'; sock.mkdir()
        owner = uuid.uuid4().hex; (home/'BE41_OWNER').write_text(owner)
        # No TCP listener. Each fixture has an exclusive Unix socket and data dir.
        run(['initdb','-D',str(data),'-A','trust','--no-locale','-E','UTF8'])
        options = f"-c listen_addresses='' -k {sock} -c max_connections=12 -c fsync=on"
        def start(): run(['pg_ctl','-D',str(data),'-l',str(home/'postgres.log'),'-o',options,'-w','start'])
        def stop(): run(['pg_ctl','-D',str(data),'-m','fast','-w','stop'])
        start()
        evidence = {'task':'BE41','status':'IN_PROGRESS','scope':('real owned PostgreSQL constraints/restart and source-qualified native adapters; not production-owner or full BE41 acceptance' if args.native_binary else 'real owned PostgreSQL SQL constraints and restart; not Rust/owner-adapter acceptance'),'checks':[], 'source_hashes':{}}
        def sql(command, database='mcr_be41_fixture', fails=False):
            p = subprocess.run(['psql','-X','-h',str(sock),'-d',database,'-v','ON_ERROR_STOP=1','-Atq'], input=command, capture_output=True, text=True, timeout=30)
            if (p.returncode == 0) == fails: raise AssertionError(f'Unexpected SQL result ({p.returncode}): {p.stderr[:300]}')
            return p.stdout.strip()
        def check(name, command, expected=None, fails=False):
            value=sql(command,fails=fails)
            if expected is not None and value != expected: raise AssertionError(f'{name}: {value!r} != {expected!r}')
            evidence['checks'].append({'name':name,'exit_code':0})
        def proof(): return "SET LOCAL mcr.lifecycle_authority_until='2100-01-01T00:00:00Z';"
        progress={'job_id':JOB,'event_id':EVENT,'state':'PENDING','revision':1,'steps':[{'name':s,'state':'SUCCEEDED' if i==0 else 'PENDING'} for i,s in enumerate(STEPS)]}
        def encoded(p):return "'"+json.dumps(p,separators=(',',':')).replace("'","''")+"'::jsonb"
        try:
            for relative in ['backend/Cargo.toml','backend/Cargo.lock',
                    'backend/crates/concert-domain/src/lib.rs',
                    'backend/crates/concert-application/src/lib.rs',
                    'backend/services/concert-service/src/lib.rs',
                    'contracts/generated/mcr-domain.openapi.json']:
                evidence['source_hashes'][relative]=hashlib.sha256((ROOT/relative).read_bytes()).hexdigest()
            sql('CREATE DATABASE mcr_be41_fixture;',database='postgres')
            assert (SERVICE/'migrations/0018_event_lifecycle.sql').read_bytes()==(SERVICE/'src/event_lifecycle/schema.sql').read_bytes(), 'Lifecycle canonical migration drift'
            for path in sorted((SERVICE/'migrations').glob('*.sql')):
                content=path.read_text(); sql(content); evidence['source_hashes'][str(path.relative_to(ROOT))]=hashlib.sha256(content.encode()).hexdigest()
            sql(f"INSERT INTO concert_events(event_id,slug,title,phase) VALUES('{EVENT}','be41-owned','BE41 fixture','OPEN');")
            check('atomic_readonly_and_durable_job', f"BEGIN;{proof()} INSERT INTO concert_closure_jobs(job_id,event_id,actor_id,start_key,original_event_revision,retention_revision,closed_at,cleanup_after,progress) VALUES('{JOB}','{EVENT}','{ACTOR}','{KEY}',1,1,clock_timestamp(),clock_timestamp()+interval '1 hour',{encoded(progress)}); UPDATE concert_events SET phase='READ_ONLY',revision=revision+1 WHERE event_id='{EVENT}'; INSERT INTO concert_closure_effects VALUES('{EVENT}','{JOB}','READ_ONLY',gen_random_uuid(),1,clock_timestamp()); COMMIT;")
            check('one_job_per_event',f"BEGIN;{proof()}INSERT INTO concert_closure_jobs SELECT * FROM concert_closure_jobs;COMMIT;",fails=True)
            false=json.loads(json.dumps(progress));false['revision']=2;false['state']='COMPLETED'
            check('false_completion_rejected',f"BEGIN;{proof()}UPDATE concert_closure_jobs SET revision=2,progress={encoded(false)};COMMIT;",fails=True)
            success=json.loads(json.dumps(progress));success['revision']=2;success['state']='RUNNING';success['steps'][1]['state']='SUCCEEDED'
            check('success_without_owner_evidence_rejected',f"BEGIN;{proof()}UPDATE concert_closure_jobs SET revision=2,progress={encoded(success)};COMMIT;",fails=True)
            check('deadline_missing_rejected',f"BEGIN;INSERT INTO concert_closure_audit VALUES(gen_random_uuid(),'{EVENT}','{JOB}','{ACTOR}',NULL,'SNAPSHOT','FAILED',1,clock_timestamp());COMMIT;",fails=True)
            check('expired_deadline_rejected',f"BEGIN;SET LOCAL mcr.lifecycle_authority_until='2000-01-01';INSERT INTO concert_closure_audit VALUES(gen_random_uuid(),'{EVENT}','{JOB}','{ACTOR}',NULL,'SNAPSHOT','FAILED',1,clock_timestamp());COMMIT;",fails=True)
            sql('CREATE TABLE be41_owned_effects(event_id uuid,step text,applied integer NOT NULL,PRIMARY KEY(event_id,step));')
            # Explicit rollback simulates a process dying before its PG commit.
            check('explicit_rollback_discards_effect_and_progress',f"BEGIN;{proof()}INSERT INTO be41_owned_effects VALUES('{EVENT}','FREEZE_MURAL',1);ROLLBACK;SELECT count(*) FROM be41_owned_effects;",'0')
            crash_command=f"BEGIN;{proof()}INSERT INTO be41_owned_effects VALUES('{EVENT}','SNAPSHOT',1);SELECT pg_sleep(30);"
            process=subprocess.Popen(['psql','-X','-h',str(sock),'-d','mcr_be41_fixture','-v','ON_ERROR_STOP=1','-Atq'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=dict(os.environ,PGAPPNAME='mcr_be41_crash'))
            process.stdin.write(crash_command);process.stdin.close()
            try:
                for _ in range(40):
                    ready=sql("SELECT count(*) FROM pg_stat_activity WHERE application_name='mcr_be41_crash' AND wait_event='PgSleep';")
                    if ready=='1': break
                    time.sleep(0.05)
                else: raise AssertionError('Crash fixture did not reach in-transaction wait')
                sql("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='mcr_be41_crash';")
                process.wait(timeout=5)
                if process.returncode==0:raise AssertionError('Backend termination did not interrupt transaction')
                check('backend_process_death_rolls_back_uncommitted_effect',"SELECT count(*) FROM be41_owned_effects;",'0')
            finally:
                if process.poll() is None:process.kill();process.wait(timeout=5)
                process.stdout.close();process.stderr.close()
            check('applied_effect_and_progress_commit_together',f"BEGIN;{proof()}INSERT INTO be41_owned_effects VALUES('{EVENT}','FREEZE_MURAL',1);INSERT INTO concert_closure_effects VALUES('{EVENT}','{JOB}','FREEZE_MURAL',gen_random_uuid(),1,clock_timestamp());UPDATE concert_closure_jobs SET revision=2,progress={encoded(success)};COMMIT;")
            failed=json.loads(json.dumps(success));failed['revision']=3;failed['state']='PARTIAL_FAILURE';failed['steps'][2]['state']='FAILED'
            check('partial_failure_persists',f"BEGIN;{proof()}SAVEPOINT effect;INSERT INTO be41_owned_effects VALUES('{EVENT}','SNAPSHOT',1);ROLLBACK TO effect;UPDATE concert_closure_jobs SET revision=3,progress={encoded(failed)};COMMIT;")
            stop();start()
            check('restart_preserves_success_and_failed_boundary',"SELECT progress->>'state'||':'||revision FROM concert_closure_jobs;",'PARTIAL_FAILURE:3')
            check('partial_failed_effect_not_committed',"SELECT string_agg(step,',') FROM be41_owned_effects;",'FREEZE_MURAL')
            regressed=json.loads(json.dumps(failed));regressed['revision']=4;regressed['steps'][1]['state']='PENDING'
            check('completed_effect_cannot_regress',f"BEGIN;{proof()}UPDATE concert_closure_jobs SET revision=4,progress={encoded(regressed)};COMMIT;",fails=True)
            resumed=json.loads(json.dumps(failed));resumed['revision']=4;resumed['state']='PENDING';resumed['steps'][2]['state']='PENDING'
            check('resume_failed_boundary_without_repeating_success',f"BEGIN;{proof()}UPDATE concert_closure_jobs SET revision=4,progress={encoded(resumed)};COMMIT;SELECT sum(applied) FROM be41_owned_effects;",'1')
            check('retention_and_actor_binding_immutable',f"BEGIN;{proof()}UPDATE concert_closure_jobs SET actor_id=gen_random_uuid(),revision=5,progress=jsonb_set(progress,'{{revision}}','5') WHERE job_id='{JOB}';COMMIT;",fails=True)
            check('retention_due_timestamp_immutable',f"BEGIN;{proof()}UPDATE concert_closure_jobs SET cleanup_after=closed_at,revision=5,progress=jsonb_set(progress,'{{revision}}','5') WHERE job_id='{JOB}';COMMIT;",fails=True)
            archive_mural={'event_id':EVENT,'state':'FROZEN','snapshot_id':JOB}
            sql(f"INSERT INTO concert_closure_archives(event_id,job_id,source_revision,final_mural) VALUES('{EVENT}','{JOB}',1,'{json.dumps(archive_mural)}');")
            check('final_mural_immutable',f"UPDATE concert_closure_archives SET revision=2,source_revision=2,final_mural=jsonb_set(final_mural,'{{state}}','\"ACTIVE\"');",fails=True)
            check('archive_revision_requires_source_advance',"UPDATE concert_closure_archives SET revision=2;",fails=True)
            check('archive_source_revision_advances_atomically',"UPDATE concert_closure_archives SET revision=2,source_revision=2;SELECT revision||':'||source_revision FROM concert_closure_archives;",'2:2')
            privacy_source=SERVICE/'src/event_lifecycle/privacy.rs'
            text=privacy_source.read_text();evidence['source_hashes'][str(privacy_source.relative_to(ROOT))]=hashlib.sha256(text.encode()).hexdigest()
            queries=dict(re.findall(r'pub const (EXPIRE_[A-Z_]+)\s*:\s*&str\s*=\s*"([^"]+)"',text))
            if set(queries)!={'EXPIRE_GRANTS','EXPIRE_REQUESTS','EXPIRE_BEACONS'}:raise AssertionError('Missing production privacy SQL constants')
            request=str(uuid.uuid4());peer=str(uuid.uuid4());foreign=str(uuid.uuid4())
            sql(f"INSERT INTO concert_events(event_id,slug,title,phase) VALUES('{foreign}','be41-foreign','Foreign owned fixture','LIVE');")
            for scoped in [EVENT,foreign]:
                sql(f"""BEGIN;SET LOCAL mcr.relationship_authority_until='2100-01-01T00:00:00Z';INSERT INTO concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) VALUES('{scoped}','{request}','{ACTOR}','{peer}',gen_random_uuid(),1,1,1,'PENDING',clock_timestamp()+interval '1 hour');UPDATE concert_social_requests SET state='APPROVED',revision=revision+1 WHERE event_id='{scoped}' AND request_id='{request}';INSERT INTO concert_social_bindings VALUES('{scoped}','{request}','X',1,1,true,true);INSERT INTO concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) VALUES('{scoped}','{request}','{ACTOR}','{peer}','X',1,1,'ACTIVE',(select expires_at from concert_social_requests where event_id='{scoped}' and request_id='{request}'));INSERT INTO concert_meeting_settings(event_id,event_cutoff) VALUES('{scoped}',clock_timestamp()+interval '1 hour');INSERT INTO concert_meeting_beacons(beacon_id,event_id,owner_id,place_id,map_revision,place_revision,selected_catalog_revision,instructions,identification,audience,expires_at) VALUES(gen_random_uuid(),'{scoped}','{ACTOR}',gen_random_uuid(),1,1,1,'Owned fixture instructions','Owned fixture identification','{{"kind":"EXPLICIT_USERS","user_ids":["{peer}"]}}',clock_timestamp()+interval '120 seconds');COMMIT;""")
            for name,query in queries.items():
                sql("BEGIN;SET LOCAL mcr.relationship_authority_until='2100-01-01T00:00:00Z';"+query.replace('$1',f"'{EVENT}'")+";COMMIT;")
            check('actual_concert_privacy_expiry_sql_revokes_only_closed_event',f"SELECT (SELECT count(*) FROM concert_social_grants WHERE event_id='{EVENT}' AND state='ACTIVE')+(SELECT count(*) FROM concert_social_requests WHERE event_id='{EVENT}' AND state in ('PENDING','APPROVED'))+(SELECT count(*) FROM concert_meeting_beacons WHERE event_id='{EVENT}' AND state='ACTIVE');",'0')
            check('actual_concert_privacy_expiry_preserves_foreign_event',f"SELECT (SELECT count(*) FROM concert_social_grants WHERE event_id='{foreign}' AND state='ACTIVE')+(SELECT count(*) FROM concert_social_requests WHERE event_id='{foreign}' AND state in ('PENDING','APPROVED'))+(SELECT count(*) FROM concert_meeting_beacons WHERE event_id='{foreign}' AND state='ACTIVE');",'3')
            check('evidence_append_only',"DELETE FROM concert_closure_effects;",fails=True)
            check('audit_append_only',f"BEGIN;{proof()}INSERT INTO concert_closure_audit VALUES(gen_random_uuid(),'{EVENT}','{JOB}','{ACTOR}',NULL,'SNAPSHOT','FAILED',4,clock_timestamp());COMMIT;DELETE FROM concert_closure_audit;",fails=True)
            evidence['native_adapter_suite']='NOT_RUN'
            if args.native_binary:
                if len(args.native_binary)!=2:raise AssertionError('Provide both event_lifecycle and event_lifecycle_archive native binaries')
                expected={'event_lifecycle_full_migration_ledger_and_reapply','event_lifecycle_idempotent_start_receipt_is_admission_only','event_lifecycle_partial_failure_rolls_back_and_explicit_resume_skips_success','event_lifecycle_retention_wait_and_current_authority_fail_closed','event_lifecycle_concurrent_workers_and_wrong_receipt','event_lifecycle_archive_deletion_invalidates_cursor_and_never_reintroduces_content','event_lifecycle_restart_seed','event_lifecycle_restart_resume','event_lifecycle_actual_privacy_sql_expires_and_cleans_only_owned_event'}
                sources=[ROOT/p for p in [
                    'backend/crates/concert-domain/src/event_lifecycle/mod.rs',
                    'backend/crates/concert-application/src/event_lifecycle/mod.rs',
                    'backend/services/concert-service/src/event_lifecycle/mod.rs',
                    'backend/services/concert-service/src/event_lifecycle/archive.rs',
                    'backend/services/concert-service/src/event_lifecycle/http.rs',
                    'backend/services/concert-service/src/event_lifecycle/privacy.rs',
                    'backend/services/concert-service/src/event_lifecycle/schema.sql',
                    'backend/services/concert-service/src/workers/cleanup.rs',
                    'backend/services/concert-service/tests/event_lifecycle.rs',
                    'backend/services/concert-service/tests/event_lifecycle_archive.rs',
                    'backend/services/concert-service/tests/support/event_lifecycle/mod.rs',
                    'backend/services/concert-service/tests/event_lifecycle_support/migrations.rs']]
                source_pin=hashlib.sha256()
                for source in sources:
                    source_pin.update(str(source.relative_to(ROOT)).encode()+b'\0'+source.read_bytes()+b'\0')
                pin=source_pin.hexdigest()
                env=dict(os.environ,MCR_BE41_DATABASE_URL=f'postgresql:///mcr_be41_fixture?host={sock}',MCR_BE41_FIXTURE_OWNER=owner)
                sql("CREATE TABLE be41_fixture_owner(owner_token text PRIMARY KEY);INSERT INTO be41_fixture_owner VALUES('"+owner+"');")
                found=set();native=[]
                for path in args.native_binary:
                    binary=path.resolve(strict=True)
                    listing=run([str(binary),'--list']).stdout
                    actual={line.split(': test')[0] for line in listing.splitlines() if line.endswith(': test')}
                    if not actual or not actual<=expected or found&actual:raise AssertionError('Native test suite missing, unexpected or duplicated')
                    found|=actual
                    restart={'event_lifecycle_restart_seed','event_lifecycle_restart_resume'}
                    normal=actual-restart
                    result=run([str(binary),'--ignored','--nocapture','--test-threads=1','--skip','event_lifecycle_restart_seed','--skip','event_lifecycle_restart_resume'],env=env)
                    if not re.search(rf'test result: ok\. {len(normal)} passed; 0 failed;',result.stdout):raise AssertionError('Native tests did not execute successfully')
                    if actual&restart:
                        if not restart<=actual:raise AssertionError('Restart test phases must share one binary')
                        before=run([str(binary),'--ignored','--nocapture','--exact','event_lifecycle_restart_seed'],env=env)
                        if 'test result: ok. 1 passed; 0 failed;' not in before.stdout:raise AssertionError('Native restart seed did not execute')
                        stop();start()
                        after=run([str(binary),'--ignored','--nocapture','--exact','event_lifecycle_restart_resume'],env=env)
                        if 'test result: ok. 1 passed; 0 failed;' not in after.stdout:raise AssertionError('Native restart recovery did not execute')
                        result.stdout+=before.stdout+after.stdout
                    pins=set(re.findall(r'BE41_SOURCE_PIN=([a-f0-9]{64})',result.stdout))
                    if pins!={pin}:raise AssertionError('Native binary compiled owned source does not match current source')
                    native.append({'exit_code':0,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'owned_source_pin':pin,'output':result.stdout})
                if found!=expected:raise AssertionError('Incomplete native test suite')
                evidence['native_adapter_suite']=native
            evidence['result']='PASS'
        finally:
            stop()
        payload=json.dumps(evidence,indent=2)
        if args.output:
            args.output.write_text(payload+'\n')
        print(payload)
if __name__=='__main__': main()
