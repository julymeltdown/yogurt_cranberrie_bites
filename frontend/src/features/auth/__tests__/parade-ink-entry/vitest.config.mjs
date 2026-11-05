import { aliases, retained } from './runtime.mjs'
export default { cacheDir: process.env.PI10_CACHE, resolve: { alias: [...aliases,
  { find: /^vitest$/, replacement: retained.resolve('vitest').replace(/index\.cjs$/, 'dist/index.js') }] },
  test: { maxWorkers: 1, fileParallelism: false, pool: 'forks',
    include: ['src/features/auth/__tests__/parade-ink-entry/entry.test.tsx', 'src/features/public/__tests__/parade-ink-entry/entry.test.tsx', 'src/features/public/__tests__/reference-target/presentation.test.tsx', 'src/features/auth/__tests__/reference-target/presentation.test.tsx'] } }
