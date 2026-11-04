"""Read-only full-inode inventory of candidate-owned shared Git objects."""
from pathlib import Path
import struct
import subprocess

BASE = '2492e35bb9fb61380bf97fa936983ec16c0722f4'


def inventory(root):
    common = Path(subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=root, text=True).strip())
    objects = {line.split()[0] for line in subprocess.check_output(
        ['git', 'rev-list', '--objects', BASE + '..HEAD'], cwd=root, text=True).splitlines()}
    charged = {}

    def charge(path):
        stat = path.stat()
        charged[(stat.st_dev, stat.st_ino)] = {'path': str(path), 'logicalBytes': stat.st_size,
            'allocatedBytes': stat.st_blocks * 512, 'chargeBytes': max(stat.st_size, stat.st_blocks * 512)}

    missing = set()
    for oid in objects:
        assert len(oid) == 40
        loose = common / 'objects' / oid[:2] / oid[2:]
        if loose.is_file(): charge(loose)
        else: missing.add(oid)
    for index in sorted((common / 'objects/pack').glob('*.idx')):
        if not missing: break
        with index.open('rb') as stream:
            assert stream.read(4) == b'\xfftOc' and struct.unpack('>I', stream.read(4))[0] == 2
            fanout = struct.unpack('>256I', stream.read(1024))
            table = stream.read(fanout[-1] * 20)
        found = set()
        for oid in missing:
            key = bytes.fromhex(oid)
            low, high = (fanout[key[0] - 1] if key[0] else 0), fanout[key[0]]
            while low < high:
                middle = (low + high) // 2
                if table[middle * 20:middle * 20 + 20] < key: low = middle + 1
                else: high = middle
            if low < fanout[-1] and table[low * 20:low * 20 + 20] == key: found.add(oid)
        if found:
            assert index.with_suffix('.pack').is_file()
            for sidecar in index.parent.glob(index.stem + '.*'):
                if sidecar.is_file(): charge(sidecar)
            missing -= found
    assert not missing, 'Owned Git objects changed or have unsupported storage; do not launch workload'
    entries = sorted(charged.values(), key=lambda entry: entry['path'])
    return {'bytes': sum(entry['chargeBytes'] for entry in entries), 'objectCount': len(objects), 'inodes': entries}
