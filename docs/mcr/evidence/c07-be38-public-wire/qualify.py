"""Owner-local generation and qualification; retained public scripts stay unchanged."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
OUT = Path(os.environ['MCR_RELEASE_OUTPUT'])
CANONICAL = os.environ['MCR_RELEASE_CANONICAL']
CONTRACT_UPDATE = 'service-policy'
DEPENDENCIES = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
assert OUT.is_relative_to(ROOT / 'docs/mcr/evidence/c07-be38-public-wire')
OUT.mkdir(parents=True, exist_ok=True)
def sha(data):
    return hashlib.sha256(data).hexdigest()
def dump(value):
    return json.dumps(value, indent=2, ensure_ascii=False) + '\n'
def module(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value
def record(name, value):
    (OUT / name).write_text(dump(value))
receipt = {'task': 'C07-BE38-PUBLIC-WIRE', 'canonical': CANONICAL,
           'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
           'started': time.time(), 'commands': [], 'status': 'IN_PROGRESS', 'hosted': 'NOT_RUN_NOT_REQUIRED'}
def run(label, arguments):
    assert time.time() < float(os.environ['MCR_RELEASE_DEADLINE'])-60
    result = subprocess.run([str(x) for x in arguments], cwd=ROOT, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, timeout=500)
    (OUT / (label + '.log')).write_text(result.stdout)
    receipt['commands'].append({'name': label, 'argv': [str(x) for x in arguments], 'exit': result.returncode})
    record('qualification.json', receipt)
    if result.returncode:
        raise RuntimeError(label + ' failed; original output retained')
def write_changed(path, text):
    destination = ROOT / path
    data = text.encode()
    if destination.exists() and destination.read_bytes() == data:
        return False
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(data)
    return True
try:
    # The guardian has already opened owner-local logs. Tracked inputs must
    # still match the frozen source commit; untracked outputs are confined here.
    subprocess.run(['git', 'diff', '--quiet', 'HEAD'], cwd=ROOT, check=True)
    untracked = subprocess.check_output(['git', 'ls-files', '--others', '--exclude-standard'], cwd=ROOT, text=True).splitlines()
    assert all(p.startswith('docs/mcr/evidence/c07-be38-public-wire/') for p in untracked)
    run('query-projection-regressions', [sys.executable, '-B', ROOT/'docs/mcr/evidence/c07-be38-public-wire/test_query_projection.py'])
    # Reuse the independently qualified identity phase: application code,
    # selected canonical candidate, assembly and toolchain are unchanged.
    identity_out = ROOT/'docs/mcr/evidence/c07-be38-public-wire/round03/identity'
    identity_receipt = json.loads((identity_out/'qualification.json').read_text())
    identity_readset = json.loads((ROOT/'docs/mcr/evidence/application-release-identity/readset.json').read_text())
    assert identity_receipt['status'] == 'PASS'
    assert all(sha((ROOT/path).read_bytes()) == value for path,value in identity_readset.items())
    assert sha((identity_out/'application-release.json').read_bytes()) == identity_receipt['manifestSha256']
    record('identity-reuse.json', {'qualificationSource':identity_receipt['source'], 'status':'PASS',
        'receiptSha256':sha((identity_out/'qualification.json').read_bytes()),
        'manifestSha256':identity_receipt['manifestSha256'], 'reason':'Exact identity readset unchanged; contract-only correction requires C07 requalification, not another identity run.'})
    qa = module('enrollment_qa01', 'scripts/mcr/contracts/qa01-coverage/run.py')
    active = qa.PIN.read_bytes()
    old = json.loads(active)
    record('active-before.json', {'canonical': old['canonical_candidate'], 'sha256': sha(active)})
    transport = ROOT / 'frontend/src/shared/api/domain-operations.generated.ts'
    before_transport = transport.read_text()
    run('domain-transport-generation', [sys.executable, '-B', ROOT / 'scripts/mcr/generate-domain-transport.py'])
    after_transport = transport.read_text()
    marker = 'export const DOMAIN_OPERATIONS = '
    def entries(text, name):
        body = text.split('export const '+name+' = ', 1)[1].split('} as const', 1)[0]
        return dict(json.loads('{'+line.strip().rstrip(',')+'}').popitem()
                    for line in body.splitlines()[1:] if line.strip())
    before_entries, after_entries = (entries(x, 'DOMAIN_OPERATIONS') for x in (before_transport, after_transport))
    added_transport = sorted(set(after_entries)-set(before_entries))
    assert added_transport == ['getConcertServicePolicy', 'setConcertServicePolicy']
    assert all(after_entries.get(key) == value for key, value in before_entries.items())
    before_queries, after_queries = (entries(x, 'DOMAIN_QUERIES') for x in (before_transport, after_transport))
    assert set(after_queries)-set(before_queries) == {'admin.ServicePolicyQuery'}
    assert all(after_queries.get(key) == value for key, value in before_queries.items())
    assert after_queries['admin.ServicePolicyQuery']['required'] == []
    assert before_transport.split('export const DOMAIN_ERROR_RULES = ',1)[1] == after_transport.split('export const DOMAIN_ERROR_RULES = ',1)[1]
    old_header = before_transport.split(marker,1)[0].splitlines()
    new_header = after_transport.split(marker,1)[0].splitlines()
    assert len(old_header) == len(new_header)
    transport_deltas = [(a,b) for a,b in zip(old_header,new_header) if a != b]
    assert all(any(a.startswith(prefix) and b.startswith(prefix) for prefix in (
        '// contracts/domain/v1/admin.schema.json: ', '// contracts/domain/v1/operations.json: ')) for a,b in transport_deltas)
    record('domain-transport-generation.json', {'beforeSha256': sha(before_transport.encode()),
        'afterSha256': sha(after_transport.encode()), 'runtimeExportsByteIdentical': False,
        'existingOperationsPreserved': True, 'addedOperations': added_transport, 'commentDeltas': transport_deltas})
    # The projection is computed in memory, then applied only to the two granted
    # machine leaves. No ordinary generator's broad write mode is invoked.
    domain = module('enrollment_domain', 'backend/scripts/docs/generate-mcr-domain.py')
    projected = dump(domain.build_document())
    changed = []
    for path in ['contracts/generated/mcr-domain.openapi.json', 'backend/docs/api/mcr-domain.openapi.json']:
        if write_changed(path, projected):
            changed.append(path)
    generator = module('enrollment_c07', 'scripts/mcr/generate-api-client.py')
    previous = json.loads((ROOT / 'contracts/fixtures/c07/responses.json').read_text())
    expected = generator.render()
    old_keys = {(x['operation'], x['status'], x['mediaType']) for x in previous['cases']}
    current = json.loads(expected['contracts/fixtures/c07/responses.json'])
    new_keys = {(x['operation'], x['status'], x['mediaType']) for x in current['cases']}
    assert old_keys <= new_keys, 'Existing C07 operation/status/media cases must remain represented'
    fixed = {'contracts/fixtures/c07/' + name + '.json' for name in ['responses', 'compatibility', 'index']}
    for path, text in expected.items():
        assert path.startswith(('contracts/generated/api-client/', 'frontend/src/shared/api/generated/')) or path in fixed
        if write_changed(path, text):
            changed.append(path)
    generator.verify(expected)
    record('generation.json', {'changed': changed, 'oldResponseCases': len(previous['cases']),
                             'newResponseCases': len(current['cases']), 'removedCaseKeys': [],
                             'addedCaseKeys': sorted(new_keys - old_keys, key=str),
                             'generatedOutputHashes': {p: sha(v.encode()) for p, v in expected.items()}})
    del expected, previous, current
    modules = {f: qa.load(f, 'scripts/mcr/validate-' + f + '.py') for f in qa.FAMILIES}
    registers = {f: qa.read(ROOT / ('contracts/' + f + '/v1/operations.json')) for f in ['core', 'domain', 'realtime']}
    operations = {k: list(v) for k, v in sorted(qa.source_operations(registers, modules['core'].generator).items())}
    inputs = {qa.relative(p): qa.sha(p.read_bytes()) for p in qa.inputs()}
    removed = sorted(set(old['operations']) - set(operations))
    altered = {k: {'old': v, 'new': operations[k]} for k, v in old['operations'].items() if k in operations and operations[k] != v}
    assert not removed and not altered, 'Forward amendment must preserve existing route identities'
    assert len(operations) == 150 and len(registers['domain']) == 117
    assert set(operations)-set(old['operations']) == {'getConcertServicePolicy','setConcertServicePolicy'}
    candidate = {'canonical_candidate': CANONICAL, 'inputs': inputs, 'operations': operations}
    temporary_pin = OUT / 'candidate-pins.json'
    temporary_pin.write_text(dump(candidate))
    audit = {'task': receipt['task'], 'oldCanonical': old['canonical_candidate'], 'newCanonical': CANONICAL,
             'oldActiveSha256': sha(active), 'newActiveSha256': sha(temporary_pin.read_bytes()),
             'inputDeltas': {p: {'old': old['inputs'].get(p), 'new': inputs.get(p)}
                             for p in sorted(set(inputs) | set(old['inputs'])) if inputs.get(p) != old['inputs'].get(p)},
             'addedOperations': {k: operations[k] for k in sorted(set(operations) - set(old['operations']))},
             'removedOperations': removed, 'changedRouteIdentities': altered,
             'provenance': 'Exact e11bd284136851544e54083c70c8679721b9ca1b input-only handoff plus explicitly granted six-pattern strict-end correction b4233d3e6d09e6b86576e1253b0c395e5dfb6faf and explicitly granted pending202/common.CommandReceipt correction 7c563cf09fe971b904ac9549f5e3e8fe1391d94a; canonical publication must include the coherent 150-operation outputs. No live provider/consumer claim.',
             'qualification': 'Pending unchanged full temporary-candidate QA01 preflight'}
    record('forward-audit.json', audit)
    run('preflight', [sys.executable, '-B', ROOT / 'scripts/mcr/contracts/c07/qualify_baseline.py',
                      '--pins', temporary_pin, '--output', OUT / 'preflight.json'])
    assert qa.PIN.read_bytes() == active, 'Preflight must not mutate active pins'
    archive = ROOT / 'scripts/mcr/contracts/qa01-coverage/archive'
    archived_pin = archive / ('pins-' + old['canonical_candidate'] + '.json')
    if archived_pin.exists():
        assert archived_pin.read_bytes() == active, 'Existing archive is immutable'
    else:
        archived_pin.write_bytes(active)
    audit['qualification'] = 'Unchanged full temporary-candidate QA01 PASS before active-pin advance'
    archived_audit = archive / ('forward-baseline-' + CANONICAL + '.json')
    assert not archived_audit.exists(), 'Never overwrite an existing audit'
    archived_audit.write_text(dump(audit))
    qa.PIN.write_bytes(temporary_pin.read_bytes())
    run('qa01', [sys.executable, '-B', ROOT / 'scripts/mcr/contracts/qa01-coverage/run.py'])
    run('qa01-regressions', [sys.executable, '-B', ROOT / 'scripts/mcr/contracts/qa01-coverage/test_coverage.py'])
    run('c07', [sys.executable, '-B', ROOT / 'scripts/mcr/contracts/c07/run.py', '--node', NODE,
                '--dependencies', DEPENDENCIES, '--artifacts', OUT / 'checks'])
    assert archived_pin.read_bytes() == active
    assert all(qa.sha((ROOT / p).read_bytes()) == value for p, value in inputs.items())
    receipt.update(status='PASS', oldPinArchive=qa.relative(archived_pin), auditArchive=qa.relative(archived_audit),
                   activePinsSha256=sha(qa.PIN.read_bytes()), operations=len(operations), inputs=len(inputs))
except Exception as error:
    receipt.update(status='FAIL', error=repr(error))
finally:
    receipt['finished'] = time.time()
    record('qualification.json', receipt)
    print(dump({'status': receipt['status'], 'error': receipt.get('error'), 'output': str(OUT)}).strip())
raise SystemExit(0 if receipt['status'] == 'PASS' else 1)
