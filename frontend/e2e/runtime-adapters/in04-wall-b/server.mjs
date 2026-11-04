// Start only inside a separately allocated, independently guarded runtime window.
import assert from 'node:assert/strict'
import { randomBytes, timingSafeEqual } from 'node:crypto'
import { createRequire } from 'node:module'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { extname, resolve, normalize } from 'node:path'
import { pathToFileURL } from 'node:url'
import { pinnedSource, sha256 } from './pinned-source.mjs'
import { ownedBytes, readSettings } from './inputs.mjs'

export function browserDependencyIncludes(manifest) {
  // Tailwind runs in the Vite server. Optimizing its native Oxide plugin as a
  // browser dependency attempts to decode a .node binary as JavaScript.
  const serverOnly = new Set(['@tailwindcss/vite', 'tailwindcss'])
  return [...Object.keys(manifest.dependencies).filter(name => !serverOnly.has(name)),
    'react-dom/client', 'react/jsx-runtime', 'react/jsx-dev-runtime', '@hookform/resolvers/zod']
}

export async function startProductionFrontend({ repo, candidate, settingsPath, dependencies, cacheDirectory, handoffPath }) {
  const source = pinnedSource(repo, candidate)
  const settingsBytes = await ownedBytes(settingsPath, { privateOnly: true })
  const settings = await readSettings(settingsPath, source)
  assert.equal(sha256(settingsBytes), sha256(await ownedBytes(settingsPath, { privateOnly: true })), 'Settings changed during qualification')
  const frontend = resolve(repo, 'frontend')
  // A different physical application tree is permitted; different dependency
  // declarations/resolutions are not. Never install or alter a dependency tree.
  for (const path of ['package.json', 'pnpm-lock.yaml']) {
    assert((await readFile(resolve(dependencies, '..', path))).equals(source.bytes('frontend/' + path)), 'Dependency source differs: ' + path)
  }
  const require = createRequire(resolve(dependencies, '.in04-resolver.cjs'))
  const manifest = JSON.parse(source.bytes('frontend/package.json').toString())
  const aliases = Object.keys(manifest.dependencies).map(name => ({ find: name, replacement: resolve(dependencies, name) }))
  const [vite, react, tailwind] = await Promise.all(['vite', '@vitejs/plugin-react', '@tailwindcss/vite'].map(name => import(pathToFileURL(require.resolve(name)).href)))
  const token = randomBytes(32).toString('hex'), origin = new URL(settings.origin)
  const settingsSha256 = sha256(settingsBytes)
  const binding = Object.freeze({ candidate, environmentId: settings.environmentId, eventId: settings.eventId, origin: settings.origin, settingsSha256 })
  let serving = false
  let phaserModuleUrl = null, phaserModulePath = null
  const mimeTypes = { '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.ttf': 'font/ttf', '.woff2': 'font/woff2', '.webp': 'image/webp', '.mp3': 'audio/mpeg', '.ogg': 'audio/ogg' }
  const server = await vite.createServer({
    root: frontend, configFile: false, envFile: false, publicDir: false, cacheDir: cacheDirectory,
    clearScreen: false, logLevel: 'warn',
    resolve: { alias: aliases },
    plugins: [source.plugin, react.default(), tailwind.default(), {
      name: 'in04-canonical-entry-assets-and-binding',
      configureServer(instance) {
        instance.middlewares.use(async (req, res, next) => {
          try {
            // This observer endpoint is loopback-only and does not authenticate
            // actors or claim service readiness. Never publish its nonce.
            const path = decodeURIComponent(new URL(req.url, settings.origin).pathname)
            if (path === '/.mcr-in04/frontend') {
              const supplied = Buffer.from(String(req.headers['x-mcr-in04-observer'] ?? ''))
              const expected = Buffer.from(token)
              if (!serving || req.method !== 'GET' || supplied.length !== expected.length || !timingSafeEqual(supplied, expected)) {
                res.statusCode = 404; res.end(); return
              }
              const body = Buffer.from(JSON.stringify({ ...binding, expiresAt: new Date(Date.now() + 60_000).toISOString(),
                configuration: settings.mural, phaserModuleUrl }))
              res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store')
              res.setHeader('Content-Length', body.length); res.end(body); return
            }
            if (req.method !== 'GET' && req.method !== 'HEAD') return next()
            const publicPath = 'frontend/public' + path
            if (source.files.has(publicPath)) {
              const body = source.bytes(publicPath)
              res.setHeader('Content-Type', mimeTypes[extname(publicPath)] ?? 'application/octet-stream')
              res.setHeader('Content-Length', body.length); res.setHeader('Cache-Control', 'no-store')
              res.end(req.method === 'HEAD' ? undefined : body); return
            }
            if (!path.startsWith('/api/') && !path.startsWith('/world-ws') && (path === '/index.html' || req.headers.accept?.includes('text/html'))) {
              res.setHeader('Content-Type', 'text/html'); res.setHeader('Cache-Control', 'no-store')
              res.end(await instance.transformIndexHtml(path, source.bytes('frontend/index.html').toString())); return
            }
            // An absent public asset must not be supplied from physical files.
            if (!path.startsWith('/src/') && !path.startsWith('/@') && !path.startsWith('/node_modules/') && !path.startsWith('/api/') && path !== '/world-ws') {
              res.statusCode = 404; res.end(); return
            }
            next()
          } catch { res.statusCode = 503; res.end('Pinned frontend input unavailable') }
        })
      },
    }],
    define: {
      'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(settings.eventId),
      'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(settings.localInput)),
      'import.meta.env.VITE_MCR_MURAL_CONFIG': JSON.stringify(JSON.stringify(settings.mural)),
    },
    // Freeze discovery to this dependency set so a later lazy route cannot
    // change Phaser's browser hash after QA captures the actual import URL.
    optimizeDeps: { include: browserDependencyIncludes(manifest), noDiscovery: true },
    server: { host: origin.hostname === '[::1]' ? '::1' : origin.hostname, port: Number(origin.port), strictPort: true,
      hmr: false, watch: null, cors: false,
      proxy: {
        '/api': { target: settings.gatewayOrigin, changeOrigin: false, ws: false },
        '/world-ws': { target: settings.worldOrigin, changeOrigin: false, ws: true },
      },
      fs: { strict: true, allow: [frontend, dependencies, cacheDirectory] },
    },
  })
  try {
    await server.listen()
    // Obtain the exact versioned module imported by the canonical scene. A
    // guessed/unversioned URL could instantiate a separate Phaser CanvasPool.
    const scene = await server.transformRequest('/src/world/scenes/MapScene.ts')
    const imports = [...scene.code.matchAll(/import\(\s*["']([^"']*\/phaser\.js\?v=[a-f0-9]+)["']\s*\)/g)]
    assert.equal(imports.length, 1, 'Canonical scene must import one optimized Phaser module')
    const moduleUrl = new URL(imports[0][1], settings.origin)
    assert.equal(moduleUrl.origin, settings.origin)
    assert(moduleUrl.pathname.startsWith('/@fs/'), 'Allocated private optimizer cache must be explicit')
    assert.equal(normalize('/' + decodeURIComponent(moduleUrl.pathname.slice('/@fs/'.length))), resolve(cacheDirectory, 'deps/phaser.js'))
    phaserModuleUrl = moduleUrl.href; phaserModulePath = moduleUrl.pathname
    const phaser = await fetch(phaserModuleUrl, { signal: AbortSignal.timeout(30_000) })
    assert.equal(phaser.status, 200, 'Actual same-app Phaser module unavailable')
    await phaser.arrayBuffer()
    serving = true
    await writeFile(handoffPath, JSON.stringify({ ...binding, token, phaserModulePath }) + '\n', { flag: 'wx', mode: 0o600 })
  } catch (error) { serving = false; await server.close(); throw error }
  return { binding, receipts: source.receipts, async close() { serving = false; await server.close() } }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [repo, candidate, settingsPath, dependencies, cacheDirectory, handoffPath] = process.argv.slice(2)
  assert([repo, candidate, settingsPath, dependencies, cacheDirectory, handoffPath].every(Boolean),
    'Usage: server.mjs REPO CANDIDATE PRIVATE_SETTINGS EXISTING_NODE_MODULES ALLOCATED_CACHE NEW_PRIVATE_HANDOFF')
  await mkdir(cacheDirectory, { recursive: false, mode: 0o700 })
  const runtime = await startProductionFrontend({ repo, candidate, settingsPath, dependencies, cacheDirectory, handoffPath })
  let closing = false
  const stop = async () => { if (closing) return; closing = true; await runtime.close() }
  process.once('SIGTERM', () => void stop()); process.once('SIGINT', () => void stop())
  process.stdout.write(JSON.stringify({ kind: 'in04-canonical-frontend', candidate, status: 'LISTENING' }) + '\n')
}
