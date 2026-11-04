import baseline from '../../../../entities/avatar/runtime/__tests__/reviewed-production/vitest.config.mjs'
export default { ...baseline, configFile: false, test: { ...baseline.test, include: [...baseline.test.include,
  'src/features/avatar/part-controls/**/*.test.tsx', 'src/features/avatar/wardrobe-options/**/*.test.ts'] } }
