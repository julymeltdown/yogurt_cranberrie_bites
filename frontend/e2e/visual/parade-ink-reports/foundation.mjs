// Immutable PI01 source/API overlay. It does not publish or qualify PI01 itself.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { resolve, dirname, extname, relative } from 'node:path'
import { root, frontend } from './config.mjs'
export const foundationPin = 'b5b44e47531e3b75febc5ee5cdd6a11b1648779c'
const contractCheckpoint = '056a8c48ff667134dee8e5e99710185294e4b898'
const readonlyRoot = '/Users/lhs/Desktop/mcr-worktrees/fe-runtime-preferences'
const canonicalPin = '5ac4bc6e69ee47e6782c2d0b25c72c5544a540d6'
export async function foundationOverlay() {
  const paths = execFileSync('git', ['diff-tree', '--no-commit-id', '--name-only', '-r', contractCheckpoint], { cwd: root, encoding: 'utf8' }).trim().split('\n').filter(p => p.startsWith('frontend/'))
  const bySource = new Map(), byPhysical = new Map(), gitOverrides = new Map(), pins = [], blobs = new Map()
  for (const path of paths) {
    const expected = execFileSync('git', ['show', `${foundationPin}:${path}`], { cwd: root })
    blobs.set(path, expected)
    const physical = resolve(readonlyRoot, path), actual = await readFile(physical)
    if (!expected.equals(actual)) {
      assert(!/\.(?:ttf|woff2?)$/.test(path), `PI01 readonly font changed: ${path}`)
      // Owner follow-up is excluded: serve the exact pinned blob without a source copy.
      gitOverrides.set(physical, expected.toString('utf8'))
    }
    bySource.set(resolve(root, path), physical); byPhysical.set(physical, resolve(root, path))
    pins.push({ path, sha256: createHash('sha256').update(expected).digest('hex'), bytes: expected.length, source: gitOverrides.has(physical) ? 'immutable Git load override' : 'verified readonly file', observedReadonlySha256: createHash('sha256').update(actual).digest('hex') })
  }
  // PostCSS resolves CSS @imports independently of Vite's resolveId/load hooks.
  // Expand these exact imports in memory, preserving cascade order and font URLs.
  async function canonicalBlob(path) {
    if (blobs.has(path)) return blobs.get(path)
    const expected = execFileSync('git', ['show', `${canonicalPin}:${path}`], { cwd: root })
    assert(expected.equals(await readFile(resolve(root, path))), `Canonical transitive source changed: ${path}`)
    blobs.set(path, expected)
    pins.push({ path, sha256: createHash('sha256').update(expected).digest('hex'), bytes: expected.length, source: `canonical ${canonicalPin}` })
    return expected
  }
  async function expandCss(path, ancestors = []) {
    assert(!ancestors.includes(path), `CSS import cycle: ${path}`)
    const text = (await canonicalBlob(path)).toString('utf8')
    const imports = [...text.matchAll(/@import\s+["']([^"']+)["']\s*;/g)]
    let expanded = '', offset = 0
    for (const match of imports) {
      const child = relative(root, resolve(root, dirname(path), match[1]))
      assert(child.startsWith('frontend/src/'), `Unexpected CSS import: ${child}`)
      expanded += text.slice(offset, match.index) + await expandCss(child, [...ancestors, path])
      offset = match.index + match[0].length
    }
    expanded += text.slice(offset)
    return expanded.replace(/url\((["']?)([^"')]+)\1\)/g, (match, _quote, url) => {
      if (/^(?:\/|data:|https?:)/.test(url)) return match
      const own = resolve(root, dirname(path), url), borrowed = bySource.get(own)
      assert(borrowed, `Unpinned relative CSS asset: ${url} in ${path}`)
      return `url("/@fs${borrowed}")`
    })
  }
  for (const path of paths.filter(path => path.endsWith('.css'))) {
    gitOverrides.set(resolve(readonlyRoot, path), await expandCss(path))
  }
  return { pins, plugin: { name: 'o04-immutable-pi01-overlay', enforce: 'pre',
    load(id) { return gitOverrides.get(id.split('?')[0]) ?? null },
    async resolveId(source, importer) {
      if (!importer || !(source.startsWith('.') || source.startsWith('/'))) return null
      const origin = byPhysical.get(importer.split('?')[0]) ?? importer.split('?')[0]
      const candidate = source.startsWith('/src/') ? resolve(frontend, source.slice(1))
        : source.startsWith('/') ? source : resolve(dirname(origin), source)
      for (const suffix of extname(candidate) ? [''] : ['', '.tsx', '.ts', '.css', '/index.tsx', '/index.ts']) {
        const borrowed = bySource.get(candidate + suffix)
        if (borrowed) return borrowed
      }
      if (byPhysical.has(importer.split('?')[0]) && source.startsWith('.'))
        return this.resolve(candidate, origin, { skipSelf: true })
      return null
    },
  } }
}
