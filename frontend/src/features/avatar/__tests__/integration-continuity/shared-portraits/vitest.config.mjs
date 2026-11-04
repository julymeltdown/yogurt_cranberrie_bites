import baseline from '../../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
const require = createRequire(resolve(process.env.MCR_FRONTEND_DEPENDENCIES, 'package.json'))
export default { ...baseline, resolve: { alias: [
  { find: '@testing-library/jest-dom/vitest', replacement: resolve(require.resolve('@testing-library/jest-dom/package.json'), '../dist/vitest.mjs') },
  ...['@testing-library/react', '@hookform/resolvers/zod'].map(name => ({ find: name, replacement: require.resolve(name) })),
  ...baseline.resolve.alias,
] }, test: { ...baseline.test, setupFiles: ['src/features/avatar/__tests__/integration-continuity/shared-portraits/admission.setup.ts'], include: [
  'src/features/avatar/__tests__/integration-continuity/shared-portraits/**/*.test.tsx',
  'src/features/gift-discovery/__tests__/ui.test.tsx',
  'src/features/gift-discovery/__tests__/reference-presentation.test.tsx',
  'src/features/party-discovery/__tests__/ui.test.tsx',
  'src/features/party-discovery/__tests__/reference-presentation.test.tsx',
  'src/app/social-composition/__tests__/provider.test.tsx',
] } }
