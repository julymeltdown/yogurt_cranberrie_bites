import { defineConfig } from 'vitest/config'
const cache = process.env.MCR_FEED_CACHE
if (!cache?.startsWith('/')) throw Error('Use the accepted private Feed allocation cache')
export default defineConfig({ cacheDir: cache + '/vitest', test: {
  include: ['src/app/feed-entry/__tests__/production.test.ts'],
  maxWorkers: 1, fileParallelism: false, environment: 'node',
} })
