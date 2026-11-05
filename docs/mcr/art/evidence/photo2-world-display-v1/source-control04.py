"""Source04 typed receipt interoperability; exact kernel binding stays mandatory."""
from pathlib import Path
import hashlib

QA06='01a1a1fe-1ab7-79b0-adbc-d4c025701acb'
def decode_declared(d,owner):
 if owner!=QA06 or 'actualArgv' not in d:return d
 assert d.get('commandEncoding')=='json-string-array-utf8'
 k=d.get('kernelIdentity');assert isinstance(k,dict)
 def same(keys,required=True):
  values=[container[key] for container,key in keys if key in container]
  assert values or not required
  assert not values or all(type(v)==type(values[0]) and v==values[0] for v in values)
  return values[0] if values else None
 sec=same([(d,'birthSeconds'),(d,'birthSec'),(d,'birthEpochSeconds'),(k,'birthSeconds')])
 usec=same([(d,'birthMicroseconds'),(k,'birthMicroseconds')]);assert type(sec) is int and type(usec) is int and 0<=usec<1000000
 pid=same([(d,'pid'),(d,'actualpid'),(k,'pid')]);parent=same([(d,'parentPid'),(d,'ppid'),(k,'ppid')]);assert type(pid) is int and type(parent) is int
 image=same([(d,'executable'),(d,'actualExecutable'),(k,'image')]);assert isinstance(image,str) and image.startswith('/')
 argv=same([(d,'actualArgv'),(d,'argv'),(k,'argv')]);assert isinstance(argv,list) and argv and all(isinstance(v,str) for v in argv)
 group=same([(d,'processGroup'),(k,'pgid')],False)
 result=dict(d,birthEpochSeconds=sec,birthMicroseconds=usec,parentPid=parent,executable=image,argv=argv)
 if group is not None:assert type(group) is int;result['processGroup']=group
 return result

def interop_check():
 import copy
 d=dict(pid=123,parentPid=12,birthSeconds=200,birthMicroseconds=30,executable='/exact/Python',actualArgv=['/exact/Python','-B','/exact/round.py','source07'],commandEncoding='json-string-array-utf8',kernelIdentity=dict(pid=123,ppid=12,birthSeconds=200,birthMicroseconds=30,image='/exact/Python'))
 assert decode_declared(d,QA06)['birthEpochSeconds']==200
 for path,value in [('birthEpochSeconds',201),('actualArgv','not-a-vector'),('argv',['/alias/Python']),('actualExecutable','/alias/Python')]:
  bad=copy.deepcopy(d);bad[path]=value
  try:decode_declared(bad,QA06)
  except AssertionError:continue
  raise AssertionError(('Conflicting typed receipt accepted',path))
 print('PASS: exact QA06 typed mapping plus four conflicting receipt cases')

raw=Path(__file__).with_name('source-control.py').read_bytes()
assert hashlib.sha256(raw).hexdigest()=='3e1aed729b2eeed94ff523f9874b1592a7d7e4848d0d4124dccd42098f2f43fe'
code=raw.decode().replace('ART231-PHOTO2-SOURCE03-0647','ART231-PHOTO2-SOURCE04-0657').replace('source03-independent-full.json','source04-independent-full.json')
code=code.replace("actual=kernel(declared['pid'])", "declared=decode_declared(declared,row['ownerThreadId'])\n    actual=kernel(declared['pid'])")
code=code.replace('max(4357560906,sum(self.baseline.values()))','max(4363784902,sum(self.baseline.values()))')
code=code.replace('positive<=64*MIB','positive+6223996<=64*MIB')
code=code.replace('221435639+37817629+1048576+16777216+67108864+34526193+16777216+positive','221435639+37817629+1048576+16777216+67108864+34788337+17027807+4887797+16777216+6223996+positive')
code=code.replace("if sys.argv[1:2]==['--guardian']:","if sys.argv[1:2]==['--interop-check']:interop_check();return 0\n if sys.argv[1:2]==['--guardian']:")
exec(compile(code,__file__,'exec'))
