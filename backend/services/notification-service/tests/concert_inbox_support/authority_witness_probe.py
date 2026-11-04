import sys, json, hashlib, subprocess, selectors, uuid, time, argparse
from pathlib import Path
root=Path(__file__).resolve().parents[5]
sys.path.insert(0, str(root/'backend/scripts/mcr'))
import migration_smoke as m
parser=argparse.ArgumentParser(description='Owned SQL-only Notification witness qualification; no Cargo or owner RPC')
parser.add_argument('--artifact-dir',type=Path,required=True)
parser.add_argument('--pg-bin-dir',type=Path,required=True)
args=parser.parse_args()
if not args.artifact_dir.is_absolute() or not args.pg_bin_dir.is_absolute(): parser.error('absolute explicit paths required')
out=args.artifact_dir; out.mkdir(mode=0o700,exist_ok=False)
tools={n:m.executable(args.pg_bin_dir/n) for n in m.PG_TOOLS}
c=m.Cluster(tools); sessions=[]; report={'scope':'SQL schema/native witness only; owner Track RPC and Rust adapter NOT_RUN','checks':{},'migrations':{}}
def check(label,condition):
    if not condition: raise RuntimeError(label)
    report['checks'][label]='PASS'
def literal(value): return "'"+value.replace("'","''")+"'"
def rejected(sql):
    c.sql('mcr_witness',"DO $$ BEGIN BEGIN "+sql+"; RAISE EXCEPTION 'fixture expected rejection'; EXCEPTION WHEN check_violation THEN NULL; END; END $$;")
def session():
    p=subprocess.Popen([str(tools['psql']),'-X','--no-password','-q','-At','-v','ON_ERROR_STOP=1'],env=c.client_env('mcr_witness'),cwd=c.root,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,bufsize=1)
    sessions.append(p); return p
def query(p,sql):
    marker='BE33_'+uuid.uuid4().hex
    p.stdin.write(sql+'\nSELECT '+literal(marker)+';\n');p.stdin.flush()
    lines=[]; stop=time.monotonic()+10
    # readline is safe after psql has flushed a row; each probe returns one row.
    while time.monotonic()<stop:
        with selectors.DefaultSelector() as selector:
            selector.register(p.stdout,selectors.EVENT_READ)
            if not selector.select(1):
                if p.poll() is not None: raise RuntimeError('owned psql exited')
                continue
        line=p.stdout.readline().rstrip('\n')
        if line==marker: return '\n'.join(lines)
        lines.append(line)
    raise RuntimeError('owned probe timeout')
try:
    c.init();c.start();report['postgres']=c.sql('postgres','show server_version;')
    c.sql('postgres','create database mcr_witness template template0;')
    for p in sorted((root/'backend/services/notification-service/migrations').glob('*.sql')):
        source=p.read_bytes(); (out/p.name).write_bytes(source)
        report['migrations'][p.name]=hashlib.sha256(source).hexdigest()
        c.sql('mcr_witness','begin;\n'+source.decode()+'\ncommit;')
    check('six-migrations-transactional-apply',len(report['migrations'])==6)
    instance=str(uuid.uuid4()); child=str(uuid.uuid4()); child_abort=str(uuid.uuid4()); child_sealed=str(uuid.uuid4())
    c.sql('mcr_witness',f"insert into concert_notification_producer_instance values(true,'{instance}');")
    for id in (child,child_abort,child_sealed):
        c.sql('mcr_witness',f"insert into concert_notification_authority_intents(child_id,binding_proto,binding_digest,request_digest,capability_hash) values('{id}',decode('01','hex'),decode(repeat('01',32),'hex'),decode(repeat('02',32),'hex'),decode(repeat('03',32),'hex'));")
    p=session(); query(p,'BEGIN ISOLATION LEVEL REPEATABLE READ;')
    identity=json.loads(query(p,f"SELECT jsonb_build_object('system_identifier',(select system_identifier::text from pg_control_system()),'database_oid',(select oid from pg_database where datname=current_database()),'full_xid',pg_current_xact_id()::text,'backend_pid',pg_backend_pid(),'backend_started_micros',(select (extract(epoch from backend_start)*1000000)::bigint from pg_stat_activity where pid=pg_backend_pid()),'producer_instance','{instance}','effect_digest',jsonb_build_array(1));"))
    identity_sql=literal(json.dumps(identity))+'::jsonb'
    c.sql('mcr_witness',f"insert into concert_notification_native_references values('{child}',{identity_sql},decode(repeat('04',32),'hex'),null,null);")
    c.sql('mcr_witness',f"update concert_notification_native_references set auth_registration='{uuid.uuid4()}',friend_registration='{uuid.uuid4()}' where child_id='{child}';")
    rejected(f"update concert_notification_native_references set identity_json='{{}}' where child_id='{child}'")
    check('native-metadata-immutable',True)
    query(p,f"DO $$ BEGIN PERFORM set_config('mcr.notification_authority_until',(clock_timestamp()+interval '20 seconds')::text,true); END $$; INSERT INTO concert_notification_transaction_witnesses(child_id,identity_json,identity_digest) values('{child}',{identity_sql},decode(repeat('04',32),'hex'));")
    check('RR-witness-after-separate-durable-registration',True)
    check('uncommitted-witness-invisible',c.sql('mcr_witness',f"select count(*) from concert_notification_transaction_witnesses where child_id='{child}';")=='0')
    try:
        c.sql('mcr_witness',f"set statement_timeout='150ms'; update concert_notification_authority_intents set sealed=true where child_id='{child}';")
        raise RuntimeError('seal did not wait on actual transaction')
    except m.SmokeError as e:
        check('sealing-waits-for-actual-witness-transaction',e.detail['exit_code']==1 and 'statement timeout' in e.detail['diagnostic'])
    query(p,'COMMIT;')
    check('committed-native-XID-with-actual-witness',c.sql('mcr_witness',f"select pg_xact_status('{identity['full_xid']}'::xid8),(select count(*) from concert_notification_transaction_witnesses where child_id='{child}');")=='committed|1')
    c.sql('mcr_witness',f"update concert_notification_authority_intents set sealed=true where child_id='{child}';")
    rejected(f"update concert_notification_authority_intents set sealed=false where child_id='{child}'")
    rejected(f"update concert_notification_authority_intents set binding_proto=decode('02','hex') where child_id='{child}'")
    rejected('delete from concert_notification_transaction_witnesses')
    rejected('truncate concert_notification_transaction_witnesses')
    rejected('truncate concert_notification_native_references')
    rejected('truncate concert_notification_authority_intents cascade')
    rejected('update concert_notification_producer_instance set instance_id=instance_id')
    check('seal-and-evidence-retention-immutable',True)
    # Cross-session replay of a committed identity is not a current native XID.
    rejected(f"insert into concert_notification_transaction_witnesses(child_id,identity_json,identity_digest) values('{child_abort}',{identity_sql},decode(repeat('05',32),'hex'))")
    check('foreign-native-XID-rejected',True)
    # Actual rollback yields aborted and no witness; there is no missing-GID guess.
    q=session(); query(q,'BEGIN;')
    xid=query(q,'select pg_current_xact_id()::text;'); query(q,'ROLLBACK;')
    check('actual-aborted-native-XID',c.sql('mcr_witness',f"select pg_xact_status('{xid}'::xid8);")=='aborted')
    c.sql('mcr_witness',f"update concert_notification_authority_intents set sealed=true where child_id='{child_sealed}';")
    rejected(f"insert into concert_notification_native_references values('{child_sealed}',{identity_sql},decode(repeat('06',32),'hex'),null,null)")
    check('sealed-intent-refuses-late-native-assignment',True)
    for s in sessions:
        s.stdin.write('\\q\n');s.stdin.flush();s.wait(timeout=5);s.stdin.close();s.stdout.close();s.stderr.close()
    sessions=[]
    before=c.sql('mcr_witness',"select md5(string_agg(row_to_json(t)::text,',' order by child_id)) from concert_notification_transaction_witnesses t;")
    pid=c.assert_postmaster()
    m.execute([str(tools['pg_ctl']),'-D',str(c.data),'-w','-t','20','-m','immediate','stop'],env=c.env,phase='owned-witness-crash',cwd=c.root,timeout=30)
    c.started=False;c.start()
    check('new-postmaster-after-WAL-recovery',c.assert_postmaster()!=pid)
    check('witness-exact-after-WAL-recovery',c.sql('mcr_witness',"select md5(string_agg(row_to_json(t)::text,',' order by child_id)) from concert_notification_transaction_witnesses t;")==before)
    check('native-XID-committed-after-WAL-recovery',c.sql('mcr_witness',f"select pg_xact_status('{identity['full_xid']}'::xid8);")=='committed')
    report['result']='PASS'
except Exception as e:
    report['result']='FAIL'; report['error']=m.redact(str(e),(c.password,))
finally:
    for p in sessions:
        if p.poll() is None: p.terminate();p.wait(timeout=5)
        for f in (p.stdin,p.stdout,p.stderr):
            if f: f.close()
    c.cleanup();report['cleanup']='PASS'
    (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
if report['result']!='PASS':sys.exit(1)
