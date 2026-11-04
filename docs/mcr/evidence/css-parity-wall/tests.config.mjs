// Private configuration for the existing four Root-owned presentation tests.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
const root = resolve(import.meta.dirname, '../../../..')
const require = createRequire(resolve(root, 'frontend/package.json'))
const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
export default {
  root: resolve(root, 'frontend'), plugins: [react()],
  cacheDir: resolve(process.env.TMPDIR, 'vitest-vite'),
  test: { include: ['src/features/mural/editor/__tests__/reference-target/presentation.test.tsx'],
    watch: false, pool: 'forks', maxWorkers: 1, cache: false },
}
