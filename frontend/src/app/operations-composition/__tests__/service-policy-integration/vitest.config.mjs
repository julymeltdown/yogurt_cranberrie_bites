import { resolve } from 'node:path'
import leaf from '../../../../features/admin/service-policy/__tests__/vitest.config.mjs'
export { aliases } from '../../../../features/admin/service-policy/__tests__/vitest.config.mjs'
export default { ...leaf, cacheDir: resolve(process.env.MCR_FE38_SCRATCH, 'composition-vitest'), test: { ...leaf.test,
  include: ['src/app/operations-composition/__tests__/service-policy-integration/*.test.tsx'] } }
