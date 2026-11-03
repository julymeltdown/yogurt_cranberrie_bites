import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: '..', testMatch: 'world.spec.ts', timeout: 180_000, workers: 1, retries: 0,
  outputDir: '/tmp/mcr-in01-world-results', reporter: [['line']],
  use: { trace: 'off', video: 'off', screenshot: 'off' },
})
