import { createRequire } from 'node:module'
import { resolve } from 'node:path'
const require = createRequire(resolve(process.env.MCR_FRONTEND_DEPENDENCIES, 'package.json'))
const packages = require('./package.json')
export const aliases = Object.entries(Object.fromEntries([...Object.keys(packages.dependencies), 'vitest', '@testing-library/react', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client'].filter(name => !name.startsWith('@types/')).map(name => [name,
  name === 'vitest' ? resolve(require.resolve('vitest/package.json'), '../dist/index.js')
    : name === 'phaser' ? resolve(require.resolve('phaser/package.json'), '../dist/phaser.js')
      : name === 'zod' ? resolve(require.resolve('zod/package.json'), '../index.js') : require.resolve(name)])))
  .sort(([a], [b]) => b.length - a.length).map(([find, replacement]) => ({ find, replacement }))
export default { resolve: { alias: aliases }, cacheDir: process.env.MCR_FE41_CACHE,
  test: { include: process.env.MCR_FE41_RETRY_VERSION === '1' ? ['src/app/errors/fe41-lifecycle/version.test.ts'] : ['src/app/errors/fe41-lifecycle/*.test.ts', 'src/app/errors/__tests__/observer.test.ts',
    'src/app/errors/__tests__/default-entry.test.ts', 'src/app/errors/__tests__/production-reconnect.test.ts'],
    setupFiles: [resolve(import.meta.dirname, 'setup.ts')], environment: 'node', maxWorkers: 1, fileParallelism: false, cache: false } }
