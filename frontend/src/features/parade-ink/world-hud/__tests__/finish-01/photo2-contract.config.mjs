import { defineConfig } from 'vitest/config'
export default defineConfig({
  cacheDir: process.env.PI08_SCRATCH + '/vite-cache',
  test: {
    include: ['src/features/parade-ink/world-hud/__tests__/finish-01/photo2-world-display-v1.test.ts'],
    environment: 'node', pool: 'threads', maxWorkers: 1, fileParallelism: false,
  },
})
