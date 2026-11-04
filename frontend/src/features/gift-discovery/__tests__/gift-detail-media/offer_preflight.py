"""Read-only preflight; emit ACCEPT-ready data only when the start floor passes."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[6]
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
PINS = {
    NODE: 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a',
    ROOT / 'frontend/pnpm-lock.yaml': '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6',
    Path('/Users/lhs/Desktop/mcr/frontend/pnpm-lock.yaml'): '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6',
    Path(__file__).parent / 'adoption-inputs/CommunityFeatures.tsx.source': 'db2b1d0330b5af0347e55fda017da4d0bd1b586e9c2e8872d8fbae1390e577d8',
    Path(__file__).parent / 'adoption-inputs/RootMediaView.tsx.source': 'f22e034423804ec41658fa26760d07926f4f2ebce339fed685072fbf09542d9a',
}

if __name__ == '__main__':
    assert all(hashlib.sha256(path.read_bytes()).hexdigest() == expected for path, expected in PINS.items()), 'input pin'
    assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip(), 'dirty source'
    assert not os.path.lexists(ROOT / 'frontend/node_modules'), 'unexpected dependency path'
    volume = os.statvfs(ROOT)
    free = volume.f_bavail * volume.f_frsize
    minimum = (12 if sys.argv[1:] == ['--browser-12gib-offer'] else 16) * 1024 ** 3
    ready = free >= minimum
    print(json.dumps({
        'status': 'READY_FOR_NAMED_OFFER_ACCEPT' if ready else 'NOT_READY_START_FLOOR',
        'checked_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'pin': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'free_bytes': free, 'minimum_start_free_bytes': minimum,
        'input_hashes': {str(path): expected for path, expected in PINS.items()},
    }))
    sys.exit(0 if ready else 1)
