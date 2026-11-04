import os,time,json,signal,subprocess,shutil,hashlib
from pathlib import Path
from datetime import datetime,timezone
r=Path.cwd(); out=r/'docs/mcr/evidence/css-parity-customize'; temp=Path('/private/tmp/mcr-css-customize-native-02'); link=r/'frontend/node_modules'; dep=Path('/Users/lhs/Desktop/mcr/frontend/node_modules'); node='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'; start=time.monotonic()
report={'job':'CSS-CUSTOMIZE-FOREGROUND-FIRST-202611041153Z','startedAt':datetime.now(timezone.utc).isoformat(),'stages':[],'processes':{},'peaks':{'temp':0,'durable':0,'whole':0},'minFree':shutil.disk_usage(r).free,'violations':[]}
assert not os.path.lexists(link) and not temp.exists(); assert report['minFree']>=16*2**30
os.symlink(dep,link); env=os.environ.copy();env['PATH']=str(Path(node).parent)+':'+env['PATH'];env['MCR_CUSTOMIZE_JOB']=report['job'];env['PYTHONDONTWRITEBYTECODE']='1';env['MCR_CUSTOMIZE_ROUND']='native-02'
# Vite runner is mandatory in the prepared native driver. No borrowed config bundler output.
assert "configLoader:'runner'" in (out/'native.mjs').read_text()
def size(p):return sum(f.stat().st_size for f in p.rglob('*') if f.is_file()) if p.exists() else 0
def guard():
    t=size(temp);d=size(out);source=(r/'frontend/src/features/avatar/whole-character/whole-character.css').stat().st_size;w=t+d+source;free=shutil.disk_usage(r).free
    report['minFree']=min(report['minFree'],free)
    for k,v in [('temp',t),('durable',d),('whole',w)]:report['peaks'][k]=max(report['peaks'][k],v)
    assert free>=8*2**30,'8GiB stop floor';assert t<=100*2**20 and d<=20*2**20 and w<=140*2**20,'byte cap';assert len(list(out.rglob('*.png')))<=11,'PNG cap';assert datetime.now(timezone.utc).isoformat()<'2026-11-04T11:57:11','accepted work deadline'
def collect(p):
    raw=subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,command=']).decode();rows=[]
    for line in raw.splitlines():
        v=line.strip().split(None,2)
        if len(v)==3:rows.append((int(v[0]),int(v[1]),v[2]))
    owned={p.pid};old=-1
    while old!=len(owned):old=len(owned);owned.update(pid for pid,ppid,cmd in rows if ppid in owned)
    for pid,ppid,cmd in rows:
        if pid in owned:report['processes'][str(pid)]={'ppid':ppid,'command':cmd}
commands=[('source-check',['/usr/bin/python3',str(out/'source-check.py')]),('node-syntax',[node,'--check',str(out/'native.mjs')]),('planning',['/usr/bin/python3',str(r/'scripts/mcr/check-plan.py')]),('native',[node,str(out/'native.mjs')])]
p=None
try:
    for name,argv in commands:
        guard();entry={'name':name,'argv':argv,'startedAt':datetime.now(timezone.utc).isoformat()};report['stages'].append(entry)
        with (out/(name+'.log')).open('wb') as log:
            p=subprocess.Popen(argv,cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);entry['pid']=p.pid
            while p.poll() is None:collect(p);guard();time.sleep(.5)
            entry['exitCode']=p.returncode;entry['finishedAt']=datetime.now(timezone.utc).isoformat();p=None
        print(json.dumps(entry),flush=True)
        if entry['exitCode']!=0:report['violations'].append(name+' failed');break
except Exception as e:
    report['violations'].append(str(e))
finally:
    if p and p.poll() is None:
        os.killpg(p.pid,signal.SIGTERM)
        try:p.wait(timeout=10)
        except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=5)
    # Kill only observed owned descendants if still alive with identical command text.
    for key,record in reversed(list(report['processes'].items())):
        pid=int(key)
        try:
            cmd=subprocess.check_output(['/bin/ps','-p',str(pid),'-o','command='],stderr=subprocess.DEVNULL).decode().strip()
            if cmd==record['command']:os.kill(pid,signal.SIGTERM)
        except (subprocess.CalledProcessError,ProcessLookupError):pass
    if link.is_symlink() and link.resolve()==dep:link.unlink()
    if temp.exists():shutil.rmtree(temp)
    report['cleanup']={'linkAbsent':not os.path.lexists(link),'tempAbsent':not temp.exists()};report['finishedAt']=datetime.now(timezone.utc).isoformat();report['seconds']=time.monotonic()-start
    (out/'supervisor-report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'result':'PASS' if not report['violations'] else 'FAIL','seconds':report['seconds'],'violations':report['violations'],'cleanup':report['cleanup'],'peaks':report['peaks']}),flush=True)
