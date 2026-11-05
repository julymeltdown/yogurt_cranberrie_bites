"""Read-only full-inode inventory of candidate-owned shared Git objects."""
from pathlib import Path
import hashlib
import struct
import subprocess

BASE = '5ac4bc6e69ee47e6782c2d0b25c72c5544a540d6'


FOUNDATION = 'b5b44e47531e3b75febc5ee5cdd6a11b1648779c'
CONTRACT = '056a8c48ff667134dee8e5e99710185294e4b898'


def foundation_inventory(root):
    paths = subprocess.check_output(['git', 'diff-tree', '--no-commit-id', '--name-only', '-r', CONTRACT], cwd=root, text=True).splitlines()
    entries = []
    for name in paths:
        if not name.startswith('frontend/'): continue
        physical = Path('/Users/lhs/Desktop/mcr-worktrees/fe-runtime-preferences') / name
        expected = subprocess.check_output(['git', 'show', FOUNDATION + ':' + name], cwd=root)
        actual = physical.read_bytes()
        if actual != expected: assert not name.endswith(('.ttf', '.woff', '.woff2')), 'Readonly PI01 font changed: ' + name
        stat = physical.stat()
        entries.append({'path': str(physical), 'sha256': hashlib.sha256(expected).hexdigest(), 'observedReadonlySha256': hashlib.sha256(actual).hexdigest(),
            'source': 'verified readonly file' if actual == expected else 'immutable Git load override',
            'bytes': len(expected), 'chargeBytes': max(stat.st_size, stat.st_blocks * 512)})
    assert len(entries) == 18
    canonical_css = []
    for name in ['frontend/src/shared/styles/cozy-identity-backgrounds-v1.css', 'frontend/src/shared/styles/cozy-community-backgrounds-v1.css']:
        expected = subprocess.check_output(['git', 'show', BASE + ':' + name], cwd=root)
        assert (root / name).read_bytes() == expected
        canonical_css.append({'path': name, 'sha256': hashlib.sha256(expected).hexdigest(), 'bytes': len(expected),
            'chargeBasis': 'Already charged entire canonical checkout; immutable in-memory CSS expansion'})
    return {'pin': FOUNDATION, 'bytes': sum(e['chargeBytes'] for e in entries), 'postimages': entries, 'canonicalCss': canonical_css}


def inventory(root, foundation=False):
    common = Path(subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=root, text=True).strip())
    objects = {line.split()[0] for line in subprocess.check_output(
        ['git', 'rev-list', '--objects', BASE + '..HEAD'], cwd=root, text=True).splitlines()}
    # Retained, already-published Reports source refs remain owned carry.
    objects.update(line.split()[0] for line in subprocess.check_output(
        ['git', 'rev-list', '--objects', '2492e35bb9fb61380bf97fa936983ec16c0722f4..696197b1b63e4ef96a832b518ff777f134a9ccf1'], cwd=root, text=True).splitlines())
    objects.update(line.split()[0] for line in subprocess.check_output(
        ['git', 'rev-list', '--objects', '0c9e9613c7e5890953f3a0de14c433ea5b7d0ee1..bc38036e75ab82fdc9fc5c436365249b572c54fd'], cwd=root, text=True).splitlines())
    if foundation:
        objects.update(line.split()[0] for line in subprocess.check_output(
            ['git', 'rev-list', '--objects', CONTRACT + '^..' + FOUNDATION], cwd=root, text=True).splitlines())
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
