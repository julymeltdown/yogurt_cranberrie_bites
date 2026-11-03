// Installed Chrome, native DOM pointer/keyboard input, actual owned Phaser camera and terrain.
import assert from 'node:assert/strict'
import { mkdir, writeFile, readdir, readFile } from 'node:fs/promises'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_FE15_EVIDENCE ?? '/tmp/mcr-fe15-evidence'
await mkdir(evidence, { recursive: true })
let browser, production
const errors = [], results = [], resources = []
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'fe15-test-resources', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/world/fe15/')) return next()
    resources.push({ url: req.url, cookie: req.headers.cookie ?? null, authorization: req.headers.authorization ?? null, referrer: req.headers.referer ?? null })
    const { movementData } = await server.ssrLoadModule('/src/world/__tests__/fixtures/movement-data.ts'), fixture = await movementData()
    res.setHeader('Content-Type', req.url.endsWith('.png') ? 'image/png' : 'application/json'); res.setHeader('Cache-Control', 'public,max-age=31536000,immutable')
    res.end(req.url.endsWith('.png') ? fixture.png : fixture.tiledBytes)
  })
} }] })
function track() {
  const frames = new Set(), urls = new Set(), observers = new Set(), listeners = [], captures = new Map()
  const raf = requestAnimationFrame, caf = cancelAnimationFrame
  window.requestAnimationFrame = callback => { let id; id = raf.call(window, time => { frames.delete(id); callback(time) }); frames.add(id); return id }
  window.cancelAnimationFrame = id => { frames.delete(id); caf.call(window, id) }
  const make = URL.createObjectURL, revoke = URL.revokeObjectURL
  URL.createObjectURL = blob => { const value = make.call(URL, blob); urls.add(value); return value }
  URL.revokeObjectURL = value => { urls.delete(value); return revoke.call(URL, value) }
  const Observer = ResizeObserver
  window.ResizeObserver = class extends Observer { constructor(callback) { super(callback); observers.add(this) } disconnect() { observers.delete(this); return super.disconnect() } }
  const add = EventTarget.prototype.addEventListener, remove = EventTarget.prototype.removeEventListener
  const capture = options => Boolean(typeof options === 'boolean' ? options : options?.capture)
  EventTarget.prototype.addEventListener = function(type, listener, options) {
    if (!options?.once && !listeners.some(r => r.target === this && r.type === type && r.listener === listener && r.capture === capture(options))) listeners.push({ target: this, type, listener, capture: capture(options) })
    return add.call(this, type, listener, options)
  }
  EventTarget.prototype.removeEventListener = function(type, listener, options) {
    const index = listeners.findIndex(r => r.target === this && r.type === type && r.listener === listener && r.capture === capture(options))
    if (index >= 0) listeners.splice(index, 1)
    return remove.call(this, type, listener, options)
  }
  const setCapture = Element.prototype.setPointerCapture, release = Element.prototype.releasePointerCapture
  Element.prototype.setPointerCapture = function(id) { const result = setCapture.call(this, id); captures.set(id, this); return result }
  Element.prototype.releasePointerCapture = function(id) { const result = release.call(this, id); captures.delete(id); return result }
  window.addEventListener('lostpointercapture', event => captures.delete(event.pointerId), true)
  let priorBlur = () => {}, priorFocus = () => {}, visibilityHits = 0
  window.onblur = priorBlur; window.onfocus = priorFocus
  document.addEventListener('visibilitychange', () => visibilityHits++)
  const documentAdd = document.addEventListener
  window.__FE15_TRACK__ = () => ({ frames: frames.size, urls: urls.size, observers: observers.size, captures: captures.size,
    listeners: listeners.map(r => `${r.target === window ? 'window' : r.target === document ? 'document' : r.target instanceof Element ? `${r.target.tagName}.${r.target.id}.${r.target.className}` : r.target.constructor.name}:${r.type}:${r.capture}`).sort(),
    callbacks: window.onblur === priorBlur && window.onfocus === priorFocus, method: document.addEventListener === documentAdd, game: Boolean(window.PHASER_GAME), canvas: document.querySelectorAll('canvas').length, controls: document.querySelectorAll('.world-controls').length, visibilityHits })
}
const state = page => page.evaluate(() => window.__FE15__.state())
const foot = async page => (await state(page)).movement?.foot
async function frames(page, count = 8) { await page.evaluate(count => new Promise(resolve => { const next = () => { if (--count <= 0) resolve(); else requestAnimationFrame(next) }; requestAnimationFrame(next) }), count) }
async function stable(page, expected) { await frames(page); assert.deepEqual(await foot(page), expected, 'input moved while paused or after release') }
async function ready(page) {
  await expect(page.getByRole('status', { name: 'Runtime status' })).toHaveText('ready')
  await expect(page.locator('canvas')).toHaveCount(1); await expect(page.locator('.world-controls')).toHaveCount(1); await frames(page, 3)
  const value = await state(page), host = await page.locator('#host').boundingBox()
  assert.equal(value.camera.width, Math.floor(host.width)); assert.equal(value.camera.height, Math.floor(host.height)); assert.equal(value.runtime.widthPx, 768); assert.equal(value.runtime.heightPx, 448)
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
}
async function stopped(page, baseline) {
  await page.evaluate(async () => { window.__FE15__.stop(); await window.__FE15__.settled() }); await frames(page, 3)
  const after = await page.evaluate(() => window.__FE15_TRACK__())
  for (const field of ['frames', 'urls', 'captures', 'canvas', 'controls']) assert.equal(after[field], 0, `owned ${field} leaked`)
  assert.equal(after.observers, baseline.observers); assert.deepEqual(after.listeners, baseline.listeners, 'persistent listeners leaked')
  assert(after.callbacks && after.method && !after.game)
  assert.equal((await state(page)).attached, (await state(page)).disposedAttachments)
}
async function clickWorld(page, point) {
  await page.locator('canvas').scrollIntoViewIfNeeded()
  const client = await page.evaluate(point => window.__FE15__.project(point), point), rect = await page.locator('canvas').boundingBox()
  assert(client && client.x >= rect.x && client.x < rect.x + rect.width && client.y >= rect.y && client.y < rect.y + rect.height, 'test world point must be inside actual canvas')
  const roundtrip = await page.evaluate(client => window.__FE15__.roundtrip(client), client)
  assert(Math.abs(roundtrip.x - point.x) < 1e-6 && Math.abs(roundtrip.y - point.y) < 1e-6, 'actual camera roundtrip mismatch')
  await page.mouse.click(client.x, client.y)
}
try {
  await dev.listen(); const origin = `http://127.0.0.1:${dev.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[360, 800], [390, 844], [430, 932], [1280, 800]]) {
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage(), requests = []
    await page.addInitScript(track); page.on('pageerror', error => errors.push(error.message)); page.on('request', request => requests.push(request.url()))
    await page.goto(`${origin}/src/world/__tests__/fixtures/movement-preview.html`)
    await expect(page.getByRole('status', { name: 'Runtime status' })).toHaveText('stopped')
    assert(!requests.some(url => /phaser(?:\.js|_)/i.test(url)), 'Phaser evaluated before explicit start')
    await page.getByRole('button', { name: 'Stop local preview' }).click(); await page.locator('body').evaluate(() => true)
    const baseline = await page.evaluate(() => window.__FE15_TRACK__())
    await page.getByRole('button', { name: 'Start local preview' }).click(); await ready(page)
    assert.deepEqual(await foot(page), { x: 80, y: 112 })
    await page.keyboard.press('Tab'); await page.locator('canvas').focus()
    assert.notEqual(await page.locator('canvas').evaluate(el => getComputedStyle(el).outlineStyle), 'none')
    await page.keyboard.down('d'); await expect.poll(async () => (await foot(page)).x).toBeGreaterThanOrEqual(104); await page.keyboard.up('d')
    await expect.poll(async () => (await state(page)).intents.at(-1)?.input.axisX).toBe(0)
    await stable(page, await foot(page))
    await page.locator('.world-joystick').scrollIntoViewIfNeeded()
    const session = await context.newCDPSession(page), box = await page.locator('.world-joystick').boundingBox(), center = { x: box.x + box.width / 2, y: box.y + box.height / 2 }
    const beforeStick = await foot(page), diagonal = { x: center.x + box.width * .4, y: center.y + box.height * .4 }
    await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ ...center, id: 1 }] })
    await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ ...diagonal, id: 1 }] })
    await expect.poll(async () => (await foot(page)).y).toBeGreaterThan(beforeStick.y + 2)
    await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ ...diagonal, id: 1 }, { x: center.x - 20, y: center.y - 20, id: 2 }] })
    const held = (await state(page)).movement.axis; assert(held.x > 0 && held.y > 0 && Math.hypot(held.x, held.y) <= 1 + 1e-9)
    await session.send('Input.dispatchTouchEvent', { type: 'touchCancel', touchPoints: [] })
    assert.equal((await page.evaluate(() => window.__FE15_TRACK__())).captures, 0)
    await stable(page, await foot(page))
    // The nearby target uses validated C06 coordinates plus the explicit local range; holding E is one-shot.
    await page.locator('canvas').focus(); const beforeInteract = (await state(page)).intents.filter(item => item.input.buttons.length).length
    await page.keyboard.down('e'); await page.keyboard.down('e'); await page.keyboard.down('e')
    await expect.poll(async () => (await state(page)).intents.filter(item => item.input.buttons.length).length).toBe(beforeInteract + 1)
    await frames(page); assert.equal((await state(page)).intents.filter(item => item.input.buttons.length).length, beforeInteract + 1); await page.keyboard.up('e')
    await page.getByRole('button', { name: /^Interact:/ }).click()
    await expect.poll(async () => (await state(page)).intents.filter(item => item.input.buttons.length).length).toBe(beforeInteract + 2)
    await frames(page); assert.equal((await state(page)).intents.filter(item => item.input.buttons.length).length, beforeInteract + 2)
    await page.locator('canvas').focus(); await page.keyboard.down('ArrowRight'); await expect.poll(async () => (await state(page)).movement.axis.x).toBe(1)
    await page.getByRole('button', { name: 'Open blocking panel' }).click(); await page.keyboard.up('ArrowRight')
    const panelFoot = await foot(page); await page.getByLabel('Message').fill('WASD and arrows type here'); await page.keyboard.press('ArrowRight'); await stable(page, panelFoot)
    await page.setViewportSize({ width, height: 420 }); await frames(page, 4); await ready(page)
    await page.getByRole('button', { name: 'Close blocking panel' }).click(); await stable(page, panelFoot)
    for (const control of await page.locator('.world-controls button').all()) {
      await control.scrollIntoViewIfNeeded(); await expect(control).toBeInViewport({ ratio: 1 })
      const rect = await control.boundingBox(); assert(rect.width >= 44 && rect.height >= 44 && rect.y >= 0 && rect.y + rect.height <= 420 + 1e-6)
    }
    await page.locator('.world-joystick').scrollIntoViewIfNeeded()
    const shortStick = await page.locator('.world-joystick').boundingBox(), shortCenter = { x: shortStick.x + shortStick.width / 2, y: shortStick.y + shortStick.height / 2 }, shortFoot = await foot(page)
    await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ ...shortCenter, id: 4 }] })
    await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: shortCenter.x + shortStick.width * .4, y: shortCenter.y, id: 4 }] })
    await expect.poll(async () => (await foot(page)).x).toBeGreaterThan(shortFoot.x)
    await session.send('Input.dispatchTouchEvent', { type: 'touchCancel', touchPoints: [] }); await stable(page, await foot(page))
    await page.getByRole('button', { name: 'Recenter', exact: true }).scrollIntoViewIfNeeded()
    const shortRecenter = await page.getByRole('button', { name: 'Recenter', exact: true }).boundingBox()
    await page.touchscreen.tap(shortRecenter.x + shortRecenter.width / 2, shortRecenter.y + shortRecenter.height / 2)
    await page.screenshot({ path: `${evidence}/short-controls-${width}.png`, fullPage: false })
    await page.setViewportSize({ width, height }); await frames(page, 4)
    // Queue and block in the same task: no delayed interaction survives resume.
    await page.locator('canvas').focus(); const countBeforeBlock = (await state(page)).intents.filter(item => item.input.buttons.length).length
    await page.evaluate(() => { document.querySelector('canvas').dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyE', bubbles: true })); window.__FE15__.blocked(true); window.__FE15__.blocked(false) })
    await frames(page); assert.equal((await state(page)).intents.filter(item => item.input.buttons.length).length, countBeforeBlock)
    await page.getByRole('button', { name: 'Hide world' }).click(); const hiddenFoot = await foot(page); await page.getByRole('button', { name: 'Show world' }).click(); await stable(page, hiddenFoot)
    // Hold the actual touch stick into the wall: ongoing intent must not tunnel the predicted foot.
    await page.locator('.world-joystick').scrollIntoViewIfNeeded()
    const wallStick = await page.locator('.world-joystick').boundingBox(), wallCenter = { x: wallStick.x + wallStick.width / 2, y: wallStick.y + wallStick.height / 2 }
    await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ ...wallCenter, id: 3 }] })
    await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: wallCenter.x + wallStick.width / 2, y: wallCenter.y, id: 3 }] })
    await expect.poll(async () => (await foot(page)).x).toBeGreaterThanOrEqual(246)
    await frames(page, 12); const wallFoot = await foot(page); assert(wallFoot.x <= 252 + 1e-6); assert.equal((await state(page)).movement.axis.x, 1)
    await stable(page, wallFoot)
    await page.evaluate(() => window.__FE15__.blocked(true)); assert.equal((await page.evaluate(() => window.__FE15_TRACK__())).captures, 0)
    await page.evaluate(() => window.__FE15__.blocked(false))
    await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: wallCenter.x - 20, y: wallCenter.y, id: 3 }] }); await stable(page, wallFoot)
    await session.send('Input.dispatchTouchEvent', { type: 'touchCancel', touchPoints: [] }); await stable(page, wallFoot)
    await page.evaluate(() => window.__FE15__.lookAt({ x: 240, y: 160 }))
    await clickWorld(page, { x: 272, y: 112 }); await expect(page.locator('.world-controls [role="status"]')).toContainText('reachable, walkable'); await stable(page, await foot(page))
    const predictionStart = (await state(page)).predictions.length
    await clickWorld(page, { x: 336, y: 112 }); await expect.poll(async () => (await state(page)).movement.path.some(point => point.y === 240)).toBe(true)
    await expect.poll(async () => { const p = await foot(page); return Math.hypot(p.x - 336, p.y - 112) }, { timeout: 12000 }).toBeLessThan(.01)
    await expect.poll(async () => (await state(page)).movement.path.length).toBe(0)
    const trace = (await state(page)).predictions.slice(predictionStart)
    assert(trace.some(p => p.x > 260 && p.x < 280), 'route never crossed the gap')
    assert(trace.filter(p => p.x > 252 + 1e-6 && p.x < 292 - 1e-6).every(p => p.y >= 230 - 1e-6 && p.y <= 256 + 1e-6), 'foot penetrated wall outside opening')
    await page.getByRole('button', { name: 'Zoom in' }).click(); assert.equal((await state(page)).camera.zoom, 2)
    // Zoom first: on desktop the entire map fits at1x, so bounded scrolling correctly stays zero.
    await page.evaluate(() => window.__FE15__.lookAt({ x: 600, y: 112 }))
    const looked = (await state(page)).camera.scrollX
    await page.getByRole('button', { name: 'Recenter', exact: true }).click(); assert.notEqual((await state(page)).camera.scrollX, looked)
    const projected = await page.evaluate(() => window.__FE15__.project(window.__FE15__.state().movement.foot))
    const back = await page.evaluate(point => window.__FE15__.roundtrip(point), projected), predicted = await foot(page)
    assert(Math.hypot(back.x - predicted.x, back.y - predicted.y) < 1e-6)
    await page.setViewportSize({ width, height: 420 }); await frames(page, 4); await ready(page)
    await page.setViewportSize({ width, height }); await frames(page, 4)
    for (const button of await page.locator('.world-controls button').all()) { const rect = await button.boundingBox(); assert(rect.width >= 44 && rect.height >= 44) }
    await page.screenshot({ path: `${evidence}/movement-${width}.png`, fullPage: true })
    await stopped(page, baseline)
    for (let i = 0; i < 3; i++) {
      await page.getByRole('button', { name: 'Unmount owner' }).click(); await page.getByRole('button', { name: 'Mount owner' }).click()
      await page.getByRole('button', { name: 'Start local preview' }).click(); await ready(page)
      await page.locator('canvas').focus(); await page.keyboard.down('d'); await expect.poll(async () => (await state(page)).movement.axis.x).toBe(1)
      await stopped(page, baseline); await page.keyboard.up('d')
    }
    const allocation = (await state(page)).attached
    await page.evaluate(() => { void window.__FE15__.start(); window.__FE15__.stop() }); await stopped(page, baseline); assert.equal((await state(page)).attached, allocation)
    for (const mode of ['invalid-map', 'invalid-policy']) {
      await page.evaluate(mode => window.__FE15__.mode(mode), mode); await page.getByRole('button', { name: 'Start local preview' }).click()
      await expect(page.getByRole('status', { name: 'Runtime status' })).toHaveText('error: invalid'); await stopped(page, baseline)
    }
    if (width === 390) for (const action of ['stop', 'dispose', 'restart']) {
      await page.evaluate(action => window.__FE15__.mode(`consumer-${action}`), action)
      await page.getByRole('button', { name: 'Start local preview' }).click(); await ready(page)
      const before = (await state(page)).intents.filter(item => item.input.axisX || item.input.axisY).length
      await page.locator('canvas').focus(); await page.keyboard.down('d')
      await expect.poll(async () => (await state(page)).intents.filter(item => item.input.axisX || item.input.axisY).length).toBe(before + 1)
      await expect(page.getByRole('status', { name: 'Runtime status' })).toHaveText(action === 'restart' ? 'ready' : action === 'dispose' ? 'disposed' : 'stopped')
      if (action === 'restart') { await ready(page); assert.deepEqual(await foot(page), { x: 80, y: 112 }); await stable(page, await foot(page)) }
      await stopped(page, baseline); await page.keyboard.up('d')
      await page.getByRole('button', { name: 'Unmount owner' }).click(); await page.getByRole('button', { name: 'Mount owner' }).click()
    }
    assert(!requests.some(url => /\/api\/|world-ws|\/avatars\/|\.jpe?g/.test(url)), 'unrelated production/auth/art request')
    if (width === 390) {
      await page.evaluate(() => window.__FE15__.mode('valid'))
      await page.getByRole('button', { name: 'Start local preview' }).click(); await ready(page)
      await page.evaluate(() => { window.__FE15__.mode('visibility-show'); window.__FE15__.visible(false) })
      await frames(page, 4)
      assert.equal(await page.locator('.world-joystick').isEnabled(), true)
      assert((await page.evaluate(() => window.__FE15_TRACK__())).frames > 0, 'newer show must leave actual Phaser loop awake')
      await page.evaluate(() => { window.__FE15__.mode('visibility-hide'); window.__FE15__.visible(true) })
      await frames(page, 4)
      assert.equal(await page.locator('.world-joystick').isEnabled(), false)
      assert.equal((await page.evaluate(() => window.__FE15_TRACK__())).frames, 0, 'newer hide must leave actual Phaser loop asleep')
      await stopped(page, baseline)
    }
    results.push({ width, height, status: 'PASS', nativeTouchCancelAndMultitouch: 'PASS', focusedKeyboardAndPanelPause: 'PASS', routedCollisionAndOneShotInteraction: 'PASS', actualCameraAndResize: 'PASS', repeatedMountCleanup: 'PASS' })
    await context.close()
  }
  assert(resources.every(r => r.cookie === null && r.authorization === null && r.referrer === null))
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const publicOrigin = `http://127.0.0.1:${production.httpServer.address().port}`, page = await browser.newPage(), requests = []
  page.on('request', request => requests.push(request.url())); page.on('pageerror', error => errors.push(error.message))
  for (const path of ['/', '/signup', '/verify-email', '/legal/privacy']) {
    await page.goto(`${publicOrigin}${path}`); await expect(page.locator('h1')).toBeVisible(); assert.equal(await page.locator('canvas,.world-controls').count(), 0); assert.equal(await page.evaluate(() => typeof window.__FE15__), 'undefined')
  }
  assert(!requests.some(url => /phaser|\/world\/|\/api\/|__tests__/.test(url)))
  for (const file of await readdir('dist/assets')) if (file.endsWith('.js')) { const bytes = await readFile(`dist/assets/${file}`, 'utf8'); assert(!bytes.includes('__FE15__') && !bytes.includes('SYNTHETIC FE15')) }
  assert.deepEqual(errors, [])
  const report = { status: 'MOCK_READY', browser: await browser.version(), results, productionIsolation: 'PASS', errors, limits: ['Synthetic terrain and geometric foot marker only; no selected character artwork consumed.', 'Injected local120px/s,8x6px foot,42px interaction radius; not BE12 authority.', 'No transport, stream reconciliation, physical device, real server/world or arrival acceptance.'] }
  await writeFile(`${evidence}/report.json`, JSON.stringify(report, null, 2)); console.log(JSON.stringify(report, null, 2))
} finally { await browser?.close(); await dev.close(); if (production) await new Promise(resolve => production.httpServer.close(resolve)) }
