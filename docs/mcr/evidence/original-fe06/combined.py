"""One finite serial FE06 source and changed-composition native cohort."""
import os
from pathlib import Path
import subprocess as sp
root = Path(__file__).resolve().parents[4]
node = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
deps = root / 'frontend/node_modules'
output = Path(os.environ['MCR_FE06_OUTPUT'])
assert output.parent == Path(__file__).resolve().parent and not output.exists()
prefix = 'src/features/theme/__tests__/original-fe06/'
jobs = [
    ('focus', [node, str(deps / 'vitest/vitest.mjs'), 'run', '--root', 'frontend', '--configLoader', 'runner', '--config', prefix + 'vitest.config.ts', '--no-cache', '--maxWorkers', '1', prefix + 'projection.test.tsx']),
    ('types', [node, str(deps / 'typescript/bin/tsc'), '--noEmit', '--incremental', 'false', '--allowImportingTsExtensions', '--project', 'frontend/tsconfig.json']),
    ('lint', [node, str(deps / 'oxlint/bin/oxlint'), '--deny-warnings', 'frontend/src/features/theme/original-fe06', 'frontend/src/features/theme/__tests__/original-fe06', 'frontend/src/app/world-panels/index.tsx']),
    ('plan', ['python3', 'scripts/mcr/check-plan.py']),
    ('syntax', [node, '--check', 'frontend/' + prefix + 'native.mjs']),
    ('native', [node, 'frontend/' + prefix + 'native.mjs']),
]
for name, command in jobs:
    log = output.parent / (output.name + '-' + name + '.log')
    assert not log.exists()
    print(name, flush=True)
    with log.open('w') as stream:
        result = sp.run(command, cwd=root, stdout=stream, stderr=sp.STDOUT)
    if result.returncode: raise SystemExit(result.returncode)
