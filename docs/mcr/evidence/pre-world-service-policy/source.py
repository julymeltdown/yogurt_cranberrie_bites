"""One serial source round; launch only inside the finite allocated supervisor."""
import os
from pathlib import Path
import subprocess as sp

root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
node = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
deps = root / 'frontend/node_modules'
temporary = Path(os.environ['MCR_PRE_WORLD_POLICY_TEMP'])
assert temporary.parent == Path('/private/tmp')
attempt = Path(os.environ['MCR_PRE_WORLD_POLICY_OUTPUT']).name
prefix = 'src/app/pre-world-policy/__tests__/'
commands = [
    ('focus', [node, str(deps / 'vitest/vitest.mjs'), 'run', '--root', str(root / 'frontend'), '--configLoader', 'runner', '--config', prefix + 'vitest.config.ts', '--no-cache', '--maxWorkers', '1']),
    ('types', [node, str(deps / 'typescript/bin/tsc'), '--noEmit', '--incremental', 'false', '--allowImportingTsExtensions', '--project', 'frontend/tsconfig.json']),
    ('lint', [node, str(deps / 'oxlint/bin/oxlint'), '--deny-warnings', 'frontend/src/app/pre-world-policy', 'frontend/src/app/ports.ts', 'frontend/src/app/WorldShell.tsx']),
    ('syntax', [node, '--check', 'frontend/src/app/pre-world-policy/__tests__/native.mjs']),
    ('plan', ['python3', 'scripts/mcr/check-plan.py']),
]
for name, command in commands:
    log = out / (attempt + '-' + name + '.log')
    assert not log.exists()
    print(name, flush=True)
    with log.open('w') as stream:
        result = sp.run(command, cwd=root, stdout=stream, stderr=sp.STDOUT)
    if result.returncode:
        raise SystemExit(result.returncode)
