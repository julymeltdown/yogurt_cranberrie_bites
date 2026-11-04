import { defineConfig, mergeConfig } from 'vitest/config'
import base from '../../../../../vite.config'

const cache = process.env.MCR_MAP_COMPOSITION_TMP
if (!cache?.startsWith('/')) throw Error('Set allocated private MCR_MAP_COMPOSITION_TMP')
export default mergeConfig(base, defineConfig({ cacheDir: cache + '/vite',
  test: { include: [
    'src/app/world-entry/__tests__/map-presentation-v2/*.test.{ts,tsx}',
    'src/app/world-entry/__tests__/map-presentation-v1/*.test.{ts,tsx}',
    'src/app/world-entry/__tests__/reference-scene-root/overview.test.ts',
    'src/app/world-entry/__tests__/reference-scene-root/admission.test.ts',
    'src/world/reference-scene/__tests__/overview.test.ts',
    'src/features/places/__tests__/map-service.test.tsx',
  ], pool: 'threads', maxWorkers: 1, fileParallelism: false },
}))
