import { defineConfig } from 'vitest/config'
// Explicitly isolated cache; no retained dependency-tree writes or root composition changes.
const cacheDir = process.env.MCR_PREFERENCES_CACHE
if (!cacheDir) throw Error('Set an isolated MCR_PREFERENCES_CACHE for this source check')
export default defineConfig({ cacheDir, test: { exclude: ['**/node_modules/**', '**/e2e/real/**/*.spec.ts'] } })
