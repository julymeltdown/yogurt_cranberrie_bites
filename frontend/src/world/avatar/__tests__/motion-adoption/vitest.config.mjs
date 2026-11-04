import { createRequire } from 'node:module'
import { resolve } from 'node:path'

// Use the retained installation without installing or writing dependency links.
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies) throw Error('Set MCR_FRONTEND_DEPENDENCIES to the retained frontend directory')
const require = createRequire(resolve(dependencies, 'package.json'))
const packages = require('./package.json')
export const aliases = Object.fromEntries([...Object.keys(packages.dependencies), 'vitest',
  'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client'].filter(name => !name.startsWith('@types/')).map(name => [name,
    name === 'vitest' ? resolve(require.resolve('vitest/package.json'), '../dist/index.js')
      : name === 'phaser' ? resolve(require.resolve('phaser/package.json'), '../dist/phaser.js') : require.resolve(name)]))
export default {
  resolve: { alias: Object.entries(aliases).sort(([a], [b]) => b.length - a.length).map(([name, replacement]) => ({ find: name, replacement })) },
  cacheDir: process.env.MCR_MOTION_CACHE ?? '/private/tmp/mcr-motion-adoption-vitest-cache',
  test: { include: ['src/world/avatar/__tests__/motion-adoption/**/*.test.ts', 'src/features/avatar/__tests__/motion-adoption/**/*.test.tsx',
    'src/app/character-entry/__tests__/motion-adoption/**/*.test.ts',
    'src/entities/avatar/whole-admission/__tests__/admission.test.ts',
    'src/world/network/__tests__/reviewed-motion-v2/boundary.test.ts'],
    environment: 'node', maxWorkers: 1, fileParallelism: false },
}
