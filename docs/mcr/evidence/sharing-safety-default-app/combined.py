"""One serial changed-composition preflight and sole native cohort. Lease required."""
import os
from pathlib import Path
import subprocess as sp
root = Path(__file__).resolve().parents[4]
node = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
deps = root / 'frontend/node_modules'
output = Path(os.environ['MCR_SHARING_SAFETY_OUTPUT'])
assert output.parent == Path(__file__).resolve().parent and not output.exists()
owned = ['frontend/src/app/safety-entry', 'frontend/src/app/social-composition/owner.ts', 'frontend/src/app/social-composition/ports.ts', 'frontend/src/app/social-composition/sharing.tsx', 'frontend/src/app/social-composition/panels.tsx', 'frontend/src/app/router.tsx', 'frontend/src/app/social-composition/__tests__/sharing-safety-default']
jobs = [
    ('types', [node, str(deps / 'typescript/bin/tsc'), '--noEmit', '--incremental', 'false', '--allowImportingTsExtensions', '--project', 'frontend/tsconfig.json']),
    ('lint', [node, str(deps / 'oxlint/bin/oxlint'), '--deny-warnings', *owned]),
    ('syntax', [node, '--check', 'frontend/src/app/social-composition/__tests__/sharing-safety-default/native.mjs']),
    ('native', [node, 'frontend/src/app/social-composition/__tests__/sharing-safety-default/native.mjs']),
]
for name, command in jobs:
    log = output.parent / (output.name + '-' + name + '.log')
    assert not log.exists()
    print(name, flush=True)
    with log.open('w') as stream:
        result = sp.run(command, cwd=root, stdout=stream, stderr=sp.STDOUT)
    if result.returncode:
        raise SystemExit(result.returncode)
