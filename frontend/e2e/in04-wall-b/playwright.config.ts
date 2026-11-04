import { defineConfig } from '@playwright/test'
// --list is source discovery only. Actual execution requires a fresh accepted
// allocation, private artifact directory, deployment port and real credentials.
export default defineConfig({
  testDir: '.', testMatch: 'wall-b.spec.ts', workers: 1, retries: 0,
  fullyParallel: false, forbidOnly: true, timeout: 600000,
  outputDir: process.env.MCR_IN04_B_ARTIFACT_ROOT ?? '.unallocated-do-not-run',
  reporter: [['line']], use: { trace: 'off', video: 'off', screenshot: 'off' },
})
