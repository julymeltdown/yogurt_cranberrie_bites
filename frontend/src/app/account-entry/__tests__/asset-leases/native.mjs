// Builds only this isolated test entry. Catalog alias is test-only; no product review receipt is invented.
import assert from 'node:assert/strict'
import { writeFile, mkdir, statfs } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { build, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
import { syntheticAvatarPng } from '../../../../world/avatar/__tests__/fixtures/png.mjs'

const disk = await statfs('.'); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3, 'No build/capture below 8 GiB')
const directory = fileURLToPath(new URL('.', import.meta.url)), evidence = process.env.MCR_ASSET_LEASE_EVIDENCE ?? '/tmp/mcr-fe-asset-lease-evidence'
await mkdir(evidence, { recursive: true })
const outDir = path.join(evidence, 'dist'), png = syntheticAvatarPng(), catalog = path.join(directory, 'catalog.ts')
await build({ configFile: false, root: process.cwd(), logLevel: 'warn', plugins: [{ name: 'synthetic-lease-catalog-only', enforce: 'pre',
  resolveId(source, importer) { if (source.endsWith('/source-catalog') && importer?.endsWith('/entities/avatar/runtime/resources.ts')) return catalog },
}], build: { outDir, emptyOutDir: true, rollupOptions: { input: path.join(directory, 'preview.html') } } })
const server = await preview({ configFile: false, root: process.cwd(), logLevel: 'warn', build: { outDir }, preview: { host: '127.0.0.1', port: 0 } })
let browser
const errors = [], cases = []
try {
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, reducedMotion: 'reduce' })
    page.on('pageerror', error => errors.push(error.message))
    await page.route('**/synthetic-lease-atlas.png', route => route.fulfill({ contentType: 'image/png', body: Buffer.from(png) }))
    await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/app/account-entry/__tests__/asset-leases/preview.html`)
    const state = () => page.evaluate(() => window.__ASSET_LEASE__.state())
    const pixel = selector => page.locator(selector).evaluate(canvas => {
      const data = canvas.getContext('2d').getImageData(0, 0, 64, 64).data
      return { width: canvas.width, height: canvas.height, pixels: Array.from(data), smoothing: canvas.getContext('2d').imageSmoothingEnabled }
    })
    const go = async route => { await page.evaluate(r => window.__ASSET_LEASE__.navigate(r), route); await expect(page.locator('h1').first()).toHaveText(route) }
    await expect(page.locator('header canvas')).toBeVisible(); const cold = await state(), initial = await pixel('header canvas')
    assert.equal(cold.reads, 1); assert.equal(cold.decodes, 1); assert.equal(initial.width, 64); assert.equal(initial.height, 64); assert.equal(initial.smoothing, false)
    await go('me'); await expect(page.locator('[data-testid="me"] canvas')).toBeVisible()
    assert.deepEqual((await pixel('[data-testid="me"] canvas')).pixels, initial.pixels)
    await go('customize'); await expect.poll(async () => (await state()).phase).toBe('ready')
    await expect.poll(async () => (await state()).preview).toBe('ready')
    await page.evaluate(() => window.__ASSET_LEASE__.draft()); await expect.poll(async () => (await state()).draft?.item_ids.accessory).toBeNull()
    await expect.poll(async () => (await state()).preview).toBe('ready')
    const draft = await pixel('[data-testid="editor"] .mcr-avatar-preview canvas')
    assert.notDeepEqual(draft.pixels, initial.pixels)
    await go('world'); await expect.poll(async () => (await state()).phase).toBe('idle')
    assert.deepEqual((await pixel('header canvas')).pixels, initial.pixels)
    await go('customize'); await expect.poll(async () => (await state()).phase).toBe('ready'); await expect.poll(async () => (await state()).preview).toBe('ready')
    assert.equal((await state()).draft.item_ids.accessory, null)
    assert.deepEqual((await pixel('[data-testid="editor"] .mcr-avatar-preview canvas')).pixels, draft.pixels)
    await go('profile'); await expect(page.locator('.intro-avatar canvas')).toBeVisible()
    assert.deepEqual((await pixel('.intro-avatar canvas')).pixels, initial.pixels)
    const warm = await state(); assert.equal(warm.reads, 1); assert.equal(warm.decodes, 1); assert(warm.meReads >= 2)
    assert.deepEqual(await page.evaluate(() => window.__ASSET_LEASE__.baseline()), initial.pixels)
    const before = await statfs('.'); assert(before.bavail * before.bsize >= 8 * 1024 ** 3, 'No capture below 8 GiB')
    await page.screenshot({ path: path.join(evidence, `profile-${width}.png`) })
    await page.evaluate(() => window.__ASSET_LEASE__.revoke())
    await expect(page.locator('canvas')).toHaveCount(0)
    const end = await state(); assert.equal(end.disposed, true); assert.equal(end.leases.entries, 0); assert.equal(end.leases.references, 0)
    assert.equal(end.createdUrls, 2); assert.equal(end.revokedUrls, 2); assert.equal(await page.evaluate(() => window.__ASSET_LEASE__.erased()), true)
    await page.evaluate(() => window.__ASSET_LEASE__.dispose())
    cases.push({ width, cold, warm, end, originalPixelsUnchanged: true, equalsUnpooledPixels: true, baselineExtraDecode: 1,
      draftPixelsRetained: true, canvasesErased: true }); await page.close()
  }
  assert.deepEqual(errors, [])
  await writeFile(path.join(evidence, 'result.json'), JSON.stringify({ result: 'PASS', productionBuiltTestEntry: true, realImageDecode: true,
    testOnlySyntheticCatalog: true, productionSelectedArt: false, actualWorldRendererSharing: 'NOT_RUN', liveServices: 'NOT_RUN', cases, errors }, null, 2) + '\n')
  console.log(JSON.stringify({ result: 'PASS', evidence, widths: cases.length, errors }))
} finally { await browser?.close(); await new Promise(resolve => server.httpServer.close(resolve)) }
