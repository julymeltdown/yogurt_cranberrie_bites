import { defineConfig } from 'vitest/config'
const cacheDir = process.env.MCR_C07_CACHE
if (!cacheDir) throw new Error('C07 targeted validation requires its named private cache allocation')
export default defineConfig({
  cacheDir,
  test: { environment: 'node', include: ['src/shared/api/__tests__/c07-social-client/*.test.ts'],
    pool: 'threads', maxWorkers: 1, fileParallelism: false, isolate: true },
})
