// Actual FE04 + FE28 controllers and canonical binary HTTP; only the backend is a fixture.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const original = JSON.parse(await readFile('../contracts/fixtures/domain/public-post.json', 'utf8'))
const mediaId = '00000000-0000-4000-8000-000000000900'
let mediaType = '', bytes = null, digest = '', byteLength = 0, post = null
const writes = { init: 0, bytes: 0, finalize: 0, post: 0 }, reads = { metadata: 0, content: 0 }
const keys = new Set(), errors = []
const ready = () => ({ media_id: mediaId, state: 'READY', revision: 3, width: 64, height: 64, content_type: mediaType, purpose: 'POST', read_path: `/api/concert/events/${original.event_id}/media/${mediaId}/content` })
const dev = await createServer({ plugins: [{ name: 'test-only-feed-media-api', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/api/concert/')) return next()
    assert.equal(req.headers.authorization, 'Bearer test-only-token')
    const path = new URL(req.url, `http://${req.headers.host}`).pathname
    res.setHeader('Cache-Control', 'no-store')
    const json = (value, status = 200) => { res.statusCode = status; res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify(value)) }
    if (req.method === 'GET') {
      if (path.endsWith('/content')) { reads.content++; res.setHeader('Content-Type', mediaType); return res.end(bytes) }
      if (path.endsWith(`/media/${mediaId}`)) { reads.metadata++; return json(ready()) }
      return post ? json({ post, comments: [], next_cursor: null }) : json({}, 404)
    }
    assert.equal(req.headers['x-mcr-intent'], 'browser'); const key = req.headers['idempotency-key']
    assert.match(key, /^[0-9a-f-]{36}$/); assert(!keys.has(key)); keys.add(key)
    const chunks = []; for await (const chunk of req) chunks.push(chunk)
    const body = Buffer.concat(chunks)
    if (path.endsWith('/media/uploads')) {
      writes.init++; const input = JSON.parse(body.toString()); assert.equal(input.purpose, 'POST'); mediaType = input.content_type; digest = input.sha256; byteLength = input.byte_length
      return json({ media_id: mediaId, state: 'PENDING', upload_path: `/api/concert/events/${original.event_id}/media/${mediaId}/bytes`, max_bytes: 10485760, expires_at: '2099-01-01T00:00:00Z', upload_method: 'PUT', revision: 1 })
    }
    if (path.endsWith('/bytes')) {
      writes.bytes++; assert.equal(req.headers['content-type'], mediaType); assert.equal(body.length, byteLength); assert.equal(createHash('sha256').update(body).digest('hex'), digest)
      bytes = body; return json({ media_id: mediaId, state: 'UPLOADED', revision: 2 })
    }
    if (path.endsWith('/finalize')) { writes.finalize++; assert.equal(JSON.parse(body.toString()).expected_revision, 2); return json(ready()) }
    if (path.endsWith('/posts')) { writes.post++; const input = JSON.parse(body.toString()); assert.deepEqual(input.media_ids, [mediaId]); post = { ...original, ...input, post_id: '00000000-0000-4000-8000-000000000901', comment_count: 0, reaction_count: 0 }; return json(post) }
    return json({}, 404)
  })
} }], server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser
try {
  await dev.listen(); const base = `http://127.0.0.1:${dev.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } }); page.on('pageerror', e => errors.push(e.message))
  await page.goto(`${base}/src/features/feed-thread/__tests__/fixtures/media-preview.html`)
  const png = await page.evaluate(() => { const canvas = document.createElement('canvas'); canvas.width = canvas.height = 64; const context = canvas.getContext('2d'); context.fillStyle = '#34485a'; context.fillRect(0, 0, 64, 64); return canvas.toDataURL('image/png').split(',')[1] })
  await page.getByLabel('Post text', { exact: true }).fill('Actual FE04 image consumer, test only')
  await page.locator('input[type=file]').setInputFiles({ name: 'test-only-image.png', mimeType: 'image/png', buffer: Buffer.from(png, 'base64') })
  await expect(page.getByRole('button', { name: 'Upload selected image' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Publish post', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: 'Upload selected image' }).click()
  await expect(page.getByRole('button', { name: 'Add uploaded image' })).toBeVisible()
  await page.getByRole('button', { name: 'Add uploaded image' }).click()
  await expect(page.getByRole('button', { name: 'Remove image 1' })).toBeVisible()
  await page.getByRole('button', { name: 'Publish post', exact: true }).click()
  await page.getByRole('button', { name: 'Open published post' }).click()
  const image = page.getByRole('img', { name: 'Post attachment' }); await expect(image).toBeVisible()
  await expect(image).toHaveJSProperty('naturalWidth', 64)
  const source = await image.getAttribute('src'); assert(source.startsWith('blob:')); assert.deepEqual(writes, { init: 1, bytes: 1, finalize: 1, post: 1 })
  assert(reads.metadata >= 1 && reads.content >= 1); assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  const evidence = 'src/features/feed-thread/__tests__/evidence'; await mkdir(evidence, { recursive: true }); await page.screenshot({ path: `${evidence}/media-thread-390.png`, fullPage: true })
  await page.getByRole('button', { name: 'Revoke privacy' }).click(); await expect(image).toHaveCount(0)
  assert.equal(await page.evaluate(async url => { try { await fetch(url); return false } catch { return true } }, source), true)
  assert.deepEqual(errors, [])
  const report = { result: 'PASS', scope: 'Actual FE04/FE28 browser controllers, test-only native HTTP backend, StrictMode; BE24/live/devices NOT_RUN', browser: await browser.version(), width: 390, writes, reads, compressedType: mediaType, uploadedBytes: bytes.length, bytesHashVerified: true, privacyBlobRevoked: true, errors }
  await writeFile(`${evidence}/media-browser-report.json`, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report))
} finally { await browser?.close(); await dev.close() }
