"""Independent outer guard. Requires an actual finite21d offer JSON."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
from metrics import Meter, ROOT, GIB, CARRY, inventory, free, tree, write, publish_receipt

cfg = json.loads(Path(os.environ['PI10_CONFIG']).read_text())
folder = Path(cfg['control'])
publish_receipt(cfg, 'ACCEPTED')
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == cfg['pins']['controller.py']
assert cfg['lease'] and cfg['mode'] in ('source', 'native')
assert cfg['full_deadline'] - cfg['work_deadline'] >= 30 and time.time() < cfg['work_deadline']
assert free() >= cfg['start_floor'] and free() - cfg['other_new'] - cfg.get('remaining_new_reservation', cfg['new_cap']) >= 9 * GIB
assert cfg['start_floor'] >= 11 * GIB
assert max(cfg['prior_whole'], sum(inventory(CARRY + [Path(p) for p in cfg.get('retained_roots', [])]).values())) + cfg.get('remaining_new_reservation', cfg['new_cap']) <= cfg['whole_cap']
assert not (ROOT / 'frontend/node_modules').exists()
assert not Path(cfg['temporary']).exists() and not Path(cfg['output']).exists()
meter = Meter(cfg, os.getpid())
write(folder / 'outer-accept.json', dict(lease=cfg['lease'], epoch=time.time(), free=free(), carry=meter.carry, controller=os.getpid()))
env = dict(os.environ, PI10_CONTROLLER=str(os.getpid()))
child = None
primary_log = None
failure = None
code = 1
try:
    publish_receipt(cfg, 'ACCEPTED')
    first = meter.check()
    write(folder / 'outer-initial.json', dict(epoch=time.time(), sample=first, global_sample=meter.last_global))
    primary_log = (folder / 'primary-control.log').open('w')
    child = subprocess.Popen([sys.executable, '-B', str(Path(__file__).with_name('guardian.py'))], env=env, cwd=ROOT, start_new_session=True, stdout=primary_log, stderr=subprocess.STDOUT)
    while child.poll() is None:
        meter.check()
        time.sleep(.2)
    code = child.returncode
    meter.check()
except BaseException as error:
    failure = repr(error)
finally:
    if child and child.poll() is None:
        live = tree(os.getpid(), meter.seen)
        groups = {row[1] for pid, row in live.items() if pid != os.getpid() and row[1] != os.getpgrp()}
        for group in groups:
            try:
                os.killpg(group, signal.SIGTERM)
            except OSError:
                pass
        time.sleep(.3)
        for group in groups:
            try:
                os.killpg(group, signal.SIGKILL)
            except OSError:
                pass
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            failure = (failure or '') + '; owned guard did not exit within cleanup wait'
    if primary_log:
        primary_log.close()
    remaining = [pid for pid, row in tree(os.getpid(), meter.seen).items() if pid != os.getpid() and Path(row[3]).name != 'ps']
    try:
        meter.check(final=True)
    except BaseException as error:
        failure = (failure or '') + '; final resource census: ' + repr(error)
    result = dict(lease=cfg['lease'], result='PASS' if code == 0 and failure is None and not remaining else 'FAIL', exit=code,
                  failure=failure, full_epoch=time.time(), before_hard=time.time() < cfg['full_deadline'], peak=meter.peak,
                  minimum_free=meter.minimum_free, remaining=remaining, execution_resources=0 if not remaining else len(remaining))
    write(folder / 'outer-full.json', result)
    if not remaining:
        publish_receipt(cfg, 'FULL')
    print(json.dumps(result), flush=True)
sys.exit(0 if result['result'] == 'PASS' else 1)
