#!/usr/bin/env python3
"""BE26 isolated schema + actual producer/race/rollback/restart tests. No provider qualification."""
from pathlib import Path
import argparse,datetime,hashlib,json,os,re,socket,stat,subprocess,sys,signal
ROOT=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(ROOT/'backend/scripts/mcr'))
import migration_smoke as migration
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def require(v,message):
    if not v:raise RuntimeError(message)
def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument('--artifact-receipt',type=Path,required=True);a.add_argument('--pg-bin',type=Path,required=True);a.add_argument('--report',type=Path,required=True);args=a.parse_args()
    require(all(p.is_absolute() for p in [args.artifact_receipt,args.pg_bin,args.report]),'absolute paths required')
    fd=os.open(args.report,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600)
    r={'scope':'BE26 isolated owned persistence and synthetic locked authority/verifier','result':'FAIL','commands':{},'cleanup':'NOT_RUN','restart':'NOT_RUN','real_provider':'NOT_RUN','real_Auth_fence':'NOT_COMPOSED','HTTP':'NOT_RUN','runtime_migration_registration':'NOT_RUN','started_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    cluster=None;reserve=None
    def interrupted(*_): raise KeyboardInterrupt
    handlers={sig:signal.signal(sig,interrupted) for sig in [signal.SIGINT,signal.SIGTERM]}
    try:
        receipt=json.loads(args.artifact_receipt.read_text());require(receipt['result']=='PASS' and receipt['exit_code']==0,'unsuccessful build')
        binary=migration.executable(Path(receipt['private_artifact']));require(sha(binary)==receipt['artifact_sha256'],'binary changed')
        for name,value in receipt['source_sha256'].items():
            p=(ROOT/name).resolve();p.relative_to(ROOT);require(sha(p)==value,'source changed')
        r.update(source_revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),dirty_paths=subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).splitlines(),source_sha256=receipt['source_sha256'],artifact_receipt_sha256=sha(args.artifact_receipt),artifact_sha256=sha(binary),harness_sha256=sha(__file__),migration_helper_sha256=sha(migration.__file__))
        require(r['source_revision']==receipt['source_revision'],'build HEAD differs')
        tools={n:migration.executable(args.pg_bin/n) for n in migration.PG_TOOLS}
        reserve=socket.socket();reserve.bind(('127.0.0.1',0));migration.PORT=reserve.getsockname()[1]
        cluster=migration.Cluster(tools);cluster.init();cluster.start();db='mcr_concert_be_age_claims'
        r['postgres_version']=cluster.sql('postgres','show server_version;');require(r['postgres_version'].startswith('16.'),'PG16 required')
        cluster.sql('postgres',f'create database {db} template template0;')
        r['database']=db;r['owned_root']=str(cluster.root);r['reserved_loopback_port']=migration.PORT;r['tcp_listeners_disabled']=cluster.sql('postgres','show listen_addresses;')==''
        paths=sorted((ROOT/'backend/services/concert-service/migrations').glob('*.sql'))+[ROOT/'backend/services/concert-service/src/adapters/out/age_provider/schema-v1.sql']
        r['schema_sha256']={str(p.relative_to(ROOT)):sha(p) for p in paths}
        for path in paths:
            result=subprocess.run([str(tools['psql']),'-X','--no-password','-v','ON_ERROR_STOP=1','-1','-f',str(path)],env=cluster.client_env(db),cwd=cluster.root,capture_output=True,timeout=30)
            r['commands']['schema-'+path.name]=result.returncode;require(result.returncode==0,'owned schema command failed: '+path.name)
        env={**cluster.env,'CONCERT_TEST_DATABASE_URL':cluster.url(db),'CONCERT_TEST_OWNER_ROOT':str(cluster.root),'CONCERT_TEST_OWNER_TOKEN':cluster.token,'CONCERT_TEST_OWNER_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}','CONCERT_TEST_OWNER_PORT':str(migration.PORT)}
        def tests(label,extra):
            result=subprocess.run([str(binary),*extra],env=env,cwd=cluster.root,capture_output=True,timeout=120)
            text=(result.stdout+result.stderr).decode(errors='replace');require(all(secret not in text for secret in [cluster.password,cluster.token,cluster.url(db)]),'test diagnostic secret leak')
            r['commands'][label]=result.returncode;r[label]=re.sub(r'(?:postgres(?:ql)?|redis)://[^\s]+','[redacted-url]',text)[-64000:]
            summary=re.search(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored',text)
            if summary:r[label+'-summary']=dict(zip(['passed','failed','ignored'],map(int,summary.groups())))
            require(result.returncode==0,'owned tests failed: '+label);require(sha(binary)==r['artifact_sha256'],'binary changed')
        tests('producer-tests',['--test-threads=1'])
        before_pid=cluster.assert_postmaster();before_time=cluster.sql('postgres','select pg_postmaster_start_time();')
        cluster.stop();cluster.start();after_pid=cluster.assert_postmaster();after_time=cluster.sql('postgres','select pg_postmaster_start_time();')
        r['postmaster']={'before_pid':before_pid,'after_pid':after_pid,'before_start':before_time,'after_start':after_time};require(before_pid!=after_pid and before_time!=after_time,'restart identity unchanged')
        tests('restart-tests',['--ignored','--exact','durable_restart_verify','--test-threads=1']);r['restart']='PASS'
        require(all(sha(ROOT/p)==v for p,v in r['source_sha256'].items()),'source drift after native');require(sha(args.artifact_receipt)==r['artifact_receipt_sha256'],'receipt drift')
        r['result']='PASS'
    except BaseException as e:
        r['failure_type']=type(e).__name__
        if type(e) is RuntimeError:r['failure_detail']=str(e)
    finally:
        if cluster:
            try:cluster.cleanup();require(not cluster.root.exists(),'owned root remains');r['cleanup']='PASS'
            except BaseException as e:r.update(result='FAIL',cleanup='FAIL',cleanup_failure=type(e).__name__)
        if reserve:reserve.close()
        for sig,handler in handlers.items(): signal.signal(sig,handler)
        r['finished_at_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat()
        with os.fdopen(fd,'w') as f:json.dump(r,f,indent=2);f.write('\n')
    print(json.dumps({k:r.get(k) for k in ['result','failure_type','failure_detail','restart','cleanup','producer-tests-summary','restart-tests-summary']}))
    return 0 if r['result']=='PASS' else 1
if __name__=='__main__':raise SystemExit(main())
