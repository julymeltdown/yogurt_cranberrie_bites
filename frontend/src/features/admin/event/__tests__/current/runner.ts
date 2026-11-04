import { defineConfig } from 'vitest/config'
if (!process.env.MCR_FE38_TEMP) throw Error('Accepted private FE38 resource job required')
export default defineConfig({ cacheDir: process.env.MCR_FE38_TEMP + '/vitest', test: {
  include: ['src/features/admin/{event,places,audit}/**/*.test.{ts,tsx}'], maxWorkers: 1,
} })
