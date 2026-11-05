"""Independent pre-child/live enforcer; never executes art or tests."""
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import time
from inventory import Inventory,process_snapshot

config=json.loads(Path(sys.argv[1]).read_text())
root=Path(config['root']);temp=Path(config['temp']);control=Path(config['control']);state=Path(config['state'])
log=Path(config['log']);scope=[Path(p) for p in config['scope']]
inventory=Inventory(config['inventoryRoots'],config['priorCarry'])
peak=dict(whole=inventory.carry,new=config['newCarry'],tmp=0,rss=0,processes=0)
minimum=shutil.disk_usage(root).free;samples=0;status='RUNNING';processes=None
def size(path):
    total=0
    for p in (path.rglob('*') if path.is_dir() else [path]):
        try:
            if p.is_file() and not p.is_symlink():total+=p.stat().st_size
        except FileNotFoundError:pass
    return total
def emit(event,**fields):
    with log.open('a') as handle:handle.write(json.dumps(dict(event=event,at=time.time(),pid=os.getpid(),**fields))+'\n')
def publish(value):
    staging=state.with_suffix('.pending');staging.write_text(json.dumps(value));staging.replace(state)
try:
    free=shutil.disk_usage(root).free
    assert free>=12*1024**3 and free-config['other']-(config['newCap']-config['newCarry'])>=9*1024**3,'Independent admission floor'
    emit('ACCEPT',supervisor=config['supervisor'],inventoryLogical=inventory.logical,carry=inventory.carry,roots=config['inventoryRoots'])
    while True:
        command=json.loads(control.read_text());now=time.time()
        assert now<config['hard'],'Independent original hard deadline'
        if command.get('workActive'):assert now<config['work'],'Independent original work deadline'
        whole,new=inventory.sample();new=config['newCarry']+max(0,new-34*1024**2);tmp=size(temp);durable=sum(size(p) for p in scope)
        free=shutil.disk_usage(root).free;minimum=min(minimum,free)
        assert free>=8*1024**3 and whole<=config['wholeCap'] and new<=config['newCap'],'Independent storage cap'
        assert max(0,durable-config['initialOwn'])<=config['outputCap'],'Independent generated evidence/output cap'
        assert tmp<=4*1024**2 and durable<=8*1024**2,'Independent TMP/durable cap'
        processes=process_snapshot(config['supervisor'],temp/'sampler.lock')
        assert free-max(config['other'],processes['globalCensus']['otherRemainingUpper'])-max(0,config['newCap']-new)>=9*1024**3,'Independent actual aggregate envelope'
        assert processes['processes']<=4 and processes['rss']<=512*1024**2,'Independent process/RSS cap'
        for k,v in dict(whole=whole,new=new,tmp=tmp,**{k:processes[k] for k in ('rss','processes')}).items():peak[k]=max(peak[k],v)
        samples+=1;publish(dict(ok=True,at=time.time(),peak=peak,minimumFree=minimum,samples=samples,**processes))
        if command.get('shutdown'):break
        time.sleep(.15)
    status='PASS'
except BaseException as error:
    status='FAIL';publish(dict(ok=False,at=time.time(),reason=repr(error),peak=peak))
    emit('ERROR',reason=repr(error))
    try:
        command=json.loads(control.read_text());worker=command.get('worker')
        if worker:os.killpg(worker,signal.SIGTERM)
    except (ProcessLookupError,FileNotFoundError):pass
finally:
    emit('FULL',status=status,peak=peak,minimumFree=minimum,samples=samples,lastLiveCensus=processes)
    try:os.kill(config['supervisor'],0)
    except ProcessLookupError:
        # The supervisor normally removes this private root after reaping us.
        # If it vanished, retain our external receipt and clean only our TMP.
        shutil.rmtree(temp,ignore_errors=True)
sys.exit(0 if status=='PASS' else 1)
