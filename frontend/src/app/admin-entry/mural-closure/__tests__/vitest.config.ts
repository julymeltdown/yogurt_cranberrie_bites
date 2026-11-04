import { defineConfig } from 'vitest/config'
export default defineConfig({ cacheDir: process.env.FE39_TMP ? `${process.env.FE39_TMP}/vite-cache` : '/private/tmp/mcr-fe39-unallocated-do-not-run',
  test: { include: ['src/app/admin-entry/mural-closure/__tests__/entry.test.tsx'], maxWorkers: 1, pool: 'threads', fileParallelism: false } })
