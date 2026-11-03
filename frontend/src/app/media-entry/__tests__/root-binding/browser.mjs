import assert from 'node:assert/strict'
import { mkdir, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const disk = await statfs(root); assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, '8GiB free disk floor')
await mkdir(evidence, { recursive: true })
const id = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`, postMedia = id(51), giftMedia = id(52), lastMedia = id(206)
const allowed = new Set([postMedia, giftMedia, ...Array.from({ length: 7 }, (_, n) => id(200 + n))])
let photo = null, browser, activePage
const served = [], errors = []
const server = await createServer({ root, configLoader: 'runner', cacheDir: '/tmp/mcr-root-media-vite', logLevel: 'warn', server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'synthetic-authorized-root-media', configureServer(server) {
  server.middlewares.use((req, res, next) => {
    const match = req.url?.match(/^\/api\/concert\/events\/([0-9a-f-]+)\/media\/([0-9a-f-]+)(\/content)?$/)
    if (!match) return next()
    const [, event, media, content] = match
    if (!photo || !allowed.has(media) || req.headers.authorization !== 'Bearer SYNTHETIC_SOCIAL') { res.statusCode = 403; res.end(); return }
    const purpose = media === giftMedia ? 'GIVEAWAY' : 'POST'
    const body = content ? photo : Buffer.from(JSON.stringify({ media_id: media, state: 'READY', revision: 1, width: 96, height: 64, content_type: 'image/png', purpose, read_path: `/api/concert/events/${event}/media/${media}/content` }))
    res.setHeader('Cache-Control', 'no-store'); res.setHeader('Content-Type', content ? 'image/png' : 'application/json'); res.setHeader('Content-Length', body.length)
    served.push({ media, purpose, kind: content ? 'content' : 'metadata', responseBodyBytes: body.length }); res.end(body)
  })
} }] })
await server.listen()
const report = { result: 'FAIL', qualification: 'Actual createAppRoutes + default SocialPanel feed/thread + CommunityFeatures GiftsPanel, with RootMediaProvider at fixture shell boundary. Explicit authorized row/identity/bootstrap/availability fixtures; loopback protected media HTTP and instrumented world host. Production WorldShell wrapping remains separate owner work; no live-service/multiuser/device/readiness claim.', errors, cases: [] }
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    const start = served.length, page = await browser.newPage({ viewport: { width, height: 844 } }); activePage = page; page.setDefaultTimeout(10000); page.on('pageerror', error => errors.push(error.message))
    // Install a synthetic PNG before protected row reads can start.
    photo = Buffer.from(await page.evaluate(async () => { const c = document.createElement('canvas'); c.width = 96; c.height = 64; const x = c.getContext('2d'); x.fillStyle = '#315269'; x.fillRect(0, 0, 96, 64); x.fillStyle = '#e4d8b9'; x.beginPath(); x.arc(64, 24, 14, 0, Math.PI * 2); x.fill(); const b = await new Promise(resolve => c.toBlob(resolve, 'image/png')); return [...new Uint8Array(await b.arrayBuffer())] }))
    await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/app/media-entry/__tests__/root-binding/preview.html`)
    const first = page.locator('.feed-post').first(), firstView = first.locator('.root-media-view')
    await firstView.scrollIntoViewIfNeeded()
    const image = first.getByRole('img', { name: 'Post attachment' }); await expect(image).toBeVisible(); await image.evaluate(image => image.decode()); const cold = await image.getAttribute('src')
    const count = media => served.slice(start).filter(row => row.media === media).length
    assert.equal(count(postMedia), 2); assert.equal(count(lastMedia), 0)
    const beforeGeometry = await firstView.boundingBox()
    const last = page.locator('.feed-post').filter({ hasText: 'Authorized below-fold row 6' })
    await last.locator('.root-media-view').scrollIntoViewIfNeeded(); await expect(last.getByRole('img', { name: 'Post attachment' })).toBeVisible()
    await expect(image).toHaveCount(0)
    const outsideGeometry = await firstView.boundingBox(); assert.equal(outsideGeometry.height, beforeGeometry.height)
    assert.equal(count(lastMedia), 2)
    await firstView.scrollIntoViewIfNeeded(); await expect(image).toBeVisible(); assert.equal(await image.getAttribute('src'), cold); assert.equal(count(postMedia), 2)
    await page.screenshot({ path: resolve(evidence, `feed-${width}.png`) })
    await page.evaluate(() => window.__ROOT_MEDIA_BINDING__.thread()); await expect(page.getByRole('img', { name: 'Post attachment' })).toBeVisible(); assert.equal(count(postMedia), 2)
    await page.evaluate(() => window.__ROOT_MEDIA_BINDING__.navigate('/gifts'))
    const gift = page.getByRole('img', { name: 'Authorized synthetic giveaway' }); await expect(gift).toBeVisible(); await gift.evaluate(image => image.decode()); assert.equal(count(giftMedia), 2)
    await page.screenshot({ path: resolve(evidence, `gifts-${width}.png`) })
    const before = await page.evaluate(() => window.__ROOT_MEDIA_BINDING__.state())
    assert.equal(before.requests.filter(path => path.includes(`/media/${postMedia}`)).length, 2)
    assert.equal(before.world.mounts, 1); assert.equal(before.world.peakGames, 1)
    const resources = await page.evaluate(() => performance.getEntriesByType('resource').filter(entry => entry.name.includes('/media/')).map(entry => ({ media: entry.name.match(/\/media\/([^/]+)/)?.[1], encodedBodySize: entry.encodedBodySize, transferSize: entry.transferSize, decodedBodySize: entry.decodedBodySize })))
    await page.evaluate(() => window.__ROOT_MEDIA_BINDING__.revoke()); await expect(page.getByRole('img', { name: 'Authorized synthetic giveaway' })).toHaveCount(0)
    const after = await page.evaluate(() => window.__ROOT_MEDIA_BINDING__.state())
    assert.equal(after.revoked.length, after.created.length); assert.equal(new Set(after.revoked).size, after.created.length)
    assert.equal(await page.evaluate(async url => { try { await fetch(url); return false } catch { return true } }, cold), true)
    report.cases.push({ width, coldPostRequests: 2, warmPostAdditionalRequests: 0, threadAdditionalPostRequests: 0, belowFoldBeforeVisibleRequests: 0, belowFoldAfterVisibleRequests: 2, giveawayRequests: 2, geometryPreservedOnExit: true, responses: served.slice(start), resourceTiming: resources, explicitDecodeCalls: 2, internalDecoderCacheClaims: 'NOT_MEASURED', before, after })
    await page.evaluate(() => window.__ROOT_MEDIA_BINDING__.dispose()); await page.close()
  }
  assert.deepEqual(errors, []); report.result = 'PASS'
} catch (error) { report.failure = { message: String(error), body: await activePage?.locator('body').innerText() }; throw error }
finally { await browser?.close(); await server.close(); await writeFile(resolve(evidence, 'browser.json'), JSON.stringify(report, null, 2) + '\n') }
