import { defineConfig } from 'vitest/config'
export default defineConfig({ cacheDir: `${process.env.ADMIN_TMP ?? '/private/tmp/mcr-admin-unallocated-do-not-run'}/vite-cache`,
  test: { include: ['src/app/operations-composition/__tests__/composition.test.tsx'], maxWorkers: 1, pool: 'threads', fileParallelism: false } })
