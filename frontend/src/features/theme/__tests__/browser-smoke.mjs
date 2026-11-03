// Installed native Chrome + real retained Phaser, synthetic labeled assets only.
import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_FE06_EVIDENCE ?? '/tmp/mcr-fe06-evidence'
await mkdir(evidence, { recursive: true })
let resourceMode = 'valid', browser
const errors = [], results = [], requests = []
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'fe06-synthetic-resources', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    const match = /^\/world\/fe06\/(revenge|parade|danger|map)\.(png|json)$/.exec(req.url ?? '')
    if (!match) return next()
    requests.push({ url: req.url, cookie: req.headers.cookie ?? null, authorization: req.headers.authorization ?? null, referer: req.headers.referer ?? null })
    const { syntheticRelease } = await server.ssrLoadModule('/src/world/theme/__tests__/fixtures/releases.ts')
    const f = await syntheticRelease(match[1] === 'map' ? 'REVENGE' : match[1].toUpperCase()), png = match[2] === 'png', bad = resourceMode === 'failure' && match[1] === 'parade'
    res.setHeader('Content-Type', png ? 'image/png' : 'application/json')
    // Negative probes must exercise the real request, not a browser cache hit.
    res.setHeader('Cache-Control', 'no-store'); res.statusCode = bad ? 503 : 200
    res.end(png ? f.png : f.tiledBytes)
  })
} }] })
function track() {
  const urls = new Set(), originalCreate = URL.createObjectURL, originalRevoke = URL.revokeObjectURL
  URL.createObjectURL = blob => { const url = originalCreate.call(URL, blob); urls.add(url); return url }
  URL.revokeObjectURL = url => { urls.delete(url); originalRevoke.call(URL, url) }
  const raf = requestAnimationFrame, caf = cancelAnimationFrame, frames = new Set()
  window.requestAnimationFrame = callback => { let id; id = raf(time => { frames.delete(id); callback(time) }); frames.add(id); return id }
  window.cancelAnimationFrame = id => { frames.delete(id); caf(id) }
  window.__FE06_RESOURCES__ = () => ({ urls: urls.size, frames: frames.size })
}
const state = page => page.evaluate(() => window.__FE06__.state())
const selected = async (page, era) => {
  await expect.poll(async () => (await state(page)).theme.selected).toBe(era)
  await expect.poll(async () => (await state(page)).theme.phase).toBe('ready')
  await expect(page.getByRole('radio', { name: era, exact: true })).toBeChecked()
  assert.equal(await page.locator('[data-mcr-theme]').getAttribute('data-mcr-theme'), era)
  assert.equal((await state(page)).skin, era)
  await expect.poll(() => page.evaluate(() => window.__FE06_RESOURCES__().urls)).toBe(1)
  assert.equal((await state(page)).textures.length, 1)
  assert.equal((await state(page)).tilemaps, 1)
}
async function pixelColors(page) {
  await page.waitForTimeout(60)
  return page.locator('canvas').evaluate(canvas => {
    const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data, colors = new Set()
    for (let i = 0; i < data.length; i += 4) colors.add(`${data[i]},${data[i + 1]},${data[i + 2]}`)
    return [...colors]
  })
}
try {
  await dev.listen()
  const url = `http://127.0.0.1:${dev.httpServer.address().port}/src/features/theme/__tests__/preview.html`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage()
    await page.addInitScript(track); page.on('pageerror', error => errors.push(error.message))
    page.on('request', request => { assert(!request.url().includes('/mcr-theme.png'), 'logical marker was fetched') })
    await page.goto(url)
    await expect(page.getByLabel('Runtime status')).toHaveText('ready')
    await expect(page.locator('canvas')).toHaveCount(1)
    await expect.poll(async () => (await state(page)).camera.camera.width).toBeGreaterThan(1)
    await page.locator('canvas').evaluate(canvas => { window.__FE06_CANVAS__ = canvas })
    // Exercise retained local movement and a non-default camera before switching.
    await page.getByRole('button', { name: 'Zoom in', exact: true }).click()
    await page.locator('canvas').focus(); await page.keyboard.down('ArrowRight'); await page.waitForTimeout(130); await page.keyboard.up('ArrowRight')
    await page.getByRole('radio', { name: 'REVENGE', exact: true }).focus()
    const before = await state(page), originalColors = await pixelColors(page)
    assert(before.movement.foot.x > 48, 'native movement did not advance')
    // Native radio arrow keys activate the next option without a custom keyboard handler.
    await page.keyboard.press('ArrowRight'); await selected(page, 'PARADE')
    assert.notDeepEqual(await pixelColors(page), originalColors, 'texture pixels did not change')
    assert.equal(await page.locator('.mcr-theme-scope').evaluate(el => getComputedStyle(el).transitionDuration), '0s')
    await page.keyboard.press('ArrowRight'); await selected(page, 'DANGER')
    assert((await pixelColors(page)).includes('36,121,135'), 'Danger terrain missing')
    const after = await state(page)
    assert.deepEqual(after.camera, before.camera); assert.deepEqual(after.movement, before.movement)
    assert.equal(after.allocations, 1); assert.equal(after.attachments, 1); assert.equal(after.detachments, 0); assert(after.identityPreserved)
    assert(await page.locator('canvas').evaluate(canvas => canvas === window.__FE06_CANVAS__), 'Phaser canvas replaced')
    for (const label of await page.locator('.mcr-theme-selector label').all()) { const box = await label.boundingBox(); assert(box.width >= 44 && box.height >= 44) }
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
    // Failed request retains old pixels and scoped preference.
    resourceMode = 'failure'
    await page.getByRole('radio', { name: 'PARADE', exact: true }).click()
    await expect(page.getByRole('status').first()).toContainText('DANGER remains selected')
    assert.equal((await state(page)).skin, 'DANGER'); assert((await pixelColors(page)).includes('36,121,135'))
    assert.equal(await page.evaluate(() => localStorage.getItem('mcr:theme:v1:synthetic-user:synthetic-event')), 'DANGER')
    resourceMode = 'valid'
    // Fail after a texture and terrain layer have been staged: rollback must remove them.
    await page.evaluate(() => window.__FE06__.mode('engine-failure'))
    await page.getByRole('radio', { name: 'REVENGE', exact: true }).click()
    await expect(page.getByRole('status').first()).toContainText('DANGER remains selected')
    assert.equal((await state(page)).textures.length, 1); assert.equal((await state(page)).skin, 'DANGER')
    await expect.poll(() => page.evaluate(() => window.__FE06_RESOURCES__().urls)).toBe(1)
    for (const mode of ['geometry-mismatch', 'avatar-mismatch', 'tilemap-binding']) {
      await page.evaluate(mode => window.__FE06__.mode(mode), mode)
      await page.getByRole('radio', { name: 'REVENGE', exact: true }).click()
      await expect(page.getByRole('status').first()).toContainText('DANGER remains selected')
      assert.equal((await state(page)).textures.length, 1); assert.equal((await state(page)).tilemaps, 1)
      assert.equal((await state(page)).skin, 'DANGER')
    }
    // A decoded ignored-abort result cannot replace or persist over a newer choice.
    await page.evaluate(() => window.__FE06__.mode('slow'))
    await page.getByRole('radio', { name: 'PARADE', exact: true }).click()
    await expect(page.getByRole('status').first()).toContainText('Loading PARADE')
    await page.getByRole('radio', { name: 'PARADE', exact: true }).focus()
    await page.keyboard.press('ArrowRight'); await selected(page, 'DANGER')
    await page.keyboard.press('ArrowRight'); await selected(page, 'REVENGE')
    await page.waitForTimeout(450); await selected(page, 'REVENGE')
    assert.equal(await page.evaluate(() => localStorage.getItem('mcr:theme:v1:synthetic-user:synthetic-event')), 'REVENGE')
    assert.deepEqual((await state(page)).camera, before.camera); assert.deepEqual((await state(page)).movement, before.movement)
    await page.evaluate(() => window.__FE06__.mode('reentrant-failure'))
    await page.getByRole('radio', { name: 'PARADE', exact: true }).click()
    await expect(page.getByRole('status').first()).toContainText('PARADE remains selected')
    assert.equal((await state(page)).skin, 'PARADE'); assert.equal((await state(page)).theme.selected, 'PARADE')
    assert.equal((await state(page)).theme.saved, false)
    assert.equal(await page.locator('[data-mcr-theme]').getAttribute('data-mcr-theme'), 'PARADE')
    assert((await pixelColors(page)).includes('89,92,103'), 'fallback skin image was disposed')
    assert.equal((await state(page)).textures.length, 1); assert.equal((await state(page)).tilemaps, 1)
    await page.evaluate(() => window.__FE06__.mode('valid'))
    await page.getByRole('radio', { name: 'REVENGE', exact: true }).click(); await selected(page, 'REVENGE')
    // A real Phaser texture-add callback selects another skin during preparation.
    await page.evaluate(() => window.__FE06__.mode('reentrant-switch'))
    await page.getByRole('radio', { name: 'PARADE', exact: true }).click(); await selected(page, 'DANGER')
    assert((await pixelColors(page)).includes('36,121,135'), 'reentrant replacement disposed the displayed image')
    assert.equal(await page.evaluate(() => localStorage.getItem('mcr:theme:v1:synthetic-user:synthetic-event')), 'DANGER')
    assert.deepEqual((await state(page)).camera, before.camera); assert.deepEqual((await state(page)).movement, before.movement)
    await page.screenshot({ path: `${evidence}/theme-${width}.png`, fullPage: true })
    // Device preference restores through a fully validated load on a new shell.
    await page.evaluate(() => window.__FE06__.mode('valid')); await page.getByRole('radio', { name: 'DANGER', exact: true }).click(); await selected(page, 'DANGER')
    await page.reload(); await expect(page.getByLabel('Runtime status')).toHaveText('ready'); await selected(page, 'DANGER')
    await page.evaluate(() => window.__FE06__.mode('slow'))
    await page.getByRole('radio', { name: 'PARADE', exact: true }).click()
    await expect(page.getByRole('status').first()).toContainText('Loading PARADE')
    await page.getByRole('button', { name: 'Dispose fixture' }).click(); await page.evaluate(() => window.__FE06__.settled())
    await page.waitForTimeout(450)
    assert.equal(await page.locator('canvas').count(), 0); assert.equal((await state(page)).textures.length, 0); assert.equal((await state(page)).tilemaps, 0)
    assert.deepEqual(await page.evaluate(() => window.__FE06_RESOURCES__()), { urls: 0, frames: 0 })
    results.push({ width, realPhaserPixels: 'PASS', keyboardAndTouchTargets: 'PASS', engineCameraMovementIdentityPreservation: 'PASS', failureAndSupersessionRollback: 'PASS', restorationAndTeardown: 'PASS' })
    await context.close()
  }
  assert(requests.every(req => !req.cookie && !req.authorization && !req.referer), 'resource credentials/referrer leaked')
  assert.deepEqual(errors, [])
  const report = { status: 'MOCK_READY', browser: await browser.version(), results, pageErrors: errors,
    limits: ['Synthetic textures only; production AR04 art is not delivered.', 'Room/avatar owner sentinels prove non-mutation only; no real socket or character rendering.', 'No real-device, backend preference or two-account multiuser acceptance.'] }
  await writeFile(`${evidence}/report.json`, JSON.stringify(report, null, 2)); console.log(JSON.stringify(report, null, 2))
} finally { await browser?.close(); await dev.close() }
