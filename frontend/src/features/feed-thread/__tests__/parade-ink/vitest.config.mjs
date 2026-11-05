import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_PARADE_FEED_TMP
if (!temporary?.startsWith('/private/tmp/mcr-parade-ink-feed-thread-')) throw Error('Accepted private Parade Ink source allocation required')
export default defineConfig({ cacheDir: `${temporary}/vitest`, test: {
  pool: 'threads', maxWorkers: 1, fileParallelism: false,
  include: ['src/features/feed-thread/__tests__/parade-ink/*.test.tsx'],
} })
