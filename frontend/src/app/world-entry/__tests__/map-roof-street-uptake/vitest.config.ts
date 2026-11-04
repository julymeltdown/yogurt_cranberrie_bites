import { defineConfig, mergeConfig } from 'vitest/config'
import base from '../../../../../vite.config'
const cache = process.env.MCR_MAP_COMPOSITION_TMP
if (!cache?.startsWith('/')) throw Error('Fresh allocated private TMP required')
export default mergeConfig(base, defineConfig({ cacheDir: cache + '/vite', test: { include: [
  'src/world/reference-scene/map-roof-street-uptake/adoption.test.ts',
  'src/app/world-entry/__tests__/map-presentation-v2/registration.test.ts',
  'src/app/world-entry/__tests__/map-presentation-v2/loader.test.ts',
], pool: 'threads', maxWorkers: 1, fileParallelism: false } }))
