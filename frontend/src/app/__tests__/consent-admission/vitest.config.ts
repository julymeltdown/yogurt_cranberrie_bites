import { defineConfig } from 'vitest/config'
if (!process.env.MCR_FE10_CACHE) throw new Error('Allocated private FE10 cache required')
export default defineConfig({ cacheDir: process.env.MCR_FE10_CACHE, test: { maxWorkers: 1, fileParallelism: false,
  include: ['src/app/__tests__/enrollment.test.tsx', 'src/app/__tests__/consent-admission/*.test.{ts,tsx}',
    'src/features/auth/__tests__/session.test.ts', 'src/features/auth/__tests__/sensitive-read.test.ts'] } })
