"""One explicitly allocated FE39 source/native round; never allocates capacity itself.
Reuses the previously qualified identity-safe process/footprint helpers read-only.
"""
from pathlib import Path
import ast
import datetime as dt
import importlib.util
import hashlib
import json
import os
import shutil
import signal
import socket
import subprocess as sp
import sys
import time

ROOT = Path(__file__).resolve().parents[6]
FRONT = ROOT / 'frontend'
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
BASE = 'ff96d53eb817c93dd1bafb5c4f125630f995413b'
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
PREFIX = 'src/app/admin-entry/mural-closure/__tests__'
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

def now():
    return dt.datetime.now(dt.UTC).isoformat()

def git(*args):
    return sp.check_output(['git', *args], cwd=ROOT, text=True).strip()

def preflight():
    source = footprint(ROOT)
    private_git = Path(git('rev-parse', '--git-dir'))
    objects = '\n'.join(line.split()[0] for line in git('rev-list', '--objects', f'{BASE}..HEAD').splitlines())+'\n'
    sizes = sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize:disk)'], input=objects, text=True, cwd=ROOT)
    charged_git = footprint(private_git)+sum(int(v) for v in sizes.splitlines() if v.isdigit())
    return dict(at=now(), pin=git('rev-parse','HEAD'), clean=not git('status','--porcelain'), source_bytes=source,
                deps_bytes=footprint(DEPS)+sum(footprint(path) for path in helper.OLD), git_bytes=charged_git, git_dir=str(private_git),
                git_dir_bytes=footprint(private_git), free_bytes=shutil.disk_usage(ROOT).free)

def guard(prefix, owner, work_end, hard_end):
    observed, peaks, reason = {}, {}, None
    minimum_free = shutil.disk_usage(ROOT).free
    while time.time() < hard_end:
        sampled=time.monotonic()
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
                      whole=state['source_bytes']+state['deps_bytes']+state['git_bytes']+new-state.get('new_offset',0))
        for key,value in values.items(): peaks[key]=max(peaks.get(key,0),value)
        free = shutil.disk_usage(ROOT).free; minimum_free=min(free,minimum_free)
        if not reason:
            if any(values[k]>v for k,v in state['limits'].items()): reason='resource limit'
            elif free<8*1024**3: reason='running disk floor'
            elif time.time()>=work_end and not state.get('work_done'): reason='work deadline'
            if reason:
                stop_owned(observed,{owner,os.getpid()})
                try: os.kill(owner,signal.SIGTERM)
                except ProcessLookupError: pass
        write(prefix/'guard.json',dict(peaks=peaks,reason=reason,minimum_free_bytes=minimum_free,observed_processes=observed,live_pids=list(live),sampled_at=now(),sample_seconds=time.monotonic()-sampled))
        if not state['running']: return
        time.sleep(max(0,.2-(time.monotonic()-sampled)))
    stop_owned(observed,{os.getpid()},force=True)

def run(prefix,kind,pin,offer,accepted,work_seconds,total_seconds):
    start=dt.datetime.fromisoformat(accepted).timestamp(); work_end=start+work_seconds; hard_end=start+total_seconds
    assert time.time()<work_end and not prefix.exists()
    p=preflight(); assert p['clean'] and p['pin']==pin and p['free_bytes']>=12*1024**3
    limits=dict(pids=8 if kind=='source' else 32,rss=(3 if kind=='source' else 4)*1024**3,whole=2560*MIB,
                temporary=(80 if kind=='source' else 200)*MIB,durable=24*MIB,other=16*MIB,new_total=(128 if kind=='source' else 240)*MIB)
    assert p['source_bytes']+p['deps_bytes']+p['git_bytes']<limits['whole']
    assert p['free_bytes']-limits['new_total']>=9*1024**3
    prefix.mkdir(); (prefix/'work').mkdir(); state={**p,'limits':limits,'running':True,'work_done':False,'new_offset':int(os.environ.get('FE39_RETAINED_NEW_BYTES','0'))}
    with socket.socket() as s: s.bind(('127.0.0.1',int(os.environ.get('FE39_FIXED_PORT','0')))); port=s.getsockname()[1]
    write(prefix/'control.json',dict(offer=offer,kind=kind,pin=pin,accepted_at=accepted,port=port,work_end=work_end,hard_end=hard_end,preflight=p,limits=limits,admission=json.loads(os.environ.get('FE39_ADMISSION_JSON','{}')),
          readset_sha256={str(f.relative_to(ROOT)):hashlib.sha256(f.read_bytes()).hexdigest() for f in Path(__file__).parent.parent.rglob('*') if f.is_file()}))
    write(prefix/'state.json',state)
    watcher=sp.Popen([sys.executable,__file__,'guard',str(prefix),str(os.getpid()),str(work_end),str(hard_end)],env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'})
    child=None; commands=[]; failure=None; created_link=False
    # No workload child until the independent guardian has completed its first sample.
    while not (prefix/'guard.json').exists():
        assert watcher.poll() is None and time.time()<work_end, 'guardian unavailable before workload'
        time.sleep(.02)
    def interrupt(_s,_f): raise RuntimeError('independent guard stopped round')
    signal.signal(signal.SIGTERM,interrupt)
    try:
        for lock in [FRONT/'pnpm-lock.yaml',DEPS.parent/'pnpm-lock.yaml']: assert hashlib.sha256(lock.read_bytes()).hexdigest()==LOCK
        assert not (FRONT/'node_modules').exists() and not (FRONT/'node_modules').is_symlink()
        (FRONT/'node_modules').symlink_to(DEPS,target_is_directory=True);created_link=True
        env={**os.environ,'PATH':str(NODE.parent)+':'+os.environ['PATH'],'TMPDIR':str(prefix/'work'),'TMP':str(prefix/'work'),'TEMP':str(prefix/'work'),
             'XDG_CACHE_HOME':str(prefix/'work/cache'),'FE39_TMP':str(prefix/'work'),'FE39_OUTPUT':str(prefix/'work/native'),'FE39_PORT':str(port),'CI':'true',
             'RAYON_NUM_THREADS':'1','PYTHONDONTWRITEBYTECODE':'1'}
        assert sp.check_output([str(NODE),'--version'],env=env,text=True).strip()=='v24.22.2'
        cmds=[[str(NODE),str(DEPS/'vitest/vitest.mjs'),'run','--config',PREFIX+'/vitest.config.ts'],
              [str(NODE),str(DEPS/'typescript/bin/tsc'),'--noEmit','--incremental','false','--allowImportingTsExtensions','--pretty','false'],
              [str(DEPS/'.bin/oxlint'),'--deny-warnings','src/app/admin-entry/mural-closure','src/features/admin/mural/transport.ts'],
              [str(NODE),'--check',PREFIX+'/native.mjs']] if kind=='source' else [[str(NODE),PREFIX+'/native.mjs']]
        if kind=='source' and os.environ.get('FE39_SOURCE_STAGE')=='remaining': cmds=cmds[1:]
        for i,cmd in enumerate(cmds):
            with (prefix/f'{i+1}.log').open('wb') as log:
                child=sp.Popen(cmd,cwd=FRONT,env=env,stdout=log,stderr=sp.STDOUT,start_new_session=True)
                code=child.wait(timeout=max(.1,work_end-time.time()))
            commands.append(dict(command=cmd,exit_code=code,finished_at=now())); print(json.dumps(commands[-1]),flush=True)
            if code: raise RuntimeError(f'command{i+1} failed')
    except BaseException as error: failure=str(error)
    finally:
        signal.signal(signal.SIGTERM,signal.SIG_IGN);state['work_done']=True;write(prefix/'state.json',state)
        observed=json.loads((prefix/'guard.json').read_text()).get('observed_processes',{}) if (prefix/'guard.json').exists() else {}
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
        write(prefix/'release.json',dict(offer=offer,finished_at=now(),commands=commands,failure=failure,signal_errors=errors,remaining_owned=remaining,
              dependency_link_absent=not link.exists() and not link.is_symlink(),own_tmp_removed=not (prefix/'work').exists(),port=port,rebind=True,guard=guarded))
        print(json.dumps(dict(failure=failure,finished_at=now(),peaks=guarded['peaks'])),flush=True)
    return int(bool(failure))

if __name__=='__main__':
    if sys.argv[1]=='preflight': print(json.dumps(preflight(),indent=2))
    elif sys.argv[1]=='guard': guard(Path(sys.argv[2]),int(sys.argv[3]),float(sys.argv[4]),float(sys.argv[5]))
    elif sys.argv[1]=='syntax': ast.parse(Path(__file__).read_text())
    else: raise SystemExit(run(Path(sys.argv[1]),sys.argv[2],sys.argv[3],sys.argv[4],sys.argv[5],int(sys.argv[6]),int(sys.argv[7])))
