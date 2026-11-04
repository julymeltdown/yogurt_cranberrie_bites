"""Run consumer refusal probes without changing original candidate receipts."""
import copy, hashlib, json, pathlib, shutil, subprocess, tempfile
root = pathlib.Path(__file__).resolve().parents[5]
folder = pathlib.Path(__file__).resolve().parent
node = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
runner = root / 'frontend/e2e/release/qa10/run.mjs'
candidate = 'f30f7871e8dafb0a0cac6d04857bc49b9cbf5d72'
original = folder / 'candidate-f30f7871/receipt.json'
rows = []
def probe(name, args, expected, needle):
    run = subprocess.run([node, str(runner), *args], cwd=root, capture_output=True, text=True, timeout=120)
    result = {'probe': name, 'argv': [str(runner.relative_to(root)), *args], 'expected_exit': expected, 'exit_code': run.returncode, 'stdout':run.stdout, 'stderr':run.stderr}
    assert run.returncode == expected and needle in (run.stdout + run.stderr), result
    rows.append(result)
probe('immutable FAIL receipt verifies with lossless archived JSON', ['verify', '--candidate', candidate, '--receipt', str(original)], 1, '"frontend_checks":"FAIL"')
probe('stale expected candidate rejected', ['verify', '--candidate', 'e227d12ef4bb3ac92b9fdbbfaf954d4a56050081', '--receipt', str(original)], 2, 'Candidate mismatch')
with tempfile.TemporaryDirectory(prefix='mcr-qa10-negative-') as temporary:
    temp = pathlib.Path(temporary)
    # Hardlink immutable evidence, then replace individual negative inputs (never write a hardlink).
    for path in original.parent.iterdir():
        if path.is_file(): (temp / path.name).hardlink_to(path)
    (temp / 'receipt.json').unlink()
    report = json.loads(original.read_text())
    for name, mutate, needle in [
        ('missing gate', lambda r:r['commands'].pop(), 'Duplicate or missing gates'),
        ('empty passing gate', lambda r:r['commands'][0].update(nonempty_count=0), 'empty coverage'),
        ('failed process falsely marked PASS', lambda r:r['commands'][0].update(exit_code=1), 'failed process'),
        ('global acceptance forbidden', lambda r:r.update(status='PASS'), 'never grants global acceptance'),
        ('missing end fence', lambda r:r.update(same_candidate_end=False), 'not rechecked'),
    ]:
        altered = copy.deepcopy(report); mutate(altered)
        (temp / 'receipt.json').write_text(json.dumps(altered))
        probe(name, ['verify', '--candidate', candidate, '--receipt', str(temp/'receipt.json')], 2, needle)
    (temp/'receipt.json').write_text(json.dumps(report))
    (temp/'dependencies.log').unlink(); (temp/'dependencies.log').write_text('altered command evidence\n')
    probe('altered command artifact rejected', ['verify','--candidate',candidate,'--receipt',str(temp/'receipt.json')],2,'Artifact changed')
    (temp/'dependencies.log').unlink(); (temp/'dependencies.log').hardlink_to(original.parent/'dependencies.log')
    (temp/'vitest.json.gz').unlink(); (temp/'vitest.json.gz').write_bytes(b'broken archive')
    probe('corrupt compressed result rejected', ['verify','--candidate',candidate,'--receipt',str(temp/'receipt.json')],2,'header')
probe('run refuses an ancestor rather than current candidate', ['run','--candidate',candidate,'--output',str(folder/'must-not-exist')],2,'Candidate mismatch')
assert not (folder/'must-not-exist').exists()
(folder/'negative-probes.json').write_text(json.dumps({'candidate':candidate,'receipt_sha256':hashlib.sha256(original.read_bytes()).hexdigest(),'status':'PASS','probes':rows},indent=2)+'\n')
print(json.dumps({'probes':len(rows),'status':'PASS'}))
