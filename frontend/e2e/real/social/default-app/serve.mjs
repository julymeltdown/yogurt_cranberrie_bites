import { execFileSync } from 'node:child_process'
import { lstatSync, readFileSync, realpathSync, writeFileSync } from 'node:fs'
import { dirname, isAbsolute, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { readRunConfig } from '../admission.mjs'
import { acceptsRequest, servingOptions } from '../serve.mjs'
import { bindDefaultInputs, localInputDefine, parseDefaultConfiguration } from './configuration.mjs'
import { bindServingTree, frozenSource, servingTreeCurrent, sha256 } from './source.mjs'

const fail = () => { throw Error('NOT_RUN: default application serving unavailable') }
export function readDefaultInputs(env = process.env, starting = true) {
  const input = parseDefaultConfiguration(readRunConfig(env.MCR_DEFAULT_APP_CONFIG))
  const bound = bindDefaultInputs(input, readRunConfig(input.servingConfig), readRunConfig(input.runtimePacket), Date.now(), starting)
  if (env.MCR_IN03_PUBLIC_ALLOCATION !== bound.serving.allocation || env.MCR_IN03_ACTIVE_UNTIL !== bound.serving.activeUntil) return fail()
  if (!Number.isSafeInteger(bound.packet.pid) || bound.packet.pid < 2 || bound.packet.uid !== process.getuid()) return fail()
  process.kill(bound.packet.pid, 0)
  return { input, ...bound }
}

/** Existing isolated Vite/proxy options, pointed at a verified borrowed frozen application tree. */
export async function serveDefaultApp(env = process.env) {
  const initial = readDefaultInputs(env), repository = resolve(dirname(fileURLToPath(import.meta.url)), '../../../../..')
  const node = env.MCR_IN03_PINNED_NODE, dependencies = env.MCR_IN03_DEPENDENCIES, cache = env.MCR_IN03_SERVE_CACHE
  if (![node, dependencies, cache].every(path => typeof path === 'string' && isAbsolute(path)) || process.version !== 'v24.22.2'
    || realpathSync(node) !== realpathSync(process.execPath)
    || sha256(readFileSync(node)) !== 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a') return fail()
  const git = args => execFileSync('git', args, { cwd: repository, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim()
  if (git(['rev-parse', 'HEAD']) !== initial.input.harnessRevision || git(['status', '--porcelain'])) return fail()
  const proof = bindServingTree(frozenSource(repository, initial.input.publishedRevision), initial.input.servedRoot)
  const frontend = resolve(proof.root, 'frontend'), info = lstatSync(cache)
  if (!info.isDirectory() || info.isSymbolicLink() || info.uid !== process.getuid() || (info.mode & 0o777) !== 0o700
    || realpathSync(cache).startsWith(repository + '/') || realpathSync(cache).startsWith(proof.root + '/')
    || realpathSync(cache).startsWith(realpathSync(dependencies) + '/')
    || realpathSync(resolve(frontend, 'node_modules')) !== realpathSync(dependencies)
    || realpathSync(resolve(repository, 'frontend/node_modules')) !== realpathSync(dependencies)
    || sha256(readFileSync(resolve(frontend, 'pnpm-lock.yaml'))) !== sha256(readFileSync(resolve(dirname(dependencies), 'pnpm-lock.yaml')))) return fail()
  // The frozen module has type-only imports; Node24 erases them without loading World or Auth.
  const validator = await import(pathToFileURL(resolve(frontend, 'src/app/world-entry/configuration.ts')).href)
  const localDefine = localInputDefine(initial.localInput, validator.validateLocalInputConfiguration)
  const releaseValidator = await import(pathToFileURL(resolve(frontend, 'src/shared/release/application.ts')).href)
  if (!releaseValidator.readApplicationRelease(initial.applicationRelease)) return fail()
  const identity = await import(pathToFileURL(resolve(frontend, 'e2e/runtime-adapters/in04-wall-b/application-release.mjs')).href)
  const application = await identity.buildApplicationRelease({ repo: proof.root, candidate: initial.input.publishedRevision,
    dependencies: realpathSync(dependencies), mode: 'vite-serve' })
  if (Object.keys(application.release).some(key => application.release[key] !== initial.applicationRelease[key])) return fail()
  writeFileSync(resolve(cache, 'application-release.json'), identity.canonicalJson(application.manifest), { flag: 'wx', mode: 0o600 })
  const world = new URL(initial.packet.world_ws_url)
  if (!['ws:', 'wss:'].includes(world.protocol) || !['127.0.0.1', '[::1]', 'localhost'].includes(world.hostname)
    || !world.port || Number(world.port) < 1024 || world.pathname !== '/world-ws' || world.search || world.hash || world.username || world.password) return fail()
  const worldTarget = new URL(world); worldTarget.protocol = world.protocol === 'wss:' ? 'https:' : 'http:'; worldTarget.pathname = '/'
  const main = proof.files.find(file => file.path === 'frontend/src/main.tsx')
  const index = proof.files.find(file => file.path === 'frontend/index.html')
  let server, timer, stopping = false
  const stop = async () => {
    if (stopping) return
    stopping = true; clearInterval(timer)
    process.off('SIGTERM', terminate); process.off('SIGINT', terminate)
    server?.httpServer?.closeAllConnections(); await server?.close()
  }
  const terminate = () => { void stop() }
  const assertCurrentInputs = () => {
    const next = readDefaultInputs(env, false)
    if (!servingTreeCurrent(proof) || JSON.stringify(next.input) !== JSON.stringify(initial.input)
      || next.serving.origin !== initial.serving.origin || next.serving.gatewayOrigin !== initial.serving.gatewayOrigin
      || next.eventId !== initial.eventId || next.packet.pid !== initial.packet.pid
      || next.packet.runtime.controlOwner !== initial.packet.runtime.controlOwner
      || next.packet.runtime.expiresAt !== initial.packet.runtime.expiresAt || next.localInputRevision !== initial.localInputRevision
      || JSON.stringify(next.localInput) !== JSON.stringify(initial.localInput)
      || JSON.stringify(next.applicationRelease) !== JSON.stringify(initial.applicationRelease)
      || next.packet.world_ws_url !== initial.packet.world_ws_url) return fail()
  }
  try {
    const [{ createServer }, { default: react }, { default: tailwind }] = await Promise.all([import('vite'), import('@vitejs/plugin-react'), import('@tailwindcss/vite')])
    const options = servingOptions(initial.serving, { frontend, repository: proof.root, dependencies: realpathSync(dependencies), cache }, [
      { name: 'default-app-frozen-main', configureServer(instance) {
        instance.middlewares.use((request, response, next) => {
          const path = request.url?.split('?')[0]
          if (!acceptsRequest(request, initial.serving) || path?.startsWith('/e2e/') || path?.includes('/__tests__/') || !servingTreeCurrent(proof)) {
            response.writeHead(421, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' }); response.end('Application source unavailable.'); return
          }
          if (path === '/src/main.tsx' || path === '/' || path === '/index.html') {
            response.setHeader('X-MCR-Source-Revision', proof.revision)
            response.setHeader('X-MCR-Source-SHA256', path === '/src/main.tsx' ? main.sha256 : index.sha256)
          }
          next()
        })
      } }, react(), tailwind(),
    ], initial.eventId)
    options.define['import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG'] = localDefine
    options.define['import.meta.env.VITE_MCR_APPLICATION_RELEASE'] = JSON.stringify(application.release)
    options.server.proxy['/world-ws'] = { target: worldTarget.origin, changeOrigin: false, ws: true }
    server = await createServer(options)
    process.on('SIGTERM', terminate); process.on('SIGINT', terminate)
    timer = setInterval(() => {
      try {
        assertCurrentInputs()
      } catch { void stop() }
    }, 250)
    assertCurrentInputs()
    await server.listen()
    if (stopping) { server.httpServer?.closeAllConnections(); await server.close(); return }
    process.stdout.write(JSON.stringify({ status: 'LISTENING_ONLY', revision: proof.revision,
      frozenBridge: proof.frozenBridge, sourceManifestSha256: proof.manifestSha256, actualIN03: 'NOT_RUN' }) + '\n')
    return { close: stop }
  } catch { await stop(); return fail() }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  serveDefaultApp().catch(() => { process.stderr.write('NOT_RUN: default application serving unavailable\n'); process.exitCode = 1 })
}
