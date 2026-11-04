import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, readdir, writeFile, statfs } from 'node:fs/promises'
import { resolve, relative, extname } from 'node:path'
import { createServer } from 'node:http'
import { gzipSync, brotliCompressSync } from 'node:zlib'
import { cpus, totalmem, platform, release, arch } from 'node:os'
import { build } from 'vite'
import { chromium, expect } from '@playwright/test'
import { summarize, distribution, growth } from './metrics.mjs'

const root = resolve(import.meta.dirname, '../../..'), frontend = resolve(root, 'frontend')
const run = process.env.QA08_RUN || 'current'; assert(/^[a-z0-9-]+$/.test(run))
const output = resolve(root, 'docs/mcr/evidence/frontend-performance/qa08', run)
assert(process.env.QA08_ALLOCATION, 'A fresh accepted resource allocation is required')
assert(process.env.QA08_CACHE_ROOT?.startsWith('/private/tmp/mcr-qa08-current-'), 'Private allocated cache/profile/TMP root required')
const cache = resolve(process.env.QA08_CACHE_ROOT), prod = resolve(cache, 'production'), fixture = resolve(cache, 'fixture')
const cycles = 5
const windowMs = Number(process.env.QA08_WINDOW_MS || 5000); assert(windowMs >= 1000 && windowMs <= 10000)
const widths = process.env.QA08_WIDTH ? [Number(process.env.QA08_WIDTH)] : [360, 390, 430]
const disk = await statfs(root); assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, '8GiB free-space floor')
await mkdir(resolve(output, '..'), { recursive: true }); await mkdir(output)
const hash = bytes => createHash('sha256').update(bytes).digest('hex'), pins = {}
async function collect(dir) {
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const path = resolve(dir, e.name)
    if (e.isDirectory()) { if (!['evidence', '.cache'].includes(e.name) && !e.name.endsWith('-evidence')) await collect(path) }
    else if (/\.(tsx?|mjs|py|html|css|json|png|svg|webp|jpe?g)$/.test(path)) pins[relative(root, path)] = hash(await readFile(path))
  }
}
for (const dir of ['frontend/src', 'frontend/public', 'frontend/e2e/qa08-rendering', 'contracts/realtime/v1', 'contracts/assets', 'contracts/fixtures/realtime', 'contracts/fixtures/core']) await collect(resolve(root, dir))
for (const p of ['frontend/package.json', 'frontend/pnpm-lock.yaml', 'frontend/vite.config.ts', 'frontend/index.html', 'MCR_IMPLEMENTATION_PLAN_V2_EN.md']) pins[p] = hash(await readFile(resolve(root, p)))
await writeFile(resolve(output, 'input-pins.json'), JSON.stringify(pins, null, 2) + '\n')
const report = { status: 'IN_PROGRESS', candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  environment: { node: process.version, platform: platform(), release: release(), arch: arch(), cpu: cpus()[0]?.model, logicalCpus: cpus().length, totalMemoryBytes: totalmem(), headless: true, viewportHeight: 844, deviceScaleFactor: 1 },
  allocation: process.env.QA08_ALLOCATION,
  method: { windowMs, widths, cycles, warmupCyclesExcludedFromGrowth: 1, optimizedFixtureMode: 'qa08', productionBuildSeparate: true, gcDuringFrameWindows: false, repetitionsPerWidth: 1, avatarSpritesRequired: 6, startupEndpoint: 'LIVE plus six actual selected-character sprites', loss: 'Drop exactly one synthetic inbound Delta, allow the next Delta to expose the C05 sequence gap, answer actual resync with a fresh fixture stream; not TCP packet loss', threshold: 'No performance pass/fail cutoff. Original mobile goal 30FPS is a comparison only; these are desktop Chrome CSS viewports.' },
  browser: null, bundles: {}, scenarios: [], productionLanding: null, coldNetwork: null, c05Policy: JSON.parse(await readFile(resolve(root, 'contracts/realtime/v1/policy.json'))),
  limits: ['Synthetic Auth/HTTP/C05/source qualification, six fixture entities; not live/production admission', 'Desktop headless Chrome viewport results do not establish mid-range mobile-device 30FPS', 'JS heap/DOM counters exclude complete process, GPU and native memory', 'Finite delayed ACK consumer and three serial reconnects are not a server reconnect storm or capacity ceiling', 'Real 200 sockets/four rooms and operator-selected production caps NOT_RUN'] }
let browser, appServer, fixtureServer
const encodedFiles = new Map()
async function inventory(dir) {
  const files = []
  async function visit(d) { for (const e of await readdir(d, { withFileTypes: true })) { const p = resolve(d, e.name); if (e.isDirectory()) await visit(p); else { const b = await readFile(p), gzip = gzipSync(b), brotli = brotliCompressSync(b); encodedFiles.set(p, { body: b, brotli }); files.push({ path: relative(dir, p), bytes: b.length, gzipBytes: gzip.length, brotliBytes: brotli.length, sha256: hash(b) }) } } }
  await visit(dir)
  return { files, totalBytes: files.reduce((n, f) => n + f.bytes, 0), jsBytes: files.filter(f => f.path.endsWith('.js')).reduce((n, f) => n + f.bytes, 0), jsGzipBytes: files.filter(f => f.path.endsWith('.js')).reduce((n, f) => n + f.gzipBytes, 0), jsBrotliBytes: files.filter(f => f.path.endsWith('.js')).reduce((n, f) => n + f.brotliBytes, 0) }
}
async function serve(dir) {
  const types = { '.html': 'text/html', '.js': 'application/javascript', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml', '.woff2': 'font/woff2', '.webp': 'image/webp', '.jpg': 'image/jpeg', '.ico': 'image/x-icon' }
  const server = createServer(async (req, res) => {
    try {
      const path = decodeURIComponent(new URL(req.url, 'http://local').pathname)
      const emitted = resolve(dir, '.' + (path === '/' ? '/index.html' : path))
      const base = encodedFiles.has(emitted) || !extname(path) ? dir : resolve(frontend, 'public')
      const file = resolve(base, '.' + (path === '/' || (base === dir && !extname(path)) ? '/index.html' : path))
      if (!file.startsWith(base + '/')) { res.writeHead(403).end(); return }
      const cached = encodedFiles.get(file), body = cached?.body || await readFile(file), ext = extname(file)
      res.setHeader('Content-Type', types[ext] || 'application/octet-stream'); res.setHeader('Cache-Control', ext === '.html' ? 'no-store' : 'public,max-age=3600'); res.setHeader('Vary', 'Accept-Encoding')
      // Pinned art/descriptor bytes keep raw Content-Length; only emitted build files are encoded.
      if (cached && /\.(js|css|html|json|svg)$/.test(file) && req.headers['accept-encoding']?.includes('br')) { res.setHeader('Content-Encoding', 'br'); res.end(cached?.brotli || brotliCompressSync(body)) }
      else { res.setHeader('Content-Length', body.length); res.end(body) }
    } catch { res.writeHead(404).end('No fixture resource') }
  })
  await new Promise(done => server.listen(0, '127.0.0.1', done))
  return { origin: `http://127.0.0.1:${server.address().port}`, close: () => new Promise(done => { server.closeAllConnections(); server.close(done) }) }
}
async function network(context, page) {
  const cdp = await context.newCDPSession(page), entries = new Map()
  await cdp.send('Network.enable'); await cdp.send('Performance.enable')
  cdp.on('Network.requestWillBeSent', e => entries.set(e.requestId, { path: new URL(e.request.url).pathname, type: e.type, start: e.timestamp, servedFromCache: false }))
  cdp.on('Network.requestServedFromCache', e => { if (entries.has(e.requestId)) entries.get(e.requestId).servedFromCache = true })
  cdp.on('Network.responseReceived', e => { const row = entries.get(e.requestId); if (row) Object.assign(row, { status: e.response.status, fromDiskCache: e.response.fromDiskCache, fromServiceWorker: e.response.fromServiceWorker, encoding: e.response.headers['Content-Encoding'] || e.response.headers['content-encoding'] || null }) })
  cdp.on('Network.loadingFinished', e => { const row = entries.get(e.requestId); if (row) Object.assign(row, { encodedDataLength: e.encodedDataLength, durationMs: (e.timestamp - row.start) * 1000 }) })
  cdp.on('Network.loadingFailed', e => { const row = entries.get(e.requestId); if (row) Object.assign(row, { failure: e.errorText, canceled: e.canceled }) })
  return { cdp, read: () => [...entries.values()], clear: () => entries.clear() }
}
async function memory(page, cdp, forceGc = false) {
  if (forceGc) await cdp.send('HeapProfiler.collectGarbage')
  const heap = await cdp.send('Runtime.getHeapUsage'), metrics = await cdp.send('Performance.getMetrics')
  const dom = await cdp.send('Memory.getDOMCounters')
  const browserApis = await page.evaluate(() => ({ crossOriginIsolated, preciseAgentMemory: typeof performance.measureUserAgentSpecificMemory === 'function' ? (crossOriginIsolated ? 'EXPOSED_NOT_SAMPLED' : 'UNAVAILABLE_WITHOUT_CROSS_ORIGIN_ISOLATION') : 'NOT_EXPOSED', legacyPerformanceMemory: performance.memory ? { usedJSHeapSize: performance.memory.usedJSHeapSize, totalJSHeapSize: performance.memory.totalJSHeapSize, jsHeapSizeLimit: performance.memory.jsHeapSizeLimit } : null }))
  return { forceGc, heap, dom, metrics: Object.fromEntries(metrics.metrics.map(m => [m.name, m.value])), browserApis }
}
const state = p => p.evaluate(() => window.__QA08__?.read())
async function sample(page, cdp, name) {
  const before = await memory(page, cdp)
  const raw = await page.evaluate(ms => window.__QA08__.sample(ms), windowMs)
  assert(raw.raf.length > 0, 'An active native RAF window is required')
  if (name !== 'full-panel-covered') assert(raw.renders.length > 0, 'Visible timing needs actual POST_RENDER callbacks')
  return { name, raw, summary: summarize(raw), before, after: await memory(page, cdp) }
}
async function singleton(page) { const s = await state(page); assert.equal(s.activeSockets, 1); assert.equal(s.peakSockets, 1); assert.equal(s.totalObservedGames, 1); assert.equal(s.sameGame, true); assert.equal(s.avatarSprites, 6); return s }
try {
  if (!process.env.QA08_REUSE_BUILD) {
    const started = performance.now()
    await build({ root: frontend, configLoader: 'runner', cacheDir: resolve(cache, 'vite'), publicDir: false, build: { outDir: prod, emptyOutDir: true, manifest: true } })
    report.bundles.productionBuildMs = performance.now() - started
    const fixtureStart = performance.now()
    await build({ root: frontend, configLoader: 'runner', cacheDir: resolve(cache, 'vite-fixture'), mode: 'qa08', publicDir: false, build: { outDir: fixture, emptyOutDir: true, manifest: true, rollupOptions: { input: resolve(import.meta.dirname, 'fixture.html') } } })
    report.bundles.fixtureBuildMs = performance.now() - fixtureStart
  }
  report.bundles.production = await inventory(prod); report.bundles.optimizedSyntheticFixture = await inventory(fixture)
  appServer = await serve(prod); fixtureServer = await serve(fixture)
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true, args: ['--enable-precise-memory-info'] }); report.browser = browser.version()
  {
    const context = await browser.newContext({ viewport: { width: 390, height: 844 }, serviceWorkers: 'block' }), page = await context.newPage(), net = await network(context, page)
    await page.goto(appServer.origin); await page.getByRole('heading', { name: 'One world. Every era.', exact: true }).waitFor(); await page.waitForLoadState('networkidle')
    report.productionLanding = { page: await page.title(), observedHeading: await page.getByRole('heading').first().innerText(), network: net.read(), memory: await memory(page, net.cdp), qualification: 'Current actual main.tsx production build with no configured event/backend; not authenticated world startup' }
    await page.screenshot({ path: resolve(output, 'default-landing-390.png') })
    report.defaultRoutes = []
    for (const path of ['/login', '/world']) {
      net.clear()
      await page.goto(appServer.origin + path); await page.getByRole('heading', { level: 1 }).waitFor(); await page.waitForLoadState('networkidle')
      const observed = { path, heading: await page.getByRole('heading', { level: 1 }).first().innerText(), canvases: await page.locator('canvas').count(), network: net.read() }
      assert.equal(observed.canvases, 0); report.defaultRoutes.push(observed)
    }
    await context.close()
  }
  for (const width of widths) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce', deviceScaleFactor: 1, serviceWorkers: 'block' }), page = await context.newPage(), net = await network(context, page)
    const result = { width, status: 'IN_PROGRESS', windows: [], reconnects: [], lifecycle: [], errors: [] }; report.scenarios.push(result)
    page.on('pageerror', e => result.errors.push(e.message)); const start = performance.now()
    try {
      await page.goto(fixtureServer.origin + '/e2e/qa08-rendering/fixture.html')
      await expect.poll(async () => (await state(page))?.phase, { timeout: 45000 }).toBe('live')
      await expect.poll(async () => (await state(page))?.avatarSprites, { timeout: 15000 }).toBe(6)
      result.startupMs = performance.now() - start; await singleton(page)
      result.gcBefore = await memory(page, net.cdp, true)
      result.windows.push(await sample(page, net.cdp, 'idle-visible'))
      const stick = page.getByRole('button', { name: 'Movement joystick. Arrow keys or W A S D when focused', exact: true })
      await stick.focus(); await page.keyboard.down('ArrowRight')
      result.windows.push(await sample(page, net.cdp, 'keyboard-moving')); await page.keyboard.up('ArrowRight')
      await page.getByRole('button', { name: 'World tools', exact: true }).click(); await page.getByRole('link', { name: 'Map', exact: true }).first().click(); await page.getByRole('img', { name: 'PARADE illustrated virtual map', exact: true }).waitFor()
      await expect.poll(async () => (await state(page)).overviewPresentation, { timeout: 15000 }).toBe('map4-service-v1')
      result.windows.push(await sample(page, net.cdp, 'full-panel-covered')); await singleton(page)
      await page.screenshot({ path: resolve(output, `map-${width}.png`) })
      await page.getByRole('button', { name: 'Back to previous panel', exact: true }).click()
      await expect.poll(async () => (await state(page)).path).toBe('/world')
      await page.evaluate(() => window.__QA08__.consumer(100))
      await stick.focus(); await page.keyboard.down('ArrowDown')
      // 2 seconds of actual public input, then measure delivery/recovery without adding synthetic frames.
      await page.waitForTimeout(2000); await page.keyboard.up('ArrowDown')
      result.windows.push(await sample(page, net.cdp, 'delayed-ack-consumer'))
      await expect.poll(async () => (await state(page)).pending, { timeout: 10000 }).toBe(0)
      await expect.poll(async () => (await state(page)).injectorQueued).toBe(0)
      result.slowConsumer = await state(page); assert(result.slowConsumer.pendingPeak <= report.c05Policy.outgoing_queue_frames)
      result.consumerDeliveryLatencyMs = distribution(result.slowConsumer.delivery.filter(d => d.serviceIntervalMs === 100).map(d => d.deliveredAt - d.queuedAt))
      await page.evaluate(() => window.__QA08__.consumer(0))
      const beforeLoss = await state(page)
      await page.evaluate(() => window.__QA08__.dropNext(1)); await stick.focus(); await page.keyboard.down('ArrowLeft')
      await expect.poll(async () => (await state(page)).resyncs, { timeout: 10000 }).toBe(beforeLoss.resyncs + 1)
      await page.keyboard.up('ArrowLeft'); await expect.poll(async () => (await state(page)).phase).toBe('live')
      result.lossRecovery = { before: beforeLoss, after: await singleton(page) }
      assert.equal(result.lossRecovery.after.droppedDeltas, beforeLoss.droppedDeltas + 1)
      for (let i = 0; i < 3; i++) {
        const before = await state(page), at = performance.now()
        const faultAt = await page.evaluate(() => window.__QA08__.disconnect()); await page.getByRole('button', { name: 'Reconnect world', exact: true }).click()
        await expect.poll(async () => (await state(page)).phase).toBe('live')
        const after = await singleton(page); assert.equal(after.sockets, before.sockets + 1)
        result.reconnects.push({ attempt: i + 1, automationElapsedMs: performance.now() - at, browserFaultToLiveMs: after.phases.find(p => p.at >= faultAt && p.phase === 'live')?.at - faultAt, phaseEvents: after.phases.filter(p => p.at >= faultAt), before: { sockets: before.sockets, activeSockets: before.activeSockets }, after: { sockets: after.sockets, activeSockets: after.activeSockets, sameGame: after.sameGame, entities: after.entities } })
      }
      result.windows.push(await sample(page, net.cdp, 'recovered-visible'))
      for (let i = 0; i < cycles; i++) {
        const before = await state(page)
        await page.evaluate(() => window.__QA08__.navigate('/world/map'))
        await expect.poll(async () => (await state(page)).path).toBe('/world/map')
        await page.getByRole('img', { name: 'PARADE illustrated virtual map', exact: true }).waitFor()
        assert.equal((await state(page)).totalObservedGames, before.totalObservedGames)
        await page.evaluate(() => window.__QA08__.navigate('/world'))
        await page.getByRole('button', { name: 'Movement joystick. Arrow keys or W A S D when focused', exact: true }).waitFor()
        assert.equal((await state(page)).sockets, before.sockets)
        await page.evaluate(() => window.__QA08__.disconnect()); await page.getByRole('button', { name: 'Reconnect world', exact: true }).click()
        await expect.poll(async () => (await state(page)).phase).toBe('live')
        const active = await state(page); assert.equal(active.activeSockets, 1); assert.equal(active.totalObservedGames, before.totalObservedGames)
        const recoveredMemory = await memory(page, net.cdp, true)
        await page.evaluate(() => window.__QA08__.navigate('/'))
        await expect.poll(async () => (await state(page)).games).toBe(0); await expect.poll(async () => (await state(page)).activeSockets).toBe(0)
        const released = await state(page)
        assert.equal(released.canvases, 0); assert.equal(released.injectorQueued, 0); assert.equal(released.fixtureTimers, 0); assert.equal(released.heartbeatTimers, 0); assert.equal(released.socketListeners, 0)
        const releasedMemory = await memory(page, net.cdp, true)
        await page.evaluate(() => { window.__QA08__.markLifetime(); return window.__QA08__.navigate('/world') })
        await expect.poll(async () => (await state(page)).phase, { timeout: 15000 }).toBe('live')
        await expect.poll(async () => (await state(page)).avatarSprites).toBe(6)
        const remounted = await state(page)
        assert.equal(remounted.games, 1); assert.equal(remounted.activeSockets, 1); assert.equal(remounted.sameGame, true); assert.equal(remounted.totalObservedGames, before.totalObservedGames + 1)
        assert.deepEqual(remounted.canonicalIdentity, before.canonicalIdentity)
        result.lifecycle.push({ cycle: i + 1, recoveredMemory, released, releasedMemory, remounted, remountedMemory: await memory(page, net.cdp, true) })
      }
      result.memoryGrowth = { remountedHeap: growth(result.lifecycle.map(c => c.remountedMemory.heap.usedSize)), releasedHeap: growth(result.lifecycle.map(c => c.releasedMemory.heap.usedSize)), retainedHarness: 'Fixture keeps explicit socket/request/delivery/cache diagnostic records; heap trend includes these records and module state. No leak cutoff is inferred.' }
      result.gcBeforeDispose = await memory(page, net.cdp, true)
      await page.screenshot({ path: resolve(output, `world-${width}.png`) })
      await page.evaluate(() => window.__QA08__.dispose())
      await expect.poll(async () => (await state(page)).games).toBe(0); await expect.poll(async () => (await state(page)).activeSockets).toBe(0)
      result.cleanup = await state(page); assert.equal(result.cleanup.injectorQueued, 0); assert.equal(result.cleanup.canvases, 0)
      assert.equal(result.cleanup.fixtureTimers, 0); assert.equal(result.cleanup.heartbeatTimers, 0); assert.equal(result.cleanup.socketListeners, 0)
      assert.equal(result.cleanup.objectUrls.active, 0)
      result.gcAfterDispose = await memory(page, net.cdp, true); result.network = net.read()
      assert.deepEqual(result.errors, []); result.status = 'MEASURED'
      console.log(`MEASURED width=${width} idle=${result.windows[0].summary.phaserPostRender.ratePerSecond.toFixed(1)}Hz moving=${result.windows[1].summary.phaserPostRender.ratePerSecond.toFixed(1)}Hz`)
    } catch (e) { result.status = 'FAIL'; result.failure = (e.stack || String(e)).replaceAll(root + '/', ''); await page.screenshot({ path: resolve(output, `failed-world-${width}.png`) }).catch(() => {}); result.lastState = await state(page).catch(() => null); result.network = net.read(); console.log(`FAIL ${width}: ${e.message}`) }
    finally { await context.close() }
    if (result.status === 'FAIL') break
  }
  if (!process.env.QA08_SKIP_COLD && report.scenarios.every(s => s.status === 'MEASURED')) {
    const context = await browser.newContext({ viewport: { width: 390, height: 844 }, serviceWorkers: 'block' }), page = await context.newPage(), net = await network(context, page)
    const profile = { offline: false, latency: 100, downloadThroughput: 1000000, uploadThroughput: 512000 }
    await net.cdp.send('Network.emulateNetworkConditions', profile)
    const start = performance.now(); await page.goto(fixtureServer.origin + '/e2e/qa08-rendering/fixture.html')
    await expect.poll(async () => (await state(page))?.phase, { timeout: 120000 }).toBe('live')
    await expect.poll(async () => (await state(page))?.avatarSprites, { timeout: 30000 }).toBe(6)
    report.coldNetwork = { profile, measuredStartupMs: performance.now() - start, window: await sample(page, net.cdp, 'cold-throttled-visible'), network: net.read(), qualification: 'Fresh browser context/CDP-limited actual static JS/art fetches; Auth/HTTP/C05 remain synthetic and are not throttled server traffic' }
    await page.evaluate(() => window.__QA08__.dispose()); report.coldNetwork.cleanup = await state(page); await context.close()
  }
  report.status = report.scenarios.length && report.scenarios.every(s => s.status === 'MEASURED') ? 'MEASURED' : 'FAIL'
  report.deliveryLatencyMs = distribution(report.scenarios.flatMap(s => s.slowConsumer?.delivery?.filter(d => d.serviceIntervalMs === 100).map(d => d.deliveredAt - d.queuedAt) || []))
  report.inputVerification = { files: Object.keys(pins).length, mismatches: [] }
  for (const [path, expected] of Object.entries(pins)) if (hash(await readFile(resolve(root, path))) !== expected) report.inputVerification.mismatches.push(path)
  assert.deepEqual(report.inputVerification.mismatches, [])
  report.staticResourceFailures = [report.productionLanding, ...report.defaultRoutes, ...report.scenarios, report.coldNetwork].flatMap(s => s?.network ?? []).filter(row => row.status >= 400 && row.path !== '/favicon.ico')
  assert.deepEqual(report.staticResourceFailures, [], 'Required actual public/build resources must be served; preserve failures rather than qualify an incomplete workload')
} catch (e) { report.status = 'FAIL'; report.failure = (e.stack || String(e)).replaceAll(root + '/', ''); throw e }
finally {
  await browser?.close(); await appServer?.close(); await fixtureServer?.close()
  await writeFile(resolve(output, 'measurements.json'), JSON.stringify(report, null, 2) + '\n')
}
if (report.status === 'FAIL') process.exitCode = 1
