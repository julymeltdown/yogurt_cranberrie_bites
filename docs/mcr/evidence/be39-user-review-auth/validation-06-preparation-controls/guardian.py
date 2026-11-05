import json, os, signal, sys, time
sys.dont_write_bytecode = True
from budget import R, Guard, atomic, load_baseline, failure_receipt
p=json.loads((R/'plan.json').read_text())
a=json.loads((R/'acceptance.json').read_text())
g=Guard(p,a,{'roots':{},'inodes':[],'bytes':0})
complete=False; failure=None
try:
    while not (R/'guardian.stop').exists():
        g.cleanup=(R/'cleanup.phase').exists()
        if not complete and (R/'baseline.json.gz').exists():
            g=Guard(p,a,load_baseline());complete=True
        if (R/'freeze-target').exists():g.frozen=True
        value=g.sample() if complete else g.fast()
        atomic('guardian.live.json',{'pid':os.getpid(),'sample':value,'complete_initial':complete,'target_frozen':g.frozen})
        if complete and not (R/'guardian.initial.json').exists():atomic('guardian.initial.json',value)
        if g.frozen:atomic('guardian.target-frozen.json',{'epoch':time.time(),'pid':os.getpid()})
        time.sleep(.25)
except BaseException as error:
    failure=type(error).__name__+': '+str(error)
    failure_receipt(error,'guardian.failure.structured')
    atomic('guardian.failure.json',{'failure':failure,'peak':g.peak})
    # A breach freezes target reads immediately. The controller still reaps
    # owned children and proves handles before releasing the warm target.
    atomic('guardian.target-frozen.json',{'epoch':time.time(),'pid':os.getpid(),'failure':failure})
    try:os.kill(a['controller_pid'],signal.SIGTERM)
    except ProcessLookupError:pass
finally:atomic('guardian.result.json',{'failure':failure,'peak':g.peak,'epoch':time.time()})
raise SystemExit(1 if failure else 0)
