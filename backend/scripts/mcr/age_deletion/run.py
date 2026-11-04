#!/usr/bin/env python3
"""Run the frozen Age owner against newly owned Auth/Concert PG stores and mTLS."""
import argparse
import json
import os
import subprocess
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[4]
sys.path.insert(0,str(ROOT/'backend/scripts/mcr'))
from migration_smoke import Cluster, PG_TOOLS, executable, execute, check_ledger, migration_manifest
from invalidation_clients_smoke import pki
from freeze import sha, snapshot

def main():
    p=argparse.ArgumentParser();p.add_argument('--frozen',type=Path,required=True);a=p.parse_args()
    frozen=a.frozen.resolve();receipt=json.loads((frozen/'freeze.json').read_text())
    assert receipt['source']==snapshot(),'source drift after target release'
    assert receipt['migrations']=={p.name:sha(p) for p in (frozen/'migrations').glob('*.sql')}
    binary=frozen/'bin/age_deletion_native';assert sha(binary)==receipt['binaries']['age_deletion_native']['sha256']
    pg=Path(os.environ['MCR_AGE_DELETION_PG_BIN']);tools={n:executable(pg/n) for n in PG_TOOLS}
    cluster=Cluster(tools);logs=frozen/'native';logs.mkdir(mode=0o700)
    s=cluster.root.stat()
    (logs/'cluster.json').write_text(json.dumps({'root':str(cluster.root),'identity':[s.st_dev,s.st_ino,s.st_uid],'token':cluster.token})+'\n')
    record={'status':'FAIL','cleanup':False,'checks':[],'source':receipt['source'],'binaries':receipt['binaries']}
    def run(name,argv,env):
        r=subprocess.run([str(x) for x in argv],env=env,cwd=ROOT/'backend',timeout=90,capture_output=True,text=True)
        (logs/(name+'.stdout')).write_text(r.stdout);(logs/(name+'.stderr')).write_text(r.stderr)
        record['checks'].append({'name':name,'exit_code':r.returncode})
        assert r.returncode==0,f'{name} failed; preserved native logs'
        return r.stdout
    try:
        cluster.init();cluster.start();pki(cluster)
        record['postgres_version']=cluster.sql('postgres','show server_version;')
        record['postmaster_before']=cluster.assert_postmaster()
        env={**cluster.env,'MCR_CANONICAL_ROOT':str(cluster.root),
             'MCR_CANONICAL_ROOT_IDENTITY':f'{s.st_dev}:{s.st_ino}',
             'MCR_CANONICAL_ROOT_TOKEN':cluster.token,
             'MCR_CANONICAL_SYSTEM':cluster.sql('postgres','select system_identifier from pg_control_system();')}
        for owner in ('auth','concert'):
            name=f'mcr_canonical_{owner}'
            run('createdb-'+owner,[tools['createdb'],'--no-password','--maintenance-db=postgres',name],cluster.client_env('postgres'))
            env[f'MCR_CANONICAL_{owner.upper()}_URL']=cluster.url(name)
        for phase in ('FIRST','RESTARTED'):
            if phase=='RESTARTED':
                cluster.stop();cluster.start();record['postmaster_after']=cluster.assert_postmaster()
                assert record['postmaster_before']!=record['postmaster_after'],'physical restart absent'
            out=run(phase,[binary,'--ignored','--exact','age_deletion_populated_auth_retry_cancel_restart','--test-threads=1','--nocapture'],
                    {**env,'MCR_AGE_DELETION_PHASE':phase})
            assert 'test result: ok. 1 passed; 0 failed;' in out,'exact test did not run'
        record['migrations']={}
        for owner in ('auth','concert'):
            record['migrations'][owner]=check_ledger(cluster,f'mcr_canonical_{owner}',migration_manifest(ROOT/'backend',f'{owner}-service'))
        assert cluster.sql('postgres','select count(*) from pg_prepared_xacts;')=='0'
        record['status']='PASS'
    finally:
        try:cluster.cleanup();record['cleanup']=not cluster.root.exists()
        finally:(frozen/'native.json').write_text(json.dumps(record,indent=2)+'\n')
if __name__=='__main__':main()
