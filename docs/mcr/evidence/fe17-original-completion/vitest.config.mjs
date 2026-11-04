import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'

const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies || !process.env.MCR_FE17_CACHE) throw Error('Retained dependencies and private cache are required')
const require = createRequire(resolve(dependencies, 'package.json'))
const target = 'src/world/network/__tests__/replication.test.ts'
const root = resolve(import.meta.dirname, '../../../..')
const packages = require('./package.json')
const aliases = Object.fromEntries([...Object.keys(packages.dependencies), 'vitest', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client'].filter(name => !name.startsWith('@types/')).map(name => [name,
  name === 'vitest' ? resolve(require.resolve('vitest/package.json'), '../dist/index.js')
    : name === 'phaser' ? resolve(require.resolve('phaser/package.json'), '../dist/phaser.js') : require.resolve(name)]))
const baseline = process.env.MCR_FE17_BASELINE === '1'
const retryCancellation = process.env.MCR_FE17_RETRY_CANCELLATION === '1'
const retryTypes = process.env.MCR_FE17_RETRY_TYPES === '1'
const plugins = []
if (baseline) {
  const original = execFileSync('git', ['show', 'ff96d53eb817c93dd1bafb5c4f125630f995413b:frontend/' + target], { cwd: root, encoding: 'utf8' })
  assert.equal(createHash('sha256').update(original).digest('hex'), '250c937446c6322ba4c40c66fd5ea96fde2e08716d18c23da10ed245ba099b79')
  plugins.push({ name: 'exact-published-fe17-fixture-baseline', enforce: 'pre', transform(source, id) {
    if (id.split('?')[0] === resolve(root, 'frontend', target)) return { code: original, map: null }
  } })
}
export default {
  plugins,
  resolve: { alias: Object.entries(aliases).sort(([a], [b]) => b.length - a.length).map(([find, replacement]) => ({ find, replacement })) },
  cacheDir: process.env.MCR_FE17_CACHE,
  test: { include: retryTypes ? ['src/world/network/__tests__/motion-transport/production.test.ts'] : retryCancellation ? ['src/world/network/__tests__/transport.test.ts'] : baseline ? [target] : [target, 'src/world/network/__tests__/prediction.test.ts', 'src/world/network/__tests__/transport.test.ts', 'src/world/network/__tests__/motion-transport/*.test.ts'], setupFiles: [resolve(import.meta.dirname, 'delayed-digest.ts')],
    environment: 'node', maxWorkers: 1, fileParallelism: false, cache: false,
    ...(retryCancellation ? { testNamePattern: 'bounds ticket processing cancellation while its fingerprint computation is pending' }
      : baseline ? { testNamePattern: 'hidden teardown|can start hidden|unexpected close|reentrant disposal' } : {}) },
}
