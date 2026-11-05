import ctypes,json,os
from pathlib import Path
from membership import identity,birth_text,verify_numeric_birth,verify_controller,processes
pid=os.getpid();control=identity(pid,Path(__file__).resolve());raw=ctypes.create_string_buffer(4096);lib=ctypes.CDLL('/usr/lib/libproc.dylib');assert lib.proc_pidinfo(pid,3,0,raw,4096)>=136
sec=ctypes.c_uint64.from_buffer(raw,120).value;us=ctypes.c_uint64.from_buffer(raw,128).value
numeric=dict(control,birthEpochSeconds=sec,birthMicroseconds=us);numeric.pop('birth')
assert birth_text(numeric)==' '.join(control['birth'].split());verify_numeric_birth(numeric,pid)
verify_controller(numeric,processes()[pid],('new-fixture','numeric-live',1),'source10-live',ordinary=True)
print(json.dumps({'fixture':'numeric_current_kernel_identity','status':'PASS'}),flush=True)
for bad in [dict(numeric,birthEpochSeconds=True),dict(numeric,birthMicroseconds=1000000),dict(numeric,birthMicroseconds=.5)]:
 try:birth_text(bad)
 except AssertionError:pass
 else:raise AssertionError('invalid unit/type accepted')
partial=dict(numeric);partial.pop('birthMicroseconds')
try:birth_text(partial)
except AssertionError:pass
else:raise AssertionError('partial birth accepted')
print(json.dumps({'fixture':'invalid_numeric_unit_types_ranges_partial','status':'PASS'}),flush=True)
for bad in [dict(numeric,birthEpochSeconds=sec+1),dict(numeric,birthMicroseconds=(us+1)%1000000)]:
 try:verify_numeric_birth(bad,pid)
 except AssertionError:pass
 else:raise AssertionError('changed kernel identity accepted')
verify_numeric_birth(numeric,pid);assert birth_text(control)==' '.join(control['birth'].split())
print(json.dumps({'fixture':'stable_kernel_and_seconds_microseconds_mismatch','status':'PASS'}),flush=True)
