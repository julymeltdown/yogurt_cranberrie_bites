import { defineConfig } from 'vitest/config'
export default defineConfig({
  cacheDir: `${process.env.TMPDIR}/vitest-cache`,
  test: { environment: 'node', include: ['src/features/paradise-city-map/__tests__/spatial/*.test.ts'], maxWorkers: 1, fileParallelism: false, pool: 'forks', testTimeout: 10000 },
})
