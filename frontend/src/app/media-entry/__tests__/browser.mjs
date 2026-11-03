import assert from 'node:assert/strict'
import { mkdir, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const disk = await statfs(root); assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, '8GiB disk floor')
await mkdir(evidence, { recursive: true })
let photo = null, browser
const event = '00000000-0000-4000-8000-000000000002', post = '00000000-0000-4000-8000-000000000003', gift = '00000000-0000-4000-8000-000000000004'
const served = [], errors = []
const middleware = (req, res, next) => {
  if (!req.url?.startsWith(`/api/concert/events/${event}/media/`)) return next()
  const match = req.url.match(/\/media\/([0-9a-f-]+)(\/content)?$/), media = match?.[1], purpose = media === post ? 'POST' : media === gift ? 'GIVEAWAY' : null
  res.setHeader('Cache-Control', 'no-store')
  if (!photo || !purpose || req.headers.authorization !== 'Bearer synthetic-media-lease') { res.statusCode = 403; res.end(); return }
  const body = match[2] ? photo : Buffer.from(JSON.stringify({ media_id: media, state: 'READY', revision: 1, width: 96, height: 64, content_type: 'image/png', purpose, read_path: `/api/concert/events/${event}/media/${media}/content` }))
  res.setHeader('Content-Type', match[2] ? 'image/png' : 'application/json'); res.setHeader('Content-Length', body.length)
  served.push({ purpose, kind: match[2] ? 'content' : 'metadata', responseBodyBytes: body.length }); res.end(body)
}
const server = await createServer({ root, configLoader: 'runner', cacheDir: '/tmp/mcr-media-lease-vite', plugins: [{ name: 'synthetic-authorized-media', configureServer(server) { server.middlewares.use(middleware) } }], server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
await server.listen()
const report = { result: 'FAIL', qualification: 'Isolated loopback HTTP fixture with synthetic authorized parent rows/identity/photos; actual canonical SessionController + privacy + shared reader and production FeedImage. Gift rendering here is a fixture, root GiftImage binding remains owner work. No live services/readiness/device claim.', errors, cases: [] }
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } }); page.on('pageerror', e => errors.push(e.message))
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/app/media-entry/__tests__/preview.html`)
  // New synthetic qualification photo only, never source/product character artwork.
  photo = Buffer.from(await page.evaluate(async () => {
    const canvas = document.createElement('canvas'); canvas.width = 96; canvas.height = 64
    const c = canvas.getContext('2d'); const gradient = c.createLinearGradient(0, 0, 96, 64); gradient.addColorStop(0, '#a94455'); gradient.addColorStop(1, '#1c4460'); c.fillStyle = gradient; c.fillRect(0, 0, 96, 64); c.fillStyle = '#ddd4b8'; c.beginPath(); c.arc(66, 22, 12, 0, Math.PI * 2); c.fill(); c.fillStyle = '#23382d'; c.fillRect(0, 48, 96, 16)
    const blob = await new Promise(resolve => canvas.toBlob(resolve, 'image/png')); return [...new Uint8Array(await blob.arrayBuffer())]
  }))
  assert.equal(served.length, 0)
  let decodeCalls = 0
  async function visiblePhoto(name) { const image = page.getByRole('img', { name }); await expect(image).toBeVisible(); await image.evaluate(image => image.decode()); decodeCalls++; assert.equal(await image.evaluate(image => image.naturalWidth), 96); return image.getAttribute('src') }
  await page.getByRole('button', { name: 'Show post' }).click(); const cold = await visiblePhoto('Authorized synthetic POST photo')
  assert(cold.startsWith('blob:')); assert.equal(served.length, 2)
  await page.getByRole('button', { name: 'Leave row' }).click(); await expect(page.getByRole('img')).toHaveCount(0)
  await page.getByRole('button', { name: 'Show post' }).click(); const warm = await visiblePhoto('Authorized synthetic POST photo')
  assert.equal(warm, cold); assert.equal(served.length, 2)
  await page.getByRole('button', { name: 'Show gift' }).click(); await visiblePhoto('Authorized synthetic GIVEAWAY photo'); assert.equal(served.length, 4)
  const before = await page.evaluate(() => window.__MEDIA_LEASE__.stats()); assert.equal(before.created.length, 2)
  const resources = await page.evaluate(() => performance.getEntriesByType('resource').filter(entry => entry.name.includes('/api/concert/')).map(entry => ({ encodedBodySize: entry.encodedBodySize, transferSize: entry.transferSize, decodedBodySize: entry.decodedBodySize })))
  await page.screenshot({ path: resolve(evidence, 'synthetic-gift-390.png') })
  await page.getByRole('button', { name: 'Revoke policy' }).click()
  await expect(page.getByRole('img')).toHaveCount(0)
  const after = await page.evaluate(() => window.__MEDIA_LEASE__.stats())
  assert.equal(after.pool.disposed, true); assert.equal(after.pool.entries, 0); assert.equal(after.revoked.length, 2)
  assert.equal(await page.evaluate(async url => { try { await fetch(url); return false } catch { return true } }, cold), true)
  await page.screenshot({ path: resolve(evidence, 'revoked-390.png') })
  await page.evaluate(() => window.__MEDIA_LEASE__.dispose())
  report.cases.push({ initialRequestsBeforeConsumer: 0, coldPostRequests: 2, warmPostAdditionalRequests: 0, giveawayAdditionalRequests: 2, serverResponses: served, serverPhotoBytes: photo.length, resourceTiming: resources, explicitImageDecodeCalls: decodeCalls, decoderCacheOrGPUClaims: 'NOT_MEASURED', before, after, revokedBlobFetchDenied: true })
  assert.deepEqual(errors, []); report.result = 'PASS'; await page.close()
} finally { await browser?.close(); await server.close(); await writeFile(resolve(evidence, 'browser.json'), JSON.stringify(report, null, 2) + '\n') }
