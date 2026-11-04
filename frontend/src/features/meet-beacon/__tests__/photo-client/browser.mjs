import assert from 'node:assert/strict'
import { mkdir, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const fs = await statfs(root); assert(fs.bavail * fs.bsize > 8 * 1024 ** 3)
await mkdir(evidence, { recursive: true })
const server = await createServer({ root, configLoader: 'runner', cacheDir: '/tmp/mcr-fe20-photo-vite', logLevel: 'warn', server: { host: '127.0.0.1', port: 0, strictPort: false } })
await server.listen()
const report = { result: 'FAIL', qualification: 'Actual feature form/detail/controller, FE19, canonical Auth request/privacy, FE04 worker compression/upload/receipt/metadata+binary reader and Blob URLs. Deterministic in-memory HTTP server storage and synthetic reviewed-place/audience/identity/permission. No root availability, live producer, server invariants, device or multiuser acceptance.', cases: [], errors: [] }
let browser
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const url = `http://127.0.0.1:${server.httpServer.address().port}/src/features/meet-beacon/__tests__/photo-client/preview.html`
  for (const { width, mode } of [{ width: 360 }, { width: 390 }, { width: 430 }, { width: 390, mode: 'unknown-upload' }]) {
    const page = await browser.newPage({ viewport: { width, height: 844 } }); page.setDefaultTimeout(15000); page.on('pageerror', e => report.errors.push(e.message))
    await page.goto(url); await expect(page.getByLabel('Reviewed place')).toBeEnabled()
    await page.getByLabel('Reviewed place').selectOption({ label: 'Synthetic reviewed landmark' })
    await page.getByLabel('Who can see this?').selectOption('synthetic-party')
    await page.getByLabel('How to find me').fill('Synthetic test marker; no venue guidance')
    await page.getByLabel('Identification', { exact: true }).fill('Synthetic test scarf')
    const png = await page.evaluate(async () => {
      const c = document.createElement('canvas'); c.width = 256; c.height = 160; const ctx = c.getContext('2d'); ctx.fillStyle = '#35454a'; ctx.fillRect(0, 0, 256, 160); ctx.fillStyle = '#d8c58b'; ctx.fillRect(52, 28, 152, 104)
      return [...new Uint8Array(await (await new Promise(resolve => c.toBlob(resolve, 'image/png'))).arrayBuffer())]
    })
    await page.getByLabel('Identification photo (optional)').setInputFiles({ name: 'synthetic-private-name.png', mimeType: 'image/png', buffer: Buffer.from(png) })
    const local = page.getByRole('img', { name: 'Private meeting photo draft', exact: true }); await expect(local).toBeVisible(); await local.evaluate(image => image.decode())
    await expect(page.getByRole('button', { name: 'Upload photo', exact: true })).toBeDisabled()
    await expect(page.getByRole('button', { name: 'Share check-in', exact: true })).toBeDisabled()
    const prepared = await page.evaluate(() => window.__MEETING_PHOTO__.state())
    assert.equal(prepared.requests.some(r => r.path.endsWith('/uploads')), false)
    await page.getByRole('checkbox', { name: 'I own this photo or have permission to share it with this audience.', exact: true }).check()
    if (mode === 'unknown-upload') await page.evaluate(() => window.__MEETING_PHOTO__.unknownUpload())
    await page.getByRole('button', { name: 'Upload photo', exact: true }).click()
    if (mode === 'unknown-upload') {
      await expect(page.getByRole('button', { name: 'Check photo upload status', exact: true })).toBeVisible()
      await expect(page.getByRole('button', { name: 'Share check-in', exact: true })).toBeDisabled()
      await expect(page.getByLabel('Identification photo (optional)')).toBeDisabled()
      await page.getByRole('button', { name: 'Check photo upload status', exact: true }).click()
      await expect.poll(() => page.evaluate(() => window.__MEETING_PHOTO__.state().upload)).toBe('uncertain')
      const unknown = await page.evaluate(() => window.__MEETING_PHOTO__.state())
      assert.equal(unknown.requests.filter(r => r.path.endsWith('/finalize')).length, 1)
      assert.equal(unknown.requests.filter(r => r.path.endsWith('/beacons') && r.method === 'POST').length, 0)
      await page.screenshot({ path: resolve(evidence, 'unknown-upload-390.png'), fullPage: true })
      await page.getByRole('button', { name: 'Remove photo from draft', exact: true }).click()
      await expect(local).toHaveCount(0); await expect(page.getByLabel('Identification photo (optional)')).toBeEnabled()
      report.cases.push({ width, mode, result: 'PASS', unknown })
      await page.evaluate(() => window.__MEETING_PHOTO__.dispose()); await page.close(); continue
    }
    await expect.poll(() => page.evaluate(() => window.__MEETING_PHOTO__.state().upload)).toBe('ready')
    await page.getByRole('checkbox', { name: 'I explicitly confirm this real-world description and its audience.', exact: true }).check()
    await page.getByRole('button', { name: 'Share check-in', exact: true }).click()
    const saved = page.getByRole('img', { name: 'Shared meeting identification photo', exact: true }); await expect(saved).toBeVisible(); await saved.evaluate(image => image.decode())
    const first = await page.evaluate(() => window.__MEETING_PHOTO__.state())
    assert.equal(first.photo.mediaId, first.persisted.meeting.media_id)
    assert(first.photo.src.startsWith('blob:')); assert(first.revoked.includes(prepared.created[0]))
    assert.equal(first.requests.filter(r => r.method === 'POST' && r.path.endsWith('/beacons')).length, 1)
    await page.evaluate(() => window.__MEETING_PHOTO__.reread())
    await expect(saved).toBeVisible(); await saved.evaluate(image => image.decode())
    const reread = await page.evaluate(() => window.__MEETING_PHOTO__.state())
    assert.equal(reread.photo.mediaId, first.photo.mediaId); assert.notEqual(reread.photo.src, first.photo.src); assert(reread.revoked.includes(first.photo.src))
    assert.equal(reread.requests.filter(r => r.path.endsWith('/uploads')).length, 1)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    await page.screenshot({ path: resolve(evidence, `saved-reread-${width}.png`), fullPage: true })
    await page.evaluate(() => window.__MEETING_PHOTO__.revoke())
    await expect(saved).toHaveCount(0); await expect(local).toHaveCount(0)
    const revoked = await page.evaluate(() => window.__MEETING_PHOTO__.state())
    assert(revoked.created.every(url => revoked.revoked.includes(url)))
    report.cases.push({ width, result: 'PASS', prepared: { upload: prepared.upload, requests: prepared.requests }, first, reread, revoked })
    await page.evaluate(() => window.__MEETING_PHOTO__.dispose()); await page.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = error.stack; throw error }
finally { await writeFile(resolve(evidence, 'browser.json'), JSON.stringify(report, null, 2) + '\n'); await browser?.close(); await server.close() }
console.log(JSON.stringify({ result: report.result, cases: report.cases.map(({ width, result }) => ({ width, result })), errors: report.errors }))
