import { defineConfig, mergeConfig } from 'vitest/config'
import base from '../../../../../vite.config'
const cache = process.env.MCR_MAP_COMPOSITION_TMP
if (!cache?.startsWith('/')) throw Error('Fresh allocated private TMP required')
export default mergeConfig(base, defineConfig({ cacheDir: cache + '/vite', test: { include: [
  'src/world/reference-scene/map-density-uptake/adoption.test.ts',
  'src/app/world-entry/__tests__/map-presentation-v2/*.test.ts',
  'src/app/world-entry/__tests__/map-presentation-v2/*.test.tsx',
  'src/app/world-entry/__tests__/map-stage-placement/placement.test.ts',
], pool: 'threads', maxWorkers: 1, fileParallelism: false } }))
