import { configDefaults, defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config'

// Real-service Playwright gates and node:test source qualifiers have their own runners.
// Their pure helper tests remain discoverable under src/ or e2e/.
export default mergeConfig(viteConfig, defineConfig({
  test: {
    setupFiles: ['./src/test-setup.ts'],
    exclude: [
      ...configDefaults.exclude,
      'e2e/**/*.spec.ts',
      'src/**/*.spec.ts',
      'e2e/**/*.test.mjs',
      'src/**/*.test.mjs',
      'src/features/paradise-city-map/art-source/qualifier.test.ts',
      'src/features/parade-ink/world-hud/__tests__/photo2-art-source-v1/qualifier.test.ts',
      'src/world/reference-scene/map-density-derived-v1/qualifier.test.ts',
      'src/world/reference-scene/map-facade-derived-v3/foundation.test.ts',
      'src/world/reference-scene/map-facade-derived-v3/qualifier.test.ts',
      'src/world/reference-scene/map-roof-street-derived-v2/qualifier.test.ts',
    ],
  },
}))
