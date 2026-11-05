"""Read-only full-inode inventory of candidate-owned shared Git objects."""
from pathlib import Path
import hashlib
import struct
import subprocess

BASE = '5ac4bc6e69ee47e6782c2d0b25c72c5544a540d6'


FOUNDATION = 'b5b44e47531e3b75febc5ee5cdd6a11b1648779c'
CONTRACT = '056a8c48ff667134dee8e5e99710185294e4b898'


def foundation_inventory(root):
    # Selected art/manifest are already charged by the entire retained checkout.
    paths = ['frontend/public/world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json',
        'frontend/public/world/avatars/derived-v1/motion-candidates-v1/avatar-candidates-64-1cd8f05af8d76d58.png',
        'contracts/assets/v2/reviewed/avatar-manifest.schema.json']
    producer = '0dd9f443945e6d6232728280d03e83933ac0eef2'
    manifest_path = 'backend/releases/selected-character-motion-v2/reviewed/manifest.json'
    manifest = subprocess.check_output(['git', 'show', producer + ':' + manifest_path], cwd=root)
    assert len(manifest) == 97407 and hashlib.sha256(manifest).hexdigest() == '1342f2b640a8135c2f128713fe1f59adc0b6da8b925a0ac84808b2d8d1ec884d'
    return {'bytes': 0, 'gitOnlyManifest': {'commit': producer, 'path': manifest_path, 'bytes': len(manifest),
        'sha256': hashlib.sha256(manifest).hexdigest(), 'chargeBasis': 'Entire containing Git inodes charged in owned inventory; no materialization'},
        'postimages': [{'path': name, 'bytes': (root / name).stat().st_size,
        'sha256': hashlib.sha256((root / name).read_bytes()).hexdigest(),
        'chargeBasis': 'Included in entire retained checkout'} for name in paths]}


def inventory(root, foundation=False):
    common = Path(subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=root, text=True).strip())
    objects = {line.split()[0] for line in subprocess.check_output(
        ['git', 'rev-list', '--objects', BASE + '..HEAD'], cwd=root, text=True).splitlines()}
    # Retained, already-published Reports source refs remain owned carry.
    objects.update(line.split()[0] for line in subprocess.check_output(
        ['git', 'rev-list', '--objects', '2492e35bb9fb61380bf97fa936983ec16c0722f4..696197b1b63e4ef96a832b518ff777f134a9ccf1'], cwd=root, text=True).splitlines())
    objects.update(line.split()[0] for line in subprocess.check_output(
        ['git', 'rev-list', '--objects', '0c9e9613c7e5890953f3a0de14c433ea5b7d0ee1..bc38036e75ab82fdc9fc5c436365249b572c54fd'], cwd=root, text=True).splitlines())
    if True:  # Retain the previously owned PI01 Git history in every later round.
        objects.update(line.split()[0] for line in subprocess.check_output(
            ['git', 'rev-list', '--objects', CONTRACT + '^..' + FOUNDATION], cwd=root, text=True).splitlines())
    # The immutable producer manifest may be Git-only in the preserved checkout.
    # Charge complete containing inodes for that exact retained producer revision.
    objects.update(line.split()[0] for line in subprocess.check_output(
        ['git', 'rev-list', '--objects', '0dd9f443945e6d6232728280d03e83933ac0eef2^..0dd9f443945e6d6232728280d03e83933ac0eef2'], cwd=root, text=True).splitlines())
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
