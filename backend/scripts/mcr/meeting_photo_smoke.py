#!/usr/bin/env python3
"""Run photo producers only in a fresh marker/inode-owned socket PostgreSQL."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import signal
import subprocess
import migration_smoke as migration
ROOT=Path(__file__).resolve().parents[3]
MIGRATIONS=['0001_concert_core.sql','0002_command_receipts_outbox.sql','0003_places_map_releases.sql','0004_bootstrap_admission.sql','0006_relationship_policy.sql','0007_community.sql','0008_meeting_beacons.sql','0009_receipt_domain_failures.sql','0011_outbox_maintenance.sql','0012_media.sql','0016_meeting_deadline_guards.sql','0029_meeting_photo.sql']
def sql_guards(cluster):
    # Schema-only synthetic rows. The single byte is NOT a decoded image/rights proof.
    event='aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
    actor='bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
    beacon='cccccccc-cccc-4ccc-8ccc-cccccccccccc'
    media='dddddddd-dddd-4ddd-8ddd-dddddddddddd'
    cluster.sql('mcr_meeting', f"insert into concert_events(event_id,slug,title,phase) values('{event}','photo-sql-fixture','Synthetic photo SQL fixture','OPEN'); insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values('{event}','{actor}','REVENGE',false,false,false); insert into concert_meeting_settings(event_id,event_cutoff) values('{event}',clock_timestamp()+interval '1 day');")
    audience=json.dumps(dict(kind='EXPLICIT_USERS',user_ids=[actor]),separators=(',',':'))
    cluster.sql('mcr_meeting', f"insert into concert_meeting_beacons(beacon_id,event_id,owner_id,place_id,map_revision,place_revision,selected_catalog_revision,instructions,identification,audience,expires_at) values('{beacon}','{event}','{actor}','{actor}',1,1,1,'Synthetic schema only','Fixture','{audience}',clock_timestamp()+interval '1 minute');")
    cluster.sql('mcr_meeting', f"insert into concert_media(media_id,event_id,owner_id,purpose,content_type,declared_bytes,input_sha256,policy_event_revision,policy_relationship_revision,policy_authority_revision,state,sanitized,width,height,target_id) values('{media}','{event}','{actor}','BEACON','image/png',1,decode(repeat('00',32),'hex'),1,1,1,'READY',decode('00','hex'),1,1,'{beacon}');")
    base=f"insert into concert_meeting_beacon_photos(event_id,beacon_id,owner_id,media_id,beacon_revision,media_revision,permission_acknowledged,expires_at) select event_id,beacon_id,owner_id,'{media}',revision,1,true,expires_at from concert_meeting_beacons where beacon_id='{beacon}';"
    cluster.sql('mcr_meeting',base)
    def refused(sql):
        try: cluster.sql('mcr_meeting',sql)
        except migration.SmokeError:return
        raise RuntimeError('photo SQL guard accepted invalid binding')
    for assignment in ['beacon_revision=2','media_revision=2','permission_acknowledged=false',"expires_at=expires_at+interval '1 second'"]:
        refused(f"update concert_meeting_beacon_photos set {assignment} where beacon_id='{beacon}';")
    refused(f"begin; update concert_meeting_beacon_photos set permission_confirmed_at=clock_timestamp() where beacon_id='{beacon}'; select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '100 milliseconds')::text,true); select pg_sleep(0.15); commit;")
    return 5

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',action='append',type=Path,default=[]);p.add_argument('--sql-only',action='store_true');p.add_argument('--report',type=Path,required=True);a=p.parse_args()
 if not a.report.is_absolute() or any(not b.is_absolute() for b in a.binary) or (not a.binary and not a.sql_only):p.error('absolute report and binaries or --sql-only required')
 fd=os.open(a.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 r=dict(scope='BE09/FE20 synthetic remote-owner photo adapter qualification',result='FAIL',cleanup='NOT_RUN',tests=[],native='NOT_RUN',canonical_owner_rpc='NOT_RUN',http='NOT_RUN',rights='NOT_ESTABLISHED')
 files=[*sorted((ROOT/'backend/services/concert-service/src/meeting_photo').glob('*.rs')),*sorted((ROOT/'backend/services/concert-service/tests').glob('meeting_photo*.rs')),*sorted((ROOT/'backend/services/concert-service/tests/meeting_photo_support').glob('*'))]
 files += [ROOT/'backend/services/concert-service/src/meeting/commands.rs',ROOT/'backend/services/concert-service/src/adapters/out/media/mod.rs',ROOT/'backend/crates/concert-application/src/media/mod.rs',Path(__file__).resolve()]
 r['source_revision']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip();r['source_sha256']={str(f.relative_to(ROOT)):hashlib.sha256(f.read_bytes()).hexdigest() for f in files if f.is_file()};r['migration_sha256']={}
 cluster=None
 previous=signal.signal(signal.SIGTERM,lambda *_:(_ for _ in ()).throw(KeyboardInterrupt()))
 try:
  binaries=[migration.executable(b) for b in a.binary];cluster=migration.Cluster({name:migration.executable(Path('/opt/homebrew/bin')/name) for name in migration.PG_TOOLS});cluster.init();cluster.start();cluster.sql('postgres','create database mcr_meeting template template0;')
  for name in MIGRATIONS:
   f=ROOT/'backend/services/concert-service/migrations'/name;r['migration_sha256'][name]=hashlib.sha256(f.read_bytes()).hexdigest();cluster.sql('mcr_meeting',f.read_text())
  r['sql_migrations']='PASS';r['sql_guard_cases']=sql_guards(cluster);r['postgres_version']=cluster.sql('mcr_meeting','show server_version;')
  env={**cluster.env,'MEETING_TEST_DATABASE_URL':cluster.url('mcr_meeting'),'MEETING_TEST_OWNER_ROOT':str(cluster.root),'MEETING_TEST_OWNER_TOKEN':cluster.token,'MEETING_TEST_OWNER_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}'}
  def run(b,phase):
   out=subprocess.run([str(b),'--test-threads=1'],cwd=cluster.root,env=env,text=True,capture_output=True,timeout=90);text=out.stdout+out.stderr
   if cluster.password in text or cluster.url('mcr_meeting') in text:raise RuntimeError('private fixture credential in output')
   row=dict(binary=b.name,phase=phase,binary_sha256=hashlib.sha256(b.read_bytes()).hexdigest(),exit_code=out.returncode,output_sha256=hashlib.sha256(text.encode()).hexdigest(),test_result=next((x for x in text.splitlines() if x.startswith('test result:')),None))
   if out.returncode:row['tail']=text[-5000:]
   r['tests'].append(row)
   if out.returncode:raise RuntimeError('native photo test failed')
  for b in binaries:run(b,'before-restart')
  snapshot="select coalesce(md5(string_agg(row_to_json(p)::text,',' order by beacon_id)),'empty') from concert_meeting_beacon_photos p"
  before=cluster.sql('mcr_meeting',snapshot);pid=cluster.assert_postmaster();cluster.stop();cluster.start();assert cluster.assert_postmaster()!=pid and cluster.sql('mcr_meeting',snapshot)==before;r['restart']='PASS';env['MEETING_RESTART_VERIFY']='1'
  for b in binaries:run(b,'after-restart')
  if binaries:r['native']='PASS'
  r['result']='PASS'
 except BaseException as error:r['failure']=getattr(error,'detail',type(error).__name__+': '+str(error))
 finally:
  signal.signal(signal.SIGTERM,previous)
  try:
   if cluster:cluster.cleanup()
   r['cleanup']='PASS'
  except BaseException:r['cleanup']='FAIL';r['result']='FAIL'
  with os.fdopen(fd,'w') as f:json.dump(r,f,indent=2);f.write('\n')
 print(json.dumps({k:r.get(k) for k in ['result','native','restart','cleanup','failure']}));return 0 if r['result']=='PASS' else 1
if __name__=='__main__':raise SystemExit(main())
