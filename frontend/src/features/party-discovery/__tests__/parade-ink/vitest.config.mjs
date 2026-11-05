// TEST_ONLY read-only composition with the functional owner's exact admission-qualified fixture postimages.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

assert.equal(process.env.PI05_SOURCE_ACCEPTED, '1', 'A separately accepted finite source round is required')
const deps = process.env.PI05_DEPS_ROOT, output = process.env.PI05_SOURCE_OUT
assert(deps && output, 'Private output and retained dependency root are required')
const root = resolve(import.meta.dirname, '../../../../..'), repo = resolve(root, '..')
const fixturePin = 'fa1e566673d61c071e2f6ce50b50a0f8a7a6d91a'
const pins = [
  ['frontend/src/features/party-discovery/__tests__/fixtures/harness.ts', 'ac340dd17a33dbe5c2f8c4e0218a833ebb653359ea2eeba0d7ecca9a693ad8c1'],
  ['frontend/src/features/party-management/__tests__/harness.ts', 'f9b05fac0ca34af49e955771cd493278ce7d9d8d55ab8eb235f03ae8a601e923'],
]
const overlays = new Map(pins.map(([path, hash]) => {
  const bytes = execFileSync('git', ['show', `${fixturePin}:${path}`], { cwd: repo })
  assert.equal(createHash('sha256').update(bytes).digest('hex'), hash)
  return [resolve(repo, path), bytes.toString()]
}))
const pkg = JSON.parse(readFileSync(resolve(root, 'package.json')))
export default {
  root, cacheDir: resolve(output, 'vite-cache'),
  resolve: { alias: Object.keys({ ...pkg.dependencies, ...pkg.devDependencies }).map(name => ({
    find: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?=/|$)`), replacement: resolve(deps, name),
  })) },
  plugins: [{ name: 'pi05-readonly-functional-fixture-postimages', enforce: 'pre', load(id) { return overlays.get(id.split('?')[0]) } }],
  test: { pool: 'forks', maxWorkers: 1, fileParallelism: false, globals: false, cache: false,
    include: [
      'src/features/party-discovery/__tests__/parade-ink/presentation.test.tsx',
      'src/features/party-discovery/__tests__/reference-presentation.test.tsx',
      'src/features/party-discovery/__tests__/ui.test.tsx',
      'src/features/party-management/__tests__/management.test.tsx',
      'src/features/party-authoring/__tests__/parade-ink/presentation.test.tsx',
      'src/features/meet-beacon/__tests__/parade-ink/presentation.test.tsx',
      'src/features/people/__tests__/parade-ink/presentation.test.tsx',
      'src/features/people/__tests__/ui.test.tsx',
      'src/features/intro/__tests__/unicode-length.test.tsx',
      'src/features/intro/__tests__/intro.test.ts',
    ],
  },
}
