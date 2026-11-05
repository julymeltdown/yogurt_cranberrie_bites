import { defineConfig } from 'vitest/config'
export default defineConfig({
  cacheDir: `${process.env.TMPDIR}/vitest-cache`,
  test: { include: ['src/app/__tests__/parade-ink-foundation/foundation.test.tsx'],
    maxWorkers: 1, fileParallelism: false, pool: 'forks', testTimeout: 10000 },
})
