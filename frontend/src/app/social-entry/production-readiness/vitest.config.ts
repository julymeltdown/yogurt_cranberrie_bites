import { defineConfig } from 'vitest/config'
const cache = process.env.MCR_FEED_CACHE
if (!cache?.startsWith('/')) throw Error('Use an accepted allocation and private cache')
export default defineConfig({ cacheDir: cache + '/production-readiness', test: {
  include: ['src/app/social-entry/production-readiness/*.test.tsx'],
  maxWorkers: 1, fileParallelism: false, environment: 'jsdom',
} })
