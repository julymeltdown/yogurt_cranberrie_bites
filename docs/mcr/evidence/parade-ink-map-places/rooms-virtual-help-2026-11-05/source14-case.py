import ctypes,fcntl,hashlib,json,os
from pathlib import Path
import membership as m
P=Path(__file__).resolve().parent
with (P/'source14/sampling.lock').open('a') as lock:
 fcntl.flock(lock,fcntl.LOCK_EX)
 c=m.identity(os.getpid(),Path(__file__).resolve());argv=m.kernel_argv(os.getpid());digest=hashlib.sha256(json.dumps(argv,ensure_ascii=False,separators=(',', ':')).encode()).hexdigest()
 raw=ctypes.create_string_buffer(4096);assert ctypes.CDLL('/usr/lib/libproc.dylib').proc_pidinfo(os.getpid(),3,0,raw,4096)>=136
 c.update(argv=argv,actualArgv=argv,argvEncoding='json-string-array-utf8',commandEncoding='json-string-array-utf8',argvSHA256=digest,commandDigest=digest,scriptSHA256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),processGroup=os.getpgrp(),birthEpochSeconds=ctypes.c_uint64.from_buffer(raw,120).value,birthMicroseconds=ctypes.c_uint64.from_buffer(raw,128).value)
 row=m.processes()[os.getpid()]
 for record in [dict(ordinaryObservation=c),dict(ordinaryObservations=[c]),dict(ordinaryActivity=dict(controller=c))]:
  controls=m.ordinary_controls(record);assert len(controls)==1;m.verify_controller(controls[0],row,ordinary=True)
 bad=dict(c,commandDigest='0'*64)
 try:m.verify_typed_vector(bad,os.getpid());raise RuntimeError('wrong vector hash accepted')
 except AssertionError as error:assert 'vector digest mismatch' in str(error)
 print('PASS ONE changed multi-record/kernel-vector case: all three typed locations actual kernel image/cwd/script/parent/group/birth/argv; wrong digest rejects',flush=True)
