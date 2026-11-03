import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import assert from 'node:assert/strict'
import { mkdir, writeFile, readFile, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
const root = resolve(import.meta.dirname, '../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const disk = await statfs(root); assert(Number(disk.bavail) * Number(disk.bsize) >= 8 * 1024 ** 3, 'Hold capture: below8GiB free')
await mkdir(evidence, { recursive: true })
const server = await createServer({ root, logLevel: 'error', cacheDir: resolve(import.meta.dirname, '.vite-cache'), server: { host: '127.0.0.1', port: 41937, strictPort: true } })
let browser
const report = { result: 'FAIL', scope: 'Production GiftDiscovery with frozen-contract fixture data; no actual backend or gift photographs; root chrome surrogate only', checks: [], captures: [], limits: ['Authorized real gift imagery/root cache integration pending', 'Shared root marching footer pending', 'No physical device or live reservation acceptance'], errors: [] }
const sourceFiles = ['src/features/gift-discovery/ui/Gifts.tsx', 'src/features/gift-discovery/ui/reference-presentation.tsx', 'src/features/gift-discovery/ui/reference-gifts.css', 'src/features/gift-discovery/ui/gifts.css', 'src/features/gift-discovery/model/controller.ts', 'src/features/gift-discovery/model/contract.ts', '../contracts/generated/mcr-domain.openapi.json', '../contracts/domain/v1/giveaway.schema.json', '../docs/mcr/party-gift-consumer-contract.md']
report.source_pins = Object.fromEntries(await Promise.all(sourceFiles.map(async file => [file.startsWith('../') ? file.slice(3) : `frontend/${file}`, createHash('sha256').update(await readFile(resolve(root, file))).digest('hex')])))
report.base = 'f837bef676cd26869c26c172a699a91b7842d832'
try {
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => report.errors.push(e.message))
    const go = scenario => page.goto(`http://127.0.0.1:41937/src/features/gift-discovery/__tests__/reference-native/preview.html?scenario=${scenario}`)
    const capture = async name => { const file = `${name}-${width}.png`; await page.screenshot({ path: resolve(evidence, file), fullPage: true }); report.captures.push({ file, width, sha256: createHash('sha256').update(await readFile(resolve(evidence, file))).digest('hex') }) }
    await go('open'); await expect(page.getByRole('link', { name: 'Fixture fan badges', exact: true })).toBeVisible()
    assert.equal(await page.locator('.gift-reference-row').count(), 3)
    const geometry = await page.locator('.gift-reference-row').evaluateAll(rows => rows.map(row => { const r = row.getBoundingClientRect(), i = row.querySelector('.gift-reference-image').getBoundingClientRect(); return { width: r.width, height: r.height, image: i.width, right: r.right } }))
    assert(geometry.every(r => r.image / r.width >= .22 && r.image / r.width <= .29 && r.right <= width && r.height < 180), 'dense quarter-width image-led rows')
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    assert.equal(await page.locator('.concert-window').count(), 1, 'no duplicate feature frame')
    const actions = await page.locator('.gift-reference-action a').evaluateAll(links => links.map(e => e.getBoundingClientRect().height)); assert(actions.every(h => h >= 44))
    await capture('browse')
    const requests = () => page.evaluate(() => window.referenceGifts.state().requests)
    const tab = page.getByRole('tab', { name: 'Browse', exact: true }); await tab.focus(); await page.keyboard.press('ArrowRight')
    await expect(page.getByRole('tab', { name: 'My giving' })).toBeFocused(); await expect(page.getByText('Only you can see this draft.')).toBeVisible()
    await page.keyboard.press('End'); await expect(page.getByRole('tab', { name: 'My pickups' })).toBeFocused(); await expect(page.getByText('No pickups on this page')).toBeVisible()
    await page.keyboard.press('Home'); await expect(tab).toBeFocused(); await expect(page.getByRole('link', { name: 'Fixture fan badges', exact: true })).toBeVisible()
    await page.getByRole('tab', { name: 'My giving' }).click(); await page.getByRole('tab', { name: 'Browse', exact: true }).click()
    const reads = await requests(); assert.equal(reads.filter(r => r.url.includes('/me/giveaways')).length, 1); assert(reads.every(r => r.method === 'GET')); assert.equal(reads.length, 2)
    const entry = page.getByRole('link', { name: 'Review reservation for Fixture fan badges' }); const href = await entry.getAttribute('href'); await entry.focus(); await page.keyboard.press('Enter'); assert(new URL(page.url()).pathname === href); assert((await requests()).every(r => r.method === 'GET'))
    await page.evaluate(() => window.referenceGifts.invalidate()); await expect(page.getByText('Gift access is unavailable')).toBeVisible(); assert.equal(await page.locator('.gift-reference-row').count(), 0)
    for (const scenario of ['unavailable', 'read-only', 'long', 'empty', 'error']) {
      await go(scenario)
      if (scenario === 'empty') await expect(page.getByText('No matching gifts')).toBeVisible()
      else if (scenario === 'error') { await expect(page.getByRole('alert')).toBeVisible(); assert.equal(await page.getByText('DO NOT DISPLAY PRIVATE TRANSPORT TEXT').count(), 0); await page.getByRole('button', { name: 'Try again' }).click(); await expect(page.getByRole('alert')).toBeVisible() }
      else { await expect(page.locator('.gift-reference-row')).toHaveCount(3); if (scenario !== 'long') { assert.equal(await page.locator('.gift-reference-action a').count(), 0); assert.equal(await page.locator('.gift-reference-action button:disabled').count(), 3) } }
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
      await capture(scenario)
    }
    report.checks.push({ width, rows: geometry, native_scale: 1, touch_targets: '>=44px', tabs_arrow_home_end: 'PASS', tab_refetch: 'NONE', reservation_entry: 'REAL_ROUTE_GET_ONLY', privacy_eviction: 'PASS', unavailable_states: 'PASS', overflow: 'NONE' })
    await page.evaluate(() => window.referenceGifts.dispose()); await expect(page.locator('.gifts-reference')).toHaveCount(0); await context.close()
  }
  const comparison = await browser.newPage({ viewport: { width: 1200, height: 1020 }, deviceScaleFactor: 1, hasTouch: true })
  await comparison.goto('http://127.0.0.1:41937/src/features/gift-discovery/__tests__/reference-native/comparison.html')
  await expect(comparison.frameLocator('iframe').getByRole('link', { name: 'Fixture fan badges', exact: true })).toBeVisible()
  await comparison.locator('img').evaluate(image => image.decode())
  await comparison.screenshot({ path: resolve(evidence, 'reference-comparison-390.png'), fullPage: true })
  report.comparison = { file: 'reference-comparison-390.png', reference_sha256: createHash('sha256').update(await readFile(resolve(evidence, 'exact-reference.png'))).digest('hex'), sha256: createHash('sha256').update(await readFile(resolve(evidence, 'reference-comparison-390.png'))).digest('hex') }
  assert.equal(report.comparison.reference_sha256, 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617')
  await comparison.close()
  assert.equal(report.errors.length, 0); report.result = 'PASS'
} finally { await browser?.close(); await server.close(); await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log('PASS: production Gifts component at360/390/430;18captures,keyboard/routes/eligibility/privacy/nooverflow; no live backend or real photos')
