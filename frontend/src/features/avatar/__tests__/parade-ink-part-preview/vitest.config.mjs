import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import baseline from '../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs'
const require = createRequire(resolve(process.env.MCR_FRONTEND_DEPENDENCIES, 'package.json'))
export default { ...baseline, resolve: { alias: [
  { find: '@testing-library/react', replacement: require.resolve('@testing-library/react') },
  ...baseline.resolve.alias,
] }, test: { ...baseline.test, pool: 'threads', include: [
  'src/entities/avatar/runtime/motion-adoption/__tests__/parade-ink-part-preview/*.test.ts',
  'src/features/avatar/__tests__/parade-ink-part-preview/*.test.tsx',
  'src/world/avatar/__tests__/motion-adoption/renderer.test.ts',
  'src/entities/avatar/runtime/__tests__/reviewed-production/authority.test.ts',
  'src/entities/avatar/runtime/__tests__/reviewed-production/resource.test.ts',
] } }
