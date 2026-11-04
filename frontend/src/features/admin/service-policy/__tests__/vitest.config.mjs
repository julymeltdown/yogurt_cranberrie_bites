import { createRequire } from 'node:module'
import { resolve } from 'node:path'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
const scratch = process.env.MCR_FE38_SCRATCH
if (!dependencies || !scratch) throw Error('Accepted FE38 resource job required')
const require = createRequire(resolve(dependencies, 'package.json'))
const packages = require('./package.json')
const names = [...Object.keys(packages.dependencies), 'vitest', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client', '@testing-library/react']
export const aliases = names.filter(name => !name.startsWith('@types/')).sort((a, b) => b.length - a.length).map(name => ({ find: new RegExp('^' + name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '$'), replacement:
  name === 'vitest' ? resolve(require.resolve('vitest/package.json'), '../dist/index.js') : name === 'phaser' ? resolve(require.resolve('phaser/package.json'), '../dist/phaser.js') : require.resolve(name) }))
export default { resolve: { alias: aliases }, cacheDir: resolve(scratch, 'vitest'), test: { environment: 'jsdom', environmentOptions: { jsdom: { url: 'http://localhost/' } },
  include: ['src/features/admin/service-policy/__tests__/*.test.{ts,tsx}'], maxWorkers: 1, fileParallelism: false } }
