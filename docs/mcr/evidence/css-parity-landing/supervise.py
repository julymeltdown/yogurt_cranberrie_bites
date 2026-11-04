"""Finite owner-only process group guard; no peer/browser attachment."""
from pathlib import Path
import subprocess, os, signal, sys, time, shutil
ROOT=Path('/Users/lhs/.codex/worktrees/fb96/mcr')
TMP=Path('/tmp/mcr-css-parity-landing')
EVIDENCE=ROOT/'docs/mcr/evidence/css-parity-landing'
NODE='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
STOP=1793784411
assert Path.cwd()==ROOT and time.time()<STOP
assert shutil.disk_usage(ROOT).free >= 8*1024**3

def size(path):
    return sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink()) if path.exists() else 0

env=dict(os.environ, PATH=str(Path(NODE).parent)+':'+os.environ['PATH'], TMPDIR=str(TMP/'tmp'))
command=sys.argv[1:]
assert command
with (EVIDENCE/(env.get('MCR_LANDING_PHASE','checks')+'-attempt.log')).open('a') as log:
    log.write(f'UTCepoch={time.time()} command={command!r}\n');log.flush()
    process=subprocess.Popen(command,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
    failure=None
    try:
        while process.poll() is None:
            temporary=size(TMP);durable=size(EVIDENCE)
            if time.time()>=STOP: failure='active deadline';break
            if shutil.disk_usage(ROOT).free<8*1024**3: failure='disk floor';break
            if temporary>128849018 or durable>15*1024**2 or temporary+durable>214748364: failure='byte cap';break
            time.sleep(.5)
    finally:
        if failure:
            os.killpg(process.pid,signal.SIGTERM)
            try: process.wait(timeout=8)
            except subprocess.TimeoutExpired: os.killpg(process.pid,signal.SIGKILL);process.wait()
        code=process.wait()
        log.write(f'EXIT={code} guard={failure}\n')
    print('EXIT',code,'guard',failure,'temporaryBytes',size(TMP),'durableBytes',size(EVIDENCE))
    sys.exit(code if not failure else 2)
