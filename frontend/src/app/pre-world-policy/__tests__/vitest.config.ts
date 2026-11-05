import { defineConfig } from 'vitest/config'
import { env } from 'node:process'
import { resolve } from 'node:path'

const temporary = env.MCR_PRE_WORLD_POLICY_TEMP
if (!temporary) throw Error('An allocated owned temporary prefix is required')
export default defineConfig({ root: resolve(import.meta.dirname, '../../../..'),
  cacheDir: resolve(temporary, 'vite-cache'),
  test: { environment: 'node', include: ['src/app/pre-world-policy/__tests__/*.test.{ts,tsx}'],
    fileParallelism: false, maxWorkers: 1 } })
