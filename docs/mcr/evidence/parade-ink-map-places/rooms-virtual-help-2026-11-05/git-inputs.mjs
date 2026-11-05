// Read-only canonical source overlay. Borrowed producer bytes never enter the checkout.
import { execFileSync } from 'node:child_process'
import { readFileSync, existsSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
export const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../../../..')
export const front = resolve(root, 'frontend')
const custody = JSON.parse(readFileSync(new URL('./custody.json', import.meta.url)))
export const canonical = custody.canonicalConsumerBase
const git = args => execFileSync('git', args, { cwd: root, maxBuffer: 64 * 1024 ** 2 })
export const paths = new Set(git(['ls-tree', '-r', '--name-only', canonical]).toString().trim().split('\n'))
const owned = new Set(custody.paths.map(p => p.path))
const tests = git(['ls-tree', '-r', '--name-only', '473b22bdbc65b550973d1c1c7f1feb3d959eee3e', 'frontend/src/features/world-nearby/__tests__/parade-ink']).toString().trim().split('\n')
tests.forEach(p => owned.add(p))
const fixturePrefix = 'frontend/src/app/world-entry/__tests__/parade-ink-map-places/'
for (const name of ['rooms-virtual-help.html', 'rooms-virtual-help.ts']) owned.add(fixturePrefix + name)
const donors = new Map(custody.borrowedProducers.map(p => [p.path, p.revision]))
for (const p of [...owned, ...donors.keys()]) paths.add(p)
const cache = new Map()
export function relative(path) { return path.startsWith(root + '/') ? path.slice(root.length + 1) : null }
export function has(path) { const p = relative(path); return p !== null && paths.has(p) }
export function bytes(path) {
  const p = relative(path) ?? path
  if (owned.has(p)) return existsSync(resolve(root, p)) ? readFileSync(resolve(root, p)) : undefined
  if (!paths.has(p)) return undefined
  if (!cache.has(p)) cache.set(p, git(['show', `${donors.get(p) ?? canonical}:${p}`]))
  return cache.get(p)
}
// Vite/PostCSS otherwise reads nested imports from disk, outside this Git overlay.
// Keep canonical order/layers and each imported file's own relative URL base.
function cssFromGit(id, parents = []) {
  if (parents.includes(id)) throw new Error('Circular frozen CSS import: ' + id)
  const source = bytes(id)
  if (!source) throw new Error('Missing frozen CSS: ' + id)
  const withAssets = source.toString().replace(/url\(\s*(['"]?)(\.[^'"\)]+)\1\s*\)/g, (_match, _quote, path) => {
    const target = relative(resolve(dirname(id), path))
    if (!target || !paths.has(target)) throw new Error('Missing frozen CSS asset: ' + path)
    return 'url("/__pi11_git/' + target + '")'
  })
  const expanded = withAssets.replace(/@import\s+(['"])(\.[^'"]+)\1(?:\s+layer\(([^)]+)\))?\s*;/g, (_match, _quote, path, layer) => {
    const imported = cssFromGit(resolve(dirname(id), path), [...parents, id])
    return layer ? '@layer ' + layer + ' {\n' + imported + '\n}\n' : imported
  })
  if (/@import\s/.test(expanded)) throw new Error('Unsupported frozen CSS import: ' + id)
  return expanded
}

export function gitSourcePlugin() {
  return {
    name: 'pi11-readonly-qualified-git-inputs', enforce: 'pre',
    resolveId(source, importer) {
      if (!importer || source.startsWith('\0') || source.includes('node_modules')) return null
      const clean = source.split('?')[0]
      const candidate = clean.startsWith('/src/') ? resolve(front, '.' + clean)
        : clean.startsWith('.') ? resolve(dirname(importer.split('?')[0]), clean)
          : clean.startsWith(root + '/') ? clean : null
      if (!candidate) return null
      for (const suffix of ['', '.ts', '.tsx', '.mjs', '.json', '/index.ts', '/index.tsx'])
        if (has(candidate + suffix)) return candidate + suffix + (source.includes('?') ? '?' + source.split('?').slice(1).join('?') : '')
      return null
    },
    load(id) {
      if (id.includes('node_modules') || id.includes('?')) return null
      if (has(id) && id.endsWith('.css')) return cssFromGit(id)
      if (has(id) && /\.(?:ts|tsx|mjs|js|json|html)$/.test(id)) return bytes(id).toString()
      return null
    },
  }
}
