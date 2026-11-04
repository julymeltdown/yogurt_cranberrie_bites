import { isAbsolute } from 'node:path'
import { defineConfig } from '@playwright/test'

const outputDir = process.env.MCR_IN03_ARTIFACT_ROOT
if (!outputDir || !isAbsolute(outputDir) || !process.env.MCR_IN03_PUBLIC_ALLOCATION)
  throw new Error('NOT_RUN: use the accepted private allocation and artifact directory')

export default defineConfig({
  testDir: '..', testMatch: 'social.spec.ts', outputDir,
  forbidOnly: true, fullyParallel: false, workers: 1, retries: 0, timeout: 180_000,
  reporter: [['line']],
  use: { headless: true, trace: 'off', video: 'off', screenshot: 'off' },
})
