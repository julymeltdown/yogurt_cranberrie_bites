import baseline from '../../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs'

export default { ...baseline, cacheDir: process.env.MCR_MOTION_CACHE,
  test: { ...baseline.test, pool: 'threads', include: [...baseline.test.include,
    'src/entities/avatar/runtime/__tests__/reviewed-production/**/*.test.ts',
    'src/features/avatar/__tests__/reviewed-production/**/*.test.ts',
    'src/features/avatar/__tests__/reviewed-production/**/*.test.tsx'] },
}
