import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import assert from 'node:assert/strict'
import { mkdir, writeFile, readFile, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
const root = resolve(import.meta.dirname, '../../../../..'), evidence = resolve(import.meta.dirname, 'evidence/touch-targets')
const disk = await statfs(root); assert(Number(disk.bavail) * Number(disk.bsize) >= 8 * 1024 ** 3, 'Below8GiB: hold browser growth')
await mkdir(evidence, { recursive: true })
// The retained root's social fixture has no Gifts HTTP handler. Supply only that
// test dependency; keep actual routes, Auth, root panels, world and styles intact.
const rootFixture = resolve(root, 'src/app/__tests__/default-shell/illustrated-overview/fixture.tsx')
const server = await createServer({ root, plugins: [{ name: 'gifts-touch-http-fixture', enforce: 'pre',
  resolveId(source, importer) { if (importer === rootFixture && source === '../../../social-composition/__tests__/fixtures') return resolve(import.meta.dirname, 'root-touch-data.ts') },
  transform(code, id) { if (id !== rootFixture) return; const needle = 'const response=await socialResponse(r)'; assert(code.includes(needle)); return code.replace(needle, needle + ";if(path.includes('/giveaways'))return response") },
}], logLevel: 'error', cacheDir: resolve(import.meta.dirname, '.vite-cache'), server: { host: '127.0.0.1', port: 41949, strictPort: true } })
const report = { result: 'FAIL', base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), qaSource: '12d987b34498fbfb434df468517eb1696b40d76b', scope: 'Actual default root/Auth/Phaser with explicit test-only C04 HTTP dependency injection; no panel override, live services or accepted avatar identity', cases: [], stress: [], errors: [] }
let browser
const geometry = () => {
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, right: r.right, bottom: r.bottom } }
  const overlap = (a, b) => Math.min(a.right, b.right) - Math.max(a.x, b.x) > .5 && Math.min(a.bottom, b.bottom) - Math.max(a.y, b.y) > .5
  return { overflow: document.documentElement.scrollWidth > innerWidth, targets: [...document.querySelectorAll('.gift-reference-copy h3 a,.gift-filters summary,.gift-reference-footnote a,.gift-reference-action :is(a,button),.gift-pagination button,.gifts-reference [role=tab]')].filter(e => e.checkVisibility()).map(e => ({ name: e.getAttribute('aria-label') ?? e.textContent, ...rect(e) })), rows: [...document.querySelectorAll('.gift-reference-row')].map(e => { const title = rect(e.querySelector('h3')), location = rect(e.querySelector('.gift-reference-location')), byline = rect(e.querySelector('.gift-reference-byline')), action = rect(e.querySelector('.gift-reference-action')); return { ...rect(e), overlap: overlap(title, action) || overlap(location, action) || overlap(byline, action) || overlap(title, byline) } }) }
}
try {
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const [width, height, hasTouch] of [[360, 608, true], [390, 608, true], [430, 608, true], [390, 844, true], [390, 608, false]]) {
    const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 1, hasTouch, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => report.errors.push(e.message))
    await page.goto('http://127.0.0.1:41949/src/app/__tests__/reference-root/preview.html?social=1')
    const state = () => page.evaluate(() => window.__ILLUSTRATED_MAP__?.state())
    await expect.poll(async () => (await state())?.published).toBe(true)
    await page.evaluate(() => window.__ILLUSTRATED_MAP__.navigate('/gifts')); await expect(page.locator('.gift-reference-row')).toHaveCount(3)
    const before = await state(), g = await page.evaluate(geometry)
    assert(!g.overflow); assert(g.targets.every(t => t.width >= 43.9 && t.height >= 43.9), JSON.stringify(g.targets)); assert(g.rows.every(r => !r.overlap && r.height <= 140), JSON.stringify(g.rows))
    const file = `root-${width}-${height}-${hasTouch ? 'touch' : 'mouse'}.png`; await page.screenshot({ path: resolve(evidence, file) })
    const reachable = []
    for (const link of await page.locator('.gift-reference-action a').all()) {
      await link.scrollIntoViewIfNeeded(); await expect(link).toBeInViewport({ ratio: 1 })
      assert(await link.evaluate(e => { const r = e.getBoundingClientRect(); return e.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)) }), 'Reserve must receive the hit after scrolling')
      reachable.push(await link.getAttribute('href'))
    }
    assert.equal(reachable.length, 3)
    const title = page.locator('.gift-reference-copy h3 a').first(), href = await title.getAttribute('href'); await title.focus(); assert.equal(await title.evaluate(e => getComputedStyle(e).outlineStyle), 'solid'); await page.keyboard.press('Enter'); await expect.poll(async () => (await state()).path).toBe(href)
    await page.evaluate(() => window.__ILLUSTRATED_MAP__.back()); await expect(title).toBeFocused()
    const summary = page.getByText('Filter giveaways', { exact: true }); await summary.focus(); await page.keyboard.press('Enter'); await expect(page.getByRole('combobox', { name: 'Status', exact: true })).toBeVisible()
    const after = await state(); assert.equal(after.games, 1); assert.equal(after.mounts, before.mounts); assert(after.activeSockets <= 1); assert(after.requests.filter(r => r.path.includes('/giveaways')).every(r => r.method === 'GET'))
    report.cases.push({ width, height, hasTouch, ...g, reserveReachableAfterScroll: reachable, keyboardDetailBackFocus: true, filterKeyboard: true, noBusinessMutation: true, singleWorld: true, file, sha256: createHash('sha256').update(await readFile(resolve(evidence, file))).digest('hex') })
    await page.evaluate(() => window.__ILLUSTRATED_MAP__.dispose()); await expect.poll(async () => (await state()).games).toBe(0); await expect.poll(async () => (await state()).activeSockets).toBe(0); await context.close()
  }
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 608 }, hasTouch: true })
    page.on('pageerror', e => report.errors.push(e.message))
    for (const scenario of ['long', 'unavailable']) {
      await page.goto(`http://127.0.0.1:41949/src/features/gift-discovery/__tests__/reference-native/preview.html?scenario=${scenario}`); await expect(page.locator('.gift-reference-row')).toHaveCount(3)
      const g = await page.evaluate(geometry); assert(!g.overflow); assert(g.rows.every(r => !r.overlap)); assert(g.targets.every(t => t.width >= 43.9 && t.height >= 43.9))
      report.stress.push({ width, scenario, ...g }); await page.evaluate(() => window.referenceGifts.dispose())
    }
    await page.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} finally { await browser?.close(); await server.close(); report.sourcePins = Object.fromEntries(await Promise.all(['ui/reference-gifts.css', 'ui/reference-presentation.tsx', '__tests__/reference-native/root-touch-data.ts'].map(async p => [p, createHash('sha256').update(await readFile(resolve(import.meta.dirname, '../..', p))).digest('hex')]))); await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log('PASS:5 actual composed-root Gifts cases, all44px targets, compact nonoverlapping rows, reachable3Reserve, keyboard detail/back/filter;6long/unavailable probes')
