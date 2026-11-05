"""Ordinary metadata/pin preparation only. No application, compiler, test or browser."""
import ctypes
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import struct
import sys
import time

ROOT=Path(__file__).resolve().parents[4]
OUT=Path(__file__).resolve().parent
RECEIPT=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a5d9-332b-7822-a8f7-40fde6c81bb4.json')
record=json.loads(RECEIPT.read_text())
assert record['state'] in ('FULL','DECLINED'), 'ordinary prep cannot replace an active workflow'
assert sys.platform == 'darwin', 'ordinary identity requires the actual Darwin kernel'
libproc=ctypes.CDLL('/usr/lib/libproc.dylib',use_errno=True)
libproc.proc_pidinfo.argtypes=[ctypes.c_int,ctypes.c_int,ctypes.c_uint64,ctypes.c_void_p,ctypes.c_int]
libproc.proc_pidinfo.restype=ctypes.c_int
info=ctypes.create_string_buffer(136)
assert libproc.proc_pidinfo(os.getpid(),3,0,info,len(info)) == len(info)
started=struct.unpack_from('=Q',info.raw,120)[0]
birth=' '.join(time.strftime('%a %b %e %H:%M:%S %Y',time.localtime(started)).split())
executable=ctypes.create_string_buffer(4096)
libproc.proc_pidpath.argtypes=[ctypes.c_int,ctypes.c_void_p,ctypes.c_uint32]
libproc.proc_pidpath.restype=ctypes.c_int
assert libproc.proc_pidpath(os.getpid(),executable,len(executable)) > 0
observation=dict(actualpid=os.getpid(),birth=birth,cwd=str(ROOT),script=str(Path(__file__).resolve()),
    executable=executable.value.decode(),
    role='ordinary-source-pin-preparation-no-runtime',booking='PARADISECITY-PI11-PREP64-0114',observedAt=time.time())
def publish():
    temporary=RECEIPT.with_name('.'+RECEIPT.name+'.'+str(os.getpid())+'.tmp')
    temporary.write_text(json.dumps(record,indent=2)+'\n');temporary.replace(RECEIPT)
record['ordinaryObservation']=observation;publish()
outcome=1
try:
    assert birth == ' '.join(subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart='],text=True).split())
    assert observation['executable'] == subprocess.check_output(['ps','-p',str(os.getpid()),'-o','comm='],text=True).strip()
    assert time.time()<datetime.datetime(2026,11,5,5,0,tzinfo=datetime.timezone.utc).timestamp()
    v=os.statvfs(ROOT);free=v.f_bavail*v.f_frsize
    assert free>=8*1024**3+5*1024**3+64*1024**2
    paths=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=ROOT).decode().split('\0')
    pins=[]
    for name in sorted(set(paths)):
        p=ROOT/name
        if name and p.is_file() and not p.is_symlink() and (name.startswith(('frontend/','contracts/')) or name.startswith('docs/mcr/evidence/paradise-city-map/') and name.endswith(('.py','.mjs'))):
            data=p.read_bytes();pins.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
    node=Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
    pins.append(dict(path=str(node),bytes=node.stat().st_size,sha256=hashlib.sha256(node.read_bytes()).hexdigest()))
    prior=6642925568
    for history in OUT.glob('*-controller.jsonl'):
        for line in history.read_text().splitlines(): prior=max(prior,json.loads(line).get('peak',{}).get('wholeBytes',0))
    result=dict(task='PARADISECITY-MAP-FRIENDS-NOTES',base='5e28dc35dda0126ca5463f810f06eda8a56dc38f',classification='UNQUALIFIED_FROZEN_CONSUMER_CANDIDATE',
        priorWholeHighWaterBytes=prior,sourcePins=pins,spatialTypeInput=dict(commit='4f94914d06d54c3b465930cc23d40be91a1255cb',pinsDeclaredIn='docs/mcr/evidence/paradise-city-map/typecheck.mjs',classification='ACTUAL_IMMUTABLE_PRODUCER_INPUT_NOT_PRODUCER_ACCEPTANCE'))
    result['pins']=result.pop('sourcePins')
    target=OUT/'readset.json';target.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(dict(at=time.time(),readsetSHA256=hashlib.sha256(target.read_bytes()).hexdigest(),pins=len(pins),freeBytes=free)))
    outcome=0
finally:
    observation['endedAt']=time.time();observation['exit']=outcome;publish()
