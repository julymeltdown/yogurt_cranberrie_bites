"""One bounded Wall slot; installed before any Node/browser child is launched."""
import datetime, hashlib, json, os, pathlib, shutil, signal, socket, subprocess, sys, time
ROOT = pathlib.Path(__file__).resolve().parents[4]
OUT = ROOT / 'docs/mcr/evidence/css-parity-wall'
TEMP = pathlib.Path('/tmp/mcr-css-wall-first-20261104')
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
LINK = ROOT / 'frontend/node_modules'
RETAINED = pathlib.Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
ACCEPT = float(sys.argv[1]); CUTOFF = ACCEPT + 240; DEADLINE = ACCEPT + 300
stamp = lambda: datetime.datetime.now(datetime.timezone.utc).isoformat()
def size(path):
    if not path.exists(): return 0
    return sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink())
def sources():
    paths = [ROOT/'frontend/src/features/mural/editor/editor.css', ROOT/'docs/mcr/frontend/css-parity-wall.md', ROOT/'frontend/src/features/mural/editor/MuralEditor.tsx', ROOT/'frontend/src/features/mural/editor/__tests__/reference-target/presentation.test.tsx']
    return sum(p.stat().st_size for p in paths)
receipt = {'offer':'CSS-WALL-FIRST-202611041128Z','acceptedEpoch':ACCEPT,'cutoffEpoch':CUTOFF,'deadlineEpoch':DEADLINE,'startedAt':stamp(),'jobs':[], 'peakTemp':0,'peakDurable':0,'peakWhole':0,'minimumFree':2**63,'ownedPids':[]}
seen = {}; child = None; made_link = False; handle = None

def inventory():
    rows={}
    raw=subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,stat=,command='],text=True)
    for line in raw.splitlines():
        fields=line.strip().split(None,3)
        if len(fields)==4:
            pid,ppid,state,command=fields; rows[int(pid)]=(int(ppid),state,command)
    owned={os.getpid()}
    changed=True
    while changed:
        changed=False
        for pid,(ppid,state,command) in rows.items():
            if pid not in owned and (ppid in owned or str(TEMP) in command and ('Chrome' in command or NODE in command)):
                owned.add(pid);changed=True
    for pid in owned:
        if pid!=os.getpid() and pid in rows: seen[pid]=rows[pid][2]
    return rows

def measure():
    free=shutil.disk_usage(ROOT).free; temp=size(TEMP); durable=size(OUT)+sources(); whole=temp+durable
    receipt['minimumFree']=min(receipt['minimumFree'],free)
    for key,value in [('peakTemp',temp),('peakDurable',durable),('peakWhole',whole)]:receipt[key]=max(receipt[key],value)
    inventory()
    if free<8*1024**3 or temp>300*1024**2 or durable>4*1024**2 or whole>304*1024**2:raise RuntimeError('Resource floor/cap exceeded')

def save():
    receipt['ownedPids']=sorted(seen)
    (OUT/'resource-live.json').write_text(json.dumps(receipt,indent=2)+'\n')

def terminate_owned():
    rows=inventory()
    for sig in [signal.SIGTERM,signal.SIGKILL]:
        for pid,command in list(seen.items()):
            current=rows.get(pid)
            if current and current[2]==command and 'Z' not in current[1]:
                try:os.kill(pid,sig)
                except ProcessLookupError:pass
        if sig==signal.SIGTERM:time.sleep(1)
        rows=inventory()

try:
    assert time.time()<CUTOFF
    assert shutil.disk_usage(ROOT).free>=16*1024**3
    assert not os.path.lexists(LINK), 'Own dependency link must be absent'
    assert not TEMP.exists(), 'Private TMP must be absent'
    assert hashlib.sha256((ROOT/'frontend/pnpm-lock.yaml').read_bytes()).hexdigest()=='47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
    assert (ROOT/'frontend/pnpm-lock.yaml').read_bytes()==(RETAINED.parent/'pnpm-lock.yaml').read_bytes()
    with socket.socket() as probe:probe.bind(('127.0.0.1',5299))
    TEMP.mkdir();LINK.symlink_to(RETAINED,target_is_directory=True);made_link=True
    env=os.environ.copy();env['PATH']=str(pathlib.Path(NODE).parent)+':'+env['PATH'];env['TMPDIR']=str(TEMP)
    env['XDG_CACHE_HOME']=str(TEMP/'xdg-cache')
    jobs=[('baseline',[NODE,str(OUT/'native.mjs')],{'WALL_PARITY_PHASE':'baseline'}),('presentation',[NODE,str(LINK/'vitest/vitest.mjs'),'run','--config',str(OUT/'tests.config.mjs'),'--configLoader','runner'],{}),('candidate-1',[NODE,str(OUT/'native.mjs')],{'WALL_PARITY_PHASE':'candidate-1'})]
    for name,argv,extra in jobs:
        if time.time()>=CUTOFF:raise TimeoutError('Work cutoff reached')
        measure();item={'name':name,'argv':argv,'environment':extra,'startedAt':stamp()};receipt['jobs'].append(item);save()
        with (OUT/(name+'.log')).open('w') as handle:
            child=subprocess.Popen(argv,cwd=ROOT,env={**env,**extra},stdout=handle,stderr=subprocess.STDOUT,start_new_session=True)
            item['pid']=child.pid
            while child.poll() is None:
                measure();save()
                if time.time()>=CUTOFF:raise TimeoutError('Work cutoff reached')
                time.sleep(.2)
            item['exitCode']=child.returncode;item['finishedAt']=stamp();child=None;save()
        print(json.dumps(item),flush=True)
    # Keep the accepted bounded slot usable for owner diagnosis/iteration.
    while time.time()<CUTOFF and not (TEMP/'release').exists():
        request=TEMP/'request.json'
        if request.exists():
            spec=json.loads(request.read_text());request.unlink()
            name=spec['name'];assert name.replace('-','').isalnum()
            extra=spec.get('env',{});argv=spec['argv'];assert argv[0]==NODE
            item={'name':name,'argv':argv,'environment':extra,'startedAt':stamp()};receipt['jobs'].append(item);save()
            with (OUT/(name+'.log')).open('w') as handle:
                child=subprocess.Popen(argv,cwd=ROOT,env={**env,**extra},stdout=handle,stderr=subprocess.STDOUT,start_new_session=True);item['pid']=child.pid
                while child.poll() is None:
                    measure();save()
                    if time.time()>=CUTOFF:raise TimeoutError('Work cutoff reached')
                    time.sleep(.2)
                item['exitCode']=child.returncode;item['finishedAt']=stamp();child=None;save()
            print(json.dumps(item),flush=True)
        measure();save();time.sleep(.2)
except BaseException as error:
    receipt['failure']=repr(error);print(repr(error),flush=True)
finally:
    terminate_owned()
    if child:
        try:child.wait(timeout=2)
        except subprocess.TimeoutExpired:pass
    measure_error=None
    try:measure()
    except BaseException as e:measure_error=repr(e)
    receipt['finalMeasurementError']=measure_error
    if made_link and LINK.is_symlink() and LINK.resolve()==RETAINED.resolve():LINK.unlink()
    if TEMP.exists():shutil.rmtree(TEMP)
    rows=inventory();live=[pid for pid,command in seen.items() if pid in rows and rows[pid][2]==command and 'Z' not in rows[pid][1]]
    try:
        with socket.socket() as probe:probe.bind(('127.0.0.1',5299))
        port_free=True
    except OSError:port_free=False
    receipt.update({'finishedAt':stamp(),'elapsedSeconds':time.time()-ACCEPT,'liveOwnedPids':live,'port5299Free':port_free,'ownLinkRemoved':not os.path.lexists(LINK),'tempRemoved':not TEMP.exists(),'releasedBeforeDeadline':time.time()<=DEADLINE})
    (OUT/'resource-release.json').write_text(json.dumps(receipt,indent=2)+'\n')
    (OUT/'resource-live.json').unlink(missing_ok=True)
    print(json.dumps({'FULLRELEASE':receipt}),flush=True)
