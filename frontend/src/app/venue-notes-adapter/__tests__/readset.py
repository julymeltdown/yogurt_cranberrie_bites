"""Freeze a dependency readset without materializing borrowed source."""
import hashlib
import json
from pathlib import Path
import posixpath
import re
import subprocess

ROOT = Path(__file__).resolve().parents[5]
BASE = '59016e7d80a96c89bdab4dd1b18f5faba22f0992'
RAW = '336375622af28116904b8b923651cad00b60806e'
SPATIAL = 'd3dd189c9a340c37f6c6814464a62802ced793b4'
HTTP = '51514ca4074fb131d7fe55577c1766935d71ce60'
PORTS = 'e9535129cffa3917f5b9b5ae3f5ee992609c4419'
OWN = 'frontend/src/app/venue-notes-adapter/'
OUT = ROOT / 'docs/mcr/evidence/venue-notes-adapter/readset.json'

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)

catalogs = {revision: set(git('ls-tree', '-r', '--name-only', revision).decode().splitlines())
            for revision in (BASE, RAW, SPATIAL, PORTS, HTTP)}
def revision(path):
    if path.startswith(OWN):
        return None
    if path in {f'frontend/src/shared/api/{name}' for name in ('http.ts', 'domain.ts', 'domain-operations.generated.ts')}:
        return HTTP
    if path in {f'frontend/src/shared/api/paradise-city-v1/{name}' for name in ('notes.ts', 'notes.generated.ts', 'schema.ts')}:
        return RAW
    if path == 'frontend/src/features/paradise-city-map/spatial/contract.ts':
        return SPATIAL
    if path == 'frontend/src/features/paradise-city-map/ports.ts':
        return PORTS
    return BASE

def exists(path):
    rev = revision(path)
    return (ROOT / path).is_file() if rev is None else path in catalogs[rev]

def resolve(path):
    for candidate in (path, path + '.ts', path + '.tsx', path + '/index.ts', path + '/index.tsx', path + '.d.ts'):
        if exists(candidate):
            return candidate
    raise ValueError('Missing borrowed dependency: ' + path)

def source(path):
    rev = revision(path)
    return (ROOT / path).read_bytes() if rev is None else git('show', rev + ':' + path)

pending = [OWN + 'index.ts', OWN + '__tests__/adapter.test.ts']
rows = {}
while pending:
    path = pending.pop()
    if path in rows:
        continue
    data = source(path)
    rows[path] = dict(path=path, revision=revision(path), sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))
    if path.endswith(('.ts', '.tsx')):
        imports = re.findall(r'(?:from\s*|import\s*\(?\s*)[\'\"](\.[^\'\"]+)[\'\"]', data.decode())
        for name in imports:
            if name.endswith(('.css', '.svg', '.png', '.jpg', '.jpeg')):
                continue
            pending.append(resolve(posixpath.normpath(posixpath.join(posixpath.dirname(path), name))))
OUT.write_text(json.dumps(dict(base=BASE, purpose='Exact focused adapter/raw-client type and runtime closure; no app qualification',
                               paths=sorted(rows.values(), key=lambda row: row['path'])), indent=2) + '\n')
print(json.dumps(dict(files=len(rows), sourceBytes=sum(row['bytes'] for row in rows.values()), readsetSHA256=hashlib.sha256(OUT.read_bytes()).hexdigest())))
