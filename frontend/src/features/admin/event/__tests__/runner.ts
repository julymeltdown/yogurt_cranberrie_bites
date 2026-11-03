import { defineConfig } from 'vitest/config'
export default defineConfig({ cacheDir: 'src/features/admin/event/__tests__/.scratch-cache/vite', test: {
  include: ['src/features/admin/{event,places,audit}/**/*.test.{ts,tsx}'], maxWorkers: 2,
}, })
