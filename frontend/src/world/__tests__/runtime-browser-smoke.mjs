// Real installed Chrome/Phaser3.90.0. Deterministic synthetic terrain only; no user profile, device or art.
import assert from 'node:assert/strict'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_FE14_EVIDENCE ?? '/tmp/mcr-fe14-evidence'
await mkdir(evidence, { recursive: true })
let resourceMode = 'valid', browser, production
const errors = [], expectedErrors = [], results = [], resourceCalls = []
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'fe14-synthetic-resources', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/world/fe14/')) return next()
    const mode = resourceMode
    resourceCalls.push({ url: req.url, cookie: req.headers.cookie ?? null, authorization: req.headers.authorization ?? null, referer: req.headers.referer ?? null })
    const { synthetic } = await server.ssrLoadModule('/src/world/__tests__/fixtures/synthetic.ts'), fixture = await synthetic()
    const png = req.url.endsWith('.png'), bytes = png ? fixture.png : fixture.tiledBytes
    if (mode === 'slow') await new Promise(resolve => setTimeout(resolve, 550))
    if (res.destroyed) return
    res.setHeader('Content-Type', png ? 'image/png' : 'application/json'); res.setHeader('Cache-Control', 'public, max-age=31536000, immutable')
    res.statusCode = mode === 'failure' ? 503 : 200
    res.end(mode === 'hash' ? new Uint8Array([1, 2, 3]) : bytes)
  })
} }] })
function track() {
  const frames = new Set(), urls = new Set(), observers = new Set(), registrations = []; let tileDraws = 0, smoothTileDraws = 0
  const drawImage = CanvasRenderingContext2D.prototype.drawImage
  CanvasRenderingContext2D.prototype.drawImage = function(...args) {
    if (args[0] instanceof HTMLImageElement && args[0].src.startsWith('blob:')) { tileDraws++; if (this.imageSmoothingEnabled) smoothTileDraws++ }
    return drawImage.apply(this, args)
  }
  const raf = window.requestAnimationFrame, caf = window.cancelAnimationFrame
  window.requestAnimationFrame = callback => { let id; id = raf.call(window, time => { frames.delete(id); callback(time) }); frames.add(id); return id }
  window.cancelAnimationFrame = id => { frames.delete(id); caf.call(window, id) }
  const create = URL.createObjectURL, revoke = URL.revokeObjectURL
  URL.createObjectURL = blob => { const url = create.call(URL, blob); urls.add(url); return url }
  URL.revokeObjectURL = url => { urls.delete(url); revoke.call(URL, url) }
  const Observer = ResizeObserver
  window.ResizeObserver = class extends Observer {
    constructor(callback) { super(callback); observers.add(this) }
    disconnect() { observers.delete(this); return super.disconnect() }
  }
  const add = EventTarget.prototype.addEventListener, remove = EventTarget.prototype.removeEventListener
  EventTarget.prototype.addEventListener = function(type, listener, options) {
    if ((this === window || this === document) && !options?.once && !registrations.some(r => r.target === this && r.type === type && r.listener === listener && r.capture === Boolean(typeof options === 'boolean' ? options : options?.capture))) registrations.push({ target: this, type, listener, capture: Boolean(typeof options === 'boolean' ? options : options?.capture) })
    return add.call(this, type, listener, options)
  }
  EventTarget.prototype.removeEventListener = function(type, listener, options) {
    const index = registrations.findIndex(r => r.target === this && r.type === type && r.listener === listener && r.capture === Boolean(typeof options === 'boolean' ? options : options?.capture))
    if (index >= 0) registrations.splice(index, 1)
    return remove.call(this, type, listener, options)
  }
  let expectedBlur = () => {}, expectedFocus = () => {}, visibilityHits = 0
  window.onblur = expectedBlur; window.onfocus = expectedFocus
  document.addEventListener('visibilitychange', () => { visibilityHits++ }, false)
  const documentAdd = document.addEventListener
  window.__FE14_SWAP_HANDLERS__ = () => { expectedBlur = () => {}; expectedFocus = () => {}; window.onblur = expectedBlur; window.onfocus = expectedFocus }
  window.__FE14_VISIBILITY_CHECK__ = () => { const before = visibilityHits; document.dispatchEvent(new Event('visibilitychange')); return visibilityHits === before + 1 }
  window.__FE14_RESOURCES__ = () => ({ frames: frames.size, urls: urls.size, observers: observers.size,
    listeners: registrations.map(r => `${r.target === window ? 'window' : 'document'}:${r.type}:${r.capture}`).sort(),
    priorBlur: window.onblur === expectedBlur, priorFocus: window.onfocus === expectedFocus, restoredAdd: document.addEventListener === documentAdd,
    tileDraws, smoothTileDraws, debugGame: Boolean(window.PHASER_GAME), canvas: document.querySelectorAll('canvas').length })
}
async function ready(page) {
  await expect(page.getByRole('status')).toHaveText('ready')
  await expect(page.locator('canvas')).toHaveCount(1)
  await page.waitForTimeout(80)
  const geometry = await page.evaluate(() => window.__FE14__.state())
  assert.deepEqual(geometry.spawnFoot, { x: 48, y: 48 }); assert.equal(geometry.widthPx, 160); assert.equal(geometry.heightPx, 160)
  const canvas = await page.locator('canvas').evaluate(el => {
    const ctx = el.getContext('2d'), data = ctx.getImageData(0, 0, el.width, el.height).data
    const colors = new Set(); for (let i = 0; i < data.length; i += 4) colors.add(`${data[i]},${data[i + 1]},${data[i + 2]},${data[i + 3]}`)
    const host = el.parentElement.parentElement.getBoundingClientRect()
    return { colors: [...colors], smooth: ctx.imageSmoothingEnabled, width: el.width, height: el.height, hostWidth: Math.floor(host.width), hostHeight: Math.floor(host.height) }
  })
  // TilemapLayerCanvasRenderer restores its saved context after drawing; inspect actual draw calls.
  const drawing = await page.evaluate(() => window.__FE14_RESOURCES__())
  assert(drawing.tileDraws > 0); assert.equal(drawing.smoothTileDraws, 0)
  assert.equal(await page.locator('canvas').evaluate(el => getComputedStyle(el).imageRendering), 'pixelated')
  for (const color of ['76,110,80,255', '44,54,64,255', '230,198,89,255']) assert(canvas.colors.includes(color), `actual terrain/overdraw pixels missing ${color}`)
  assert.equal(canvas.width, Math.max(1, Math.min(2048, canvas.hostWidth))); assert.equal(canvas.height, Math.max(1, Math.min(2048, canvas.hostHeight)))
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
}
async function stopped(page, baseline) {
  await page.evaluate(async () => { window.__FE14__.stop(); await window.__FE14__.settled() })
  await expect(page.locator('canvas')).toHaveCount(0); await page.waitForTimeout(40)
  const after = await page.evaluate(() => window.__FE14_RESOURCES__())
  assert.equal(after.frames, 0, 'RAF leaked'); assert.equal(after.urls, 0, 'object URL leaked'); assert.equal(after.observers, baseline.observers, 'ResizeObserver leaked')
  assert.deepEqual(after.listeners, baseline.listeners, 'window/document listeners leaked')
  assert(after.priorBlur && after.priorFocus, 'prior or later window callbacks not preserved'); assert(after.restoredAdd, 'document method interception retained'); assert(await page.evaluate(() => window.__FE14_VISIBILITY_CHECK__()), 'prior document visibility listener lost'); assert.equal(after.debugGame, false, 'Phaser debug game retained')
}
try {
  await dev.listen(); const base = `http://127.0.0.1:${dev.httpServer.address().port}`, fixture = `${base}/src/world/__tests__/fixtures/runtime-preview.html`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[360, 800], [390, 844], [430, 932], [1280, 800]]) {
    resourceMode = 'valid'
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' }), page = await context.newPage(), requests = []
    await page.addInitScript(track)
    page.on('pageerror', error => (error.message.includes('FE14_SYNC_START_THROW') ? expectedErrors : errors).push(error.message)); page.on('request', r => requests.push(r.url()))
    await page.goto(fixture); await expect(page.getByRole('status')).toHaveText('stopped')
    assert(!requests.some(url => /phaser(?:\.js|_)/i.test(url)), 'Phaser fetched before explicit start')
    // Prime Playwright's own injected input listeners before the resource baseline.
    await page.getByRole('button', { name: 'Stop runtime' }).click()
    await page.locator('body').evaluate(() => true) // Initialize main-world locator instrumentation, too.
    const baseline = await page.evaluate(() => window.__FE14_RESOURCES__())
    await page.keyboard.press('Tab'); await page.getByRole('button', { name: 'Start runtime' }).focus(); await expect(page.getByRole('button', { name: 'Start runtime' })).toBeFocused()
    assert.notEqual(await page.getByRole('button', { name: 'Start runtime' }).evaluate(el => getComputedStyle(el).outlineStyle), 'none')
    await page.evaluate(() => { window.__FE14__.start(); window.__FE14__.start() }); await ready(page)
    await page.screenshot({ path: `${evidence}/map-${width}.png`, fullPage: true })
    for (const button of await page.getByRole('button').all()) { const box = await button.boundingBox(); assert(box.width >= 44 && box.height >= 44) }
    await page.setViewportSize({ width, height: 420 }); await page.waitForTimeout(60); await ready(page)
    await page.getByRole('button', { name: 'Stop runtime' }).scrollIntoViewIfNeeded(); await expect(page.getByRole('button', { name: 'Stop runtime' })).toBeInViewport()
    await page.setViewportSize({ width, height }); await stopped(page, baseline)
    for (let i = 0; i < 5; i++) {
      await page.getByRole('button', { name: 'Start runtime' }).click(); await ready(page)
      if (i === 2) await page.evaluate(() => window.__FE14_SWAP_HANDLERS__())
      await page.evaluate(() => window.__FE14__.visible(false)); await stopped(page, baseline)
      await page.getByRole('button', { name: 'Unmount runtime' }).click(); await expect(page.locator('canvas')).toHaveCount(0)
      await page.getByRole('button', { name: 'Mount runtime' }).click(); await expect(page.getByRole('status')).toHaveText('stopped')
    }
    if (width === 390) {
      // Deliberately throw inside synchronous Phaser.start after recording its listener.
      await page.evaluate(() => {
        const original = document.addEventListener
        const throwing = function(type, listener, options) {
          const result = original.call(this, type, listener, options)
          if (type === 'visibilitychange') throw new Error('FE14_SYNC_START_THROW')
          return result
        }
        document.addEventListener = throwing
        window.__FE14_CHECK_THROW_RESTORE__ = () => { const restored = document.addEventListener === throwing; document.addEventListener = original; return restored }
        void window.__FE14__.start()
      })
      await expect.poll(() => expectedErrors.length).toBe(1)
      assert(await page.evaluate(() => window.__FE14_CHECK_THROW_RESTORE__()), 'synchronous throw failed to restore exact document method')
      await stopped(page, baseline)
    }
    const allocations = await page.evaluate(() => window.__FE14__.allocations())
    await page.evaluate(() => { window.__FE14__.mode('stop-boot'); void window.__FE14__.start() })
    await expect(page.getByRole('status')).toHaveText('stopped')
    await page.waitForTimeout(200); await stopped(page, baseline)
    assert.equal(await page.evaluate(() => window.__FE14__.allocations()), allocations + 1)
    await page.evaluate(() => window.__FE14__.mode('slow-import'))
    await page.getByRole('button', { name: 'Start runtime' }).click(); await expect(page.getByRole('status')).toHaveText('loading')
    await stopped(page, baseline); await page.waitForTimeout(600); await stopped(page, baseline)
    await page.evaluate(() => window.__FE14__.mode('invalid'))
    await page.getByRole('button', { name: 'Start runtime' }).click(); await expect(page.getByRole('status')).toHaveText('error: invalid')
    await page.screenshot({ path: `${evidence}/invalid-${width}.png`, fullPage: true }); await stopped(page, baseline)
    assert(!requests.some(url => /\/api\/|world-ws|avatars|\.jpe?g/.test(url)), 'unrelated auth/socket/art request')
    results.push({ width, height, realPhaserPixels: 'PASS', terrainAndOverdraw: 'PASS', footGeometry: 'PASS', repeatedStrictModeMount: 'PASS', resizeAndReducedHeight: 'PASS', resourcesAndLateImport: 'PASS', invalidVisible: 'PASS' })
    await context.close()
  }
  for (const mode of ['hash', 'failure', 'slow']) {
    resourceMode = mode
    const context = await browser.newContext({ viewport: { width: 390, height: 844 } }), page = await context.newPage()
    await page.addInitScript(track); page.on('pageerror', error => (error.message.includes('FE14_SYNC_START_THROW') ? expectedErrors : errors).push(error.message))
    await page.goto(fixture); await expect(page.getByRole('status')).toHaveText('stopped')
    // Prime Playwright's own injected input listeners before the resource baseline.
    await page.getByRole('button', { name: 'Stop runtime' }).click()
    await page.locator('body').evaluate(() => true) // Initialize main-world locator instrumentation, too.
    const baseline = await page.evaluate(() => window.__FE14_RESOURCES__()), before = resourceCalls.length
    await page.getByRole('button', { name: 'Start runtime' }).click()
    if (mode === 'slow') {
      await expect.poll(() => resourceCalls.length).toBeGreaterThan(before)
      await page.getByRole('button', { name: 'Unmount runtime' }).click(); await page.waitForTimeout(650)
      assert.equal(await page.locator('canvas').count(), 0); await page.getByRole('button', { name: 'Mount runtime' }).click()
    } else await expect(page.getByRole('status')).toHaveText(mode === 'hash' ? 'error: invalid' : 'error: load-failed')
    await stopped(page, baseline); await page.screenshot({ path: `${evidence}/${mode}-390.png`, fullPage: true }); await context.close()
  }
  assert(resourceCalls.every(call => call.cookie === null && call.authorization === null && call.referer === null), 'credentials/referrer on public resource')
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const origin = `http://127.0.0.1:${production.httpServer.address().port}`, context = await browser.newContext({ viewport: { width: 390, height: 844 } }), page = await context.newPage(), requests = []
  page.on('request', request => requests.push(request.url())); page.on('pageerror', error => (error.message.includes('FE14_SYNC_START_THROW') ? expectedErrors : errors).push(error.message))
  for (const path of ['/', '/signup', '/verify-email', '/legal/privacy']) {
    await page.goto(`${origin}${path}`); await expect(page.locator('h1')).toBeVisible(); assert.equal(await page.locator('canvas').count(), 0)
    assert.equal(await page.evaluate(() => typeof window.__FE14__), 'undefined')
  }
  assert(!requests.some(url => /phaser|MapScene|\/world\/|\/api\/|__tests__/.test(url)), 'production entry loaded runtime/fixture/API')
  for (const file of await readdir('dist/assets')) if (file.endsWith('.js')) {
    const text = await readFile(`dist/assets/${file}`, 'utf8'); assert(!text.includes('SYNTHETIC FE14') && !text.includes('__FE14__'), 'fixture bundled in production')
  }
  await page.screenshot({ path: `${evidence}/production-public-390.png`, fullPage: true })
  assert.deepEqual(errors, [])
  const report = { browser: await browser.version(), status: 'MOCK_READY', results, negativeResources: 'PASS: hash/failure/late resource teardown',
    productionIsolation: 'PASS', requests: resourceCalls.length, pageErrors: errors, expectedInjectedStartErrors: expectedErrors,
    limits: ['Synthetic controlled resources only; no production map, venue or character artwork.', 'No shell wiring, avatar, movement, socket, world admission or physical device qualification.'] }
  await writeFile(`${evidence}/report.json`, JSON.stringify(report, null, 2)); console.log(JSON.stringify(report, null, 2)); await context.close()
} finally { await browser?.close(); await dev.close(); if (production) await new Promise(resolve => production.httpServer.close(resolve)) }
