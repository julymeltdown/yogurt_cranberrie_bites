#!/usr/bin/env python3
"""Exact owned source SIGKILL/reap, cold full-cluster backup, restore and new PID."""
from __future__ import annotations
import hashlib,json,os,selectors,shutil,signal,stat,subprocess,time,uuid
from pathlib import Path
TEST='canonical_owner_restore_process'
READY=b'MCR_RESTORE_SOURCE_READY'
OBSERVED=b'MCR_RESTORE_OBSERVED'
class CheckFailed(RuntimeError):pass
def require(ok,reason):
    if not ok:raise CheckFailed(reason)
def private(path,limit=1_000_000):
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW)
    try:
        m=os.fstat(fd);require(stat.S_ISREG(m.st_mode)and stat.S_IMODE(m.st_mode)==0o600 and m.st_uid==os.getuid()and m.st_size<=limit,'private handoff identity')
        with os.fdopen(os.dup(fd),'rb')as f:return f.read()
    finally:os.close(fd)
def sha(p):
    h=hashlib.sha256()
    with p.open('rb')as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()
def manifest(directory):
    require(directory.is_dir()and not directory.is_symlink(),'owned physical directory')
    result={}
    for parent,dirs,files in os.walk(directory,followlinks=False):
        for name in dirs+files:
            p=Path(parent)/name;require(not p.is_symlink(),'physical backup refuses symlinks/tablespaces')
        for name in files:
            p=Path(parent)/name;require(p.is_file(),'regular physical file');result[p.relative_to(directory).as_posix()]=sha(p)
    require('global/pg_control'in result and any(p.startswith('pg_xact/')for p in result),'whole cluster transaction history required')
    return result
class Child:
    def __init__(self,binary,env,log):
        self.log=os.fdopen(os.open(log,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),'wb')
        try:self.p=subprocess.Popen([str(binary),'--ignored','--exact',TEST,'--test-threads=1','--nocapture'],env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,close_fds=True,start_new_session=True)
        except BaseException:self.log.close();raise
        os.set_blocking(self.p.stdout.fileno(),False);self.selector=selectors.DefaultSelector();self.selector.register(self.p.stdout,selectors.EVENT_READ);self.buffer=b'';self.lines=set();self.eof=False;self.size=0
    def pump(self,duration):
        for key,_ in self.selector.select(duration):
            b=os.read(key.fileobj.fileno(),65536)
            if not b:self.selector.unregister(key.fileobj);self.eof=True;continue
            self.size+=len(b);require(self.size<=2_000_000,'bounded child log');self.log.write(b);self.log.flush();self.buffer+=b
            while b'\n'in self.buffer:
                line,self.buffer=self.buffer.split(b'\n',1)
                if line.rstrip(b'\r')in(READY,OBSERVED):self.lines.add(line.rstrip(b'\r'))
            require(len(self.buffer)<=65536,'bounded child line')
    def until(self,line,timeout=45):
        stop=time.monotonic()+timeout
        while line not in self.lines:
            require(time.monotonic()<stop,'exact protocol line timeout');self.pump(.1)
            if self.p.poll()is not None and self.eof:require(line in self.lines,'child exited before exact observation')
    def finish(self,timeout=15):
        stop=time.monotonic()+timeout
        while self.p.poll()is None or not self.eof:require(time.monotonic()<stop,'child exit timeout');self.pump(.1)
        return self.p.wait(timeout=1)
    def kill_reap(self):
        if self.p.poll()is None:self.p.kill()
        return self.p.wait(timeout=10)
    def close(self):
        try:self.kill_reap()
        finally:self.selector.close();self.p.stdout.close();self.log.close()
def env_for(cluster,run,instance,case,phase):
    env={**cluster.env,'CONCERT_SERVICE_CREDENTIAL':'concert-fixture','NOTIFICATION_SERVICE_CREDENTIAL':'notification-fixture','MCR_CANONICAL_ROOT':str(cluster.root),'MCR_CANONICAL_ROOT_IDENTITY':f'{cluster.identity[0]}:{cluster.identity[1]}','MCR_CANONICAL_ROOT_TOKEN':cluster.token,'MCR_CANONICAL_SYSTEM':cluster.sql('postgres','select system_identifier from pg_control_system();'),'MCR_RESTORE_RUN':run,'MCR_RESTORE_INSTANCE':instance,'MCR_RESTORE_CASE':case,'MCR_RESTORE_PHASE':phase}
    for owner in('auth','friend','concert','profile','notification'):env['MCR_CANONICAL_'+owner.upper()+'_URL']=cluster.url('mcr_canonical_'+owner)
    return env
def db_identity(cluster):return {o:cluster.sql('mcr_canonical_'+o,"select (pg_control_system()).system_identifier::text||':'||(select oid::text from pg_database where datname=current_database());")for o in('auth','friend','concert','profile','notification')}
def run(binary,source,restored,case,report):
    run_id=str(uuid.uuid4());instance=str(uuid.uuid4());old=new=None
    try:
        old=Child(binary,env_for(source,run_id,instance,case,'SOURCE'),source.root/'restore-source.log');old.until(READY)
        report['source_pid']=old.p.pid
        saved=private(source.root/'restore-private-handoff.json');require(json.loads(saved)['old_pid']==old.p.pid,'source actual PID')
        report['handoff_sha256']=hashlib.sha256(saved).hexdigest();report['source_database_identities_sha256']=hashlib.sha256(json.dumps(db_identity(source),sort_keys=True).encode()).hexdigest()
        source_identity=db_identity(source);source_pg=source.assert_postmaster()
        code=old.kill_reap();require(code==-signal.SIGKILL,'actual SIGKILL/reap required');report['source_sigkill_reaped']=True;old.close();old=None
        source.stop();require(not(source.data/'postmaster.pid').exists(),'source postmaster stopped before copy')
        try:os.kill(source_pg,0);pg_gone=False
        except ProcessLookupError:pg_gone=True
        require(pg_gone,'source postmaster PID remains');report['source_postmaster_stopped']=True
        # Full stopped-cluster backup: pg_xact, WAL, pg_twophase, all five DBs,
        # catalog OIDs and control identity travel together. No SQL phase edits,
        # force rollback, GID rewrite, in-memory reset or pg_dump substitution.
        original=manifest(source.data);backup=source.root/'physical-backup';require(not backup.exists(),'new backup destination')
        shutil.copytree(source.data,backup,copy_function=shutil.copy2);require(manifest(backup)==original,'physical backup file mismatch')
        backup_stat=backup.stat();require(not restored.data.exists(),'fresh restored data destination')
        shutil.copytree(backup,restored.data,copy_function=shutil.copy2);require(manifest(restored.data)==original,'restored full file mismatch')
        require((restored.data.stat().st_dev,restored.data.stat().st_ino)!=(source.data.stat().st_dev,source.data.stat().st_ino),'physical destination must differ')
        report['physical_backup']={'kind':'STOPPED_FULL_CLUSTER_COPY','file_count':len(original),'source_manifest_sha256':hashlib.sha256(json.dumps(original,sort_keys=True).encode()).hexdigest(),'backup_matches_every_source_file':True,'restored_matches_every_backup_file_before_start':True,'contains_pg_twophase':any(p.startswith('pg_twophase/')for p in original),'source_postmaster_absent_before_backup':True,'separate_owned_destination':True}
        # Only explicitly retained operator material follows the physical copy.
        # The new fixture keeps its independently checked directory/marker/socket.
        restored.password=source.password
        shutil.copytree(source.root/'pki',restored.root/'pki',copy_function=shutil.copy2)
        fd=os.open(restored.root/'restore-private-handoff.json',os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(fd,'wb')as f:f.write(saved);f.flush();os.fsync(f.fileno())
        restored.start();report['restored_postmaster_distinct']=restored.assert_postmaster()!=source_pg;require(report['restored_postmaster_distinct'],'distinct restored postmaster PID')
        require(db_identity(restored)==source_identity,'restored system/database identity drift')
        report['physical_backup']['system_identifier_and_all_database_oids_preserved']=True
        # The immutable backup copy itself stays unmodified while restored PG runs.
        require((backup.stat().st_dev,backup.stat().st_ino)==(backup_stat.st_dev,backup_stat.st_ino),'backup identity changed')
        new=Child(binary,env_for(restored,run_id,instance,case,'RESTORED'),restored.root/'restore-new.log');new.until(OBSERVED);code=new.finish();require(code==0,'restored process must exit 0')
        report['new_pid']=new.p.pid;require(report['new_pid']!=report['source_pid'],'distinct new Root PID')
        observed=json.loads(private(restored.root/'restore-observation.json'));require(observed['status']=='PASS'and observed['case']==case,'exact case proof')
        require(observed['old_pid']==report['source_pid']and observed['new_pid']==report['new_pid'],'actual observation PIDs')
        report['observations']=observed;report['restored_process_exit_code']=code;report['backup_unmodified_after_recovery']=manifest(backup)==original;require(report['backup_unmodified_after_recovery'],'recovery changed immutable backup')
        report['status']='PASS'
    finally:
        if old:old.close()
        if new:new.close()
