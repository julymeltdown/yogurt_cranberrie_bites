import { defineConfig } from 'vitest/config'
import { env } from 'node:process'
import { resolve } from 'node:path'
const temporary = env.MCR_COMMUNITY_CLOCK_TEMP
if (!temporary) throw Error('An allocated owned temporary prefix is required')
export default defineConfig({ cacheDir: resolve(temporary, 'vite-cache'), test: { environment: 'node',
  include: ['src/app/community-clock-runtime/__tests__/*.test.ts', 'src/app/community-adoption/__tests__/binding.test.ts'],
  fileParallelism: false, maxWorkers: 1 } })
