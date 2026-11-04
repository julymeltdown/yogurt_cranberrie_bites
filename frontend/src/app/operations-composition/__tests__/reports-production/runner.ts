import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_OPS_SOURCE_TMP
if (!temporary?.startsWith('/private/tmp/mcr-operations-current-')) throw Error('Accepted private source cache required')
export default defineConfig({ cacheDir: `${temporary}/vite`, test: {
  include: ['src/app/operations-composition/__tests__/reports-production/*.test.tsx',
    'src/features/admin/reports/__tests__/production/*.test.ts',
    'src/app/operations-composition/__tests__/composition.test.tsx'], maxWorkers: 1, fileParallelism: false,
} })
