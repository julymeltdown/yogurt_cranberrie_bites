import { defineConfig } from 'vitest/config'
import { env } from 'node:process'
import { resolve } from 'node:path'
const temporary = env.MCR_COMMUNITY_AUTHORITY_TEMP
if (!temporary) throw Error('An allocated owned temporary prefix is required')
export default defineConfig({ cacheDir: resolve(temporary, 'vite-cache'),
  test: { environment: 'node', include: ['src/app/community-authority/__tests__/*.test.ts'], fileParallelism: false, maxWorkers: 1 } })
