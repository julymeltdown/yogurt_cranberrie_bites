import ctypes,fcntl,json,os,subprocess,sys
from pathlib import Path
import membership as m
P=Path(__file__).resolve().parent
with (P/'source13/sampling.lock').open('a') as lock:
 fcntl.flock(lock,fcntl.LOCK_EX)
 child=subprocess.Popen([sys.executable,'-B','-c','import sys;sys.stdin.buffer.read(1)'],stdin=subprocess.PIPE)
 lib=ctypes.CDLL('/usr/lib/libproc.dylib');raw=ctypes.create_string_buffer(4096)
 assert lib.proc_pidinfo(child.pid,3,0,raw,4096)>=136
 control=dict(birthEpochSeconds=ctypes.c_uint64.from_buffer(raw,120).value,birthMicroseconds=ctypes.c_uint64.from_buffer(raw,128).value)
 child.communicate(b'x');assert child.returncode==0
 original=m._sample_once;calls=[]
 proposal=dict(offerId='FIXTURE_RETAINED_FINITE_CLAIM',acceptedAtEpoch=1234567890,processCap=4,rssCapBytes=536870912,remainingNewUpperBytes=262144)
 frozen=json.dumps(proposal,sort_keys=True)
 try:
  def absent(p):
   calls.append(p);assert p is proposal
   if len(calls)==1:m.verify_numeric_birth(control,child.pid)
   return dict(retainedClaim=dict(p))
  m._sample_once=absent;result=m.sample(proposal)
  assert len(calls)==2 and result['retainedClaim']==proposal and json.dumps(proposal,sort_keys=True)==frozen
  print('PASS actual exited child kernel-read + fresh PS absence retries whole-census fixture; finite claim unchanged',flush=True)
  calls.clear()
  def present(p):
   calls.append(p);raise m.KernelIdentityUnavailable(os.getpid(),(123,456),0)
  m._sample_once=present
  try:m.sample(proposal);raise RuntimeError('still-present error accepted')
  except m.KernelIdentityUnavailable as error:assert error.pid==os.getpid() and len(calls)==1
  print('PASS simulated unavailable read for actual still-present PID rejects without retry',flush=True)
  calls.clear()
  def exhausted(p):
   calls.append(p);m.verify_numeric_birth(control,child.pid)
  m._sample_once=exhausted
  try:m.sample(proposal);raise RuntimeError('exhaustion accepted')
  except AssertionError as error:assert 'exhausted after 3' in str(error) and len(calls)==3 and str(child.pid) in str(error)
  assert json.dumps(proposal,sort_keys=True)==frozen
  print('PASS actual absent-child kernel-read exhaustion is exactly 3 whole-census fixtures, PID context and retained caps/clocks unchanged',flush=True)
 finally:m._sample_once=original
