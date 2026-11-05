"""Independent finite source guardian. Pin this file before launching any test child."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import sys
import time

config_path = Path(sys.argv[1])
config = json.loads(config_path.read_text())
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == config['guardianSha256']
assert hashlib.sha256(Path(config['supervisorFile']).read_bytes()).hexdigest() == config['supervisorSha256']
root, temporary, output = (Path(config[key]) for key in ['root', 'scratch', 'output'])
supervisor = config['supervisor']
ready, stop, receipt = (Path(str(output) + suffix) for suffix in ['.ready', '.stop', '.json'])
assert not any(path.exists() for path in [ready, stop, receipt])
known, births = {supervisor}, {}
peaks = {'processes': 0, 'rss': 0, 'temporary': 0}
failure = None

def processes():
    probe = sp.Popen(['ps', '-axo', 'pid=,ppid=,rss=,lstart='], stdout=sp.PIPE, text=True)
    text, _ = probe.communicate()
    rows = []
    for line in text.splitlines():
        parts = line.split(None, 3)
        if len(parts) == 4 and int(parts[0]) != probe.pid:
            rows.append((int(parts[0]), int(parts[1]), int(parts[2]) * 1024, parts[3]))
    known.intersection_update({pid for pid, _, _, birth in rows if pid in known and (pid not in births or births[pid] == birth)})
    while True:
        children = {pid for pid, parent, _, _ in rows if parent in known}
        if children <= known:
            break
        known.update(children)
    births.update({pid: birth for pid, _, _, birth in rows if pid in known})
    return [(pid, rss) for pid, _, rss, _ in rows if pid in known]

try:
    ready.write_text(str(time.time()))
    while not stop.exists():
        rows = processes()
        assert supervisor in known, 'supervisor exited before FULL cleanup'
        usage = 0
        for directory, _, files in os.walk(temporary, followlinks=False):
            for name in files:
                try:
                    path = Path(directory) / name
                    if not path.is_symlink(): usage += path.stat().st_size
                except FileNotFoundError:
                    pass
        values = {'processes': len(rows), 'rss': sum(rss for _, rss in rows), 'temporary': usage}
        for key, value in values.items():
            peaks[key] = max(peaks[key], value)
            assert value <= config[key], (key, value)
        assert shutil.disk_usage(root).free >= 8 * 1024**3, 'live disk floor'
        assert time.time() < config['cutoff'], 'work cutoff'
        time.sleep(.25)
except BaseException as error:
    failure = str(error)
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        processes()
        for pid in known - {supervisor, os.getpid()}:
            try: os.kill(pid, sig)
            except ProcessLookupError: pass
finally:
    receipt.write_text(json.dumps({'failure': failure, 'peaks': peaks, 'finished': time.time(),
        'guardianSha256': config['guardianSha256']}, indent=2) + '\n')
raise SystemExit(1 if failure else 0)
