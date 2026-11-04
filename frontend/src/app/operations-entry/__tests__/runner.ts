import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_OPS_SOURCE_TMP
if (!temporary?.startsWith('/private/tmp/mcr-operations-current-')) throw Error('Accepted private source cache required')
export default defineConfig({ cacheDir: `${temporary}/vite`, test: {
  include: ['src/app/operations-entry/__tests__/*.test.ts'], maxWorkers: 1, fileParallelism: false,
} })
