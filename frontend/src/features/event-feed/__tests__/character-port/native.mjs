import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
const root = resolve(import.meta.dirname, '../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const disk = await statfs(root); assert(Number(disk.bavail) * Number(disk.bsize) >= 8 * 1024 ** 3, 'Below8GiB: hold native growth')
await mkdir(evidence, { recursive: true })
const server = await createServer({ root, logLevel: 'error', cacheDir: resolve(import.meta.dirname, '.vite-cache'), server: { host: '127.0.0.1', port: 41951, strictPort: true } })
let browser
const report = { result: 'FAIL', base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), scope: 'Published EventFeed/renderAuthor/canonical Feed parser/privacy with actual reviewed whole-character runtime/view/art. Explicit fixture author-config pairs and simulated HTTP; no root binding, no live Feed availability.', cases: [], errors: [] }
try {
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, hasTouch: true }), page = await context.newPage(); page.on('pageerror', e => report.errors.push(e.message))
    const go = mode => page.goto(`http://127.0.0.1:41951/src/features/event-feed/__tests__/character-port/preview.html?mode=${mode}`)
    const state = () => page.evaluate(() => window.feedCharacterPort.state())
    await go('loaded'); await expect(page.locator('.feed-portrait [data-whole-character-state=ready]')).toHaveCount(6)
    const pixelSnapshot = () => page.locator('.feed-portrait canvas').evaluateAll(canvases => canvases.map(c => {
      const r = c.getBoundingClientRect(), p = c.closest('.feed-portrait').getBoundingClientRect()
      return { pixels: [...c.getContext('2d').getImageData(0, 0, c.width, c.height).data], width: r.width, height: r.height, contained: r.x >= p.x && r.y >= p.y && r.right <= p.right + .1 && r.bottom <= p.bottom + .1, smoothing: c.getContext('2d').imageSmoothingEnabled }
    }))
    const first = await pixelSnapshot(), hashes = first.map(c => createHash('sha256').update(new Uint8Array(c.pixels)).digest('hex'))
    assert.equal(new Set(hashes).size, 6); assert(first.every(c => c.contained && !c.smoothing), JSON.stringify(first.map(({ pixels: _pixels, ...rest }) => rest)))
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    let s = await state(); assert.equal(s.authors.length, 6); assert.equal(s.assets.length, 2); assert.equal(s.runtime.decodedAtlases, 1); assert(s.reads.every(r => r.method === 'GET'))
    const file = `loaded-${width}.png`; await page.screenshot({ path: resolve(evidence, file), fullPage: true })
    // Center each actual slot before capture so the retained sticky footer cannot mask an identity in a full-page screenshot.
    const portraits = []
    for (let i = 0; i < 6; i++) {
      const slot = page.locator('.feed-portrait').nth(i); await slot.evaluate(e => e.scrollIntoView({ block: 'center' }))
      const name = `author-${i + 1}-${width}.png`; await slot.screenshot({ path: resolve(evidence, name) })
      portraits.push({ file: name, sha256: createHash('sha256').update(await readFile(resolve(evidence, name))).digest('hex') })
    }
    await page.getByRole('button', { name: 'View 42 reactions in thread' }).first().focus(); await page.keyboard.press('Enter'); assert.equal((await state()).actions.length, 1)
    await page.getByRole('button', { name: 'Open party', exact: true }).first().click(); await page.getByRole('button', { name: 'Write a post' }).click(); assert.equal((await state()).actions.length, 3)
    await page.getByRole('tab', { name: 'NOTICE', exact: true }).click(); await expect(page.getByText('No published notices.')).toBeVisible(); assert.equal((await state()).runtime.leases, 0)
    await page.getByRole('tab', { name: 'ALL', exact: true }).click(); await expect(page.locator('.feed-portrait [data-whole-character-state=ready]')).toHaveCount(6); assert.equal((await state()).assets.length, 2)
    await page.getByRole('button', { name: 'Omit portrait callback' }).click(); await expect(page.locator('.feed-initial')).toHaveCount(6); assert.equal((await state()).runtime.leases, 0); assert.equal((await state()).current, true)
    await page.getByRole('button', { name: 'Install portrait callback' }).click(); await expect(page.locator('.feed-portrait [data-whole-character-state=ready]')).toHaveCount(6)
    await page.getByRole('button', { name: 'Unmount feed' }).click(); await expect(page.locator('.event-feed')).toHaveCount(0); assert.equal((await state()).runtime.leases, 0); assert.equal((await state()).current, true)
    await page.getByRole('button', { name: 'Remount feed' }).click(); await expect(page.locator('.feed-portrait [data-whole-character-state=ready]')).toHaveCount(6); assert.equal((await state()).assets.length, 2)
    await page.getByRole('button', { name: 'Reload changed author' }).click(); await expect.poll(async () => createHash('sha256').update(new Uint8Array((await pixelSnapshot())[0]?.pixels ?? [])).digest('hex')).toBe(hashes[5])
    await page.getByRole('button', { name: 'Make read-only' }).click(); await expect(page.getByRole('button', { name: 'Write a post' })).toHaveCount(0); await expect(page.locator('.feed-portrait [data-whole-character-state=ready]')).toHaveCount(6)
    const synchronousClear = await page.evaluate(() => { const canvases = [...document.querySelectorAll('.feed-portrait canvas')]; [...document.querySelectorAll('button')].find(b => b.textContent === 'Revoke privacy').click(); return canvases.every(c => !c.getContext('2d').getImageData(0, 0, c.width, c.height).data.some((v, i) => i % 4 === 3 && v > 0)) })
    assert(synchronousClear); await expect(page.locator('.feed-portrait canvas')).toHaveCount(0); s = await state(); assert.equal(s.runtime.leases, 0); assert.equal(s.runtime.decodedAtlases, 0); assert.equal(s.runtime.cleanupFailures, 0)
    const checks = { width, portraits, sixDistinctHashes: hashes, portraitRects: first.map(({ pixels: _pixels, ...rest }) => rest), assets: s.assets, metadataAndCompactOnly: true, threadLinkComposeKeyboard: true, noticesAndRemountBorrowOwner: true, noCallbackFallback: true, changedIdentityPixelsMatch: true, readOnlyKeepsPortraits: true, synchronousClear, final: s.runtime, file, sha256: createHash('sha256').update(await readFile(resolve(evidence, file))).digest('hex') }
    for (const mode of ['fallback', 'null', 'denied']) {
      await page.evaluate(() => window.feedCharacterPort.dispose()); await go(mode)
      if (mode === 'fallback') await expect(page.locator('.feed-initial')).toHaveCount(6)
      else if (mode === 'null') { await expect(page.locator('.feed-portrait')).toHaveCount(6); assert.equal(await page.locator('.feed-portrait').evaluateAll(es => es.every(e => !e.hasChildNodes())), true) }
      else { await expect(page.getByRole('alert')).toBeVisible(); await expect(page.locator('.feed-portrait')).toHaveCount(0) }
      const idle = await state(); assert.equal(idle.assets.length, 0); assert.equal(idle.runtime.leases, 0); assert.equal(idle.authors.length, 0)
    }
    report.cases.push({ ...checks, fallbackNullDeniedNoAssetReads: true }); await page.evaluate(() => window.feedCharacterPort.dispose()); await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} finally {
  await browser?.close(); await server.close()
  report.sourcePins = Object.fromEntries(await Promise.all(['src/features/event-feed/EventFeed.tsx', 'src/features/event-feed/index.ts', 'src/features/event-feed/feed.css', 'src/features/event-feed/controller.ts', 'src/features/event-feed/runtime.ts', '../contracts/assets/v1/avatar-config.schema.json', '../contracts/fixtures/assets/selected-static/avatar-manifest.json'].map(async p => [p, createHash('sha256').update(await readFile(resolve(root, p))).digest('hex')])))
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log('PASS: actual published Feed callback +6 reviewed identities at360/390/430, exactfallback/null/denial, interactions, sharedowner reuse, changedauthor, syncrevoke+cleanup')
