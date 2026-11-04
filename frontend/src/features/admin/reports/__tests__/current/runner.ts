import { defineConfig } from 'vitest/config'

const temporary = process.env.MCR_OPS_SOURCE_TMP
if (!temporary?.startsWith('/private/tmp/mcr-operations-current-')) throw new Error('Accepted private source cache required')
export default defineConfig({ cacheDir: `${temporary}/vite`, test: {
  include: ['src/features/admin/reports/__tests__/controller.test.ts',
    'src/features/admin/reports/__tests__/current/*.test.ts'],
  fileParallelism: false, maxWorkers: 1,
} })
