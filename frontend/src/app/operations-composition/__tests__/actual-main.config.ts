import { defineConfig } from '@playwright/test'
const remaining = Number(process.env.ADMIN_WORK_END_EPOCH_MS) - Date.now()
export default defineConfig({ testDir: '.', testMatch: 'actual-main.spec.ts', workers: 1, retries: 0,
  fullyParallel: false, timeout: Number.isFinite(remaining) ? Math.max(1, Math.min(120000, remaining)) : 120000, expect: { timeout: 7000 }, reporter: 'line',
  outputDir: `${process.env.ADMIN_TMP ?? '/private/tmp/mcr-admin-unallocated-do-not-run'}/playwright`,
})
