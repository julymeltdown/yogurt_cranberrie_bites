import { resolve } from 'node:path'
import { defaultInputs, frontend } from './default-readset.mjs'
import { aliases } from '../../src/features/admin/service-policy/__tests__/vitest.config.mjs'
export default { root: frontend, plugins: [defaultInputs()], resolve: { alias: aliases },
 cacheDir: resolve(process.env.MCR_FE38_SCRATCH, 'default-venue-vitest'), test: {
  include: ['src/app/__tests__/venue-host-integration/*.test.tsx'], environment: 'jsdom',
  environmentOptions: { jsdom: { url: 'http://localhost/' } }, maxWorkers: 1, fileParallelism: false,
 } }
