import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_PI_PRIVATE_TMP
if (!temporary?.startsWith('/private/tmp/mcr-parade-ink-private-')) throw Error('Accepted private PI12 source allocation required')
export default defineConfig({ cacheDir: `${temporary}/vitest`, test: {
  pool: 'threads', maxWorkers: 1, fileParallelism: false,
  include: ['src/app/__tests__/parade-ink-private-pages/*.test.tsx'],
} })
