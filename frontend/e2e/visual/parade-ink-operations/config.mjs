import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { resolve, dirname } from 'node:path'

export const repo = resolve(import.meta.dirname, '../../../..')
export const frontend = resolve(repo, 'frontend')
export const deps = process.env.PI12_DEPS_ROOT
assert(deps, 'Retained dependency root required')
export const requireDependency = createRequire(resolve(deps, '../package.json'))
const pkg = JSON.parse(readFileSync(resolve(frontend, 'package.json')))
export const aliases = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies }).map(name => ({ find: name, replacement: resolve(deps, name) }))
export const readset = JSON.parse(readFileSync(resolve(repo, 'docs/mcr/evidence/parade-ink-operations/readset.json')))
export function pinnedComposition() {
  // TEST_ONLY source-qualified immutable composition; never overwrite protected files.
  const overlay = new Map(readset.fe37SourcePassedReadonlyPostimages.map(entry => {
    const bytes = execFileSync('git', ['show', `${entry.source}:${entry.path}`], { cwd: repo })
    assert.equal(createHash('sha256').update(bytes).digest('hex'), entry.sha256)
    return [resolve(repo, entry.path), bytes.toString()]
  }))
  return {
    name: 'pi12-readonly-fe37-source-qualified-composition', enforce: 'pre',
    resolveId(source, importer) {
      const path = source.startsWith('/src/') ? resolve(frontend, source.slice(1))
        : source.startsWith('.') && importer ? resolve(dirname(importer.split('?')[0]), source) : source
      return [path, `${path}.ts`, `${path}.tsx`, resolve(path, 'index.ts')].find(candidate => overlay.has(candidate))
    },
    load(id) { return overlay.get(id.split('?')[0]) },
  }
}
