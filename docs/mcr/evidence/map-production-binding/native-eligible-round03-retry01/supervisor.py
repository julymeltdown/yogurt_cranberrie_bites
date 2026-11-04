from pathlib import Path
import datetime, hashlib, json, os, shutil, signal, subprocess, time

root = Path('/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish')
front = root / 'frontend'
node = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
driver = 'src/app/world-entry/__tests__/map-presentation-v1/native.mjs'
pin = os.environ['MCR_MAP4_PIN']
job = os.environ['MCR_MAP4_LEASE']
out = Path(os.environ['MCR_MAP4_OUTPUT'])
temp = Path(os.environ['MCR_MAP4_TEMP'])
receipt = Path(os.environ['MCR_MAP4_RECEIPT'])
link = front / 'node_modules'
start = time.monotonic()
ownlink = False
proc = None
failure = None
entries = []
peaks = dict(whole=0, temp=0, durable=0)
minimum = 10**30
carry = int(os.environ['MCR_MAP4_CARRY'])
retained = Path(os.environ['MCR_MAP4_STAGE_RETAINED']) if os.environ.get('MCR_MAP4_STAGE_RETAINED') else None
started = datetime.datetime.now(datetime.timezone.utc)
deadline = datetime.datetime.fromisoformat(os.environ['MCR_MAP4_DEADLINE'])

def size(path):
    return sum(max(p.stat().st_size, p.stat().st_blocks * 512) for p in path.rglob('*')
               if p.is_file() and not p.is_symlink()) if path.exists() else 0

def guard():
    global minimum
    t, d = size(temp), size(out) + size(receipt) + (size(retained) if retained else 0)
    free = shutil.disk_usage(root).free
    minimum = min(minimum, free)
    for key, value in dict(whole=t+d+carry, temp=t, durable=d).items():
        peaks[key] = max(peaks[key], value)
    assert time.monotonic() - start < 355 and datetime.datetime.now(datetime.timezone.utc) < deadline - datetime.timedelta(seconds=65), 'absolute work cutoff reserves 65 seconds cleanup'
    assert t <= 80*1024**2 and d <= 48*1024**2 and t+d+carry <= 193273528, 'allocation byte cap'
    assert free >= 8*1024**3, 'continuous disk floor'

try:
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip() == pin
    assert not subprocess.check_output(['git','status','--porcelain'],cwd=root)
    assert not link.exists() and not link.is_symlink()
    assert not out.exists() and not temp.exists() and not receipt.exists()
    assert out.is_relative_to(root/'docs/mcr/evidence/map-production-binding')
    assert str(temp).startswith('/private/tmp/mcr-root-map4-')
    assert str(receipt).startswith('/private/tmp/mcr-root-map4-')
    assert shutil.disk_usage(root).free >= 16*1024**3
    receipt.mkdir(); temp.mkdir()
    (receipt/'supervisor.py').write_bytes(Path(__file__).read_bytes())
    (receipt/'carry-inventory.json').write_bytes(Path('/private/tmp/mcr-root-map4-carry-inventory.json').read_bytes())
    link.symlink_to('/Users/lhs/Desktop/mcr/frontend/node_modules',target_is_directory=True); ownlink = True
    env = {**os.environ,'PATH':str(Path(node).parent)+':'+os.environ['PATH'],
           'TMPDIR':str(temp),'TMP':str(temp),'TEMP':str(temp),
           'XDG_CACHE_HOME':str(temp/'xdg-cache'),'CI':'true',
           'MCR_MAP4_DEADLINE':deadline.isoformat()}
    for i, cmd in enumerate([[node,'--check',driver],[node,driver]],1):
        at = datetime.datetime.now(datetime.timezone.utc).isoformat()
        with (receipt/f'{i}.log').open('wb') as log:
            proc = subprocess.Popen(cmd,cwd=front,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            while proc.poll() is None:
                guard(); time.sleep(.25)
            guard()
        entries.append(dict(command=cmd,exit_code=proc.returncode,started_at=at,log=f'{i}.log'))
        print(json.dumps(entries[-1]),flush=True)
        assert proc.returncode == 0, f'command-{i}-exit-{proc.returncode}'
except BaseException as error:
    failure = str(error)
finally:
    if proc is not None:
        try: os.killpg(proc.pid,signal.SIGTERM)
        except ProcessLookupError: pass
        try: proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            try: os.killpg(proc.pid,signal.SIGKILL)
            except ProcessLookupError: pass
            proc.wait(timeout=5)
    if ownlink and link.is_symlink() and os.readlink(link) == '/Users/lhs/Desktop/mcr/frontend/node_modules': link.unlink()
    shutil.rmtree(temp,ignore_errors=True)
    for phase in ['before','after']:
        shutil.rmtree(out/f'vite-{phase}',ignore_errors=True)
    report = dict(job=job,candidate=pin,commands=entries,failure=failure,
                  started_at=started.isoformat(),finished_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  elapsed_seconds=time.monotonic()-start,peaks=peaks,carry_bytes=carry,stage_retained_durable=str(retained) if retained else None,minimum_free_bytes=minimum,
                  temp_absent=not temp.exists(),owned_link_absent=not link.exists(),
                  driver_sha256=hashlib.sha256((front/driver).read_bytes()).hexdigest())
    if receipt.exists(): (receipt/'supervisor.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report),flush=True)
raise SystemExit(1 if failure else 0)
