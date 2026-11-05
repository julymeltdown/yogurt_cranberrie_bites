"""PI09 finite source controller and independent guardian; no install/link/build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
sys.dont_write_bytecode = True
import accounting
import resource_receipts

GiB = 1024 ** 3
MiB = 1024 ** 2
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
NODE_ROOT = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64')
NODE = NODE_ROOT / 'bin/node'
TOOLING = Path('/Users/lhs/Desktop/mcr/frontend')
PYTHON = Path('/Library/Frameworks/Python.framework')
GIT = Path(subprocess.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=REPO, text=True).strip()).resolve()

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def rows():
    data = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,command='], text=True)
    return [(int(p), int(parent), int(rss) * 1024, command) for p, parent, rss, command in (line.strip().split(None, 3) for line in data.splitlines() if len(line.strip().split(None, 3)) == 4)]

def descendants(data, root):
    found = {root}
    for _ in range(30):
        expanded = found | {p for p, parent, _, _ in data if parent in found}
        if expanded == found:
            return found
        found = expanded
    raise RuntimeError('Process-tree closure incomplete')

def capture(temp):
    paths = {'checkout': REPO, 'entireGit': GIT, 'entireNode': NODE_ROOT,
             'entirePython': PYTHON, 'readOnlyDependencies': TOOLING / 'node_modules', 'phaseTemporary': temp, 'receiptDiscovery': resource_receipts.BASE/'owner-resource-receipts'}
    return {name: accounting.RootIdentity.capture(path) for name, path in paths.items()}

def process_sample(controller):
    data = rows(); owned = descendants(data, controller)
    current = [row for row in data if row[0] in owned]
    global_roots = {p for p, _, _, command in data if '/mcr' in command and 'Codex.app' not in command}
    global_ids = set()
    for pid in global_roots:
        global_ids.update(descendants(data, pid))
    global_rows = [row for row in data if row[0] in global_ids]
    return {'pids': len(current), 'rss': sum(row[2] for row in current),
            'globalPids': len(global_rows), 'globalRss': sum(row[2] for row in global_rows),
            'ownedPids': sorted(owned), 'free': shutil.disk_usage(REPO).free}

def fast(config, require_peer=None):
    sample = process_sample(config['controller'])
    sample['cohorts'] = resource_receipts.census(config, rows())
    assert time.time() < config['workDeadline'], 'Absolute WORK deadline'
    assert sample['free'] >= 8 * GiB, '8GiB live floor'
    assert sample['pids'] <= 12 and sample['rss'] <= 3 * GiB, 'Owned PID/RSS cap'
    assert sample['globalPids'] <= 96 and sample['globalRss'] <= 18 * GiB, 'Global PID/RSS cap'
    if require_peer:
        os.kill(require_peer, 0)
    return sample

def full_sample(config, roots, carry, peer=None):
    sample = fast(config, peer)
    last_live = time.monotonic()
    def live_checkpoint():
        nonlocal last_live
        assert time.time() < config['workDeadline'], 'Absolute WORK deadline'
        if time.monotonic() - last_live >= .25:
            fast(config, peer); last_live = time.monotonic()
    allocation = accounting.collect(roots, deadline=time.monotonic() + 8, checkpoint=live_checkpoint)
    value = carry.observe(allocation['scopes']); sample.update(value)
    assert value['new'] <= config['newCap'], 'Accepted NEW cap'
    assert value['whole'] <= 8 * GiB, '8GiB WHOLE cap'
    remaining = max(0, config['newCap'] - value['new'])
    assert sample['free'] - remaining - config['otherNew'] >= 9 * GiB, '9GiB after all remaining reservations'
    sample['time'] = time.time(); sample['allocation'] = allocation
    return sample

def guardian(path):
    config = json.loads(path.read_text()); root = path.parent
    roots = capture(root)
    carry = accounting.Carry(config['baseline'], 240 * MiB, config['priorWhole'], 240 * MiB)
    try:
        initial = full_sample(config, roots, carry, config['controller'])
        (root / 'guardian.ready').write_text(json.dumps(initial))
        with (root / 'guardian.jsonl').open('a') as log:
            while not (root / 'stop').exists():
                sample = full_sample(config, roots, carry, config['controller'])
                log.write(json.dumps(sample) + '\n'); log.flush()
                time.sleep(.5)
    except BaseException as error:
        (root / 'guardian.failure').write_text(json.dumps({'error':repr(error),'accounting':error.receipt() if isinstance(error,accounting.AccountingError) else None}))
        sys.exit(2)

p = argparse.ArgumentParser()
p.add_argument('--guardian', type=Path)
p.add_argument('--preflight', action='store_true')
p.add_argument('--job', default='PI09-WALLB-SOURCE01-20261105T0007Z')
p.add_argument('--other-gib', type=int, default=4)
p.add_argument('--new-mib', type=int, choices=[128,200,256], default=128)
a = p.parse_args()
if a.guardian:
    guardian(a.guardian)
    sys.exit(0)
if a.preflight:
    roots = capture(REPO / 'docs/mcr/evidence/parade-ink-wall-b')
    result = accounting.collect(roots, deadline=time.monotonic() + 20, checkpoint=lambda: None)
    pins = {str(path): digest(path) for path in [NODE, HERE/'guard.py', HERE/'accounting.py', HERE/'source.mjs', HERE/'dependencies.mjs',HERE/'sparse-source.mjs',HERE/'resource_receipts.py', Path(sys.executable).resolve(), TOOLING/'package.json', TOOLING/'pnpm-lock.yaml']}
    print(json.dumps({'allocated': result, 'pins': pins, 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()}))
    sys.exit(0)

# Invocation is permitted only after the actual predecessor FULL/DECLINE and acceptance.
assert Path(sys.argv[0]).is_absolute(), 'Invoke the exact absolute controller path to match its published identity'
assert os.environ.get('PI09_SOURCE_ACCEPTED') == a.job
assert subprocess.check_output(['git', 'status', '--porcelain'], cwd=REPO, text=True) == ''
assert shutil.disk_usage(REPO).free >= 11 * GiB
assert shutil.disk_usage(REPO).free - a.other_gib * GiB - max(0, a.new_mib * MiB - 240 * MiB) >= 9 * GiB
for name in ['package.json','pnpm-lock.yaml']:
    assert (REPO/'frontend'/name).read_bytes() == (TOOLING/name).read_bytes()
assert digest(NODE) == 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a'
started = time.time()
temp = Path('/private/tmp/mcr-pi09-source-' + str(int(started)))
temp.mkdir(mode=0o700)
roots = capture(temp)
config = {'job': a.job, 'controller': os.getpid(), 'accepted': started, 'workDeadline': started+180,
          'fullDeadline': started+240, 'otherNew': a.other_gib*GiB, 'baseline': {}, 'censusRoot':str(temp),
          'newCap': a.new_mib*MiB, 'kind': 'source', 'processCap': 12, 'rssCap': 3*GiB, 'priorWhole': 5*GiB, 'pins': {str(p):digest(p) for p in [NODE,HERE/'guard.py',HERE/'accounting.py',HERE/'source.mjs',HERE/'dependencies.mjs',HERE/'sparse-source.mjs',HERE/'resource_receipts.py',Path(sys.executable).resolve(),TOOLING/'package.json',TOOLING/'pnpm-lock.yaml']}, 'head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip(),
          'carryPolicy': 'Conservative5GiB WHOLE and240MiB retained NEW upper preserve legacy/prep/closed-round positive maxima; old64MiB prep cap/history unchanged, no deletion credits.'}
path = temp/'config.json'
durable = REPO/'docs/mcr/evidence/parade-ink-wall-b'/a.job.lower()
durable.mkdir(parents=True,exist_ok=True)
(durable/'ACCEPT.json').write_text(json.dumps(config,indent=2)+'\n')
resource_receipts.publish(config,'ACCEPTED',HERE/'guard.py',durable)
child = watcher = None
status = 'FAIL'; failure = None; peaks = {}
try:
    baseline = accounting.collect(roots, deadline=time.monotonic()+15, checkpoint=lambda: fast(config))
    assert baseline['bytes'] + a.new_mib*MiB < 8*GiB
    config['baseline'] = {k:v['bytes'] for k,v in baseline['scopes'].items()}
    path.write_text(json.dumps(config,indent=2))
    carry = accounting.Carry(config['baseline'],240*MiB,config['priorWhole'],240*MiB)
    watcher = subprocess.Popen([sys.executable,str(HERE/'guard.py'),'--guardian',str(path)])
    resource_receipts.publish(config,'ACCEPTED',HERE/'guard.py',durable,watcher.pid)
    ready_deadline = min(started+20,config['workDeadline'])
    while not (temp/'guardian.ready').exists():
        assert watcher.poll() is None and time.time()<ready_deadline, 'Independent guardian not ready'
        fast(config, watcher.pid); time.sleep(.1)
    initial = full_sample(config, roots, carry, watcher.pid)
    (temp/'controller.ready').write_text(json.dumps(initial))
    env = {**os.environ,'PATH':str(NODE.parent)+os.pathsep+os.environ['PATH'], 'TMPDIR':str(temp),
           'PI09_TOOLING_ROOT':str(TOOLING),'PI09_SOURCE_OUTPUT':str(temp/'tests.json'),'PI09_SOURCE_TEMP':str(temp),
           'NO_COLOR':'1','CI':'1'}
    # Both independent guards have completed full initial samples before this Node.
    with (temp/'source.log').open('w') as log:
        child = subprocess.Popen([str(NODE),'--import',str(HERE/'dependencies.mjs'),str(HERE/'source.mjs')],cwd=REPO,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        with (temp/'controller.jsonl').open('w') as samples:
            while child.poll() is None:
                assert not (temp/'guardian.failure').exists(), 'Independent guardian failed'
                sample=full_sample(config,roots,carry,watcher.pid)
                for key in ['new','whole','pids','rss','globalPids','globalRss']:peaks[key]=max(peaks.get(key,0),sample[key])
                samples.write(json.dumps(sample)+'\n');samples.flush();time.sleep(.25)
        status='PASS' if child.returncode==0 else 'FAIL'
except BaseException as error:
    failure=repr(error)
finally:
    if child and child.poll() is None:
        os.killpg(child.pid,signal.SIGTERM)
        try: child.wait(timeout=5)
        except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=5)
    (temp/'stop').write_text('owned work stopped')
    if watcher:
        try:watcher.wait(timeout=10)
        except subprocess.TimeoutExpired:watcher.terminate();watcher.wait(timeout=5)
    remaining=process_sample(os.getpid())['ownedPids']
    # ps itself is ephemeral; no owned Node/guardian may remain before FULL.
    data=rows(); live=[row for row in data if row[0] in descendants(data,os.getpid()) and row[0]!=os.getpid() and 'ps -axo' not in row[3]]
    full=not live and (not child or child.poll() is not None) and (not watcher or watcher.poll() is not None) and time.time()<config['fullDeadline']
    result={'job':a.job,'status':status,'error':failure,'FULL':full,'finished':time.time(),'elapsed':time.time()-started,'peaks':peaks,'config':config,'phaseTemporary':str(temp),'remainingOwned':live}
    durable=REPO/'docs/mcr/evidence/parade-ink-wall-b'/a.job.lower()
    durable.mkdir(parents=True,exist_ok=True)
    for proof in temp.iterdir():
        if proof.is_file():shutil.copy2(proof,durable/proof.name)
    shutil.rmtree(temp)
    result['temporaryAbsent']=not temp.exists()
    result['FULL']=full and result['temporaryAbsent']
    (durable/'FULL.json').write_text(json.dumps(result,indent=2)+'\n')
    if result['FULL']:resource_receipts.publish(config,'FULL',HERE/'guard.py',durable,result=result)
    sparse=Path(subprocess.check_output(['git','rev-parse','--git-dir'],cwd=REPO,text=True).strip())/'info/sparse-checkout'
    with sparse.open('a') as patterns:
        for proof in sorted(durable.iterdir()):patterns.write('/'+proof.relative_to(REPO).as_posix()+'\n')
    print(json.dumps(result),flush=True)
    if status!='PASS' or not full:sys.exit(1)
