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
started,started_usec=struct.unpack_from('=QQ',info.raw,120)
birth=' '.join(time.strftime('%a %b %e %H:%M:%S %Y',time.localtime(started)).split())
executable=ctypes.create_string_buffer(4096)
libproc.proc_pidpath.argtypes=[ctypes.c_int,ctypes.c_void_p,ctypes.c_uint32]
libproc.proc_pidpath.restype=ctypes.c_int
assert libproc.proc_pidpath(os.getpid(),executable,len(executable)) > 0
libc=ctypes.CDLL(None,use_errno=True)
mib=(ctypes.c_int*3)(1,49,os.getpid())  # CTL_KERN, KERN_PROCARGS2, this PID only.
length=ctypes.c_size_t()
assert libc.sysctl(mib,3,None,ctypes.byref(length),None,0)==0
arguments=ctypes.create_string_buffer(length.value)
assert libc.sysctl(mib,3,arguments,ctypes.byref(length),None,0)==0
raw=arguments.raw[:length.value]; argc=struct.unpack_from('=i',raw)[0]
position=raw.index(b'\0',4)+1
while raw[position]==0: position+=1
argv=[]
for _ in range(argc):
    end=raw.index(b'\0',position);argv.append(raw[position:end].decode());position=end+1
assert argc==3 and argv[1]=='-B' and argv[2]==str(Path(__file__).resolve())
observation=dict(actualpid=os.getpid(),pid=os.getpid(),parentPid=os.getppid(),groupPid=os.getpgrp(),birth=birth,
    birthSec=started,birthUsec=started_usec,birthEpoch=started+started_usec/1000000,
    cwd=str(ROOT),script=str(Path(__file__).resolve()),scriptSHA256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    executable=executable.value.decode(),
    argv=argv,commandDigest=hashlib.sha256(json.dumps(argv,separators=(',',':'),ensure_ascii=True).encode()).hexdigest(),
    commandDigestEncoding='sha256:kern-procargs2-argv-json-utf8-v1',
    role='ordinary-source-pin-preparation-no-runtime',booking='PARADISECITY-PI11-PREP64-0114',observedAt=time.time())
history=OUT/'ordinary-preparation';history.mkdir(exist_ok=True)
stem=f'{os.getpid()}-{started}-{started_usec}'
begin=history/(stem+'-begin.json');ended=history/(stem+'-end.json')
begin.write_text(json.dumps(observation,indent=2)+'\n')
activity=dict(ownerThreadId=record['ownerThreadId'],offerId=observation['booking'],kind='ordinary',
    activityState='ACCEPTED',phase='SOURCE_PIN_PREPARATION',identity=observation,
    sameCumulativeAllowanceBytes=64*1024**2,expiryEpoch=datetime.datetime(2026,11,5,6,30,tzinfo=datetime.timezone.utc).timestamp(),
    accounting='Existing SAME PREP64 only; no new allowance, refund or reset',
    beginReceipt=str(begin),beginSHA256=hashlib.sha256(begin.read_bytes()).hexdigest(),endReceipt=str(ended))
def publish():
    temporary=RECEIPT.with_name('.'+RECEIPT.name+'.'+str(os.getpid())+'.tmp')
    temporary.write_text(json.dumps(record,indent=2)+'\n');temporary.replace(RECEIPT)
record['ordinaryObservation']=observation
record.setdefault('ordinaryActivities',[]).append(activity)
publish()
observation['canonicalReplaceReturnedAt']=time.time();publish()
outcome=1
try:
    assert birth == ' '.join(subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart='],text=True).split())
    assert observation['executable'] == subprocess.check_output(['ps','-p',str(os.getpid()),'-o','comm='],text=True).strip()
    assert time.time()<datetime.datetime(2026,11,5,6,30,tzinfo=datetime.timezone.utc).timestamp()
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
    observation['endedAt']=time.time();observation['exit']=outcome
    activity.update(activityState='FULL',endedAt=observation['endedAt'],exit=outcome)
    ended.write_text(json.dumps(activity,indent=2)+'\n')
    activity['endSHA256']=hashlib.sha256(ended.read_bytes()).hexdigest();publish()
