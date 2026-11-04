import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_SOCIAL_WIRE_TMP
if (!temporary?.startsWith('/')) throw Error('Set allocated private MCR_SOCIAL_WIRE_TMP')
export default defineConfig({ cacheDir: temporary + '/vite', test: {
  environment: 'node', maxWorkers: 1, fileParallelism: false,
  include: ['src/world/network/__tests__/social-policy-wire.test.ts',
    'src/world/network/__tests__/transport.test.ts', 'src/world/network/__tests__/replication.test.ts'],
} })
