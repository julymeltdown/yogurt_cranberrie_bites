#!/usr/bin/env python3
"""Run frozen Auth/Friend owner stores against private socket-only PG16."""
from __future__ import annotations
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0,str(ROOT / 'backend/scripts/mcr'))
from migration_smoke import Cluster, PG_TOOLS, SmokeError, executable, execute, migration_manifest, check_ledger
from auth_invalidation_smoke import frozen_binary
from invalidation_clients_smoke import pki

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def source_manifest():
    paths={ROOT/'backend/Cargo.toml',ROOT/'backend/Cargo.lock',Path(__file__).resolve()}
    for prefix in ['backend/crates','backend/proto','backend/services/auth-service','backend/services/friend-service','backend/services/api-gateway','backend/services/profile-service']:
        paths.update(p for p in (ROOT/prefix).rglob('*') if p.is_file() and p.suffix in {'.rs','.sql','.toml','.proto','.py'})
    for name in ['migration_smoke.py','auth_invalidation_smoke.py','invalidation_clients_smoke.py']:
        paths.add(ROOT/'backend/scripts/mcr'/name)
    return {str(p.relative_to(ROOT)):sha(p) for p in sorted(paths)}
def qualify(metadata, pg, report):
    current=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    dirty=subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip()
    if dirty or metadata['source_commit']!=current or metadata['source_sha256']!=source_manifest(): raise SmokeError('preflight','source checkpoint changed')
    if set(metadata['artifacts'])!={'auth','friend'}: raise SmokeError('preflight','exact two owner artifacts required')
    artifacts={k:frozen_binary(Path(v['path']),v['sha256']) for k,v in metadata['artifacts'].items()}
    extras=metadata.get('extra_artifacts',{})
    if not {'auth-server','friend-server','auth-unit','friend-unit'}<=set(extras):raise SmokeError('preflight','frozen production and unit binaries required')
    for data in extras.values():frozen_binary(Path(data['path']),data['sha256'])
    if shutil.disk_usage(ROOT).free<8*1024**3: raise SmokeError('preflight','8 GiB free required')
    cluster=Cluster({n:executable(pg/n) for n in PG_TOOLS})
    report.update(source_commit=current,source_sha256=metadata['source_sha256'],artifacts_sha256={k:sha(p) for k,p in artifacts.items()},owned_root=str(cluster.root),phases={})
    try:
        cluster.init();cluster.assert_owned()
        with (cluster.data/'postgresql.conf').open('a') as out:out.write('\nmax_prepared_transactions=64\n')
        cluster.start();pki(cluster)
        # Reuse the audited private fixture PKI creation, then issue Post for the
        # workload-authorization test using that same short-lived authority.
        cert=cluster.root/'pki'
        for role in ['post','profile']:
            ext=cert/f'{role}.ext';ext.write_text(f'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:{role}.svc.internal,URI:spiffe://internal/{role}\n')
            for command in [['req','-new','-newkey','rsa:2048','-nodes','-keyout',f'{role}.key','-out',f'{role}.csr','-subj',f'/CN={role}.svc.internal'],['x509','-req','-in',f'{role}.csr','-CA','ca.crt','-CAkey','ca.key','-CAcreateserial','-out',f'{role}.crt','-days','2','-extfile',f'{role}.ext']]:execute(['/usr/bin/openssl',*command],env=cluster.env,phase=f'pki-{role}',cwd=cert)
            (cert/f'{role}.key').chmod(0o600)
        version=cluster.sql('postgres','show server_version_num;')
        if int(version)//10000!=16:raise SmokeError('preflight','PG16 required')
        report['postgres_version']=cluster.sql('postgres','show server_version;')
        system=cluster.sql('postgres','select system_identifier from pg_control_system();')
        envs={}
        for owner in artifacts:
            for db in [f'mcr_owner_{owner}',f'mcr_owner_{owner}_producer']:execute([str(cluster.tools['createdb']),'--no-password','--maintenance-db=postgres',db],env=cluster.client_env('postgres'),phase='createdb',cwd=cluster.root)
            envs[owner]={**cluster.env,'MCR_AUTHORITY_ROOT':str(cluster.root),'MCR_AUTHORITY_SYSTEM':system,'MCR_AUTHORITY_OWNER_URL':cluster.url(f'mcr_owner_{owner}'),'MCR_AUTHORITY_PRODUCER_URL':cluster.url(f'mcr_owner_{owner}_producer')}
        def test(owner,name,pure=False):
            args=[str(artifacts[owner]),'--test-threads=1','--nocapture']
            if not pure:args+=['--ignored','--exact',name]
            result=execute(args,env=envs[owner],phase=f'{owner}:{name}',cwd=ROOT/'backend',timeout=60)
            if 'test result: ok. 1 passed; 0 failed;' not in result.stdout:raise SmokeError('inventory','expected one exact passing test')
            report['phases'][f'{owner}:{name}']={'status':'PASS','exit_code':result.returncode,'stdout':result.stdout}
        for owner in artifacts:
            test(owner,'pure',True)
            for name in ['transport::owner_rpc_requires_actual_workload_and_signed_acquisition','native::actual_transactions_enrollment_sealing_and_quarantine','native::cancellation_and_capacity_survive_expiry']:
                test(owner,name)
            if owner=='auth':test(owner,'native::actual_security_generation_distinguishes_rotation_and_invalidation')
            test(owner,'native::prepare_restart')
            check_ledger(cluster,f'mcr_owner_{owner}',migration_manifest(ROOT/'backend',f'{owner}-service'))
        before=cluster.assert_postmaster();cluster.stop();cluster.start();after=cluster.assert_postmaster()
        if before==after:raise SmokeError('restart','postmaster did not change')
        report['restart']={'before_pid':before,'after_pid':after,'status':'PASS'}
        for owner in artifacts:
            test(owner,'native::recover_restart')
            check_ledger(cluster,f'mcr_owner_{owner}',migration_manifest(ROOT/'backend',f'{owner}-service'))
            if cluster.sql(f'mcr_owner_{owner}',"select count(*) from pg_prepared_xacts where database=current_database();")!='0':raise SmokeError('cleanup','prepared locks remain')
    finally:
        cluster.cleanup();report['cleanup']={'status':'PASS','owned_root_absent':not cluster.root.exists()}
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--artifacts',type=Path,required=True);parser.add_argument('--pg-bin-dir',type=Path,required=True);parser.add_argument('--report',type=Path,required=True);args=parser.parse_args()
    if not args.report.is_absolute() or args.report.exists() or not args.pg_bin_dir.is_absolute():parser.error('new absolute report and PG directory required')
    report={'status':'FAIL','limitations':['Owner-store native qualification only; combined Concert/Post/Profile production flow belongs to authority owner207.','Fixture producers use actual PG transactions/receipts and configured mTLS; this is not release acceptance.']}
    try:qualify(json.loads(args.artifacts.read_text()),args.pg_bin_dir,report);report['status']='PASS'
    except SmokeError as error:report['failure']=error.detail
    except Exception as error:report['failure']={'phase':'runner','diagnostic':type(error).__name__,'detail':str(error)}
    finally:args.report.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
    print(json.dumps({'status':report['status'],'report':str(args.report)}));return 0 if report['status']=='PASS' else 1
if __name__=='__main__':sys.exit(main())
