#!/usr/bin/env python3
"""C07 source-only qualification using caller-provided retained dependencies."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', required=True, type=Path)
    parser.add_argument('--dependencies', required=True, type=Path)
    parser.add_argument('--artifacts', required=True, type=Path)
    args = parser.parse_args()
    args.artifacts.mkdir(parents=True, exist_ok=True)
    receipt = []

    def run(label, command, expected=0, contains=None, env=None):
        result = subprocess.run([str(x) for x in command], cwd=ROOT, env=env,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                text=True, timeout=180)
        (args.artifacts / (label + '.log')).write_text(result.stdout)
        receipt.append({'name': label, 'argv': [str(x) for x in command], 'exit': result.returncode})
        if result.returncode != expected or (contains and contains not in result.stdout):
            raise RuntimeError(label + ' failed; see ' + str(args.artifacts / (label + '.log')))

    try:
        generator = ROOT / 'scripts/mcr/generate-api-client.py'
        run('drift', [sys.executable, '-B', generator, '--check'])
        for probe in ('schema-mismatch', 'missing-operation', 'output-drift'):
            run(probe, [sys.executable, '-B', generator, '--probe', probe], 1, 'C07 generated drift:')
        run('inventory-regressions', [sys.executable, '-B', HERE / 'test_inventory.py'])
        run('motion-regressions', [sys.executable, '-B', HERE / 'test_motion.py'])
        with tempfile.TemporaryDirectory(prefix='compile-', dir=args.artifacts) as directory:
            temporary = Path(directory)
            for source in (ROOT / 'frontend/src/generated/api-client').iterdir():
                if source.suffix == '.ts' or source.name == 'THIRD_PARTY_NOTICES.txt':
                    shutil.copyfile(source, temporary / source.name)
            shutil.copyfile(HERE / 'typecheck.ts', temporary / 'typecheck.ts')
            (temporary / 'node_modules').symlink_to(args.dependencies.resolve(), target_is_directory=True)
            (temporary / 'package.json').write_text('{"type":"module"}\n')
            config = {'compilerOptions': {'target': 'ES2022', 'module': 'NodeNext', 'moduleResolution': 'NodeNext',
                      'strict': True, 'noUncheckedIndexedAccess': True, 'exactOptionalPropertyTypes': True,
                      'lib': ['ES2022', 'DOM', 'DOM.Iterable'], 'types': [], 'outDir': './build'}, 'include': ['*.ts']}
            (temporary / 'tsconfig.json').write_text(json.dumps(config))
            run('typescript', [args.node, args.dependencies / 'typescript/bin/tsc', '--project', temporary / 'tsconfig.json'])
            # Build output resolves Zod through the parent's read-only dependency link.
            env = {**os.environ, 'C07_REPO_ROOT': str(ROOT), 'C07_COMPILED_ROOT': str(temporary / 'build')}
            run('runtime', [args.node, '--test', HERE / 'test-runtime.mjs'], env=env)
        result = 'PASS'
    except Exception as error:
        result = 'FAIL'
        print(str(error), file=sys.stderr)
    (args.artifacts / 'receipt.json').write_text(json.dumps({'result': result, 'checks': receipt}, indent=2) + '\n')
    print(json.dumps({'result': result, 'checks': len(receipt)}))
    return 0 if result == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
