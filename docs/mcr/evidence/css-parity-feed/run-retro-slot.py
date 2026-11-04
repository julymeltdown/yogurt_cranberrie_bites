"""Automatic lifetime guard. Execute only after a fresh offer and explicit acceptance."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import tempfile
import time

p = argparse.ArgumentParser()
p.add_argument('--slot', required=True)
p.add_argument('--accepted-epoch', type=float, required=True)
a = p.parse_args()
root = Path('/Users/lhs/.codex/worktrees/d51f/mcr')
assert Path.cwd().resolve() == root
assert time.time() - a.accepted_epoch < 5
out = root / 'docs/mcr/evidence/css-parity-feed'
node_bin = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin'
link = root / 'frontend/node_modules'
deps = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
assert not link.exists() and not link.is_symlink()
assert shutil.disk_usage(root).free >= 16 * 1024**3
with socket.socket() as probe:
    probe.bind(('127.0.0.1', 5292))
tmp = Path(tempfile.mkdtemp(prefix='mcr-css-feed-retro-'))
link.symlink_to(deps, target_is_directory=True)
env = os.environ.copy()
env.update(PATH=node_bin + ':' + env['PATH'], TMPDIR=str(tmp))
argv = ['python3', str(out / 'supervisor-intrinsic.py'), '--slot', a.slot, '--accepted-epoch', str(a.accepted_epoch), '--driver', 'retro']
receipt = {'slot': a.slot, 'acceptedEpoch': a.accepted_epoch, 'argv': argv, 'processPeak': 0, 'rssPeakBytes': 0}
child = None
known = set()

def processes():
    rows = []
    for row in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,command='], text=True).splitlines():
        parts = row.strip().split(None, 3)
        if len(parts) == 4:
            rows.append((int(parts[0]), int(parts[1]), int(parts[2]) * 1024, parts[3]))
    ids = {child.pid} if child else set()
    changed = True
    while changed:
        new = {pid for pid, parent, _, _ in rows if parent in ids}
        changed = bool(new - ids)
        ids |= new
    ids |= {pid for pid, _, _, command in rows if str(tmp) in command or str(out / 'native-retro.mjs') in command}
    selected = [row for row in rows if row[0] in ids and row[0] != os.getpid()]
    known.update(row[0] for row in selected)
    # Include the supervising wrapper and its short-lived ps monitor in ALL-process accounting.
    accounting = selected + [row for row in rows if row[0] == os.getpid() or (row[1] == os.getpid() and row[0] not in ids)]
    receipt['processPeak'] = max(receipt['processPeak'], len(accounting))
    receipt['rssPeakBytes'] = max(receipt['rssPeakBytes'], sum(row[2] for row in accounting))
    return selected

try:
    with (out / ('outer-' + a.slot + '.log')).open('x') as log:
        child = subprocess.Popen(argv, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        while child.poll() is None:
            rows = processes()
            assert receipt['processPeak'] <= 32 and receipt['rssPeakBytes'] <= 3 * 1024**3
            assert time.time() < a.accepted_epoch + 125
            time.sleep(.4)
        receipt['supervisorExit'] = child.returncode
finally:
    for pid, _, _, _ in processes():
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    end = time.time() + 3
    while time.time() < end and any(row[0] != (child.pid if child else None) for row in processes()):
        time.sleep(.1)
    for pid, _, _, _ in processes():
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    if child:
        child.wait(timeout=5)
    if link.is_symlink() and link.resolve() == deps.resolve():
        link.unlink()
    shutil.rmtree(tmp, ignore_errors=True)
    receipt.update(releasedEpoch=time.time(), elapsedSinceAccept=time.time()-a.accepted_epoch, ownLinkRemoved=not link.is_symlink(), ownTmpRemoved=not tmp.exists())
    with socket.socket() as probe:
        try:
            probe.bind(('127.0.0.1', 5292))
            receipt['port5292Bindable'] = True
        except OSError:
            receipt['port5292Bindable'] = False
    receipt['remainingOwnedProcesses'] = [row[0] for row in processes()]
    (out / ('release-' + a.slot + '.json')).write_text(json.dumps(receipt, indent=2)+'\n')
    print('AUTO_RELEASE ' + json.dumps(receipt), flush=True)
