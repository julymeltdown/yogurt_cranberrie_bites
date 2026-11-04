import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { lstatSync, readFileSync, realpathSync } from 'node:fs'
import { dirname, isAbsolute, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { readRunConfig } from './admission.mjs'

const revision = value => typeof value === 'string' && /^[0-9a-f]{40}$/.test(value)
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value)
const unavailable = () => { throw Error('IN03 serving prerequisites unavailable') }
const adapterPath = 'frontend/src/app/in03-entry/availability.ts'
const launcherPath = 'frontend/e2e/real/social/serve.mjs'
const digest = bytes => createHash('sha256').update(bytes).digest('hex')

function loopbackOrigin(value) {
  if (typeof value !== 'string') return unavailable()
  const url = new URL(value)
  if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || !url.port || Number(url.port) <= 1024
    || url.username || url.password || url.pathname !== '/' || url.search || url.hash || url.origin !== value) return unavailable()
  return url
}

/** Private config is a serving instruction, never a producer/authorization receipt. */
export function parseServingConfiguration(input, now = Date.now()) {
  const keys = ['version', 'allocation', 'origin', 'gatewayOrigin', 'availabilityRevision', 'frontendRevision', 'runtimePacket', 'activeUntil']
  if (!record(input) || Object.keys(input).length !== keys.length || keys.some(key => !Object.hasOwn(input, key))
    || input.version !== 'mcr-in03-serving/v1' || typeof input.allocation !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(input.allocation)
    || !revision(input.availabilityRevision) || !revision(input.frontendRevision)
    || typeof input.runtimePacket !== 'string' || !isAbsolute(input.runtimePacket)
    || typeof input.activeUntil !== 'string' || !Number.isFinite(now) || !(Date.parse(input.activeUntil) > now)) return unavailable()
  const origin = loopbackOrigin(input.origin), gateway = loopbackOrigin(input.gatewayOrigin)
  if (origin.origin === gateway.origin) return unavailable()
  return Object.freeze({ ...input })
}

/** Only checks live borrowed custody/origin/expiry; does not requalify Auth, age or Social. */
export function bindRuntimePacket(config, packet, now = Date.now()) {
  const runtime = packet?.runtime, producer = packet?.authProducer
  if (!record(packet) || !record(runtime) || !record(producer) || runtime.private !== true
    || runtime.origin !== config.origin || packet.browser_origin !== config.origin || packet.gateway_url !== config.gatewayOrigin
    || typeof runtime.eventId !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(runtime.eventId)
    || packet.event_id !== runtime.eventId || producer.eventId !== runtime.eventId || producer.origin !== config.origin
    || typeof runtime.controlOwner !== 'string' || runtime.controlOwner.length < 16 || producer.controlOwner !== runtime.controlOwner
    || producer.custody !== 'PRIVATE_MODE_0600' || typeof runtime.expiresAt !== 'string' || producer.expiresAt !== runtime.expiresAt
    || !Number.isFinite(now) || !(Date.parse(config.activeUntil) > now) || !(Date.parse(runtime.expiresAt) > now)) return unavailable()
  return Math.min(Date.parse(config.activeUntil), Date.parse(runtime.expiresAt))
}

/** Runs before Vite/proxy middleware. Forward the original path, Origin and cookies unchanged. */
export function acceptsRequest(request, config) {
  if (request.headers.host !== new URL(config.origin).host || request.headers.origin !== undefined && request.headers.origin !== config.origin
    || request.headers['sec-fetch-site'] === 'cross-site' || typeof request.url !== 'string' || !request.url.startsWith('/')
    || request.url.startsWith('//') || request.url.includes('\\')) return false
  try {
    const pathname = decodeURIComponent(request.url.split('?')[0])
    return !pathname.startsWith('//') && !pathname.includes('\\') && !pathname.split('/').some(part => part === '..' || part === '.git')
  } catch { return false }
}

export function servingOptions(config, paths, plugins = []) {
  const origin = new URL(config.origin)
  return {
    configFile: false, root: paths.frontend, cacheDir: paths.cache, plugins,
    // Neither root .env files nor ambient VITE_* values are exposed to this harness.
    envDir: false, envPrefix: [], clearScreen: false, logLevel: 'silent',
    define: { 'import.meta.env.VITE_MCR_IN03_AVAILABILITY_REVISION': JSON.stringify(config.availabilityRevision) },
    server: {
      host: '127.0.0.1', port: Number(origin.port), strictPort: true, origin: config.origin,
      allowedHosts: ['127.0.0.1'], cors: false, hmr: false, ws: false,
      fs: { strict: true, allow: [paths.repository, paths.dependencies], deny: ['**/.git/**', '**/.env', '**/.env.*', '**/*.pem', '**/*.key'] },
      proxy: { '^/api(?:/|\\?|$)': {
        target: config.gatewayOrigin, changeOrigin: false, xfwd: false, ws: false, followRedirects: false,
        timeout: 15_000, proxyTimeout: 15_000,
        configure(proxy) {
          proxy.on('error', (_error, _request, response) => {
            if (!response || !('writeHead' in response)) return
            if (!response.headersSent) response.writeHead(502, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' })
            response.end('Gateway connection unavailable.')
          })
        },
      } },
    },
  }
}

/** The accepted allocation's outer guardian owns resources, dependency link and private cache. */
export async function serve(env = process.env) {
  const config = parseServingConfiguration(readRunConfig(env.MCR_IN03_SERVE_CONFIG))
  if (env.MCR_IN03_PUBLIC_ALLOCATION !== config.allocation || env.MCR_IN03_ACTIVE_UNTIL !== config.activeUntil) return unavailable()
  const actualRepository = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..')
  const actualFrontend = resolve(actualRepository, 'frontend')
  const node = env.MCR_IN03_PINNED_NODE, dependencies = env.MCR_IN03_DEPENDENCIES, cache = env.MCR_IN03_SERVE_CACHE
  if (![node, dependencies, cache].every(value => typeof value === 'string' && isAbsolute(value)) || process.version !== 'v24.22.2'
    || realpathSync(process.execPath) !== realpathSync(node) || digest(readFileSync(node)) !== 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a'
    || realpathSync(resolve(actualFrontend, 'node_modules')) !== realpathSync(dependencies)
    || digest(readFileSync(resolve(actualFrontend, 'pnpm-lock.yaml'))) !== digest(readFileSync(resolve(dirname(dependencies), 'pnpm-lock.yaml')))) return unavailable()
  const cacheInfo = lstatSync(cache)
  if (!cacheInfo.isDirectory() || cacheInfo.isSymbolicLink() || cacheInfo.uid !== process.getuid() || (cacheInfo.mode & 0o777) !== 0o700
    || realpathSync(cache).startsWith(realpathSync(actualRepository) + '/') || realpathSync(cache).startsWith(realpathSync(dependencies) + '/')) return unavailable()
  const git = args => execFileSync('git', args, { cwd: actualRepository, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim()
  if (git(['rev-parse', 'HEAD']) !== config.frontendRevision || git(['status', '--porcelain'])) return unavailable()
  for (const [pin, path] of [[config.availabilityRevision, adapterPath], [config.frontendRevision, launcherPath]]) {
    const source = execFileSync('git', ['show', `${pin}:${path}`], { cwd: actualRepository, stdio: ['ignore', 'pipe', 'ignore'] })
    if (digest(source) !== digest(readFileSync(resolve(actualRepository, path)))) return unavailable()
  }
  const readPacket = () => {
    const packet = readRunConfig(config.runtimePacket)
    if (!Number.isSafeInteger(packet.pid) || packet.pid < 2 || packet.uid !== process.getuid()) return unavailable()
    process.kill(packet.pid, 0)
    return bindRuntimePacket(config, packet)
  }
  readPacket()
  let server = null, timer = null, stopping = false
  const stop = async reason => {
    if (stopping) return
    stopping = true; if (timer) clearInterval(timer)
    process.off('SIGTERM', terminate); process.off('SIGINT', terminate)
    server?.httpServer?.closeAllConnections()
    await server?.close()
    process.stdout.write(JSON.stringify({ status: 'STOPPED', reason, actualIN03: 'NOT_RUN' }) + '\n')
  }
  const terminate = () => { void stop('allocation-owner-stop') }
  try {
    // The serving owner accepts runtime resources first; imports do not start a server before preflight.
    const [{ createServer }, { default: react }, { default: tailwindcss }] = await Promise.all([import('vite'), import('@vitejs/plugin-react'), import('@tailwindcss/vite')])
    server = await createServer(servingOptions(config, { repository: actualRepository, frontend: actualFrontend, dependencies: realpathSync(dependencies), cache }, [
      { name: 'in03-strict-borrowed-origin', configureServer(instance) {
        instance.middlewares.use((request, response, next) => {
          if (acceptsRequest(request, config)) return next()
          response.writeHead(421, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' }); response.end('Origin unavailable.')
        })
      } }, react(), tailwindcss(),
    ]))
    process.on('SIGTERM', terminate); process.on('SIGINT', terminate)
    timer = setInterval(() => { try { if (readPacket() <= Date.now()) throw Error(); } catch { void stop('borrowed-runtime-or-clock-ended') } }, 250)
    readPacket(); await server.listen()
    if (stopping) { server.httpServer?.closeAllConnections(); await server.close(); return }
    process.stdout.write(JSON.stringify({ status: 'LISTENING_ONLY', actualIN03: 'NOT_RUN', socialPolicy: 'NOT_QUALIFIED' }) + '\n')
  } catch { await stop('serving-preflight-or-start-failed'); throw Error('IN03 serving unavailable') }
  return { close: () => stop('allocation-owner-stop') }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  serve().catch(() => { process.stderr.write('NOT_RUN: IN03 serving unavailable\n'); process.exitCode = 1 })
}
