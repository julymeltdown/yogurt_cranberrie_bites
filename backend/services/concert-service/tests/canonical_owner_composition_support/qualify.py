#!/usr/bin/env python3
"""Run only frozen CF01 native cases against newly owned PG16/mTLS stores."""
from __future__ import annotations
import argparse, hashlib, json, os, shutil, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(ROOT/'backend/scripts/mcr'))
import migration_smoke as core
from invalidation_clients_smoke import pki
OWNERS=('auth','friend','concert','profile','notification')
CASES=('actual_factory_retains_profile_and_local_children_until_outer_body_eof',
       'fresh_factory_recovers_original_notification_binding_without_target_policy')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def sources():
    paths=subprocess.check_output(['git','ls-files','-z','backend','contracts','frontend/public'],cwd=ROOT).decode().split('\0')
    return {s:sha(ROOT/s) for s in sorted(paths) if s and (ROOT/s).is_file()}
def verify(checkpoint):
    receipt=json.loads(checkpoint.read_text())
    if subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip():raise RuntimeError('clean source required')
    if subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()!=receipt['source_commit']:raise RuntimeError('commit drift')
    if sources()!=receipt['source_sha256']:raise RuntimeError('source drift')
    artifact=Path(receipt['artifacts']['canonical_owner_composition']['path'])
    if artifact.is_symlink() or not artifact.is_absolute() or sha(artifact)!=receipt['artifacts']['canonical_owner_composition']['sha256']:raise RuntimeError('artifact drift')
    if receipt['exit_code']!=0:raise RuntimeError('build failed')
    return artifact,receipt
def certificate(cluster,role):
    p=cluster.root/'pki';ext=p/f'{role}.ext'
    ext.write_text(f'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:{role}.svc.internal,URI:spiffe://internal/{role}\n')
    for args in (['req','-new','-newkey','rsa:2048','-nodes','-keyout',f'{role}.key','-out',f'{role}.csr','-subj',f'/CN={role}.svc.internal'],['x509','-req','-in',f'{role}.csr','-CA','ca.crt','-CAkey','ca.key','-CAcreateserial','-out',f'{role}.crt','-days','2','-extfile',str(ext)]):
        core.execute(['/usr/bin/openssl',*args],env=cluster.env,cwd=p,phase='cf01-pki',timeout=30)
    (p/f'{role}.key').chmod(0o600)
def qualify(args,report):
    binary,receipt=verify(args.checkpoint)
    report.update(source_commit=receipt['source_commit'],source_sha256=receipt['source_sha256'],binary_sha256=sha(binary),cases={})
    tools={name:core.executable(args.pg_bin_dir/name) for name in core.PG_TOOLS}
    for case in CASES:
        if shutil.disk_usage(ROOT).free<8*1024**3:raise RuntimeError('8GiB floor')
        cluster=core.Cluster(tools);observed={};report['cases'][case]=observed
        try:
            cluster.init()
            with (cluster.data/'postgresql.conf').open('a') as f:f.write('\nmax_prepared_transactions=256\n')
            cluster.start();pki(cluster)
            for role in ('profile','notification'):certificate(cluster,role)
            version=cluster.sql('postgres','show server_version_num;')
            if int(version)//10000!=16:raise RuntimeError('PG16 required')
            observed['postgres_version']=cluster.sql('postgres','show server_version;')
            env={**cluster.env,'CONCERT_SERVICE_CREDENTIAL':'concert-fixture','NOTIFICATION_SERVICE_CREDENTIAL':'notification-fixture','MCR_CANONICAL_ROOT':str(cluster.root),'MCR_CANONICAL_ROOT_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}','MCR_CANONICAL_ROOT_TOKEN':cluster.token,'MCR_CANONICAL_SYSTEM':cluster.sql('postgres','select system_identifier from pg_control_system();')}
            for owner in OWNERS:
                db='mcr_canonical_'+owner
                core.execute([str(tools['createdb']),'--no-password','--maintenance-db=postgres',db],env=cluster.client_env('postgres'),cwd=cluster.root,phase='create-'+owner)
                env['MCR_CANONICAL_'+owner.upper()+'_URL']=cluster.url(db)
            # Binary exact-case inventory and exit status are both required.
            inventory=subprocess.check_output([str(binary),'--list','--format','terse'],env=env,text=True,cwd=ROOT/'backend')
            if f'{case}: test' not in inventory:raise RuntimeError('exact test missing')
            command=[str(binary),'--ignored','--exact',case,'--test-threads=1','--nocapture']
            start=time.monotonic()
            result=subprocess.run(command,env=env,cwd=ROOT/'backend',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=90)
            # Fixed case diagnostics only; no SQL/transport capability output.
            observed.update(exit_code=result.returncode,elapsed_seconds=round(time.monotonic()-start,3),stdout=result.stdout,stderr=result.stderr)
            if result.returncode or '1 passed; 0 failed' not in result.stdout:raise RuntimeError('actual native case failed')
            observed['migration_receipts']={o:core.check_ledger(cluster,'mcr_canonical_'+o,core.migration_manifest(ROOT/'backend',o+'-service')) for o in OWNERS}
            observed['remaining_prepared']={o:int(cluster.sql('mcr_canonical_'+o,'select count(*) from pg_prepared_xacts where database=current_database();')) for o in OWNERS}
            if any(observed['remaining_prepared'].values()):raise RuntimeError('unsettled prepared work')
            observed['status']='PASS'
        finally:
            cluster.cleanup();observed['cleanup']={'owned_root_absent':not cluster.root.exists(),'kind':'OWNED_CLUSTER_TEARDOWN','certifies_native_settlement':False}
    verify(args.checkpoint)
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--checkpoint',type=Path,required=True);p.add_argument('--pg-bin-dir',type=Path,required=True);p.add_argument('--report',type=Path,required=True);a=p.parse_args()
    if not a.report.is_absolute() or a.report.exists() or not a.pg_bin_dir.is_absolute():p.error('new absolute report and explicit PG path required')
    report={'status':'FAIL','limitations':['Test-owned guarded RPC services share the test PID; public Gateway HTTP and physical process death are not claimed.','Gift and Social actual adapters are composed but their business flows are NOT_RUN here.','No Notification root target provider is supplied: inbox execution refuses; only actual durable close-before-dispatch cleanup is positive. Full public target/inbox acceptance NOT_RUN.','Main mounting, complete native resolver union and child worker supervision remain integrator obligations.']}
    try:qualify(a,report);report['status']='PASS'
    except Exception as e:report['failure']=type(e).__name__+': '+str(e)
    finally:
        with os.fdopen(os.open(a.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),'w') as f:json.dump(report,f,indent=2,sort_keys=True);f.write('\n')
    print(json.dumps({'status':report['status'],'report':str(a.report)}));sys.exit(0 if report['status']=='PASS' else 1)
