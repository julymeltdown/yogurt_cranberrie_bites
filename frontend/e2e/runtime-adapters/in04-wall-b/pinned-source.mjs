// Adapted from Profile213's qualified frozen-source.mjs at 28701b24.
// Every application/public response comes from the selected Git tree.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, extname, relative, resolve } from 'node:path'

export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')

export function pinnedSource(repo, revision) {
  assert.match(revision, /^[a-f0-9]{40}$/, 'An exact published source commit is required')
  const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 64 * 1024 ** 2 })
  assert.equal(git('rev-parse', revision + '^{commit}').toString().trim(), revision)
  // Use the local, explicitly fetched canonical ref; no network or branch mutation.
  git('merge-base', '--is-ancestor', revision, 'refs/remotes/origin/master')
  const frontend = resolve(repo, 'frontend')
  const entries = git('ls-tree', '-r', revision, '--', 'frontend', 'contracts').toString().trim().split('\n')
  const files = new Set(entries.filter(line => /^100(?:644|755) blob /.test(line)).map(line => line.split('\t')[1]))
  const cache = new Map(), receipts = new Map()
  let byteCount = 0
  function bytes(path) {
    assert(files.has(path), 'Pinned regular source is absent: ' + path)
    if (!cache.has(path)) {
      const value = git('show', revision + ':' + path)
      byteCount += value.length
      assert(byteCount <= 64 * 1024 ** 2, 'Pinned source memory bound exceeded')
      cache.set(path, value)
      receipts.set(path, { path, sha256: sha256(value), bytes: value.length })
    }
    return cache.get(path)
  }
  const pathOf = id => relative(repo, id.split('?')[0]).replaceAll('\\', '/')
  const plugin = {
    name: 'in04-canonical-git-source', enforce: 'pre',
    resolveId(source, importer) {
      if (!source.startsWith('.') && !source.startsWith('/')) return null
      const query = source.includes('?') ? source.slice(source.indexOf('?')) : ''
      const clean = source.split('?')[0]
      const absolute = clean.startsWith('/src/') ? resolve(frontend, '.' + clean)
        : clean.startsWith('/') ? clean : importer ? resolve(dirname(importer.split('?')[0]), clean) : null
      if (!absolute) return null
      const path = pathOf(absolute)
      for (const suffix of ['', '.ts', '.tsx', '.mjs', '.js', '.json', '/index.ts', '/index.tsx', '/index.mjs']) {
        if (files.has(path + suffix)) return resolve(repo, path + suffix) + query
      }
      // Never fall back to another physical application source revision.
      if (path.startsWith('frontend/src/')) throw new Error('Import absent from pinned application: ' + path)
      return null
    },
    load(id) {
      const path = pathOf(id)
      if (!path.startsWith('frontend/src/') && !path.startsWith('contracts/')) return null
      if (!/\.(?:tsx?|m?js|json|css)$/.test(path)) {
        assert(readFileSync(resolve(repo, path)).equals(bytes(path)), 'Physical source asset differs from pinned source: ' + path)
        return null
      }
      const value = bytes(path).toString()
      if (extname(path) === '.css') {
        // PostCSS loads nested imports directly rather than via Vite's load hook.
        // Require byte equality, including transitive relative imports, first.
        const visited = new Set()
        const check = (name, text) => {
          if (visited.has(name)) return
          visited.add(name)
          for (const match of text.matchAll(/@import\s+(?:url\(\s*)?["'](\.[^"']+)["']/g)) {
            const dependency = relative(repo, resolve(repo, dirname(name), match[1]))
            const pinned = bytes(dependency)
            assert(existsSync(resolve(repo, dependency)) && readFileSync(resolve(repo, dependency)).equals(pinned),
              'Nested CSS physical bytes differ from pinned source: ' + dependency)
            check(dependency, pinned.toString())
          }
        }
        check(path, value)
      }
      return value
    },
  }
  return { revision, files, bytes, plugin, receipts: () => [...receipts.values()] }
}
