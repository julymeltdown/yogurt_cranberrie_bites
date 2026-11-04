"""Narrow regression for the observed Chrome shutdown race and fatal error boundaries."""
from pathlib import Path
import json
import subprocess
import tempfile
from guard_disk import measured_du

checks = []
with tempfile.TemporaryDirectory(prefix='guard-disk-', dir=__import__('os').environ['QA07_TEMP']) as directory:
    root = Path(directory)
    (root / 'retained').write_bytes(b'x' * 8192)
    actual = measured_du(root, root, [])
    assert actual >= 8192
    checks.append('actual allocated blocks counted')
    def completed(code, stdout, stderr):
        return lambda args, **_kwargs: subprocess.CompletedProcess(args, code, stdout, stderr)
    races = []
    subtotal = measured_du(root, root, races, completed(1, '12\t' + str(root) + '\n', 'du: ' + str(root / 'TransportSecurity.rotated') + ': No such file or directory\n'))
    assert subtotal == 12288 and len(races) == 1
    checks.append('observed private Chrome ENOENT preserves measured subtotal and records race')
    for label, ephemeral, stderr in [
        ('permission error remains fatal', root, 'du: ' + str(root) + ': Permission denied\n'),
        ('nonprivate missing input remains fatal', root / 'other', 'du: ' + str(root) + ': No such file or directory\n'),
    ]:
        try:
            measured_du(root, ephemeral, [], completed(1, '12\t' + str(root) + '\n', stderr))
        except subprocess.CalledProcessError:
            checks.append(label)
        else:
            raise AssertionError(label)
    missing = root / 'removed-private-root'
    assert measured_du(missing, missing, []) == 0
    checks.append('completed private TMP cleanup counts zero')
print(json.dumps({'status': 'PASS', 'checks': checks}))
