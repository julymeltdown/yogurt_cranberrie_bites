import { configDefaults, defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config'

// Real-service Playwright gates must not run under the unit-test runner.
// Their pure helper tests remain discoverable and require no live credentials.
export default mergeConfig(viteConfig, defineConfig({
  test: { exclude: [...configDefaults.exclude, 'e2e/real/**/*.spec.ts'] },
}))
