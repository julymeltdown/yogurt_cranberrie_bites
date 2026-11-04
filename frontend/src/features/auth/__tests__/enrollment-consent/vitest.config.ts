import { defineConfig } from 'vitest/config'
if (!process.env.MCR_FE10_CACHE) throw new Error('Allocated private FE10 cache required')
export default defineConfig({ cacheDir: process.env.MCR_FE10_CACHE,
  test: { maxWorkers: 1, fileParallelism: false, include: ['src/features/auth/__tests__/enrollment.test.ts',
    'src/features/auth/__tests__/enrollment-consent/view.test.tsx', 'src/shared/api/__tests__/http.test.ts', 'src/shared/api/__tests__/enrollment-transport/transport.test.ts'] } })
