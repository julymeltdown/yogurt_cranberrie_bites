import fcntl, hashlib, json, os, re, shutil, signal, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode=True
from budget import R, Guard, atomic, baseline, save_baseline, process_table, failure_receipt

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
p=json.loads((R/'plan.json').read_text());prepared=json.loads((R/'prepared-inputs.json').read_text())
root=Path(p['worktree']);target=Path(p['target']);child=None;guardian=None;failure=None;g=None;groups=[];commands=[];copies=[]
assert not (R/'acceptance.json').exists(), 'single use round'
assert p['source']==prepared['source']
assert p.get('execution_grant_issued') is True, 'no execution grant exists for this successor request'
assert p['offer']==prepared['offer']=='AUTH232-NONIGNORED06-REQUEST-0326', 'fresh grant binding must replace the prospective request before use'
assert time.time()<p['offer_expires_epoch']
for name,digest in p['control_sha256'].items():assert sha(R/name)==digest,'control pin '+name
readset=None
def source():
    assert subprocess.check_output([p['git'],'rev-parse','HEAD'],cwd=root,text=True).strip()==p['source']
    assert not subprocess.check_output([p['git'],'status','--porcelain'],cwd=root,text=True)
    assert readset is not None, 'source readset not yet validated'
    for path,digest in readset.items():assert sha(root/path)==digest,'input '+path
assert p['queue_ready'] is True, 'literal predecessor release/yield routing required'
assert p['historical_whole_floor']+p['new_bytes']<=p['whole_bytes'], 'reported carry plus full own NEW does not fit WHOLE'
for proof in p['predecessor_proofs']:
    raw=Path(proof['path']).read_bytes()
    assert hashlib.sha256(raw).hexdigest()==proof['sha256'], 'predecessor immutable receipt'
    value=json.loads(raw)
    for key,expected in proof['fields'].items():assert value[key]==expected, 'predecessor '+key
free=shutil.disk_usage(R).free
assert free>=p['start_floor_bytes'], 'fresh start floor'
lock=(R/'round.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
now=time.time();a={'controller_pid':os.getpid(),'source':p['source'],'offer':p['offer'],'accepted_epoch':now,'work_deadline':now+900,'full_deadline':now+1080,'plan_sha256':sha(R/'plan.json'),'controlFreeze':p['control_sha256']}
atomic('acceptance.json',a)
def publish_owner(state,guardian_pid=None):
    rows=process_table([os.getpid()]+([guardian_pid] if guardian_pid is not None else []));controller=rows.get(os.getpid())
    assert controller is not None, 'actual own controller identity'
    guards=[]
    if guardian_pid is not None:
        assert guardian_pid in rows, 'actual guard identity before expensive census'
        guards=[{'pid':guardian_pid,'birth':rows[guardian_pid][4],
                 'executable':rows[guardian_pid][3].split(maxsplit=1)[0],'script':str(R/'guardian.py'),'cwd':str(R)}]
    record={'ownerThreadId':p['thread_id'],'offerId':p['offer'],'state':state,
        'acceptedAtEpoch':a['accepted_epoch'],'workDeadlineEpoch':a['work_deadline'],
        'fullDeadlineEpoch':a['full_deadline'],'kind':'source' if (R/'TARGET.json').exists() else 'target','processCap':p['max_processes'],
        'rssCapBytes':p['max_rss_bytes'],'remainingNewUpperBytes':0 if state=='FULL' else p['new_bytes'],
        'controller':{'pid':os.getpid(),'birth':controller[4],'executable':controller[3].split(maxsplit=1)[0],
                      'script':str(R/'run.py'),'outputRoot':str(R),'cwd':str(R)},'guardControllers':guards,
        'receiptPaths':{'accept':str(R/'acceptance.json'),'full':str(R/'FULL.json') if state=='FULL' else None},
        'receiptSHA256':{'accept':sha(R/'acceptance.json')},'observedAtEpoch':time.time(),
        'targetAccess':'NONE' if state=='FULL' or (R/'TARGET.json').exists() else p['target'],'source':p['source']}
    record['controlFreeze']={'planSHA256':a['plan_sha256'],'files':a['controlFreeze']}
    record['closedHistory']=json.loads((R/'closed-history-index.json').read_text())
    record['leasePhase']='FROZEN_EXECUTABLE_TESTS' if (R/'TARGET.json').exists() else 'TARGET'
    if state=='FULL':
        record['receiptSHA256']['full']=sha(R/'FULL.json')
        full=json.loads((R/'FULL.json').read_text())
        record['closedHistory'].append({'offerId':p['offer'],'state':'FULL','acceptedAtEpoch':a['accepted_epoch'],
            'actualFullEpoch':full['epoch'],'resourcesReleased':True,'fullPath':str(R/'FULL.json'),
            'fullSHA256':sha(R/'FULL.json'),'qualification':'Resource release; separate technical disposition retained'})
    path=Path(p['owner_receipt_directory'])/(p['thread_id']+'.json')
    tmp=path.with_suffix('.tmp');tmp.write_text(json.dumps(record,indent=2)+'\n');tmp.replace(path)

def interrupted(*args):raise RuntimeError('guard or signal stopped round')
signal.signal(signal.SIGTERM,interrupted);signal.signal(signal.SIGINT,interrupted)
def sample(cleanup=False):
    assert guardian is None or guardian.poll() is None,'independent guardian stopped'
    g.cleanup=cleanup
    value=g.sample();atomic('primary.live.json',{'pid':os.getpid(),'sample':value});return value
def witnessed_members(rows,group):
    witnesses={}
    for path in R.glob('owned-identities-*.json'):
        assert not path.is_symlink() and path.stat().st_size<=512*1024
        witnesses.update(json.loads(path.read_text()))
    return [pid for pid,row in rows.items() if row[1]==group and str(pid) in witnesses and
            row[4]==witnesses[str(pid)]['birth'] and row[1]==witnesses[str(pid)]['group']]
def reap():
    global child
    if child is not None:
        if child.poll() is None:
            try:os.killpg(child.pid,signal.SIGTERM)
            except ProcessLookupError:pass
            try:child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                try:os.killpg(child.pid,signal.SIGKILL)
                except ProcessLookupError:pass
                child.wait(timeout=5)
        if commands and commands[-1]['exit_code'] is None:
            commands[-1].update(exit_code=child.returncode,stopped_during_guard_or_cleanup=True)
            atomic('commands.json',commands)
        child=None
    for group in list(groups):
        rows=process_table()
        members=witnessed_members(rows,group)
        if members:
            for pid in members:
                try:os.kill(pid,signal.SIGTERM)
                except ProcessLookupError:pass
            stop=min(time.time()+5,a['full_deadline']-30)
            while time.time()<stop:
                rows=process_table()
                if not witnessed_members(rows,group):break
                time.sleep(.05)
            else:
                for pid in witnessed_members(process_table(),group):
                    try:os.kill(pid,signal.SIGKILL)
                    except ProcessLookupError:pass
        rows=process_table()
        if not witnessed_members(rows,group):groups.remove(group)
    atomic('groups.json',groups)
def run(argv,label,allowed=(0,),env=None):
    global child
    sample();live=json.loads((R/'guardian.live.json').read_text())
    assert live['complete_initial'] and time.time()-live['sample']['epoch']<30,'fresh independent complete sample'
    start=time.time()
    with (R/(label+'.stdout')).open('wb') as out,(R/(label+'.stderr')).open('wb') as err:
        with (R/'identity-gates.lock').open('a') as gate_lock:
            fcntl.flock(gate_lock,fcntl.LOCK_EX)
            state=json.loads((R/'identity-gates.json').read_text())
            assert not any(state['pending'].values()) and not state['paused'], 'identity gate before own workload launch'
            child=subprocess.Popen(argv,cwd=root/'backend',env=env,stdout=out,stderr=err,start_new_session=True)
            groups.append(child.pid);atomic('groups.json',groups)
            rows=process_table([os.getpid(),child.pid])
            witnesses=json.loads((R/'group-identities.json').read_text()) if (R/'group-identities.json').exists() else {}
            if child.pid in rows:
                assert rows[child.pid][0]==os.getpid(),'actual direct own child'
                witnesses[str(child.pid)]={'birth':rows[child.pid][4],'parent':os.getpid(),'command':rows[child.pid][3]}
            atomic('group-identities.json',witnesses)
        commands.append({'label':label,'argv':argv,'cwd':str(root/'backend'),'pid':child.pid,'started_epoch':start,'exit_code':None})
        atomic('commands.json',commands)
        while child.poll() is None:sample();time.sleep(.25)
        code=child.returncode;finished=child.pid;child=None
        rows=process_table()
        live=[]
        for pid,row in rows.items():
            if row[1]!=finished:continue
            try:os.kill(pid,0)
            except ProcessLookupError:continue
            live.append(pid)
        if not live:
            groups.remove(finished);atomic('groups.json',groups)
    commands[-1].update(exit_code=code,seconds=time.time()-start)
    atomic('commands.json',commands)
    print(json.dumps({'phase':'COMMAND_END','label':label,'exit_code':code,'seconds':time.time()-start}),flush=True)
    assert code in allowed,'command failed '+label
    return code
def release_target():
    if (R/'TARGET.json').exists():return
    (R/'freeze-target').touch()
    if guardian is not None:
        stop=min(time.time()+30,a['full_deadline']-20)
        while not (R/'guardian.target-frozen.json').exists():
            assert guardian.poll() is None and time.time()<stop, 'independent target freeze'
            time.sleep(.1)
    if g is not None:g.frozen=True;g.cleanup=False
    q=subprocess.run([p['lsof'],'-nP','+D',p['target']],capture_output=True,text=True,timeout=45)
    assert q.returncode==1 and not q.stdout and not q.stderr,'target handles remain'
    receipt={'state':'TARGET_RELEASED','offer':p['offer'],'source':p['source'],'epoch':time.time(),
             'target':p['target'],'open_handles':0,'samplers_frozen':True}
    atomic('TARGET.json',receipt)
    publish_owner('ACCEPTED',guardian.pid if guardian is not None and guardian.poll() is None else None)
    print(json.dumps({'phase':'ACTUAL_TARGET','receipt':receipt,'sha256':sha(R/'TARGET.json')}),flush=True)

try:
    publish_owner('ACCEPTED')
    print(json.dumps({'phase':'ACTUAL_ACCEPT','acceptance':a,'root':str(R)}),flush=True)
    guardian=subprocess.Popen([sys.executable,'-B',str(R/'guardian.py')],cwd=R,start_new_session=True)
    publish_owner('ACCEPTED',guardian.pid)
    temporary=Guard(p,a,{'roots':{},'inodes':[],'bytes':0})
    temporary.fast() # Verify current peer/class/floor reservations before any target root lstat.
    b=baseline(p,temporary.checkpoint);save_baseline(b);g=Guard(p,a,b)
    initial=sample();atomic('primary.initial.json',initial)
    stop=min(time.time()+45,a['work_deadline'])
    while not (R/'guardian.initial.json').exists():
        assert guardian.poll() is None and time.time()<stop,'independent initial sample'
        temporary.fast();time.sleep(.1)
    initial2=json.loads((R/'guardian.initial.json').read_text());assert initial2['complete']
    print(json.dumps({'phase':'BOTH_COMPLETE_INITIAL','primary':initial,'independent':initial2}),flush=True)
    # Protocol fixtures do not publish shared identities or replace real guards.
    from protocol_fixtures import check_protocol
    atomic('protocol-fixtures.json',check_protocol(p))
    print(json.dumps({'phase':'PROTOCOL_FIXTURES_PASS'}),flush=True)
    assert sha(prepared['readset_path'])==prepared['readset_sha256']
    readset=json.loads(Path(prepared['readset_path']).read_text())
    assert set(prepared['readset_overrides'])<={'docs/mcr/backend/auth-deletion-observation.md','docs/mcr/backend/moderation-user-review-auth.md'}, 'readset overlay ownership'
    readset.update(prepared['readset_overrides'])
    for path,v in json.loads(Path(prepared['tools_path']).read_text()).items():
        info=Path(path).stat()
        assert (info.st_dev,info.st_ino,info.st_size)==(v['device'],v['inode'],v['bytes']) and sha(path)==v['sha256'],'tool pin'
        sample()
    source();sample()
    # Exact bytes/inodes remain fixed. Refresh only private backend Cargo inputs,
    # never source originals, peers, common Git, toolchain or generated outputs.
    refresh=[]
    for path,digest in readset.items():
        f=root/path
        if not path.startswith('backend/') or (f.suffix not in ('.rs','.proto') and f.name not in ('Cargo.toml','Cargo.lock')):continue
        info=f.lstat();assert f.is_file() and not f.is_symlink() and info.st_uid==os.getuid() and sha(f)==digest
        os.utime(f,None,follow_symlinks=False);after=f.stat()
        assert (after.st_dev,after.st_ino,after.st_size)==(info.st_dev,info.st_ino,info.st_size) and sha(f)==digest
        refresh.append({'path':path,'before_mtime_ns':info.st_mtime_ns,'after_mtime_ns':after.st_mtime_ns,'sha256':digest})
    atomic('mtime-refresh.json',refresh);source();sample()
    env={k:v for k,v in os.environ.items() if not (k.startswith(('PG','MCR_')) or k.endswith('DATABASE_URL') or k in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','GIT_INDEX_FILE'))}
    env.update(prepared['environment']);env['RUSTUP_TOOLCHAIN']='1.88.0';env['PYTHONDONTWRITEBYTECODE']='1'
    env['PATH']=str(Path(env['RUSTC']).parent)+':'+env['PATH']
    (R/'tmp').mkdir(exist_ok=True)
    fmt=prepared['commands'][0];format_failed=run(fmt['argv'],'format-check',allowed=(0,) if p['format_failure_blocks'] else (0,1),env=env)!=0
    compile=prepared['commands'][1];run(compile['argv'],'cargo-build',env=env)
    records=[]
    for line in (R/'cargo-build.stdout').read_text().splitlines():
        try:v=json.loads(line)
        except ValueError:continue
        if v.get('reason')=='compiler-artifact' and v.get('executable'):records.append(v)
    wanted=['auth_service','concert_moderation_lifecycle','concert_deletion_observation']
    for name in wanted:
        found=[v for v in records if v['target']['name']==name and v['profile'].get('test')]
        assert len(found)==1,'exact executable '+name
        v=found[0];f=Path(v['executable'])
        assert Path(v['manifest_path']).resolve()==(root/'backend/services/auth-service/Cargo.toml').resolve()
        assert f.is_file() and not f.is_symlink() and f.resolve().is_relative_to(target.resolve())
        sample();dest=R/(name+'-frozen');assert not dest.exists();shutil.copy2(f,dest);dest.chmod(0o500)
        digest=sha(dest);assert digest==sha(f)
        copies.append({'name':name,'path':str(dest),'bytes':dest.stat().st_size,'sha256':digest,'artifact':v})
        atomic('executables.json',copies);sample()
    release_target()
    source()
    for c in copies:
        assert sha(c['path'])==c['sha256']
        argv=[c['path']]+(['concert_moderation::'] if c['name']=='auth_service' else [])+['--test-threads=1']
        label='test-'+c['name'];run(argv,label,env=env)
        summary=re.search(r'test result: ok\. ([0-9]+) passed;', (R/(label+'.stdout')).read_text())
        assert summary and int(summary.group(1))>0, 'nonzero executed nonignored test count '+c['name']
        assert sha(c['path'])==c['sha256']
    source();sample()
    assert not format_failed, 'required Rust format CHECK failed (independent compile/tests retained)'
except BaseException as error:
    failure=type(error).__name__+': '+str(error)
    failure_receipt(error,'primary.failure')
finally:
    signal.signal(signal.SIGTERM,signal.SIG_IGN);signal.signal(signal.SIGINT,signal.SIG_IGN)
    reap();(R/'cleanup.phase').touch()
    if g is not None and not (R/'guardian.failure.json').exists():
        try:sample(cleanup=True)
        except BaseException as error:failure=failure or 'final complete sample '+type(error).__name__+': '+str(error)
    try:
        if not (R/'TARGET.json').exists():release_target()
    except BaseException as error:failure=failure or 'cleanup '+type(error).__name__+': '+str(error)
    (R/'guardian.stop').touch()
    if guardian is not None:
        try:guardian.wait(timeout=20)
        except subprocess.TimeoutExpired:guardian.terminate();guardian.wait(timeout=5)
    if (R/'tmp').exists():shutil.rmtree(R/'tmp')
    try:
        assert not subprocess.check_output([p['git'],'status','--porcelain'],cwd=root,text=True), 'source dirty at cleanup'
    except BaseException as error:failure=failure or 'source metadata '+str(error)
    external=json.loads((R/'guardian.result.json').read_text()) if (R/'guardian.result.json').exists() else {'failure':'missing independent result'}
    failure=failure or external['failure']
    if time.time()>=a['full_deadline']:failure=failure or 'FULL deadline'
    rows=process_table()
    remaining=sorted({pid for group in groups for pid in witnessed_members(rows,group)})
    if remaining:failure=failure or 'unreaped owned group members'
    released=not remaining and (R/'TARGET.json').exists()
    lock.close()
    result={'state':'FULL_RELEASED' if released else 'FULL_INCOMPLETE','offer':p['offer'],'source':p['source'],'failure':failure,'exit_code':1 if failure else 0,'epoch':time.time(),'target_access':'NONE' if released else 'HELD','workflows':0 if released else 1,'remaining_new_bytes':0 if released else p['new_bytes'],'owned_children':len(remaining),'remaining_pids':remaining,'commands':commands,'copies':copies,'primary_peak':g.peak if g else {},'guardian':external}
    atomic('FULL.json',result)
    publish_owner('FULL' if released else 'ACCEPTED')
    print(json.dumps({'phase':'ACTUAL_FULL','result':result,'sha256':sha(R/'FULL.json')}),flush=True)
raise SystemExit(1 if failure else 0)
