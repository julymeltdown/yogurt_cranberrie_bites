import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
const frontend = resolve(import.meta.dirname, '../../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const disk = await statfs(frontend); assert(Number(disk.bavail) * Number(disk.bsize) >= 8 * 1024 ** 3, 'Below8GiB: hold browser growth')
await mkdir(evidence, { recursive: true })
const vite = await createServer({ root: frontend, logLevel: 'error', cacheDir: resolve(import.meta.dirname, '.vite-cache'), server: { host: '127.0.0.1', port: 41948, strictPort: true } })
let browser
const report = { result: 'FAIL', scope: 'Real WholeCharacterView/runtime with golden canonical configs and reviewed art; fixture canonical session, no live backend/world', checks: [], screenshots: [], errors: [], cleanup: 'NOT_RUN' }
try {
  await vite.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, hasTouch: true }), page = await context.newPage(); page.on('pageerror', e => report.errors.push(e.message))
    await page.goto('http://127.0.0.1:41948/src/entities/avatar/whole-character/__tests__/native/preview.html')
    await expect(page.locator('[data-whole-character-state=ready]')).toHaveCount(7)
    const snapshot = () => page.locator('.choices canvas').evaluateAll(canvases => canvases.map(c => ({ pixels: [...c.getContext('2d').getImageData(0, 0, 64, 64).data], smoothing: c.getContext('2d').imageSmoothingEnabled, width: c.getBoundingClientRect().width })))
    const first = await snapshot(); assert(first.every(c => c.width === 64 && c.smoothing === false && c.pixels.some((v, i) => i % 4 === 3 && v > 0)))
    const hashes = first.map(c => createHash('sha256').update(new Uint8Array(c.pixels)).digest('hex')); assert.equal(new Set(hashes).size, 6)
    for (const era of ['REVENGE', 'PARADE', 'DANGER']) {
      await page.getByRole('button', { name: era, exact: true }).click(); assert.deepEqual((await snapshot()).map(c => createHash('sha256').update(new Uint8Array(c.pixels)).digest('hex')), hashes)
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
      const file = `${era.toLowerCase()}-${width}.png`; await page.screenshot({ path: resolve(evidence, file), fullPage: true }); report.screenshots.push({ file, sha256: createHash('sha256').update(await readFile(resolve(evidence, file))).digest('hex') })
    }
    let state = await page.evaluate(() => window.wholeRuntime.state()); assert.equal(state.requests.length, 2); assert.equal(state.runtime.decodedAtlases, 1)
    await page.getByRole('button', { name: 'Preview photo01_front_06' }).focus(); await page.keyboard.press('Enter'); await expect(page.getByText('Current preset: photo01_front_06')).toBeVisible()
    await page.getByRole('button', { name: 'Request detail atlas' }).click(); await expect.poll(async () => (await page.evaluate(() => window.wholeRuntime.state())).runtime.decodedAtlases).toBe(2)
    await expect(page.locator('[data-whole-character-state=ready]')).toHaveCount(7); state = await page.evaluate(() => window.wholeRuntime.state()); assert.equal(state.requests.length, 3)
    await page.getByRole('button', { name: 'Unmount views' }).click(); await expect.poll(async () => (await page.evaluate(() => window.wholeRuntime.state())).runtime.leases).toBe(0)
    await page.getByRole('button', { name: 'Remount views' }).click(); await expect(page.locator('[data-whole-character-state=ready]')).toHaveCount(7); assert.equal((await page.evaluate(() => window.wholeRuntime.state())).requests.length, 3)
    // Exercise canonical revoke and synchronous consumer notification, then inspect the actual pixels.
    const sync = await page.evaluate(() => { const canvases = [...document.querySelectorAll('canvas')]; document.querySelectorAll('button').forEach(b => { if (b.textContent === 'Revoke scope') b.click() }); return canvases.every(c => !c.getContext('2d').getImageData(0, 0, c.width, c.height).data.some((v, i) => i % 4 === 3 && v > 0)) })
    assert.equal(sync, true); await expect(page.locator('[data-whole-character-state=ready]')).toHaveCount(0)
    const ended = await page.evaluate(() => window.wholeRuntime.state()); assert.equal(ended.runtime.leases, 0); assert.equal(ended.runtime.decodedAtlases, 0)
    report.checks.push({ width, sixDistinctPixelHashes: hashes, exact64px: true, noSmoothing: true, eraIdentityUnchanged: true, resourceReads: 3, detailOnlyOnRequest: true, consumerUnmountPreservesOwner: true, synchronousRevocationClearsPixels: true, final: ended.runtime })
    await page.evaluate(() => window.wholeRuntime.dispose()); await context.close()
  }
  assert.equal(report.errors.length, 0); report.cleanup = 'PASS'; report.result = 'PASS'
} finally { await browser?.close(); await vite.close(); await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log('PASS:9 genuine-art captures,6distinctidentities,3widths,stableeras,one64atlas,explicit256,independentleases+syncrevocation/unmount cleanup')
