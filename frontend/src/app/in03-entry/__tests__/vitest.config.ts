import { defineConfig } from 'vitest/config'
const cache = process.env.MCR_FEED_CACHE
if (!cache?.startsWith('/')) throw Error('Use an accepted named allocation and owned private cache')
export default defineConfig({ cacheDir: cache + '/vitest-in03', test: {
  include: ['src/app/in03-entry/__tests__/*.test.ts'],
  maxWorkers: 1, fileParallelism: false, environment: 'node',
} })
