import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from '../../../../../vite.config'
const scratch = process.env.MCR_MURAL_SCRATCH
if (!scratch) throw new Error('Use the allocated private scratch directory')
export default mergeConfig(viteConfig, defineConfig({ cacheDir: `${scratch}/vitest`, test: { cache: false, maxWorkers: 1, fileParallelism: false,
  include: ['src/app/world-entry/mural-production/__tests__/*.test.{ts,tsx}', 'src/app/mural-entry/__tests__/*.test.{ts,tsx}',
    'src/world/mural-entry-lifecycle/__tests__/*.test.{ts,tsx}', 'src/app/world-entry/__tests__/production.test.tsx'] } }))
