import { resolve } from 'node:path'
import { immutableInputs, frontend } from './readset.mjs'
import { aliases } from '../../src/features/admin/service-policy/__tests__/vitest.config.mjs'
export default { root: frontend, plugins: [immutableInputs()], resolve: { alias: aliases },
  cacheDir: resolve(process.env.MCR_FE38_SCRATCH, 'venue-root-vitest'), test: {
    include: ['e2e/venue-root-binding/*.test.{ts,tsx}'], environment: 'jsdom',
    environmentOptions: { jsdom: { url: 'http://localhost/' } }, maxWorkers: 1, fileParallelism: false,
  } }
