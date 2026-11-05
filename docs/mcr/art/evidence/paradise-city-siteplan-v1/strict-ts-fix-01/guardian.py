"""Independent complete initial/live enforcement; no test workload."""
import json
import os
from pathlib import Path
import sys
import time
from safety import Meter,atomic

c=json.loads(Path(sys.argv[1]).read_text());control=Path(c['temp'])/'control.json';state=Path(c['temp'])/'state.json'
log=Path(c['guardianLog']);meter=None;status='FAIL';failure=None
try:
 meter=Meter(c)
 while True:
  command=json.loads(control.read_text());sample=meter.check(command['workActive'])
  atomic(state,dict(ok=True,at=time.time(),peak=meter.peak,minimum=meter.minimum,census=sample))
  if command.get('shutdown'):break
  time.sleep(.15)
 status='PASS'
except BaseException as error:
 failure=repr(error);atomic(state,dict(ok=False,at=time.time(),failure=failure))
finally:
 log.write_text(json.dumps(dict(event='FULL',status=status,at=time.time(),pid=os.getpid(),failure=failure,peak=meter.peak if meter else None,samples=meter.samples if meter else 0,lastCensus=meter.last if meter else None),indent=2)+'\n')
sys.exit(0 if status=='PASS' else 1)
