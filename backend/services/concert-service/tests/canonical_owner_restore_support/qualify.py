#!/usr/bin/env python3
"""Frozen BE15/QA09 canonical physical-backup/restore qualification, owned PG16 only."""
from __future__ import annotations
import argparse,hashlib,json,os,shutil,signal,subprocess,sys,time
from pathlib import Path
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(ROOT/'backend/scripts/mcr'))
import migration_smoke as core
from invalidation_clients_smoke import pki
from controller import run as restore_run
OWNERS=('auth','friend','concert','profile','notification')
CASES=('FINISHED','ACTIVE','MISSING_WITNESS')
TARGET='canonical_owner_restore'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def sources():
    files=subprocess.check_output(['git','ls-files','-z','backend','contracts','frontend/public','frontend/src/entities/avatar/character-source-pack.json','.cargo','rust-toolchain.toml','rust-toolchain','Cargo.toml','Cargo.lock'],cwd=ROOT).decode().split('\0')
    result={}
    for relative in sorted(files):
        if not relative:continue
        p=ROOT/relative
        if not p.is_file():continue
        if p.is_symlink():raise RuntimeError('source symlink refused')
        result[relative]=sha(p)
    return result
def verify(checkpoint):
    c=json.loads(checkpoint.read_text());require=lambda ok,msg:(_ for _ in()).throw(RuntimeError(msg))if not ok else None
    require(not subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=ROOT,text=True).strip(),'clean committed fixture required')
    require(subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==c['source_commit'],'commit drift')
    require(c['exit_code']==0 and c['source_sha256']==sources(),'source/build drift')
    require(c['target_name']==TARGET,'exact single restore target required')
    command=c['argv'];require(command.count('--test')==1 and command[command.index('--test')+1]==TARGET and all(x in command for x in('--no-run','--locked','--offline')),'exact build scope')
    binary=Path(c['binary']);require(binary.is_absolute()and binary.is_file()and not binary.is_symlink()and sha(binary)==c['binary_sha256'],'frozen executable drift')
    require(not any(p in binary.parts for p in('target','targets')),'outside-target artifact required')
    return binary,c
def certificate(cluster,role):
    p=cluster.root/'pki';ext=p/f'{role}.ext';ext.write_text(f'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:{role}.svc.internal,URI:spiffe://internal/{role}\n')
    for args in (['req','-new','-newkey','rsa:2048','-nodes','-keyout',f'{role}.key','-out',f'{role}.csr','-subj',f'/CN={role}.svc.internal'],['x509','-req','-in',f'{role}.csr','-CA','ca.crt','-CAkey','ca.key','-CAcreateserial','-out',f'{role}.crt','-days','2','-extfile',str(ext)]):core.execute(['/usr/bin/openssl',*args],env=cluster.env,cwd=p,phase='restore-pki',timeout=30)
    (p/f'{role}.key').chmod(0o600)
def migration_receipts(cluster):return{o:core.check_ledger(cluster,'mcr_canonical_'+o,core.migration_manifest(ROOT/'backend',o+'-service'))for o in OWNERS}
def run(args,report):
    binary,checkpoint=verify(args.checkpoint);tools={n:core.executable(args.pg_bin_dir/n)for n in core.PG_TOOLS}
    report.update(source_commit=checkpoint['source_commit'],source_sha256=checkpoint['source_sha256'],binary_sha256=sha(binary),cases={})
    inventory=subprocess.check_output([str(binary),'--list','--format','terse'],cwd=ROOT/'backend',text=True,env={'PATH':'/usr/bin:/bin','LC_ALL':'C'})
    if 'canonical_owner_restore_process: test'not in inventory:raise RuntimeError('exact native process host absent')
    for case in CASES:
        if shutil.disk_usage(ROOT).free<8*1024**3:raise RuntimeError('8GiB stop floor')
        source=core.Cluster(tools);restored=core.Cluster(tools);item={};report['cases'][case]=item;start=time.monotonic()
        try:
            source.init()
            with(source.data/'postgresql.conf').open('a')as f:f.write('\nmax_prepared_transactions=256\n')
            source.start()
            version=source.sql('postgres','show server_version_num;')
            if int(version)//10000!=16:raise RuntimeError('PG16 required')
            item['postgres_version']=source.sql('postgres','show server_version;');pki(source)
            for role in('profile','notification'):certificate(source,role)
            for owner in OWNERS:core.execute([str(tools['createdb']),'--no-password','--maintenance-db=postgres','mcr_canonical_'+owner],env=source.client_env('postgres'),cwd=source.root,phase='restore-create-'+owner)
            restore_run(binary,source,restored,case,item)
            item['migration_receipts']=migration_receipts(restored)
            # Actual SQL observation BEFORE teardown: retained negatives are
            # quarantine, never forced rollback or an accepted clean settlement.
            item['prepared_before_teardown']={o:int(restored.sql('mcr_canonical_'+o,'select count(*) from pg_prepared_xacts where database=current_database();'))for o in OWNERS}
            expected={'auth':0 if case=='FINISHED'else 1,'friend':0 if case=='FINISHED'else 1,'concert':1 if case=='ACTIVE'else 0,'profile':1 if case=='ACTIVE'else 0,'notification':0}
            if item['prepared_before_teardown']!=expected:raise RuntimeError('exact supported settlement/quarantine mismatch')
            item['normal_all_owner_native_settlement']=case=='FINISHED';item['elapsed_seconds']=round(time.monotonic()-start,3)
        finally:
            diagnostics=[]
            for cluster in(source,restored):
                # Bounded fixed diagnostics only. Private handoff and GIDs never
                # become portable evidence. Raw logs live only in owned roots.
                for log in cluster.root.glob('restore-*.log'):
                    for line in log.read_text(errors='replace').splitlines():
                        if line.startswith(('Error:','thread ')):diagnostics.append(line[:300])
            if diagnostics:item['fixed_diagnostics']=diagnostics[:6]
            restored.cleanup();source.cleanup()
            item['cleanup']={'owned_source_root_absent':not source.root.exists(),'owned_restored_root_absent':not restored.root.exists(),'kind':'EXACT_OWNED_CLUSTER_TEARDOWN','certifies_native_settlement':False}
    verify(args.checkpoint)
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--checkpoint',type=Path,required=True);p.add_argument('--pg-bin-dir',type=Path,required=True);p.add_argument('--report',type=Path,required=True);a=p.parse_args()
    if not a.report.is_absolute()or a.report.exists()or not a.pg_bin_dir.is_absolute():p.error('new absolute report and explicit PG16 directory required')
    def terminate(_signal,_frame):raise KeyboardInterrupt('owned restore qualifier interrupted')
    signal.signal(signal.SIGTERM,terminate)
    report={'status':'FAIL','scope':'BE15/QA09 supporting canonical-owner physical backup/restore segment','limitations':['No overall BE15/QA09 acceptance: actual domain outbox downstream deduplication, committed pickup preservation, deletion and partial event closure are separate scenarios.','No external Gateway HTTP, sustained World/browser or production composition activation.','Auth/Friend/Profile/Notification servers share each owned Root test process; independent owner process deployment is not claimed.','Finished-root child recovery cannot recreate lost registry authority or DeliveryCompleted.','Actual committed native SQL without producer-specific commit witness remains UNKNOWN with owner fences retained. A test-owned effect counter is not a business receipt.','Only an offline stopped WHOLE cluster copy, operator-retained keys/instance/capabilities and exact PG16 schema vector are qualified. No pg_dump, live snapshot or split-service independent recovery is claimed.']}
    try:run(a,report);report['status']='PASS'
    except BaseException as e:report['failure']=type(e).__name__+': '+str(e)
    finally:
        with os.fdopen(os.open(a.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),'w')as f:json.dump(report,f,indent=2,sort_keys=True);f.write('\n')
    print(json.dumps({'status':report['status'],'report':str(a.report)}));return 0 if report['status']=='PASS'else 1
if __name__=='__main__':sys.exit(main())
