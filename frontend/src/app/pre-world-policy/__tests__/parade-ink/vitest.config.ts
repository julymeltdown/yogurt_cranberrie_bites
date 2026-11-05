import { defineConfig } from 'vitest/config'
import { env } from 'node:process'
import { resolve } from 'node:path'

const temporary = env.MCR_PI12_PREWORLD_TEMP
if (!temporary) throw Error('A fresh allocated PI12 temporary prefix is required')
export default defineConfig({ root: resolve(import.meta.dirname, '../../../../..'),
  cacheDir: resolve(temporary, 'vite-cache'),
  test: { environment: 'jsdom', include: ['src/app/pre-world-policy/__tests__/parade-ink/*.test.tsx'],
    fileParallelism: false, maxWorkers: 1 } })
