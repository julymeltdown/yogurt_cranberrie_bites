import { defineConfig } from 'vitest/config'
const cache = process.env.MCR_FEED_CACHE
if (!cache?.startsWith('/')) throw Error('Use an accepted allocation and private cache')
export default defineConfig({ cacheDir: cache + '/social-lifetime', test: {
  include: ['src/app/social-entry/__tests__/availability-lifetime/*.test.ts', 'src/app/social-entry/__tests__/availability-lifetime/*.test.tsx'],
  maxWorkers: 1, fileParallelism: false, environment: 'jsdom',
} })
