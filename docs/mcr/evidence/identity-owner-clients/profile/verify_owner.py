import hashlib,json,os,subprocess,sys,time
from pathlib import Path
ROOT=Path('/Users/lhs/Desktop/mcr-worktrees/be-profile-owner')
OUT=Path('/Users/lhs/Desktop/mcr-build/profile-identity-owner')
sys.path.insert(0,str(ROOT/'backend/scripts/mcr'))
import migration_smoke as m
pg=Path('/opt/homebrew/opt/postgresql@16/bin')
tools={name:pg/name for name in m.PG_TOOLS}
report={'base':'3fdb977d6a64798d76ea137099026879a68a67f9','commands':{},'result':'IN_PROGRESS','mode':'owned-native-PG16; Unix socket only; no Docker/default service'}
cluster=m.Cluster(tools)
db='mcr_profile_identity_owner'
def save():
 (OUT/'native.json').write_text(json.dumps(report,indent=2)+'\n')
def run(label,args,env):
 cluster.assert_owned()
 with (OUT/(label+'.log')).open('w') as log:
  p=subprocess.run(args,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=60)
 text=(OUT/(label+'.log')).read_text()
 assert cluster.password not in text and cluster.url(db) not in text,'credential redaction failed'
 for private in ['private-jwt-canary','private-deletion-reason-canary','private-handle-','raw-email@example.test']:
  assert private not in text,'private fixture diagnostic leak'
 report['commands'][label]={'args':[str(a) for a in args],'exit_code':p.returncode,'log':str(OUT/(label+'.log'))};save()
 assert p.returncode==0,label
try:
 cluster.init();cluster.start()
 report['postgres_version']=cluster.sql('postgres','show server_version;');report['owned_root']=str(cluster.root)
 cluster.sql('postgres','create database mcr_profile_identity_owner template template0;')
 migrations=sorted((ROOT/'backend/services/profile-service/migrations').glob('*.sql'))
 assert len(migrations)==7
 report['migration_sha256']={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in migrations}
 for p in migrations:
  cluster.sql(db,p.read_text());report['commands']['migration:'+p.name]={'exit_code':0}
 assert cluster.sql(db,"select to_regclass('profile_user_deletion_tombstones') is not null;")=='t'
 cluster.sql(db,"create role mcr_profile_identity_reader nologin; create role mcr_profile_identity_noaccess nologin; grant select(user_id,nickname,bio,deleted_at) on profiles to mcr_profile_identity_reader; grant select(user_id) on profile_user_deletion_tombstones to mcr_profile_identity_reader;")
 report['fixture_ready']=True;save();print('Owned PG16 fixture ready; seven retained migrations applied',flush=True)
 binary_file=OUT/'owner-binary.path';until=time.monotonic()+900
 while not binary_file.exists():
  if time.monotonic()>until:raise RuntimeError('compiled test binary not supplied')
  time.sleep(.2)
 binary=Path(binary_file.read_text().strip()).resolve()
 assert str(binary).startswith('/Users/lhs/Desktop/mcr-build/backend/debug/deps/concert_identity_owner-') and binary.is_file()
 report['binary_sha256']=hashlib.sha256(binary.read_bytes()).hexdigest()
 env={**cluster.env,'MCR_PROFILE_OWNER_TEST_DATABASE_URL':cluster.url(db),'MCR_PROFILE_OWNER_TEST_ROOT':str(cluster.root),'MCR_PROFILE_OWNER_TEST_TOKEN':cluster.token,'MCR_PROFILE_OWNER_TEST_DEV':str(cluster.identity[0]),'MCR_PROFILE_OWNER_TEST_INO':str(cluster.identity[1])}
 run('owner-pg',[str(binary),'--skip','owner_database_unavailable_is_not_cached_content','--test-threads=1'],env)
 cluster.stop();report['stopped_server_outage']=True
 run('owner-outage',[str(binary),'--exact','owner_database_unavailable_is_not_cached_content','--test-threads=1'],env)
 cluster.start()
 run('owner-recovered',[str(binary),'--exact','active_private_self_is_minimal_and_updates_are_uncached','--test-threads=1'],env)
 for p,digest in report['migration_sha256'].items():assert hashlib.sha256((ROOT/p).read_bytes()).hexdigest()==digest
 report['result']='PASS'
except BaseException as exc:
 report['result']='FAIL';report['failure_type']=type(exc).__name__
 print('Owned Profile verification failed: '+type(exc).__name__,flush=True)
 raise
finally:
 owned=str(cluster.root)
 cluster.cleanup()
 report['cleanup']={'owned_root_absent':not Path(owned).exists(),'stopped_owned_server':True}
 save();print('Owned PG16 cleanup complete; result '+report['result'],flush=True)
