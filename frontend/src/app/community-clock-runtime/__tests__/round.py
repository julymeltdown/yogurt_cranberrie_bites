"""One explicitly allocated COMMUNITY CLOCK source round; never allocates capacity itself.
Reuses the previously qualified identity-safe process/footprint helpers read-only.
"""
from pathlib import Path
import ast
import datetime as dt
import importlib.util
import hashlib
import inspect
import json
import os
import shutil
import signal
import socket
import subprocess as sp
import sys
import time

ROOT = Path(__file__).resolve().parents[5]
FRONT = ROOT / 'frontend'
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
BASE = '2492e35bb9fb61380bf97fa936983ec16c0722f4'
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
PREFIX = 'src/app/community-clock-runtime/__tests__'
HELPER = FRONT / 'src/features/gift-discovery/__tests__/gift-detail-media/native_round.py'
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('retained_guard_helpers', HELPER)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
rows, stop_owned, write = helper.process_rows, helper.stop_owned, helper.write

def footprint(path):
    # One stat per entry keeps the independent guard responsive on the retained tree.
    try:
        if path.is_symlink(): return 0
        if path.is_file():
            st=path.stat(); return max(st.st_size,st.st_blocks*512)
        total=0; pending=[path]
        while pending:
            with os.scandir(pending.pop()) as entries:
                for entry in entries:
                    if entry.is_symlink(): continue
                    if entry.is_dir(follow_symlinks=False): pending.append(entry.path)
                    elif entry.is_file(follow_symlinks=False):
                        st=entry.stat(follow_symlinks=False); total+=max(st.st_size,st.st_blocks*512)
        return total
    except FileNotFoundError: return 0
MIB = 1024**2
START_FLOOR_BYTES = 11*1024**3  # Allocator21d future bounded-job policy; never changes an accepted round.
LIVE_FLOOR_BYTES = 8*1024**3

def now():
    return dt.datetime.now(dt.UTC).isoformat()

def git(*args):
    return sp.check_output(['git', *args], cwd=ROOT, text=True).strip()

def preflight():
    source = footprint(ROOT)
    private_git = Path(git('rev-parse', '--git-dir'))
    ranges = [f'{helper.BASE}..codex/mcr-fe-gifts-media-detail',
              'ff96d53eb817c93dd1bafb5c4f125630f995413b..codex/mcr-fe-FE39-current-20261105', '306626d49cd8b55916cf973ab6eea06569ec98d4..codex/mcr-fe-default-admin-composition', f'{BASE}..HEAD']
    owned_objects = {line.split()[0] for span in ranges for line in git('rev-list', '--objects', span).splitlines()}
    common = Path(git('rev-parse','--git-common-dir')).resolve()
    missing = set(); inode_inventory = {}
    def charge(path):
        if not path.is_file(): return
        st=path.stat(); key=f'{st.st_dev}:{st.st_ino}'
        inode_inventory[key]=dict(path=str(path),logical=st.st_size,allocated=st.st_blocks*512,charged=max(st.st_size,st.st_blocks*512))
    for oid in owned_objects:
        loose=common/'objects'/oid[:2]/oid[2:]
        if loose.is_file(): charge(loose)
        else: missing.add(oid)
    for index in (common/'objects/pack').glob('*.idx'):
        if not missing: break
        with index.open('rb') as stream:
            entries=sp.check_output(['git','show-index'],stdin=stream,text=True).splitlines()
        members={line.split()[1] for line in entries}
        present=missing & members
        if present:
            for member in index.parent.glob(index.stem+'.*'): charge(member)
            charge(index.parent/'multi-pack-index'); missing-=present
    assert not missing, 'Owned Git object inode custody unavailable'
    for name in ['config','packed-refs']: charge(common/name)
    for ref in ['codex/mcr-fe-gifts-media-detail','codex/mcr-fe-FE39-current-20261105','codex/mcr-fe-default-admin-composition','codex/mcr-fe-community-clock-runtime']:
        charge(common/'refs/heads'/ref); charge(common/'logs/refs/heads'/ref)
    charged_git=footprint(private_git)+sum(v['charged'] for v in inode_inventory.values())
    charge_node=footprint(NODE.resolve())
    return dict(at=now(), pin=git('rev-parse','HEAD'), clean=not git('status','--porcelain'), source_bytes=source,
                deps_bytes=max(789164032,footprint(DEPS)+sum(footprint(path) for path in helper.OLD))+charge_node, git_bytes=charged_git, git_dir=str(private_git),
                git_dir_bytes=footprint(private_git), git_inode_inventory=inode_inventory, node_inode_bytes=charge_node, owned_object_ranges=ranges, owned_object_count=len(owned_objects), free_bytes=shutil.disk_usage(ROOT).free)

def primary_sample(prefix,state,observed,work_end):
    live_rows=rows(); selected={os.getpid()}
    selected.update(pid for pid,row in live_rows.items() if str(pid) in observed and row['uid']==observed[str(pid)]['uid'] and row['started']==observed[str(pid)]['started'])
    for _ in live_rows:
        found={pid for pid,row in live_rows.items() if row['parent'] in selected}
        if found<=selected: break
        selected.update(found)
    live={str(pid):row for pid,row in live_rows.items() if pid in selected}; observed.update(live)
    tmp=footprint(prefix/'work'); durable=footprint(prefix)-tmp
    other=max(0,footprint(ROOT)-state['source_bytes'])+max(0,footprint(Path(state['git_dir']))-state['git_dir_bytes'])
    values=dict(pids=len(live),rss=sum(r['rss']*1024 for r in live.values()),temporary=tmp,durable=durable,other=other,
      new_total=state['new_offset']+tmp+durable+other,whole=max(state['whole_floor'],state['source_bytes']+state['deps_bytes']+state['git_bytes']+tmp+durable+other))
    free=shutil.disk_usage(ROOT).free
    reason='primary resource limit' if any(values[k]>v for k,v in state['limits'].items()) else 'primary live floor' if free<LIVE_FLOOR_BYTES else 'primary work deadline' if time.time()>=work_end else None
    write(prefix/'primary.json',dict(at=now(),values=values,free_bytes=free,reason=reason,observed_processes=observed))
    if reason: raise RuntimeError(reason)

def guard(prefix, owner, work_end, hard_end):
    observed, peaks, reason, samples = {}, {}, None, 0
    minimum_free = shutil.disk_usage(ROOT).free
    while time.time() < hard_end:
        sampled=time.monotonic(); first_sample=samples==0; samples+=1
        state = json.loads((prefix/'state.json').read_text()); live_rows = rows(); selected = {owner}
        selected.update(pid for pid,row in live_rows.items() if str(pid) in observed and row['uid']==observed[str(pid)]['uid'] and row['started']==observed[str(pid)]['started'])
        for _ in live_rows:
            found = {pid for pid,row in live_rows.items() if row['parent'] in selected}
            if found <= selected: break
            selected.update(found)
        live = {str(pid):row for pid,row in live_rows.items() if pid in selected}; observed.update(live)
        tmp = footprint(prefix/'work'); durable = footprint(prefix)-tmp
        other = max(0,footprint(ROOT)-state['source_bytes'])+max(0,footprint(Path(state['git_dir']))-state['git_dir_bytes'])
        new = tmp+durable+other+state.get('new_offset',0)
        values = dict(pids=len(live),rss=sum(r['rss']*1024 for r in live.values()),temporary=tmp,durable=durable,other=other,new_total=new,
                      whole=max(state.get('whole_floor',0),state['source_bytes']+state['deps_bytes']+state['git_bytes']+new-state.get('new_offset',0)))
        for key,value in values.items(): peaks[key]=max(peaks.get(key,0),value)
        free = shutil.disk_usage(ROOT).free; minimum_free=min(free,minimum_free)
        if not reason:
            if any(values[k]>v for k,v in state['limits'].items()): reason='resource limit'
            elif first_sample and free<state['start_floor_bytes']: reason='start disk floor'
            elif free<state['live_floor_bytes']: reason='running disk floor'
            elif time.time()>=work_end and not state.get('work_done'): reason='work deadline'
            if reason and not first_sample:
                stop_owned(observed,{owner,os.getpid()})
                try: os.kill(owner,signal.SIGTERM)
                except ProcessLookupError: pass
        write(prefix/'guard.json',dict(peaks=peaks,reason=reason,minimum_free_bytes=minimum_free,observed_processes=observed,live_pids=list(live),sampled_at=now(),sample_seconds=time.monotonic()-sampled))
        if first_sample and reason: return  # Parent checks this before creating any workload/link.
        if not state['running']: return
        time.sleep(max(0,.2-(time.monotonic()-sampled)))
    stop_owned(observed,{os.getpid()},force=True)

def run(prefix,kind,pin,offer,accepted,work_seconds,total_seconds):
    assert kind == 'source', 'This driver grants no browser/native workload'
    prefix=prefix.resolve()
    start=dt.datetime.fromisoformat(accepted).timestamp(); work_end=start+work_seconds; hard_end=start+total_seconds
    admission=json.loads(os.environ['ADMIN_ADMISSION_JSON'])
    valid_until=dt.datetime.fromisoformat(admission['valid_until_utc'])
    assert valid_until.tzinfo is not None and start<=valid_until.timestamp() and time.time()<=valid_until.timestamp(), 'Offer expired before any validation child'
    assert admission['offer']==offer and admission['accepted_at']==accepted and admission['source_pin']==pin
    assert time.time()<work_end and not prefix.exists()
    p=preflight(); assert p['clean'] and p['pin']==pin and p['free_bytes']>=START_FLOOR_BYTES
    if kind=='actual':
        inputs=[Path(os.environ[name]) for name in ['MCR_IN04_CONFIG','MCR_ADMIN_FRONTEND_HANDOFF','MCR_ADMIN_BACKEND_OBSERVER','MCR_IN04_BACKEND_DESCRIPTOR']]
        assert all(path.is_absolute() and path.is_file() and not path.is_symlink() for path in inputs), 'actual borrowed inputs unavailable'
        p['borrowed_actual_input_bytes']=sum(footprint(path) for path in inputs)
        p['deps_bytes']+=p['borrowed_actual_input_bytes']
    limits=dict(pids=8 if kind=='source' else 32,rss=(3 if kind=='source' else 4)*1024**3,whole=2560*MIB,
                temporary=(80 if kind=='source' else 200)*MIB,durable=24*MIB,other=16*MIB,new_total=(128 if kind=='source' else 240)*MIB)
    if os.environ.get('MCR_CLOCK_SOURCE_STAGE')=='lint-syntax': limits.update(pids=6,rss=1024**3)
    assert p['source_bytes']+p['deps_bytes']+p['git_bytes']<limits['whole']
    assert p['free_bytes']-limits['new_total']>=9*1024**3
    assert admission['retained_new_bytes']>=69820790 and admission['accounted_whole_floor_bytes']>=2254325026
    prefix.mkdir(); (prefix/'work').mkdir(); state={**p,'limits':limits,'running':True,'work_done':False,'new_offset':int(admission['retained_new_bytes']),
      'start_floor_bytes':max(START_FLOOR_BYTES,int(admission.get('start_floor_bytes',START_FLOOR_BYTES))),'live_floor_bytes':LIVE_FLOOR_BYTES,'whole_floor':int(admission.get('accounted_whole_floor_bytes',0))}
    with socket.socket() as s: s.bind(('127.0.0.1',int(os.environ.get('ADMIN_FIXED_PORT','0')))); port=s.getsockname()[1]
    write(prefix/'control.json',dict(offer=offer,kind=kind,pin=pin,accepted_at=accepted,port=port,work_end=work_end,hard_end=hard_end,preflight=p,limits=limits,controls={fn.__name__:hashlib.sha256(inspect.getsource(fn).encode()).hexdigest() for fn in [primary_sample,guard]},admission=json.loads(os.environ.get('ADMIN_ADMISSION_JSON','{}')),
          readset_sha256={str(f.relative_to(ROOT)):hashlib.sha256(f.read_bytes()).hexdigest() for f in [*Path(FRONT/'src').rglob('*'),*Path(ROOT/'contracts').rglob('*'),ROOT/'docs/mcr/frontend/community-clock-runtime.md',FRONT/'tsconfig.json',FRONT/'pnpm-lock.yaml',FRONT/'package.json'] if f.is_file()}))
    write(prefix/'state.json',state)
    watcher=sp.Popen([sys.executable,__file__,'guard',str(prefix),str(os.getpid()),str(work_end),str(hard_end)],env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'})
    child=None; commands=[]; failure=None; created_link=False; primary_observed={}
    # No workload child until the independent guardian has completed its first sample.
    while not (prefix/'guard.json').exists():
        assert watcher.poll() is None and time.time()<work_end, 'guardian unavailable before workload'
        time.sleep(.02)
    def interrupt(_s,_f): raise RuntimeError('independent guard stopped round')
    signal.signal(signal.SIGTERM,interrupt)
    try:
        assert json.loads((prefix/'guard.json').read_text())['reason'] is None, 'guardian rejected before workload'
        assert not (FRONT/'docs/mcr/evidence/community-clock-runtime').exists(), 'Misdirected owned cache requires cleanup before workload'
        primary_sample(prefix,state,primary_observed,work_end)
        for lock in [FRONT/'pnpm-lock.yaml',DEPS.parent/'pnpm-lock.yaml']: assert hashlib.sha256(lock.read_bytes()).hexdigest()==LOCK
        assert not (FRONT/'node_modules').exists() and not (FRONT/'node_modules').is_symlink()
        (FRONT/'node_modules').symlink_to(DEPS,target_is_directory=True);created_link=True
        env={**os.environ,'PATH':str(NODE.parent)+':'+os.environ['PATH'],'TMPDIR':str(prefix/'work'),'TMP':str(prefix/'work'),'TEMP':str(prefix/'work'),
             'XDG_CACHE_HOME':str(prefix/'work/cache'),'NODE_COMPILE_CACHE':str(prefix/'work/node-compile-cache'),'NODE_DISABLE_COMPILE_CACHE':'1','MCR_COMMUNITY_CLOCK_TEMP':str(prefix/'work'),'ADMIN_TMP':str(prefix/'work'),'ADMIN_OUTPUT':str(prefix/'work/native'),'ADMIN_PORT':str(port),'CI':'true',
             'RAYON_NUM_THREADS':'1','PYTHONDONTWRITEBYTECODE':'1','ADMIN_WORK_END_EPOCH_MS':str(int(work_end*1000))}
        assert sp.check_output([str(NODE),'--version'],env=env,text=True).strip()=='v24.22.2'
        cmds=[[str(NODE),str(DEPS/'vitest/vitest.mjs'),'run','--config',PREFIX+'/vitest.config.ts','--configLoader','runner','--pool=threads','--cache=false','--no-file-parallelism','--maxWorkers=1'],
              [str(NODE),str(DEPS/'typescript/bin/tsc'),'--noEmit','--incremental','false','--allowImportingTsExtensions','--pretty','false'],
              [str(DEPS/'.bin/oxlint'),'--deny-warnings','src/app/community-clock-runtime','src/app/community-adoption/bootstrap.ts','src/app/community-adoption/__tests__/fixtures.ts','src/app/ports.ts'],
              [sys.executable,str(Path(__file__)),'syntax']] if kind=='source' else [[str(NODE),PREFIX+'/native.mjs']]
        if kind=='source' and os.environ.get('MCR_CLOCK_SOURCE_STAGE')=='remaining': cmds=cmds[1:]
        if os.environ.get('MCR_CLOCK_SOURCE_STAGE')=='lint-syntax': cmds=cmds[2:]
        primary_sample(prefix,state,primary_observed,work_end)
        for i,cmd in enumerate(cmds):
            primary_sample(prefix,state,primary_observed,work_end)
            with (prefix/f'{i+1}.log').open('wb') as log:
                child=sp.Popen(cmd,cwd=FRONT,env=env,stdout=log,stderr=sp.STDOUT,start_new_session=True)
                while child.poll() is None:
                    primary_sample(prefix,state,primary_observed,work_end)
                    try: child.wait(timeout=min(.2,max(.01,work_end-time.time())))
                    except sp.TimeoutExpired: pass
                code=child.returncode
            commands.append(dict(command=cmd,exit_code=code,finished_at=now())); print(json.dumps(commands[-1]),flush=True)
            if code: raise RuntimeError(f'command{i+1} failed')
    except BaseException as error: failure=str(error)
    finally:
        signal.signal(signal.SIGTERM,signal.SIG_IGN);state['work_done']=True;write(prefix/'state.json',state)
        observed=json.loads((prefix/'guard.json').read_text()).get('observed_processes',{}) if (prefix/'guard.json').exists() else {}
        observed.update(primary_observed)
        errors=stop_owned(observed,{os.getpid(),watcher.pid})
        if child:
            try:child.wait(timeout=2)
            except sp.TimeoutExpired:child.kill();child.wait()
        errors+=stop_owned(observed,{os.getpid(),watcher.pid},force=True)
        if (prefix/'work/native').exists():shutil.move(str(prefix/'work/native'),prefix/'native')
        shutil.rmtree(prefix/'work')
        link=FRONT/'node_modules'
        if created_link and link.is_symlink() and link.resolve()==DEPS:link.unlink()
        state['running']=False;write(prefix/'state.json',state);watcher.wait(timeout=max(1,hard_end-time.time()))
        guarded=json.loads((prefix/'guard.json').read_text());failure=failure or guarded['reason'] or ('signal errors' if errors else None)
        sp.run(['lsof','-nP',f'-iTCP:{port}','-sTCP:LISTEN'],stdout=(prefix/'listener.log').open('wb'))
        with socket.socket() as s:s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);s.bind(('127.0.0.1',port));s.listen(1)
        live_now=rows();remaining=[pid for pid,old in observed.items() if int(pid) not in {os.getpid(),watcher.pid} and int(pid) in live_now and live_now[int(pid)]['uid']==old['uid'] and live_now[int(pid)]['started']==old['started']]
        failure=failure or ('owned processes remain' if remaining else None) or ('late total cleanup' if time.time()>hard_end else None)
        misdirected_temp_absent=not (FRONT/'docs/mcr/evidence/community-clock-runtime').exists()
        failure=failure or ('misdirected owned cache remains' if not misdirected_temp_absent else None)
        write(prefix/'release.json',dict(offer=offer,finished_at=now(),commands=commands,failure=failure,signal_errors=errors,remaining_owned=remaining,
              dependency_link_absent=not link.exists() and not link.is_symlink(),own_tmp_removed=not (prefix/'work').exists(),misdirected_temp_absent=misdirected_temp_absent,port=port,rebind=True,guard=guarded))
        print(json.dumps(dict(failure=failure,finished_at=now(),peaks=guarded['peaks'])),flush=True)
    return int(bool(failure))

if __name__=='__main__':
    if sys.argv[1]=='preflight': print(json.dumps(preflight(),indent=2))
    elif sys.argv[1]=='guard': guard(Path(sys.argv[2]),int(sys.argv[3]),float(sys.argv[4]),float(sys.argv[5]))
    elif sys.argv[1]=='syntax':
        for file in Path(__file__).parent.glob('*.py'): ast.parse(file.read_text())
    else: raise SystemExit(run(Path(sys.argv[1]),sys.argv[2],sys.argv[3],sys.argv[4],sys.argv[5],int(sys.argv[6]),int(sys.argv[7])))
