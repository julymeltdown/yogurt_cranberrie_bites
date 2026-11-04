import { defineConfig } from '@playwright/test'
const prefix = process.env.MCR_CLOCK_BROWSER_TEMP
if (!prefix || !process.env.MCR_CLOCK_BROWSER_OFFER) throw Error('A fresh allocated clock browser prefix is required')
export default defineConfig({ testDir: '.', testMatch: 'measurement.native.ts', workers: 1, retries: 0,
  fullyParallel: false, timeout: 90000, reporter: 'line', outputDir: `${prefix}/playwright`,
  use: { trace: 'off', screenshot: 'off', video: 'off' } })
