// Exact current canonical sources in memory. No borrowed path is materialized.
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { dirname, resolve, relative, extname } from 'node:path'
export const frontend = resolve(import.meta.dirname, '../..'), root = resolve(frontend, '..')
export const canonical = 'ea6027af478981376bbb25f6f80b5bcf347d87ae'
const git = args => execFileSync('git', args, { cwd: root, maxBuffer: 96 * 1024 * 1024 })
const records = git(['ls-tree', '-rz', '--full-tree', canonical, 'frontend/src', 'frontend/public', 'contracts', 'frontend/e2e/reference-integrated']).toString().split('\0').filter(Boolean)
export const catalog = new Map(records.map(row => { const [meta, path] = row.split('\t'); return [path, { hash: meta.split(' ')[2], path }] }))
const owned = new Set(['frontend/src/app/WorldShell.tsx', 'frontend/src/app/router.tsx'])
const sources = [...catalog.values()].filter(x => !owned.has(x.path) && !x.path.includes('/evidence/') && /\.(ts|tsx|css|json|mjs)$/.test(x.path) && !x.path.endsWith('.log'))
const batch = execFileSync('git', ['cat-file', '--batch'], { cwd: root, input: sources.map(x => x.hash).join('\n') + '\n', maxBuffer: 96 * 1024 * 1024 })
export const overlay = new Map(), pins = [], consumed = new Set()
let offset = 0
for (const entry of sources) {
  const end = batch.indexOf(10, offset), [hash, kind, rawSize] = batch.subarray(offset, end).toString().split(' '), size = Number(rawSize)
  if (hash !== entry.hash || kind !== 'blob' || !Number.isSafeInteger(size)) throw Error('Invalid canonical source batch')
  const bytes = batch.subarray(end + 1, end + 1 + size); offset = end + 2 + size
  overlay.set(resolve(root, entry.path), bytes.toString())
  pins.push({ source: canonical, path: entry.path, gitBlob: hash, bytes: size, sha256: createHash('sha256').update(bytes).digest('hex') })
}
export function asset(path) {
  const entry = catalog.get(path)
  if (!entry) throw Error('Unpinned canonical asset: ' + path)
  const bytes = git(['cat-file', 'blob', entry.hash]); consumed.add(path)
  return bytes
}
const assetURL = path => '/__venue_file/' + encodeURIComponent(path)
const suffixes = ['', '.ts', '.tsx', '.mjs', '.css', '.json', '/index.ts', '/index.tsx']
function css(path, stack = []) {
  if (stack.includes(path)) throw Error('Canonical CSS cycle')
  const raw = overlay.get(resolve(root, path)); if (raw === undefined) throw Error('Unpinned CSS')
  consumed.add(path)
  return raw.replace(/@import\s+["'](\.[^"']+)["']\s*(?:layer\(\s*([\w.-]+)\s*\))?\s*;/g, (_m, child, layer) => {
    const content = css(relative(root, resolve(root, dirname(path), child)), [...stack, path])
    return layer ? `@layer ${layer} {\n${content}\n}` : content
  })
    .replace(/url\(\s*["']?([^"')]+)["']?\s*\)/g, (match, url) => {
      if (/^(?:\/|data:|https?:|#)/.test(url)) return match
      const target = relative(root, resolve(root, dirname(path), url)); if (!catalog.has(target)) throw Error('Unpinned CSS asset: ' + target)
      return `url("${assetURL(target)}")`
    })
}
export function defaultInputs() {
  return { name: 'default-current-venue-git-inputs', enforce: 'pre',
    resolveId(id, importer) {
      if (id === 'virtual:venue-root-art') return '\0default-venue-art'
      let clean = id.split('?')[0]
      if (clean.startsWith('/src/') || clean.startsWith('/e2e/')) clean = resolve(frontend, '.' + clean)
      if (clean.startsWith('/@fs/')) clean = clean.slice(4)
      const absolute = clean.startsWith('.') && importer ? resolve(dirname(importer.split('?')[0]), clean) : clean
      for (const suffix of suffixes) if (overlay.has(absolute + suffix)) return absolute + suffix
      const path = relative(root, absolute)
      if (catalog.has(path) && !/\.(ts|tsx|mjs|css|json)$/.test(path)) return '\0default-venue-asset:' + path
    },
    load(id) {
      if (id === '\0default-venue-art') return `export const manifestText=${JSON.stringify(asset('frontend/public/venue/paradise-city/v1/asset-interface.json').toString())};export const imageBase64=${JSON.stringify(asset('frontend/public/venue/paradise-city/v1/siteplan-photo-5.jpg').toString('base64'))};`
      if (id.startsWith('\0default-venue-asset:')) return `export default ${JSON.stringify(assetURL(id.slice(21)))}`
      const clean = id.split('?')[0], source = overlay.get(clean)
      if (source !== undefined) { const path = relative(root, clean); consumed.add(path); return clean.endsWith('.css') ? css(path) : source }
    },
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const url = new URL(request.url, 'http://localhost'), path = url.pathname.startsWith('/__venue_file/')
          ? decodeURIComponent(url.pathname.slice('/__venue_file/'.length)) : 'frontend/public' + url.pathname
        if (!catalog.has(path)) return next()
        const type = { '.json': 'application/json', '.png': 'image/png', '.jpg': 'image/jpeg', '.svg': 'image/svg+xml', '.woff2': 'font/woff2', '.ttf': 'font/ttf' }[extname(path)]
        if (type) response.setHeader('Content-Type', type)
        response.end(asset(path))
      })
    },
  }
}
