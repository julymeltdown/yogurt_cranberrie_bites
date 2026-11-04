import baseline from '../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs'

export default { ...baseline, test: { ...baseline.test,
  include: ['src/features/avatar/__tests__/integration-continuity/**/*.test.tsx',
    'src/features/avatar/__tests__/reviewed-production/owner.test.tsx',
    'src/features/avatar/__tests__/reviewed-production/controller.test.tsx'],
} }
