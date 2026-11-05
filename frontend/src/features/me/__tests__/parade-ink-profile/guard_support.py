"""Shared accounting inputs; each live guard samples independently."""
import hashlib
import os
from pathlib import Path
import subprocess


def inventory(roots):
    ledger = {}
    seen = set()
    rows = []
    for root in map(Path, roots):
        count = total = 0
        for parent, directories, files in os.walk(root, followlinks=False):
            for name in ['.', *directories, *files]:
                path = Path(parent) if name == '.' else Path(parent) / name
                try:
                    item = path.lstat()
                except FileNotFoundError:
                    continue
                key = (item.st_dev, item.st_ino)
                if key in seen:
                    continue
                seen.add(key)
                count += 1
                total += item.st_blocks * 512
                ledger[f'{item.st_dev}:{item.st_ino}'] = item.st_blocks * 512
        rows.append({'path': str(root), 'inodes': count, 'allocatedBytes': total})
    return {'roots': rows, 'uniqueInodes': len(seen), 'allocatedBytes': sum(row['allocatedBytes'] for row in rows), 'ledger': ledger}


def processes(root_pid):
    output = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,command='], text=True)
    rows = {}
    for line in output.splitlines():
        parts = line.split(None, 3)
        if len(parts) == 4:
            pid, parent, rss = map(int, parts[:3])
            rows[pid] = {'pid': pid, 'parent': parent, 'rssKiB': rss,
                         'commandHash': hashlib.sha256(parts[3].encode()).hexdigest()}
    ids = {root_pid}
    while True:
        expanded = ids | {pid for pid, row in rows.items() if row['parent'] in ids}
        if expanded == ids:
            return [row for pid, row in rows.items() if pid in ids]
        ids = expanded


def directory_bytes(path, suffix=None):
    total = 0
    for parent, _, files in os.walk(path, followlinks=False):
        for name in files:
            file = Path(parent) / name
            if suffix and file.suffix != suffix:
                continue
            try:
                if not file.is_symlink():
                    total += file.stat().st_size
            except FileNotFoundError:
                continue
    return total


_highwater = {}

def sample(config):
    import shutil
    family = processes(config['rootPid'])
    whole = inventory(config['roots'])
    baseline = config['initialLedger']
    for key, value in whole.pop('ledger').items():
        _highwater[key] = max(_highwater.get(key, baseline.get(key, 0)), value)
    positive = sum(max(0, value - baseline.get(key, 0)) for key, value in _highwater.items())
    return {'chargedWholeBytes': max(whole['allocatedBytes'], config['initialWholeBytes'], config['priorHistoricalMaxBytes']), 'positiveGrowthBytes': positive, 'freeBytes': shutil.disk_usage(config['repo']).free, 'processes': family,
            'rssKiB': sum(row['rssKiB'] for row in family), 'whole': whole,
            'tempBytes': directory_bytes(config['temporary']),
            'durableBytes': directory_bytes(config['output']),
            'logBytes': directory_bytes(config['output'], '.log')}


def violation(value, config):
    limits = config['limits']
    if value['freeBytes'] < 8 * 1024**3:
        return '8GiB live floor'
    if len(value['processes']) > limits['processes'] or value['rssKiB'] > limits['rssMiB'] * 1024:
        return 'Whole descendant process/RSS cap'
    if value['chargedWholeBytes'] > limits['wholeBytes']:
        return 'Complete unique-inode WHOLE cap'
    if value['positiveGrowthBytes'] > limits['newGrowthBytes']:
        return 'NEW positive growth cap'
    if value['tempBytes'] > limits['tempBytes'] or value['durableBytes'] > limits['durableBytes'] or value['logBytes'] > limits['logBytes']:
        return 'TEMP/durable/log cap'
    return None
