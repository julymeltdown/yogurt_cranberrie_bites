import { defineConfig } from 'vitest/config'
const cache = process.env.MCR_FEED_CACHE
if (!cache?.startsWith('/')) throw Error('Use the accepted private Thread author allocation cache')
export default defineConfig({ cacheDir: cache + '/thread-author-vitest', test: {
  include: ['src/features/event-feed/__tests__/thread-author/author.test.tsx'],
  maxWorkers: 1, fileParallelism: false, environment: 'jsdom',
  environmentOptions: { jsdom: { url: 'https://mcr.example' } },
} })
