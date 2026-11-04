// TEST ONLY. Run actual main.tsx with a narrow inspection transform and native HTTPS fixtures.
import assert from 'node:assert/strict'
import { createHash, X509Certificate } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, statfs, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const root = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(root, 'frontend')
const output = process.env.MCR_PRODUCTION_REFRESH_OUTPUT
assert(output?.startsWith('/'), 'Set MCR_PRODUCTION_REFRESH_OUTPUT to a new absolute directory')
const disk = await statfs(root); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3, '8GiB disk floor')
await mkdir(output, { mode: 0o700 })
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const json = async path => JSON.parse(await readFile(resolve(root, path), 'utf8'))
const auth = await json('contracts/fixtures/core/auth-success.json')
const bootstrap = await json('contracts/fixtures/core/bootstrap-open.json')
const capabilities = await json('contracts/fixtures/core/auth-capabilities-disabled.json')
capabilities.providers = capabilities.providers.map(p => p.provider === 'EMAIL' ? { ...p, enabled: true, disabled_reason: null } : p)
const report = { status: 'IN_PROGRESS', qualification: 'ACTUAL_PRODUCTION_ENTRY_AND_CONTROLLER_WITH_DETERMINISTIC_HTTP_FIXTURES',
  candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  backendProducer: '0a8c422645458c9397af9289e15089ee3f9cb0ad',
  backendNativeReceiptSha256: 'e46cac5739809af1ead93d707efa699b2790a180d85346e771d82911fc439957',
  sourcePins: {}, checks: [], limits: ['Frontend HTTP responses are deterministic fixtures, not native backend responses',
    'Actual producer evidence is the separately published BE03 qualification at the exact pin',
    'No deployed cookie-domain/TLS, OAuth, Safari, physical-device or world/multiuser acceptance'] }
for (const path of ['frontend/src/main.tsx', 'frontend/src/app/ports.ts', 'frontend/src/app/router.tsx',
  'frontend/src/features/auth/model/session.ts', 'frontend/src/features/auth/model/cookie-policy.ts',
  'frontend/src/features/auth/model/contract.ts', 'frontend/src/shared/api/http.ts', 'frontend/src/shared/query/privacy.ts',
  'frontend/src/app/__tests__/production-refresh/bridge.ts', 'frontend/src/app/__tests__/production-refresh/native.mjs',
  'contracts/fixtures/core/auth-success.json', 'contracts/fixtures/core/auth-capabilities-disabled.json',
  'contracts/fixtures/core/bootstrap-open.json', 'docs/mcr/backend/browser-refresh-qualification.md']) {
  report.sourcePins[path] = hash(await readFile(resolve(root, path)))
}
report.sourcePins['backend/docs/api/browser-refresh-contract.md'] = hash(execFileSync('git', ['show', 'HEAD:backend/docs/api/browser-refresh-contract.md'], { cwd: root }))
const certificateDirectory = await mkdtemp(resolve(tmpdir(), 'mcr-production-refresh-'))
execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-subj', '/CN=127.0.0.1',
  '-keyout', resolve(certificateDirectory, 'key.pem'), '-out', resolve(certificateDirectory, 'cert.pem')], { stdio: 'ignore' })
const key = await readFile(resolve(certificateDirectory, 'key.pem')), cert = await readFile(resolve(certificateDirectory, 'cert.pem'))
const spki = createHash('sha256').update(new X509Certificate(cert).publicKey.export({ type: 'spki', format: 'der' })).digest('base64')
const deferred = () => { let resolve; const promise = new Promise(yes => { resolve = yes }); return { promise, resolve } }
let mode, stats, refreshGate, oldReadGate, loginGate, server, browser
const errors = [], contexts = new Set()
function reset(next = 'success') {
  mode = next; refreshGate = deferred(); oldReadGate = deferred(); loginGate = deferred()
  stats = { refresh: 0, login: 0, logout: 0, active: 0, peak: 0, oldReads: 0, newReads: 0, mutation: 0,
    intent: true, origin: true, originHeaders: [], automaticCookie: true, noRefreshBearer: true }
}
const deny = { code: 'UNAUTHENTICATED', message: 'Synthetic unauthorized', details: { reason: 'AUTHENTICATION_REQUIRED' } }
const cookie = value => `refresh_token=${value}; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=${value ? 1209600 : 0}`
const state = page => page.evaluate(() => window.__PRODUCTION_REFRESH__?.state())
const invoke = (page, method, ...args) => page.evaluate(({ method, args }) => window.__PRODUCTION_REFRESH__[method](...args), { method, args })
const telemetry = page => page.evaluate(() => ({ fetches: window.__REFRESH_TRANSPORT__.fetches, messages: window.__REFRESH_TRANSPORT__.messages,
  channels: window.__REFRESH_TRANSPORT__.channels, setCookieVisible: window.__REFRESH_TRANSPORT__.setCookieVisible, storage: localStorage.length + sessionStorage.length }))
const refreshCalls = async page => (await telemetry(page)).fetches.filter(f => f.path === '/api/auth/refresh').length
async function context() {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, reducedMotion: 'reduce', serviceWorkers: 'block' })
  contexts.add(context)
  await context.addInitScript(() => {
    const transport = { fetches: [], messages: [], channels: 0, setCookieVisible: false }; window.__REFRESH_TRANSPORT__ = transport
    const fetch = window.fetch.bind(window)
    window.fetch = (input, init) => {
      const request = input instanceof Request ? input : new Request(input, init), path = new URL(request.url).pathname
      if (path.startsWith('/api/')) transport.fetches.push({ path, method: request.method, credentials: request.credentials })
      return fetch(input, init).then(response => { transport.setCookieVisible ||= response.headers.has('Set-Cookie'); return response })
    }
    const Channel = window.BroadcastChannel
    window.BroadcastChannel = class extends Channel {
      constructor(name) { super(name); transport.channels++; this.__closed = false }
      postMessage(message) { transport.messages.push(structuredClone(message)); return super.postMessage(message) }
      close() { if (!this.__closed) { this.__closed = true; transport.channels-- } super.close() }
    }
  })
  return context
}
async function pageIn(context, uncertainPeer = false) {
  const page = await context.newPage(); page.on('pageerror', error => errors.push(error.message))
  await page.goto(server.resolvedUrls.local[0] + 'login')
  if (uncertainPeer) await expect.poll(async () => ['ready', 'error'].includes((await state(page))?.auth.capabilities)).toBe(true)
  else await expect.poll(async () => (await state(page))?.auth.capabilities).toBe('ready')
  assert.equal((await state(page)).canonicalPorts, true)
  return page
}
async function login(page) { assert.equal(await invoke(page, 'login'), true); assert.equal((await state(page)).authenticated, true) }
async function prime(page) { await invoke(page, 'prime'); await expect.poll(async () => (await state(page)).private.observed).toBe(true) }
function cleared(value) {
  assert.equal(value.authenticated, false); assert.equal(value.credentialPresent, false)
  assert.deepEqual(value.private, { queries: 0, observed: false, draft: false, subscriptions: 0, oldSignalAborted: true, oldScopeCurrent: false })
}
async function finish(context, name, pages, details = {}) {
  assert.equal(stats.intent, true, 'Exact browser intent'); assert.equal(stats.origin, true, 'Exact allowlisted Origin'); assert.equal(stats.automaticCookie, true); assert.equal(stats.noRefreshBearer, true)
  const snapshots = [], transports = []
  for (const page of pages) {
    snapshots.push(await state(page)); const transport = await telemetry(page)
    assert.equal(transport.storage, 0)
    assert.equal(transport.setCookieVisible, false)
    for (const message of transport.messages) {
      assert.deepEqual(Object.keys(message).sort(), ['type', 'version']); assert.equal(message.version, 1)
      assert(['revoke', 'uncertain', 'hello'].includes(message.type))
    }
    transports.push(transport)
    await invoke(page, 'dispose')
    assert.equal((await telemetry(page)).channels, 0)
  }
  await expect.poll(async () => stats.active).toBe(0)
  const locks = await pages[0].evaluate(() => navigator.locks.query())
  assert.deepEqual(locks, { held: [], pending: [] })
  report.checks.push({ name, status: 'PASS', ...details, server: structuredClone(stats), snapshots, transports,
    cleanup: { channels: 0, heldLocks: 0, pendingLocks: 0 } })
  await context.close(); contexts.delete(context)
}
try {
  server = await createServer({ root: frontend, cacheDir: resolve(output, 'vite-cache'), logLevel: 'error',
    define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(bootstrap.event.event_id) },
    plugins: [{ name: 'production-refresh-root-inspection', enforce: 'pre',
      transform(source, id) {
        if (id !== resolve(frontend, 'src/main.tsx')) return
        // Inspect the actual root-owned objects; never replace its controller/options/router/ports or fetch.
        const suffix = `\nimport { installQualificationBridge } from '/src/app/__tests__/production-refresh/bridge';\ninstallQualificationBridge({auth,ports,router,dispose:()=>{application.unmount();router.dispose();ports.dispose?.();auth.dispose()}});\n`
        report.mainTransform = { originalSha256: hash(source), transformedSha256: hash(source + suffix),
          operation: 'Append test-only inspection of actual main closure and equivalent teardown; factory/options unmodified' }
        return { code: source + suffix, map: null }
      },
      configureServer(vite) {
        vite.middlewares.use(async (req, res, next) => {
          if (!req.url?.startsWith('/api/')) return next()
          res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store')
          const send = (status, value) => { res.statusCode = status; res.end(value === undefined ? undefined : JSON.stringify(value)) }
          // HTTP/2 supplies :authority, not Host. Compare the actual fixed application origin.
          const expectedOrigin = new URL(server.resolvedUrls.local[0]).origin
          if (req.url === '/api/qualification/origin') return send(req.headers.origin === expectedOrigin ? 200 : 403,
            { origin: req.headers.origin === expectedOrigin ? 'same-origin' : 'missing-or-other' })
          if (req.url === '/api/auth/capabilities') return send(200, capabilities)
          if (req.url.endsWith('/bootstrap')) return send(200, bootstrap)
          if (req.url.includes('/qualification/')) {
            if (req.method !== 'GET') { stats.mutation++; return send(401, deny) }
            if (req.headers.authorization === `Bearer ${auth.accessToken}`) {
              stats.oldReads++
              if (req.url.endsWith('/delayed')) await oldReadGate.promise
              return send(401, deny)
            }
            stats.newReads++; return send(200, { authorized: true })
          }
          if (!['/api/auth/login', '/api/auth/refresh', '/api/auth/logout'].includes(req.url)) return send(404, {})
          stats.intent &&= req.headers['x-mcr-intent'] === 'browser'
          stats.origin &&= req.headers.origin === expectedOrigin
          stats.originHeaders.push({ operation: req.url.split('/').at(-1), origin: req.headers.origin === expectedOrigin ? 'same-origin' : 'missing-or-other' })
          if (!stats.intent || !stats.origin) return send(403, { code: 'FORBIDDEN', message: 'Synthetic exact origin/intent policy' })
          stats.active++; stats.peak = Math.max(stats.peak, stats.active)
          try {
            if (req.url === '/api/auth/login') {
              stats.login++
              if (mode === 'locked-login' && stats.login === 1) await loginGate.promise
              res.setHeader('Set-Cookie', cookie('fixture-predecessor')); return send(200, auth)
            }
            stats.automaticCookie &&= req.headers.cookie?.includes('refresh_token=') === true
            if (req.url === '/api/auth/logout') {
              stats.logout++; res.setHeader('Set-Cookie', cookie('')); return send(204)
            }
            stats.refresh++; stats.noRefreshBearer &&= req.headers.authorization === undefined
            if (mode === 'eof' && stats.refresh === 1) { req.socket.destroy(); return }
            if (mode !== 'eof') await refreshGate.promise
            if (mode === '401') return send(401, deny)
            if (mode === '409' || mode === 'eof') return send(409, { code: 'CONFLICT', message: 'Synthetic predecessor conflict' })
            res.setHeader('Set-Cookie', cookie('fixture-successor'))
            return send(200, mode === 'malformed' ? { ...auth, expiresIn: 0 } : { ...auth, accessToken: 'fixture-rotated-access' })
          } finally { stats.active-- }
        })
      } }],
    server: { host: '127.0.0.1', port: 0, strictPort: false, https: { key, cert }, hmr: false } })
  await server.listen()
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: [`--ignore-certificate-errors-spki-list=${spki}`] })
  report.browser = browser.version(); report.node = process.version

  reset(); {
    const c = await context(), page = await pageIn(c)
    report.browserOriginPolicy = await page.evaluate(async () => {
      const results = []
      for (const referrerPolicy of ['no-referrer', 'same-origin']) {
        const response = await fetch('/api/qualification/origin', { method: 'POST', referrerPolicy })
        results.push({ referrerPolicy, status: response.status, ...await response.json() })
      }
      return results
    })
    assert.equal(await refreshCalls(page), 0, 'No automatic cookie restoration at startup')
    await login(page); await prime(page)
    const before = await state(page)
    const cookies = await c.cookies(); assert(cookies.some(c => c.name === 'refresh_token' && c.httpOnly && c.secure && c.sameSite === 'Lax'))
    assert.equal(await page.evaluate(() => document.cookie.includes('refresh_token')), false)
    await invoke(page, 'read', 'first'); await invoke(page, 'read', 'second'); await invoke(page, 'read', 'delayed')
    await expect.poll(() => stats.oldReads).toBe(3); await expect.poll(() => stats.refresh).toBe(1)
    refreshGate.resolve()
    await expect.poll(async () => (await state(page)).results.first?.ok).toBe(true)
    await expect.poll(async () => (await state(page)).results.second?.ok).toBe(true)
    assert.equal(stats.newReads, 2); oldReadGate.resolve()
    await expect.poll(async () => (await state(page)).results.delayed?.ok).toBe(true)
    const after = await state(page)
    assert.equal(after.generation, before.generation); assert.equal(after.tokenVersion, 1)
    assert.equal(after.private.oldScopeCurrent, true); assert.equal(after.private.observed, true)
    assert.equal(stats.refresh, 1); assert.equal(stats.newReads, 3); assert.equal(await refreshCalls(page), 1)
    assert((await telemetry(page)).fetches.filter(f => f.path === '/api/auth/refresh').every(f => f.credentials === 'same-origin'))
    await finish(c, 'actual main concurrent401 single-flight and delayed old-token401', [page], { readReplays: 3, identityGenerationPreserved: true })
  }
  reset(); {
    const c = await context(), page = await pageIn(c); await login(page); await prime(page)
    await invoke(page, 'read', 'mutation', true)
    await expect.poll(async () => (await state(page)).results.mutation?.ok).toBe(false)
    cleared(await state(page)); assert.equal(stats.mutation, 1); assert.equal(stats.refresh, 0); assert.equal(await refreshCalls(page), 0)
    await finish(c, 'mutation401 never refreshes or replays', [page])
  }
  for (const failure of ['401', '409', 'malformed', 'eof']) {
    reset(failure)
    const c = await context(), page = await pageIn(c); await login(page); await prime(page)
    await invoke(page, 'read', 'first'); await invoke(page, 'read', 'second')
    if (failure !== 'eof') { await expect.poll(() => stats.refresh).toBe(1); refreshGate.resolve() }
    await expect.poll(async () => Object.keys((await state(page)).results).length).toBe(2)
    const snapshot = await state(page); cleared(snapshot)
    assert(Object.values(snapshot.results).every(result => !result.ok)); assert.equal(stats.newReads, 0)
    assert.equal(await refreshCalls(page), 1, 'No application retry even when Chromium resends internally')
    assert(stats.refresh >= 1 && stats.refresh <= (failure === 'eof' ? 2 : 1))
    await invoke(page, 'read', 'afterFailure')
    await expect.poll(async () => (await state(page)).results.afterFailure?.ok).toBe(false)
    assert.equal(await refreshCalls(page), 1)
    const peers = [page]
    if (failure === '409') {
      const peer = await pageIn(c, true); peers.push(peer)
      await expect.poll(async () => (await state(peer)).auth.emailEnabled).toBe(false)
      assert.equal(await invoke(peer, 'login'), false, 'Uncertainty reaches a later hello peer')
    }
    await invoke(page, 'logout'); await expect.poll(async () => (await state(page)).results.logout?.ok).toBe(true)
    assert(!(await c.cookies()).some(cookie => cookie.name === 'refresh_token'))
    await finish(c, `failed renewal ${failure} clears private authority without app retry`, peers,
      { applicationRefreshFetches: 1, transportAttempts: stats.refresh, nativeBackendRotation: 'NOT_RUN in this frontend fixture' })
  }
  reset(); {
    const c = await context(), page = await pageIn(c); await login(page); await prime(page)
    await invoke(page, 'read', 'first'); await expect.poll(() => stats.refresh).toBe(1)
    await invoke(page, 'logout'); cleared(await state(page))
    // The browser may already abort the refresh transport. A held server response cannot restore the revoked generation.
    refreshGate.resolve()
    await expect.poll(async () => (await state(page)).results.logout?.ok).toBe(true)
    await expect.poll(async () => (await state(page)).results.first?.ok).toBe(false)
    cleared(await state(page)); assert.equal(stats.newReads, 0); assert.equal(stats.logout, 1); assert.equal(await refreshCalls(page), 1)
    assert(!(await c.cookies()).some(cookie => cookie.name === 'refresh_token'))
    await finish(c, 'logout immediately clears private state and fences held refresh', [page])
  }
  reset('locked-login'); {
    const c = await context(), a = await pageIn(c), b = await pageIn(c)
    const first = invoke(a, 'login'); await expect.poll(() => stats.login).toBe(1)
    const second = invoke(b, 'login')
    await expect.poll(async () => (await b.evaluate(() => navigator.locks.query())).pending.length).toBe(1)
    assert.equal(stats.login, 1); loginGate.resolve(); assert.equal(await first, true); assert.equal(await second, true)
    await expect.poll(async () => (await state(a)).authenticated).toBe(false)
    assert.equal((await state(b)).authenticated, true); assert.equal(stats.peak, 1)
    await prime(b); assert.equal(await invoke(a, 'login'), true)
    await expect.poll(async () => (await state(b)).authenticated).toBe(false)
    cleared(await state(b))
    await finish(c, 'actual browser cookie-policy serializes writers and peer revocation scrubs private state', [a, b])
  }
  assert.deepEqual(errors, []); report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.failure = String(error); report.failedServer = structuredClone(stats); throw error }
finally {
  refreshGate?.resolve(); oldReadGate?.resolve(); loginGate?.resolve()
  for (const context of contexts) await context.close()
  await browser?.close(); await server?.close()
  await rm(resolve(output, 'vite-cache'), { recursive: true, force: true })
  await rm(certificateDirectory, { recursive: true, force: true })
  report.pageErrors = errors; report.finishedAt = new Date().toISOString()
  report.cleanup = { browserClosed: !browser?.isConnected(), serverClosed: !server?.httpServer?.listening, temporaryCertificateRemoved: true }
  await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n')
}
