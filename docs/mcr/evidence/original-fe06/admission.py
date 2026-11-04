"""Read-only full carry measurement; freeze once at allocator acceptance."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess as sp
import time
root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
def inventory(root, exclude=()):
    count = total = 0
    for directory, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = [d for d in dirs if d not in exclude and not (Path(directory) / d).is_symlink()]
        for name in files:
            p = Path(directory) / name
            try:
                if not p.is_symlink():
                    count += 1
                    total += p.stat().st_size
            except FileNotFoundError:
                pass
    return count, total
parser = argparse.ArgumentParser()
parser.add_argument('--offer')
parser.add_argument('--cutoff', type=int)
parser.add_argument('--release', type=int)
parser.add_argument('--output')
parser.add_argument('--other', type=int, default=0)
parser.add_argument('--browser', action='store_true')
parser.add_argument('--start-floor', type=int, choices=[11, 12], default=12)
args = parser.parse_args()
git = Path(sp.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=root, text=True).strip())
metadata = Path(sp.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=root, text=True).strip())
result = dict(measured=time.time(), free=shutil.disk_usage(root).free)
for name, path, excludes in [('source', root, ('.git', 'node_modules', 'target')), ('git', git / 'objects', ()), ('metadata', metadata, ()), ('dependency', Path('/Users/lhs/Desktop/mcr/frontend/node_modules'), ()), ('out', out, ())]:
    count, size = inventory(path, excludes)
    result.update({name + 'Files': count, name + 'Bytes': size})
if args.browser:
    count, size = inventory(Path('/Applications/Google Chrome.app'))
    result.update(browserFiles=count, browserBytes=size, nodeFiles=1, nodeBytes=Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node').stat().st_size)
if args.output:
    target = out / args.output
    assert not target.exists() and args.offer and args.cutoff and args.release
    result.update(offer=args.offer, cutoff=args.cutoff, release=args.release, otherWorstGrowth=args.other, startFloorGiB=args.start_floor)
    target.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result))
