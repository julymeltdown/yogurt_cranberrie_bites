// Run only under an accepted named browser allocation. No source materialization.
import { execFileSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'

export async function serve() {
  const packet = path.dirname(fileURLToPath(import.meta.url))
  const repo = path.resolve(packet, '../../../..')
  const sourceCommit = process.env.MCR_ART_SOURCE_COMMIT
  if (!/^[0-9a-f]{40}$/.test(sourceCommit ?? '')) throw new Error('An exact source commit is required')
  const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 12 * 1024 * 1024 })
  const files = new Set(git('ls-tree', '-r', '--name-only', sourceCommit, 'frontend', 'contracts').toString().trim().split('\n'))
  const virtual = path.join(packet, '.readonly-git') + path.sep
  const readPaths = new Set()
  const readPinned = (file, parents = []) => {
    if (!files.has(file) || parents.includes(file)) throw new Error(`Invalid pinned input: ${file}`)
    readPaths.add(file)
    const source = git('show', `${sourceCommit}:${file}`).toString()
    if (!file.endsWith('.css')) return source
    // PostCSS reads relative imports from disk before Vite's virtual loader.
    // Inline only pinned relative imports, preserving their unchanged bytes.
    return source.replace(/@import\s+["']([^"']+)["']\s*;/g, (statement, target) => {
      if (!target.startsWith('.')) return statement
      return readPinned(path.posix.normalize(path.posix.join(path.posix.dirname(file), target)), [...parents, file])
    })
  }
  const find = candidate => [candidate, ...['.ts', '.tsx', '.js', '.json', '.css', '/index.ts', '/index.tsx'].map(ext => candidate + ext)].find(file => files.has(file))
  const plugin = {
    name: 'art-readonly-pinned-consumer', enforce: 'pre',
    resolveId(source, importer) {
      if (source.startsWith('\0')) return null
      const queryAt = source.indexOf('?')
      const query = queryAt < 0 ? '' : source.slice(queryAt)
      source = queryAt < 0 ? source : source.slice(0, queryAt)
      importer = importer?.split('?')[0]
      if (source.startsWith('/@fs/')) {
        source = source.slice('/@fs/'.length)
        if (!source.startsWith('/')) source = '/' + source
      }
      if (source.startsWith(virtual)) {
        const file = source.slice(virtual.length)
        return files.has(file) ? source + query : null
      }
      if (source.startsWith('/.readonly-git/')) {
        const file = source.slice('/.readonly-git/'.length)
        return files.has(file) ? virtual + file + query : null
      }
      let candidate
      if (source.startsWith(repo + path.sep)) candidate = path.relative(repo, source)
      else if (source.startsWith('/frontend/') || source.startsWith('/contracts/')) candidate = source.slice(1)
      else if (source.startsWith('.') && importer) {
        const base = importer.startsWith(virtual) ? path.join(repo, importer.slice(virtual.length)) : importer
        candidate = path.relative(repo, path.resolve(path.dirname(base), source))
      }
      if (!candidate || !/^(frontend\/src\/|contracts\/)/.test(candidate)) return null
      const resolved = find(candidate)
      return resolved ? virtual + resolved + query : null
    },
    load(id) {
      if (!id.startsWith(virtual)) return null
      const file = id.slice(virtual.length).split('?')[0]
      if (!files.has(file)) throw new Error(`Unpinned consumer input: ${file}`)
      return readPinned(file)
    },
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        const url = new URL(req.url ?? '/', 'http://127.0.0.1')
        let requestPath = decodeURIComponent(url.pathname)
        if (requestPath.startsWith('/frontend/public/')) requestPath = requestPath.slice('/frontend/public'.length)
        const own = /^\/reference-gifts-content-v1\/(sticker-pack|hand-banner|diy-buttons)\.svg$/.exec(requestPath)
        try {
          if (own) {
            res.setHeader('Content-Type', 'image/svg+xml'); res.setHeader('Cache-Control', 'no-store')
            res.end(await readFile(path.join(repo, 'frontend/public', requestPath.slice(1)))); return
          }
          const file = 'frontend/public' + requestPath
          if (files.has(file) && !requestPath.includes('..')) {
            const mime = { '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2', '.woff': 'font/woff', '.jpg': 'image/jpeg' }[path.extname(file)]
            if (mime) { res.setHeader('Content-Type', mime); res.end(git('show', `${sourceCommit}:${file}`)); return }
          }
          next()
        } catch (error) { next(error) }
      })
    },
  }
  const server = await createServer({ configFile: false, configLoader: 'runner', root: packet, publicDir: false,
    plugins: [plugin, react()], cacheDir: path.join(packet, 'private-cache'),
    server: { host: '127.0.0.1', port: Number(process.env.MCR_ART_PORT ?? 5197), strictPort: true, hmr: false },
    optimizeDeps: { noDiscovery: true, include: ['react', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client', 'react-router-dom', '@tanstack/react-query', 'ky', 'zod'] },
    build: { write: false },
  })
  try { await server.listen() } catch (error) { await server.close(); throw error }
  return { server, packet, sourceCommit, readPaths, baseUrl: `http://127.0.0.1:${server.config.server.port}` }
}
