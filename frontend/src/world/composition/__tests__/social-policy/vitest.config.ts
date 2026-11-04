import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from '../../../../../vite.config'
const scratch = process.env.MCR_MURAL_SCRATCH
if (!scratch) throw new Error('Use a fresh allocated source round')
export default mergeConfig(viteConfig, defineConfig({ cacheDir: `${scratch}/vitest`, test: {
  cache: false, maxWorkers: 1, fileParallelism: false,
  include: ['src/world/composition/__tests__/social-policy/*.test.ts',
    'src/world/composition/__tests__/composition.test.ts', 'src/app/world-entry/__tests__/production.test.tsx'],
} }))
