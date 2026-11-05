import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
assert.equal(process.env.PI12_SOURCE_ACCEPTED, '1')
const root = resolve(import.meta.dirname, '../../../..')
const deps = process.env.PI12_DEPS_ROOT, output = process.env.PI12_SOURCE_OUT
assert(deps && output)
const pkg = JSON.parse(readFileSync(resolve(root, 'package.json')))
export default {
  root, cacheDir: resolve(output, 'vite-cache'),
  resolve: { alias: Object.keys({ ...pkg.dependencies, ...pkg.devDependencies }).map(name => ({ find: name, replacement: resolve(deps, name) })) },
  test: { pool: 'forks', maxWorkers: 1, fileParallelism: false, globals: false, cache: false,
    include: ['src/app/__tests__/parade-ink-operations/presentation.test.tsx'],
  },
}
