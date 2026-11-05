"""Explicit typed kernel-vector profiles; no path or command-domain aliases."""
from pathlib import Path
import hashlib

PI08='01a1a5d9-6955-7820-bcd4-8e0a21629aa2'
BE='01a1a1fd-6b5d-75b0-8d42-01ade79b9393'
HOST='01a1a1fe-1eed-7d12-a247-879cd521e0ec'
def decode_extended(d,owner):
 if owner==QA06:return decode_qa06(d,owner)
 result=dict(d)
 if owner==PI08 and isinstance(d.get('kernelArgv'),dict):
  k=d['kernelArgv'];assert k['source']=='sysctl CTL_KERN KERN_PROCARGS2' and k['boundedExactlyByArgc'] is True and k['aliasesNormalized'] is False
  vector=k['argv'];assert isinstance(vector,list) and len(vector)==k['argc'] and all(isinstance(v,str) for v in vector)
  assert k['executablePath']==d['executable']
  if 'argv' in d:assert d['argv']==vector
  result['argv']=vector
  if 'processGroupId' in d:
   if 'processGroup' in d:assert d['processGroup']==d['processGroupId']
   result['processGroup']=d['processGroupId']
 if owner==BE and 'kernelArgumentObservation' in d:
  k=d['kernelArgumentObservation'];assert k['source']=='macOS sysctl KERN_PROCARGS2 actual current PID'
  vector=k['argv'];assert isinstance(vector,list) and len(vector)==k['argc'] and all(isinstance(v,str) for v in vector)
  assert k['kernelExecutableArgument']==d['executable']
  if 'argv' in d:assert d['argv']==vector
  result['argv']=vector
 if owner in (PI08,BE,HOST) and 'scriptSha256' in d:
  if 'scriptSHA256' in d:assert d['scriptSHA256']==d['scriptSha256']
  result['scriptSHA256']=d['scriptSha256']
 return result

raw=Path(__file__).with_name('source-control04.py').read_bytes()
assert hashlib.sha256(raw).hexdigest()=='78afe84050d0d428506c3b558b7e085a63729018daf3ad9f02ed7c0709f313f3'
source=raw.decode()
tail="""
decode_qa06=decode_declared;decode_declared=decode_extended
code=code.replace('ART231-PHOTO2-SOURCE04-0657','ART231-PHOTO2-SOURCE06-0715').replace('source04-independent-full.json','source06-independent-full.json')
code=code.replace('positive+6223996<=64*MIB','positive+6223996+16777216+8879309<=64*MIB')
code=code.replace('+6223996+positive<=536870912','+6223996+16777216+8879309+positive<=536870912')
code=code.replace("bind(declared,actual);roots.append", "assert 'argv' in declared and 'scriptSHA256' in declared,('Unrecognized exact receipt schema',row['ownerThreadId'],declared)\\n    bind(declared,actual);roots.append")
code=code.replace("result=census(c)","self.peak['positive']=max(self.peak['positive'],positive);self.peak['whole']=max(self.peak['whole'],whole)\\n  result=census(c)")
code=code.replace('max(4363784902,sum(self.baseline.values()))','max(4389441427,sum(self.baseline.values()))')
interop_qa06=interop_check
def interop_check():
 interop_qa06()
 fixture=dict(pid=123,parentPid=12,birthEpochSeconds=200,birthMicroseconds=30,executable='/exact/Python',script='/exact/round.py',scriptSha256='a'*64,processGroupId=123,kernelArgv=dict(source='sysctl CTL_KERN KERN_PROCARGS2',argc=3,argv=['/exact/Python','-B','/exact/round.py'],executablePath='/exact/Python',boundedExactlyByArgc=True,aliasesNormalized=False))
 assert decode_extended(fixture,PI08)['argv']==fixture['kernelArgv']['argv']
 import copy
 for key,value in [('argv',['/alias/Python']),('scriptSHA256','b'*64),('processGroup',999)]:
  bad=copy.deepcopy(fixture);bad[key]=value
  try:decode_extended(bad,PI08)
  except AssertionError:continue
  raise AssertionError(('Conflicting nested receipt accepted',key))
 print('PASS: exact nested PI08 vector and three conflicting fields')
code=code.replace("if __name__=='__main__':sys.exit(main())",'')
exec(compile(code,__file__,'exec'))
census_once=census
def receipt_revision():
 return [(str(p),sha(p.read_bytes())) for p in sorted((BASE/'owner-resource-receipts').glob('*.json'))]+[(str(BASE/'resource-membership-current.json'),sha((BASE/'resource-membership-current.json').read_bytes()))]
def census(config):
 for attempt in range(3):
  before=receipt_revision()
  try:return census_once(config)
  except AssertionError as e:
   reason=e.args[0] if e.args else None
   eligible=isinstance(reason,tuple) and reason[0] in ('unknown actual MCR roots','accepted root birth unavailable/mismatch')
   if not eligible or before==receipt_revision() or attempt==2:raise
 raise AssertionError('Complete census retry limit')
if __name__=='__main__':sys.exit(main())
"""
exec(compile(source.replace("exec(compile(code,__file__,'exec'))",tail),__file__,'exec'))
