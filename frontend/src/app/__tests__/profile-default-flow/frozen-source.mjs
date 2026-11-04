// Read-only pinned Git inputs. No peer checkout or historical source is edited.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, existsSync } from 'node:fs'
import { dirname, extname, resolve, relative } from 'node:path'

export function frozenSource(repo, revision) {
  assert(/^[0-9a-f]{40}$/.test(revision), 'An explicit frozen Root source commit is required')
  const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 64 * 1024 ** 2 })
  assert.equal(git('rev-parse', revision).toString().trim(), revision)
  const frontend = resolve(repo, 'frontend'), owned = 'frontend/src/app/__tests__/profile-default-flow/'
  const files = new Set(git('ls-tree', '-r', '--name-only', revision, 'frontend', 'contracts').toString().trim().split('\n'))
  const receipts = new Map(), cache = new Map()
  let cacheBytes = 0
  function bytes(path) {
    assert(files.has(path), 'Path is absent from frozen input: ' + path)
    if (!cache.has(path)) {
      const value = git('show', `${revision}:${path}`)
      cacheBytes += value.length; assert(cacheBytes <= 64 * 1024 ** 2, 'Bound frozen source/public memory')
      cache.set(path, value)
      receipts.set(path, { path, sha256: createHash('sha256').update(value).digest('hex'), bytes: value.length })
    }
    return cache.get(path)
  }
  function pathOf(id) { return relative(repo, id.split('?')[0]).replaceAll('\\', '/') }
  const plugin = {
    name: 'profile-default-frozen-root-source', enforce: 'pre',
    resolveId(source, importer) {
      if (!source.startsWith('.') && !source.startsWith('/')) return null
      const query = source.includes('?') ? source.slice(source.indexOf('?')) : ''
      const clean = source.split('?')[0]
      const absolute = clean.startsWith('/src/') ? resolve(frontend, '.' + clean)
        : clean.startsWith('/') ? clean : importer ? resolve(dirname(importer.split('?')[0]), clean) : null
      if (!absolute) return null
      const path = pathOf(absolute)
      if (path.startsWith(owned)) return null
      for (const suffix of ['', '.ts', '.tsx', '.mjs', '.js', '.json', '/index.ts', '/index.tsx', '/index.mjs']) {
        if (files.has(path + suffix)) return resolve(repo, path + suffix) + query
      }
      return null
    },
    load(id) {
      const path = pathOf(id)
      if (path.startsWith(owned) || !files.has(path) || !/\.(?:tsx?|m?js|json|css)$/.test(path)) return null
      const text = bytes(path).toString()
      if (extname(path) === '.css') {
        // PostCSS reads nested @imports directly. Require those physical bytes to
        // match the same frozen source instead of silently mixing candidates.
        for (const match of text.matchAll(/@import\s+["'](\.[^"']+)["']/g)) {
          const dependency = relative(repo, resolve(repo, dirname(path), match[1]))
          assert(existsSync(resolve(repo, dependency)) && readFileSync(resolve(repo, dependency)).equals(bytes(dependency)), 'Nested CSS needs exact physical frozen bytes: ' + dependency)
        }
      }
      return text
    },
  }
  return { revision, files, bytes, plugin, receipts: () => [...receipts.values()] }
}
