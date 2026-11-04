import { defineConfig } from 'vitest/config'

// Run only the assigned existing regression, with real ticket hashing delayed.
export default defineConfig({
  test: {
    include: ['src/world/network/__tests__/replication.test.ts'],
    setupFiles: ['src/world/network/__tests__/fe17-baseline-recovery/delayed-digest.ts'],
    testNamePattern: 'a gap clears ghosts, requests one resync, discards queued old changes and replaces the complete baseline',
    maxWorkers: 1,
  },
})
