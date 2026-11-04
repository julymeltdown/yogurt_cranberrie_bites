import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { createServer as createPortProbe } from 'node:net'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { adoptionPlugin } from '../adoption-inputs.ts'

const directory = process.env.MCR_GIFTS_MEDIA_NATIVE_OUTPUT
if (!directory) throw Error('Explicit allocated evidence directory required')
await mkdir(directory, { recursive: true })
let allocatedPort = Number(process.env.MCR_GIFTS_NATIVE_PORT)
if (!Number.isSafeInteger(allocatedPort) || allocatedPort < 1 || allocatedPort > 65535) {
  const probe = createPortProbe()
  await new Promise(resolve => probe.listen(0, '127.0.0.1', resolve))
  allocatedPort = probe.address().port
  await new Promise((resolve, reject) => probe.close(error => error ? reject(error) : resolve()))
}
const dev = await createServer({ configLoader: 'runner', plugins: [adoptionPlugin()], cacheDir: `${directory}/vite-cache`,
  server: { host: '127.0.0.1', port: allocatedPort, strictPort: true, hmr: false } })
const errors = [], cases = [], report = { classification: 'TEST_ONLY', realService: false, status: 'NOT_RUN', errors, cases }
let browser
try {
  await dev.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const origin = `http://127.0.0.1:${dev.httpServer.address().port}`
  report.listenerPort = dev.httpServer.address().port
  await writeFile(`${directory}/admission.json`, `${JSON.stringify({ listenerPort: report.listenerPort, allocatedPort, source: process.env.MCR_GIFTS_NATIVE_PIN, at: new Date().toISOString() }, null, 2)}\n`)
  for (const width of [360, 390, 430]) for (const scenario of ['direct', 'list-detail', 'none', 'denied', 'held']) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' })
    const page = await context.newPage()
    page.on('pageerror', error => errors.push({ width, scenario, error: error.message }))
    const giftId = '00000000-0000-4000-8000-000000000005', mode = ['direct', 'list-detail'].includes(scenario) ? 'ready' : scenario
    const entry = scenario === 'list-detail' ? '/gifts' : `/gifts/${giftId}`
    const state = () => page.evaluate(() => window.__GIFTS_MEDIA_NATIVE__.state())
    const mediaCount = async () => (await state()).requests.filter(r => r.path.includes('/media/')).length
    const shot = async name => { await page.evaluate(() => document.fonts.ready); await page.screenshot({ path: `${directory}/${name}-${width}.png` }) }
    try {
      await page.goto(`${origin}/src/features/gift-discovery/__tests__/gift-detail-media/native/preview.html?entry=${entry}&mediaMode=${mode}`)
      await page.waitForFunction(() => !!window.__GIFTS_MEDIA_NATIVE__, undefined, { timeout: 15000 })
      await expect.poll(async () => (await state()).published, { timeout: 25000 }).toBe(true)
      await expect.poll(async () => (await state()).canvases).toBe(1)
      const initial = await state()
      // Cold start is covered before presence begins. Once started, composition keeps presence through panel coverage.
      assert.equal(initial.activeSockets, 0); assert.equal(initial.socketPeak, 0)
      if (scenario === 'list-detail') {
        await expect(page.getByRole('img', { name: 'Fan badges', exact: true })).toBeVisible()
        await shot('list-photo')
        assert.equal(await mediaCount(), 2)
        await page.getByRole('link', { name: 'Fan badges', exact: true }).click()
      }
      await expect(page.getByRole('heading', { name: 'Fan badges', exact: true })).toBeVisible()
      const media = page.getByRole('group', { name: 'Giveaway photo', exact: true })
      await expect(media).toBeVisible()
      if (scenario === 'none') {
        await expect(media.getByText('No photo', { exact: true })).toBeVisible(); assert.equal(await mediaCount(), 0)
      } else if (scenario === 'denied') {
        await expect(media.getByText('Image unavailable.', { exact: true })).toBeVisible()
        await expect(media.getByRole('img')).toHaveCount(0); assert.equal(await mediaCount(), 1)
        await shot('denied')
        await page.evaluate(() => window.__GIFTS_MEDIA_NATIVE__.setMediaMode('ready'))
        await media.getByRole('button', { name: 'Retry image read' }).click()
        await expect(media.getByRole('img', { name: 'Fan badges' })).toBeVisible(); assert.equal(await mediaCount(), 3)
      } else if (scenario === 'held') {
        await expect(media.getByText('Loading image…', { exact: true })).toBeAttached()
        await expect.poll(async () => (await state()).pending).toBe(1)
        await shot('loading')
        await page.evaluate(() => window.__GIFTS_MEDIA_NATIVE__.suspendPrivacy())
        await expect(media).toHaveCount(0)
        await page.evaluate(() => window.__GIFTS_MEDIA_NATIVE__.releaseMedia())
        await expect(page.locator('.root-media-image')).toHaveCount(0)
        const final = await state()
        assert.equal(final.protectedUrls.length, 0)
        assert.ok(final.requests.some(r => r.path.endsWith('/content') && r.aborted))
      } else {
        const image = media.getByRole('img', { name: 'Fan badges' })
        await expect(image).toBeVisible()
        assert.ok((await image.getAttribute('src')).startsWith('blob:'))
        await expect.poll(() => image.evaluate(node => node.complete && node.naturalWidth > 0)).toBe(true)
        assert.equal(await mediaCount(), 2)
      }
      if (scenario !== 'held') {
        await expect(page.getByText('1 remaining', { exact: true })).toBeVisible()
        await expect(page.getByRole('link', { name: 'Review reservation', exact: true })).toBeVisible()
        assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
        await shot(`detail-${scenario}`)
      }
      if (scenario === 'list-detail') {
        await page.getByRole('link', { name: '← All giveaways' }).click()
        await expect(page.getByRole('img', { name: 'Fan badges' })).toBeVisible(); assert.equal(await mediaCount(), 2)
        await page.getByRole('link', { name: 'Fan badges', exact: true }).click()
        await expect(media.getByRole('img', { name: 'Fan badges' })).toBeVisible(); assert.equal(await mediaCount(), 2)
      }
      let worldReturn = null
      if (scenario !== 'held') {
        await page.evaluate(() => window.__GIFTS_MEDIA_NATIVE__.navigate('/world'))
        await expect.poll(async () => (await state()).activeSockets).toBe(1)
        worldReturn = await state(); assert.equal(worldReturn.canvases, 1); assert.equal(worldReturn.socketPeak, 1)
        await page.evaluate(path => window.__GIFTS_MEDIA_NATIVE__.navigate(path), `/gifts/${giftId}`)
        await expect(media).toBeVisible()
        await expect.poll(async () => (await state()).activeSockets).toBe(1)
        if (scenario === 'none') await expect(media.getByText('No photo', { exact: true })).toBeVisible()
        else await expect(media.getByRole('img', { name: 'Fan badges' })).toBeVisible()
        assert.equal(await mediaCount(), scenario === 'none' ? 0 : scenario === 'denied' ? 3 : 2)
      }
      const before = await state()
      assert.ok(before.canvases <= 1); assert.ok(before.activeSockets <= 1); assert.equal(before.socketPeak, scenario === 'held' ? 0 : 1)
      if (scenario !== 'held') { assert.equal(before.canvases, 1); assert.equal(before.activeSockets, 1) }
      assert.equal(before.requests.filter(r => r.path.endsWith('/bootstrap')).length, 1)
      assert.ok(before.requests.filter(r => r.path.includes('/media/')).every(r => r.authorized))
      assert.equal(before.requests.filter(r => /\/(?:giveaways|reservations|beacons)(?:\/|$)/.test(r.path) && r.method !== 'GET').length, 0)
      if (['direct', 'list-detail', 'denied'].includes(scenario)) {
        await page.evaluate(() => window.__GIFTS_MEDIA_NATIVE__.suspendPrivacy())
        await expect(page.locator('.root-media-image')).toHaveCount(0)
        const withdrawn = await state()
        assert.ok(withdrawn.protectedUrls.every(url => withdrawn.revokedUrls.includes(url)))
      }
      await page.evaluate(() => window.__GIFTS_MEDIA_NATIVE__.dispose())
      const disposed = await state()
      assert.equal(disposed.canvases, 0); assert.equal(disposed.activeSockets, 0)
      assert.ok(disposed.protectedUrls.every(url => disposed.revokedUrls.includes(url)))
      cases.push({ width, scenario, initial, worldReturn, before, disposed, status: 'PASS' })
      await writeFile(`${directory}/native-progress.json`, `${JSON.stringify(report, null, 2)}\n`)
      console.log(JSON.stringify({ width, scenario, status: 'PASS' }))
    } catch (error) {
      await page.screenshot({ path: `${directory}/failure-${scenario}-${width}.png` }).catch(() => undefined)
      report.diagnostic = { width, scenario, state: await state().catch(() => null), text: await page.locator('body').innerText().catch(() => '') }
      throw error
    } finally { await context.close() }
  }
  assert.deepEqual(errors, [])
  Object.assign(report, { status: 'PASS', browser: browser.version(), limits: [
    'Synthetic HTTP/C05 and explicit TEST_ONLY clock policy; actual default router/providers/Root media/whole-character Phaser',
    'Existing 1x1 shared reader test bytes; no production artwork, media registration, rights mapping or real-service acceptance',
    'FE24 MOCK_READY and live IN02 remain separate',
  ] })
} catch (error) { Object.assign(report, { status: 'FAIL', failure: error.stack }); throw error }
finally {
  await writeFile(`${directory}/native.json`, `${JSON.stringify(report, null, 2)}\n`)
  const closing = { listenerPort: report.listenerPort, browserClosed: false, serverClosed: false, errors: [] }
  try { await browser?.close(); closing.browserClosed = true } catch (error) { closing.errors.push(String(error)) }
  try { await dev.close(); closing.serverClosed = true } catch (error) { closing.errors.push(String(error)) }
  await writeFile(`${directory}/native-cleanup.json`, `${JSON.stringify(closing, null, 2)}\n`)
  if (closing.errors.length) throw Error('Native cleanup failed; see native-cleanup.json')
}
console.log(JSON.stringify(report))
