import assert from 'node:assert/strict'
import { rm } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { here, root, requireDependency } from './runtime.mjs'

const { createVitest } = await import(pathToFileURL(requireDependency.resolve('vitest/node')).href)
const { configDefaults } = await import(pathToFileURL(requireDependency.resolve('vitest/config')).href)
const cacheDir = resolve(here, '.discovery-cache')
let context
try {
  // Mirror the inspected discovery rules in memory; do not load or edit root plugins/config.
  // This enumerates files only. It never imports or executes test modules.
  context = await createVitest('test', {
    root: resolve(root, 'frontend'), config: false, watch: false,
    include: configDefaults.include,
    exclude: [...configDefaults.exclude, 'e2e/real/**/*.spec.ts'],
  }, { cacheDir, server: { watch: null } })
  const specs = await context.globTestSpecifications(['e2e/privacy', 'src/features/social-sharing/__tests__'])
  const files = specs.map(spec => spec.moduleId)
  assert.ok(files.some(file => file.includes('/src/features/social-sharing/__tests__/')), 'positive Vitest discovery control')
  assert.equal(files.filter(file => file.includes('/e2e/privacy/')).length, 0, 'Node selftest is outside Vitest discovery')
  console.log(JSON.stringify({ result: 'PASS', qualification: 'VITEST_FILE_DISCOVERY_ONLY',
    include: configDefaults.include, privacyVitestFiles: 0,
    positiveControlFiles: files.length, testModulesExecuted: 0,
    sharedConfigChanged: false, broadDiagnostic: 'NOT_GREEN; not rerun' }))
} finally {
  await context?.close()
  await rm(cacheDir, { recursive: true, force: true })
}
