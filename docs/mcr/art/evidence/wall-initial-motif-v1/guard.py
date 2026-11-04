"""Independent source-job guardian. Never starts browser/server or installs libs."""
from pathlib import Path
import datetime as dt
import ctypes
import json
import os
import resource
import shutil
import signal
import struct
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parents[5]
OUT=Path(__file__).resolve().parent
GIT=Path('/Users/lhs/Desktop/mcr/.git')
PACK=ROOT/'frontend/public/wall/initial-motif-v1'
DOC=ROOT/'docs/mcr/art/wall-initial-motif-v1.md'
GIB=1024**3;MIB=1024**2
LIBPROC=ctypes.CDLL('/usr/lib/libproc.dylib')
LIBPROC.proc_pidinfo.argtypes=[ctypes.c_int,ctypes.c_int,ctypes.c_uint64,ctypes.c_void_p,ctypes.c_int]


def resident(pid,child=None):
    info=ctypes.create_string_buffer(96)
    count=LIBPROC.proc_pidinfo(pid,4,0,info,96)  # PROC_PIDTASKINFO, no monitor child.
    if count==0:
        if child is not None and child.poll() is not None:return 0
        try:os.kill(pid,0)
        except ProcessLookupError:return 0
        raise RuntimeError('cannot measure live process RSS')
    assert count==96,'unexpected proc_taskinfo layout'
    return struct.unpack_from('Q',info.raw,8)[0]


def size(root):
    total=0
    for base,dirs,files in os.walk(root):
        for name in files:
            try:total+=os.lstat(os.path.join(base,name)).st_size
            except FileNotFoundError:pass
    return total


def main():
    reserve=int(sys.argv[1]);deadline=dt.datetime.fromisoformat(sys.argv[2]);now=dt.datetime.now(dt.timezone.utc)
    assert now<deadline,'offer expired'
    free=shutil.disk_usage(ROOT).free
    assert free>=12*GIB and free-reserve-12*MIB>9*GIB,'admission reserve/free floor'
    baseline_roots={str(p):size(p) for p in [ROOT,GIT]}
    high_water=dict(baseline_roots)
    baseline=sum(baseline_roots.values());start=time.monotonic();work_end=start+120;finish_end=start+180
    record={'job':sys.argv[3],'acceptUtc':now.isoformat(),
            'workDeadlineUtc':(now+dt.timedelta(seconds=120)).isoformat(),'cleanupDeadlineUtc':(now+dt.timedelta(seconds=180)).isoformat(),
            'roots':[str(ROOT),str(GIT)],'privateGitIncludedInSharedGit':True,'tempRoots':[],
            'baselineBytes':baseline,'baselineRootBytes':baseline_roots,'wholeCapBytes':baseline+12*MIB,'otherReservedBytes':reserve,
            'ownWorstCaseGrowthBytes':12*MIB,'startFreeBytes':free,'startAfterWorstCaseReserveBytes':free-reserve-12*MIB,
            'guardianPid':os.getpid(),'workerPids':[],'peakWorkerAndGuardianRssBytes':0,'peakGrowthBytes':0,'minFreeBytes':free,'maxGuardIntervalSeconds':0,'checks':[]}
    prefix='resource-'+record['job'].lower()
    (OUT/(prefix+'-accept.json')).write_text(json.dumps(record,indent=2)+'\n')
    print('ACCEPT '+json.dumps(record),flush=True)
    worker=None
    try:
        assert input('Awaiting explicit dispatch after ACCEPT notice: ').strip()=='GO'
        assert shutil.disk_usage(ROOT).free>=12*GIB and size(ROOT)+size(GIT)<=baseline+12*MIB
        for filename in (sys.argv[4:] or ['author.py','qualify.py']):
            assert filename in ['author.py','qualify.py']
            assert time.monotonic()<work_end
            env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'}
            worker=subprocess.Popen([sys.executable,str(OUT/filename)],cwd=ROOT,env=env,start_new_session=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
            record['workerPids'].append(worker.pid);child_start=time.monotonic()
            last_sample=time.monotonic()
            while True:
                current_free=shutil.disk_usage(ROOT).free
                for p in [ROOT,GIT]:high_water[str(p)]=max(high_water[str(p)],size(p))
                growth=sum(high_water[p]-baseline_roots[p] for p in baseline_roots)
                whole=baseline+growth;durable=size(PACK)+size(OUT)+DOC.stat().st_size
                # macOS libproc avoids a second child competing with the one worker.
                rss=resident(os.getpid())+resident(worker.pid,worker)
                record['peakWorkerAndGuardianRssBytes']=max(record['peakWorkerAndGuardianRssBytes'],rss)
                record['peakGrowthBytes']=max(record['peakGrowthBytes'],whole-baseline)
                record['minFreeBytes']=min(record['minFreeBytes'],current_free)
                interval=time.monotonic()-last_sample;last_sample=time.monotonic()
                record['maxGuardIntervalSeconds']=max(record['maxGuardIntervalSeconds'],interval)
                assert interval<=.25,'guard sampling deadline'
                assert current_free>=8*GIB and current_free-reserve-12*MIB>9*GIB,'live free/reserve floor'
                assert whole<=baseline+12*MIB and durable<=4*MIB and rss<=512*MIB,'byte/RSS cap'
                assert time.monotonic()<work_end and time.monotonic()-child_start<=60,'work deadline'
                if worker.poll() is not None:break
                time.sleep(.1)
            output=worker.communicate()[0];print(output,flush=True)
            assert worker.returncode==0,filename+' failed'
            # Darwin ru_maxrss is bytes; include a fast child's retained peak.
            retained_peak=resident(os.getpid())+resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
            record['peakWorkerAndGuardianRssBytes']=max(record['peakWorkerAndGuardianRssBytes'],retained_peak)
            assert retained_peak<=512*MIB,'retained child peak RSS cap'
            record['checks'].append({'script':filename,'exit':worker.returncode,'output':output.strip()})
        record['result']='PASS'
    except BaseException as exc:
        record['result']='FAIL';record['error']=str(exc)
        if worker is not None and worker.poll() is None:os.killpg(worker.pid,signal.SIGKILL);worker.wait(timeout=10)
        raise
    finally:
        assert time.monotonic()<finish_end
        record['finishUtc']=dt.datetime.now(dt.timezone.utc).isoformat();record['elapsedSeconds']=round(time.monotonic()-start,3)
        record['finalWholeBytes']=size(ROOT)+size(GIT);record['finalDurableBytes']=size(PACK)+size(OUT)+DOC.stat().st_size
        record['tempBytes']=0;record['portsCreated']=[];record['workerIdentitiesAlive']=[]
        for pid in record['workerPids']:
            try:os.kill(pid,0);record['workerIdentitiesAlive'].append(pid)
            except ProcessLookupError:pass
        (OUT/(prefix+'-result.json')).write_text(json.dumps(record,indent=2)+'\n')
        print('FULL '+json.dumps(record),flush=True)


if __name__=='__main__':main()
