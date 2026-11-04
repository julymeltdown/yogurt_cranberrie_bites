"""Separate independent prechild/live capacity guardian; no work execution."""
from pathlib import Path
import ctypes
import hashlib
import json
import os
import shutil
import signal
import struct
import sys
import threading
import time

ROOT=Path(__file__).resolve().parents[5]
OUT=Path(__file__).resolve().parent
PACK=ROOT/'frontend/public/world/map-lawns-derived-v3'
TESTS=ROOT/'frontend/src/world/reference-scene/map-lawns-derived-v3'
DOC=ROOT/'docs/mcr/art/map-lawns-derived-v3.md'
GIT=Path('/Users/lhs/Desktop/mcr/.git')
RUNTIME=Path('/Library/Frameworks/Python.framework/Versions/3.14')
NODE=Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
PYTHON=str(RUNTIME/'bin/python3')
MIB=1024**2;GIB=1024**3
WHOLE_CAP=4*GIB;NEW_CAP=64*MIB;DURABLE_CAP=8*MIB;TEMP_CAP=4*MIB
RSS_CAP=512*MIB;PROCESS_CAP=4;PRIOR_WHOLE_FLOOR=2304045500
ROOTS=(ROOT,GIT,RUNTIME,NODE)
LIB=ctypes.CDLL('/usr/lib/libproc.dylib')
LIB.proc_pidinfo.argtypes=[ctypes.c_int,ctypes.c_int,ctypes.c_uint64,ctypes.c_void_p,ctypes.c_int]
LIB.proc_listchildpids.argtypes=[ctypes.c_int,ctypes.c_void_p,ctypes.c_int]


def inventory(roots):
    """Logical file/symlink bytes deduplicated by device/inode; never follow links."""
    result={}
    def add(path):
        try:s=os.lstat(path)
        except FileNotFoundError:return
        key=f'{s.st_dev}:{s.st_ino}';result[key]=max(result.get(key,0),s.st_size)
    for root in roots:
        if root.is_file() or root.is_symlink():add(root);continue
        for base,dirs,files in os.walk(root):
            for name in files:add(os.path.join(base,name))
            for name in dirs:
                path=os.path.join(base,name)
                if os.path.islink(path):add(path)
    return result


def size(path):return sum(inventory((path,)).values())
def pin(path):
    raw=path.read_bytes()
    return {'path':str(path.relative_to(ROOT)),'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}


def children(pid):
    buffer=(ctypes.c_int*128)()
    count=LIB.proc_listchildpids(pid,buffer,ctypes.sizeof(buffer))
    # Apple's wrapper already divides proc_listpids bytes by sizeof(int).
    assert 0<=count<len(buffer),'cannot bound process tree'
    return {p for p in list(buffer)[:count] if p>0}


def resident(pid,worker=None):
    # A dying task can disappear before its PID is reaped. The primary reaps
    # its known Popen; the independent sibling observes that transition for
    # at most 120ms. A genuinely unreadable live task still fails closed.
    deadline=time.monotonic()+.12
    while True:
        data=ctypes.create_string_buffer(96)
        result=LIB.proc_pidinfo(pid,4,0,data,96)
        if result:
            assert result==96,'unexpected Darwin taskinfo layout'
            return struct.unpack_from('Q',data.raw,8)[0]
        if worker is not None and worker.pid==pid and worker.poll() is not None:return 0
        bsd=ctypes.create_string_buffer(136);got=LIB.proc_pidinfo(pid,3,0,bsd,136)
        if got==136 and struct.unpack_from('I',bsd.raw,4)[0]==5:return 0
        try:os.kill(pid,0)
        except ProcessLookupError:return 0
        if time.monotonic()>=deadline:raise RuntimeError(f'unreadable live RSS pid={pid} taskBytes={result} bsdBytes={got}')
        time.sleep(.005)


class Accounting:
    def __init__(self,request):
        self.request=request;self.high=dict(request['baselineInodes'])
        self.peak={'whole':request['carry'],'new':0,'tmp':0,'rss':0,'processes':0,'durable':0}
        self.minimum_free=shutil.disk_usage(ROOT).free;self.samples=0;self.last=time.monotonic();self.max_interval=0
        self.worker=None
        self.lock=threading.Lock();self.stop_scan=threading.Event();self.first_scan=threading.Event()
        self.growth=0;self.scan_error=None;self.scan_finished=self.last;self.max_scan_seconds=0
        self.scanner=threading.Thread(target=self.scan,name='inode-accounting',daemon=True);self.scanner.start()

    def scan(self):
        try:
            while not self.stop_scan.is_set():
                began=time.monotonic();current=inventory(ROOTS)
                with self.lock:
                    for key,value in current.items():self.high[key]=max(self.high.get(key,0),value)
                    self.growth=sum(max(0,v-self.request['baselineInodes'].get(k,0)) for k,v in self.high.items())
                    self.scan_finished=time.monotonic();self.max_scan_seconds=max(self.max_scan_seconds,self.scan_finished-began)
                self.first_scan.set();self.stop_scan.wait(.15)
        except BaseException as error:self.scan_error=repr(error);self.first_scan.set()

    def close(self):
        self.stop_scan.set();self.scanner.join(timeout=3)
        assert not self.scanner.is_alive(),'accounting scanner did not stop'

    def sample(self):
        r=self.request;assert time.monotonic()<r['hardEnd'],'original hard deadline'
        assert 0<=r['otherWorkflows']<4 and 0<=r['otherSource']<3 and 0<=r['otherBrowser']<=2,'prospective GLOBAL4/SOURCE3/browser2 allocation'
        assert 0<=r['otherProcessCaps']<=96-PROCESS_CAP and 0<=r['otherRssCaps']<=18*GIB-RSS_CAP,'sum of active PID/RSS caps'
        free=shutil.disk_usage(ROOT).free;self.minimum_free=min(self.minimum_free,free)
        assert free>=8*GIB and free-r['otherGrowthBytes']-NEW_CAP>=9*GIB,'live/after-all reserve floor'
        assert self.scan_error is None,'inode accounting failed: '+str(self.scan_error)
        with self.lock:growth=self.growth;last_scan=self.scan_finished
        assert time.monotonic()-last_scan<=3,'live inode accounting freshness exceeds 3s'
        tmp=size(Path(r['temp']));new=growth+tmp+2*MIB;whole=r['carry']+new
        durable=size(PACK)+size(TESTS)+size(OUT)+DOC.stat().st_size
        active=children(r['supervisor']);assert len(active)<=2,'more than guardian plus one worker'
        if os.getpid()!=r['supervisor']:assert os.getpid() in active,'independent guardian must be visible in PID inventory'
        for pid in active:assert not children(pid),'unexpected descendant'
        rss=resident(r['supervisor'])+sum(resident(pid,self.worker) for pid in active)
        processes=2+len(active)  # Supervisor + reserved exec wrapper + direct children.
        for key,value in dict(whole=whole,new=new,tmp=tmp,rss=rss,processes=processes,durable=durable).items():self.peak[key]=max(self.peak[key],value)
        assert new<=NEW_CAP and whole<=WHOLE_CAP and tmp<=TEMP_CAP and durable<=DURABLE_CAP,'whole/new/temp/durable cap'
        assert rss<=RSS_CAP and processes<=PROCESS_CAP,'RSS/process cap'
        interval=time.monotonic()-self.last;self.last=time.monotonic();self.max_interval=max(self.max_interval,interval)
        assert interval<=.25,'guardian sampling exceeds 250ms'
        self.samples+=1


def main():
    request_path=Path(sys.argv[1]);assert hashlib.sha256(request_path.read_bytes()).hexdigest()==sys.argv[2],'admission request pin'
    request=json.loads(request_path.read_text());temp=Path(request['temp'])
    result={'status':'FAIL','guardian':os.getpid()};account=Accounting(request)
    try:
        assert request['supervisor']==os.getppid(),'guardian custody'
        for expected in request['sourcePins']:assert pin(ROOT/expected['path'])==expected,'guardian frozen source pin'
        assert shutil.disk_usage(ROOT).free>=11*GIB,'independent prechild floor'
        while not account.first_scan.is_set():account.sample();time.sleep(.06)
        account.sample();(temp/'guardian-ready').write_text(str(os.getpid()))
        while not (temp/'stop').exists():account.sample();time.sleep(.06)
        result['status']='PASS'
    except BaseException as error:
        result['error']=repr(error)
        for pid in children(request['supervisor'])-{os.getpid()}:
            try:os.killpg(pid,signal.SIGKILL)
            except ProcessLookupError:pass
    account.close()
    result.update(peak=account.peak,samples=account.samples,minimumFree=account.minimum_free,maxGuardIntervalSeconds=account.max_interval,maxByteScanSeconds=account.max_scan_seconds)
    (temp/'guardian-result.json').write_text(json.dumps(result,indent=2)+'\n')
    return 0 if result['status']=='PASS' else 1


if __name__=='__main__':sys.exit(main())
