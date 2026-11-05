import os
import time
os.environ.setdefault('MCR_VENU_ACCEPT_EPOCH', str(time.time()))
import signal
import subprocess
import sys
import traceback
sys.dont_write_bytecode = True
from finite import EVIDENCE, ROOT, Round
import membership

round = Round('controller')
membership.publish(round.resource())  # Actual absolute-script identity before heavy census/work.
child, code, failure = None, 1, None
signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(RuntimeError('Orderly stop requested')))
try:
    round.initialize()
    child = subprocess.Popen([sys.executable,'-B',str(EVIDENCE/'guardian.py')],cwd=ROOT,start_new_session=True)
    round.seen[child.pid]=membership.processes()[child.pid]['birth']
    while child.poll() is None:
        round.check(); time.sleep(.25)
    code=child.returncode
except BaseException as error:
    failure=traceback.format_exc(); code=1
finally:
    value=round.release(code,failure,child)
raise SystemExit(code if not value['resources'] else 2)
