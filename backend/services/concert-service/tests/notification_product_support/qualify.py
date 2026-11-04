#!/usr/bin/env python3
"""Frozen BE33 component checks; no permissive target provider or ambient PG."""
from __future__ import annotations
import argparse, hashlib, json, shutil, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(ROOT/'backend/scripts/mcr'))
from migration_smoke import Cluster,PG_TOOLS,SmokeError,check_ledger,executable,execute,migration_manifest
from auth_invalidation_smoke import frozen_binary
from invalidation_clients_smoke import pki
CASES=("support::native::missing_target_policy_denies_all_four_under_real_owners_and_terminal_body",
       "support::native::sealed_parent_mutations_and_missing_task_local_never_register_child",
       "support::native::dropped_body_and_expiry_do_not_finish_registered_child")
OWNERS=('auth','friend','concert','notification')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def sources():
    paths=subprocess.check_output(['git','ls-files','backend','contracts'],cwd=ROOT,text=True).splitlines()
    return {p:sha(ROOT/p) for p in paths}
def notification_pki(c):
    pki(c);d=c.root/'pki';tool=executable(Path('/usr/bin/openssl'))
    ext=d/'notification.ext';ext.write_text('basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:notification.svc.internal,URI:spiffe://internal/notification\n')
    for args in [('req','-new','-newkey','rsa:2048','-nodes','-keyout','notification.key','-out','notification.csr','-subj','/CN=notification.svc.internal'),('x509','-req','-in','notification.csr','-CA','ca.crt','-CAkey','ca.key','-CAcreateserial','-out','notification.crt','-days','2','-extfile',str(ext))]:
        execute([str(tool),*args],env=c.env,cwd=d,phase='notification-pki',timeout=30)
    (d/'notification.key').chmod(0o600)
def exact(binary,env,case,ignored=False):
    args=[str(binary),'--exact',case,'--test-threads=1','--nocapture']
    if ignored:args.append('--ignored')
    result=execute(args,env=env,phase=case,cwd=ROOT/'backend',timeout=60)
    if 'test result: ok. 1 passed; 0 failed;' not in result.stdout:raise SmokeError('inventory',f'exact case did not pass: {case}')
    return {'status':'PASS','exit_code':result.returncode,'stdout':result.stdout}
def qualify(receipt,pg,report):
    pin=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    if receipt['source_commit']!=pin or receipt['source_sha256']!=sources() or subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip():raise SmokeError('preflight','clean complete source pin required')
    artifacts={k:frozen_binary(Path(v['private_artifact']),v['artifact_sha256']) for k,v in receipt['frozen_artifacts'].items()}
    if set(artifacts)!={'notification_product','concert_service'}:raise SmokeError('inventory','two exact Cargo emitted test artifacts required')
    report.update(source_commit=pin,source_sha256=receipt['source_sha256'],artifacts=receipt['frozen_artifacts'],phases={},native=[])
    for name in ('all_four_frozen_routes_and_query_bounds','duplicate_encoded_unknown_and_malformed_query_are_closed','closed_mutation_body_revision_type_and_idempotency','event_actor_family_request_path_and_method_cannot_be_relabelled'):
        report['phases'][name]=exact(artifacts['notification_product'],{},name)
    for name in ('all_four_outputs_validate_against_frozen_c04_without_private_envelopes','foreign_malformed_unknown_kind_target_revision_and_times_are_refused','output_operation_page_filter_count_and_mutation_identity_are_bound','mutation_conflicts_remain_409_and_diagnostics_never_escape'):
        case='notification_live::product::tests::'+name;report['phases'][case]=exact(artifacts['concert_service'],{},case)
    tools={n:executable(pg/n) for n in PG_TOOLS}
    for case in CASES:
        if shutil.disk_usage(ROOT).free<8*1024**3:raise SmokeError('preflight','8 GiB free required')
        c=Cluster(tools);entry={'case':case,'owned_root':str(c.root)};report['native'].append(entry)
        try:
            c.init()
            with (c.data/'postgresql.conf').open('a') as f:f.write('\nmax_prepared_transactions=256\n')
            c.start();notification_pki(c)
            version=c.sql('postgres','show server_version_num;')
            if int(version)//10000!=16:raise SmokeError('preflight','PG16 required')
            entry['postgres_version']=c.sql('postgres','show server_version;')
            env={**c.env,'CONCERT_SERVICE_CREDENTIAL':'concert-fixture','NOTIFICATION_SERVICE_CREDENTIAL':'notification-fixture','MCR_CANONICAL_ROOT':str(c.root),'MCR_CANONICAL_ROOT_IDENTITY':f'{c.identity[0]}:{c.identity[1]}','MCR_CANONICAL_ROOT_TOKEN':c.token,'MCR_CANONICAL_SYSTEM':c.sql('postgres','select system_identifier from pg_control_system();')}
            for owner in OWNERS:
                db=f'mcr_canonical_{owner}';execute([str(tools['createdb']),'--no-password','--maintenance-db=postgres',db],env=c.client_env('postgres'),phase='create-'+owner,cwd=c.root)
                env[f'MCR_CANONICAL_{owner.upper()}_URL']=c.url(db)
            entry.update(exact(artifacts['notification_product'],env,case,True));entry['migrations']={};entry['prepared_after']={}
            for owner in OWNERS:
                db=f'mcr_canonical_{owner}';entry['migrations'][owner]=check_ledger(c,db,migration_manifest(ROOT/'backend',owner+'-service'))
                count=int(c.sql(db,'select count(*) from pg_prepared_xacts where database=current_database();'));entry['prepared_after'][owner]=count
                expected=1 if case==CASES[-1] and owner in ('auth','friend') else 0
                if count!=expected:raise SmokeError('cleanup','unexpected prepared transaction inventory')
            entry['expected_quarantine']=case==CASES[-1]
        finally:
            c.cleanup();entry['cleanup']={'status':'PASS','owned_root_absent':not c.root.exists(),'quarantine_teardown_is_not_settlement':case==CASES[-1]}
    if sources()!=receipt['source_sha256']:raise SmokeError('postflight','source drift')
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--checkpoint',required=True,type=Path);p.add_argument('--pg-bin-dir',required=True,type=Path);p.add_argument('--report',required=True,type=Path);a=p.parse_args()
    if not a.report.is_absolute() or a.report.exists() or not a.pg_bin_dir.is_absolute():p.error('new absolute report and explicit absolute PG directory required')
    r={'status':'FAIL','limitations':['Component refusal/child mechanics only; positive real target policy and four success flows NOT_RUN.','Root ProductRpcService routing, actual ScopedProductOwner no-store, Gateway outer HTTP and target invalidation/restart are separate NOT_RUN gates.']}
    try:qualify(json.loads(a.checkpoint.read_text()),a.pg_bin_dir,r);r['status']='PASS'
    except SmokeError as e:r['failure']=e.detail
    except Exception as e:r['failure']={'phase':'runner','diagnostic':type(e).__name__,'detail':str(e)}
    finally:a.report.write_text(json.dumps(r,indent=2,sort_keys=True)+'\n')
    print(json.dumps({'status':r['status'],'report':str(a.report)}));return 0 if r['status']=='PASS' else 1
if __name__=='__main__':sys.exit(main())
