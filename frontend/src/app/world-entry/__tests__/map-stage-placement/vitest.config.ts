import { defineConfig, mergeConfig } from 'vitest/config'
import base from '../../../../../vite.config'
const cache = process.env.MCR_MAP_COMPOSITION_TMP
if (!cache?.startsWith('/')) throw Error('Fresh allocated private TMP required')
export default mergeConfig(base, defineConfig({ cacheDir: cache + '/vite', test: { include: [
  'src/app/world-entry/__tests__/map-stage-placement/placement.test.ts',
  'src/app/world-entry/__tests__/map-presentation-v1/layout.test.ts',
  'src/app/world-entry/__tests__/map-presentation-v2/consumer.test.tsx',
  'src/features/places/__tests__/reference-map/plaque-layout.test.ts',
], pool: 'threads', maxWorkers: 1, fileParallelism: false } }))
