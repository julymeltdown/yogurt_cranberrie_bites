#!/usr/bin/env python3
"""Frozen-binary, socket-only owned PG qualification. Never accesses Cargo targets."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import uuid

PG=Path('/opt/homebrew/opt/postgresql@16/bin')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser();mode=parser.add_mutually_exclusive_group(required=True);mode.add_argument('--frozen',type=Path);mode.add_argument('--sql-only',action='store_true');a=parser.parse_args()
    if a.sql_only:
        from freeze import snapshot
        frozen=Path(tempfile.mkdtemp(prefix='mcr-event-cleanup-sql-evidence-',dir='/private/tmp')).resolve()
        receipt={'source':snapshot(),'binaries':{}}
    else:
        frozen=a.frozen.resolve();receipt=json.loads((frozen/'freeze.json').read_text())
    for name,info in receipt['binaries'].items():
        path=frozen/'bin'/name;assert sha(path)==info['sha256'] and path.stat().st_size==info['size']
    logs=frozen/'native';logs.mkdir(mode=0o700)
    root=Path(tempfile.mkdtemp(prefix='mcr-event-cleanup-',dir='/private/tmp')).resolve()
    root_stat=root.stat();token=uuid.uuid4().hex;(root/'owner').write_text(token)
    socket=root/'socket';socket.mkdir();data=root/'data'
    record={'status':'IN_PROGRESS','checks':[],'source':receipt['source'],'binaries':receipt['binaries'],'cluster':str(root),'cleanup':False}
    started=False
    env={k:v for k,v in os.environ.items() if not k.startswith(('PG','DATABASE','CONCERT_DATABASE','MCR_'))}
    env.update(MCR_EVENT_CLEANUP_SOCKET=str(socket),MCR_EVENT_CLEANUP_OWNER=token,RUST_BACKTRACE='1')
    def run(name,cmd,timeout=60,check=True):
        proc=subprocess.run([str(v) for v in cmd],env=env,capture_output=True,text=True,timeout=timeout)
        (logs/f'{name}.stdout').write_text(proc.stdout);(logs/f'{name}.stderr').write_text(proc.stderr)
        record['checks'].append({'name':name,'exit_code':proc.returncode})
        if check and proc.returncode:raise RuntimeError(f'{name} failed; preserved native logs')
        return proc.stdout
    def sql(name,statement):return run(name,[PG/'psql','-X','-v','ON_ERROR_STOP=1','-At','-h',socket,'-p','55446','-U','mcr_event_cleanup_fixture','-d','mcr_event_cleanup_fixture','-c',statement])
    def sql_file(name,path):
        out=run(name,[PG/'psql','-X','-v','ON_ERROR_STOP=1','-v',f'fixture_owner={token}','-At','-h',socket,'-p','55446','-U','mcr_event_cleanup_fixture','-d','mcr_event_cleanup_fixture','-f',path])
        record['checks'][-1]['assertions']=[line for line in out.splitlines() if line.startswith('SQL_GUARD_PASS:')]
        assert record['checks'][-1]['assertions'], 'SQL guard file ran no assertions'
    def native(name,filter_name,ignored=True):
        out=run(name,[frozen/'bin'/'event_cleanup_runtime',filter_name,*(['--ignored'] if ignored else []),'--nocapture','--test-threads=1'],timeout=120)
        match=re.search(r'test result: ok\. (\d+) passed; 0 failed;',out);assert match and int(match.group(1))>0,f'no tests ran: {name}'
        record['checks'][-1]['passed']=int(match.group(1))
        if 'BE41_CLEANUP_SOURCE_PIN=' in out:
            assert receipt['source']['owned_source_pin'] in out,'frozen binary/source pin mismatch'
    try:
        version=run('pg-version',[PG/'postgres','--version']);assert '16.' in version
        if not a.sql_only:
            out=run('application',[frozen/'bin'/'event_cleanup','--nocapture']);assert '4 passed; 0 failed' in out
            native('config','event_cleanup_config_',False)
        run('initdb',[PG/'initdb','-D',data,'-A','trust','-U','mcr_event_cleanup_fixture','--no-locale','-E','UTF8'])
        with (data/'postgresql.conf').open('a') as f:f.write(f"\nlisten_addresses = ''\nport = 55446\nunix_socket_directories = '{socket}'\nunix_socket_permissions = 0700\nfsync = on\nsynchronous_commit = on\nmax_connections = 16\n")
        run('pg-start',[PG/'pg_ctl','-D',data,'-l',root/'postgres.log','-w','start']);started=True
        run('database',[PG/'createdb','-h',socket,'-p','55446','-U','mcr_event_cleanup_fixture','mcr_event_cleanup_fixture'])
        sql('marker',f"create table event_cleanup_fixture_marker(owner_token text not null);insert into event_cleanup_fixture_marker values('{token}');")
        if a.sql_only:
            directory=Path(__file__).resolve().parents[3]/'services/concert-service/migrations'
            for migration in sorted(directory.glob('*.sql')):
                run(migration.stem,[PG/'psql','-X','-v','ON_ERROR_STOP=1','-h',socket,'-p','55446','-U','mcr_event_cleanup_fixture','-d','mcr_event_cleanup_fixture','-f',migration])
            sql_file('schema-guards',Path(__file__).with_name('schema_guards.sql'))
            statement="begin;select set_config('mcr.lifecycle_authority_until',(clock_timestamp()+interval '100 milliseconds')::text,true);update concert_event_cleanup_progress set state='PARTIAL_FAILURE',failure='OWNER_UNAVAILABLE';select pg_sleep(0.2);commit;"
            expired=run('expired-commit',[PG/'psql','-X','-v','ON_ERROR_STOP=1','-h',socket,'-p','55446','-U','mcr_event_cleanup_fixture','-d','mcr_event_cleanup_fixture','-c',statement],check=False)
            assert record['checks'][-1]['exit_code']!=0 and 'closure authority expired' in (logs/'expired-commit.stderr').read_text(), 'deferred expiry was not enforced'
            record['checks'][-1]['expected_rejection']=True
            assert sql('expiry-rollback',"select state||'|'||ordinal||'|'||processed from concert_event_cleanup_progress;").strip()=='PENDING|1|2'
        else:
            native('migrations','event_cleanup_fixture_prepare')
            native('runtime','event_cleanup_runtime_')
            native('restart-prepare','event_cleanup_restart_prepare')
        before=sql('before-restart','select pg_postmaster_start_time(), pg_backend_pid();').strip();pid_before=int((data/'postmaster.pid').read_text().splitlines()[0])
        run('pg-stop-restart',[PG/'pg_ctl','-D',data,'-m','fast','-w','stop']);started=False
        run('pg-start-restart',[PG/'pg_ctl','-D',data,'-l',root/'postgres.log','-w','start']);started=True
        after=sql('after-restart','select pg_postmaster_start_time(), pg_backend_pid();').strip();pid_after=int((data/'postmaster.pid').read_text().splitlines()[0]);assert before.split('|')[0]!=after.split('|')[0] and pid_before!=pid_after
        record['restart']={'before':before,'after':after,'pid_before':pid_before,'pid_after':pid_after}
        if a.sql_only:
            sql_file('schema-resume',Path(__file__).with_name('schema_resume.sql'))
        else:
            native('restart-verify','event_cleanup_restart_verify')
        assert sql('prepared','select count(*) from pg_prepared_xacts;').strip()=='0'
        record['status']='PASS_SQL_ONLY' if a.sql_only else 'PASS'
    except BaseException as error:
        record['status']='FAIL';record['failure']=str(error);raise
    finally:
        try:
            assert root.stat().st_ino==root_stat.st_ino and root.stat().st_dev==root_stat.st_dev and (root/'owner').read_text()==token,'cluster ownership changed'
            if started or (data/'postmaster.pid').exists():
                pid_lines=(data/'postmaster.pid').read_text().splitlines()
                assert Path(pid_lines[1]).resolve()==data.resolve(), 'postmaster data directory changed'
                run('pg-final-stop',[PG/'pg_ctl','-D',data,'-m','fast','-w','stop'])
            assert not (data/'postmaster.pid').exists(),'postmaster still active'
            if (root/'postgres.log').exists():shutil.copy2(root/'postgres.log',logs/'postgres.log')
            shutil.rmtree(root);record['cleanup']=not root.exists()
        finally:
            record['log_hashes']={p.name:sha(p) for p in sorted(logs.iterdir()) if p.is_file()}
            (frozen/'native.json').write_text(json.dumps(record,indent=2)+'\n')
            print(json.dumps({'receipt':str(frozen/'native.json'),'status':record['status'],'cleanup':record['cleanup']}),flush=True)
    print(json.dumps({'status':record['status'],'cleanup':record['cleanup'],'receipt':str(frozen/'native.json')}))
if __name__=='__main__':main()
