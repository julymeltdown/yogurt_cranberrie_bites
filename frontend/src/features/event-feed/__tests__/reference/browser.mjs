import assert from 'node:assert/strict'
import { chromium, expect } from '@playwright/test'
import { build, preview } from 'vite'
import react from '@vitejs/plugin-react'
import { readFile, writeFile, mkdir, statfs } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
const frontend = resolve(import.meta.dirname, '../../../../..')
const evidence = resolve(import.meta.dirname, 'evidence'), outDir = '/tmp/mcr-fe-ref-feed-build'
const disk = await statfs(frontend); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3, '8 GiB disk floor')
await mkdir(evidence, { recursive: true })
const report = { result: 'FAIL', scope: 'FE-REF-FEED actual production EventFeed/FeedImage/canonical privacy; deterministic fixture transport and venue test imagery. Caller portrait mapping, real service/device/IN03 NOT_RUN.', browser: '', widths: [], errors: [], sourceHashes: {}, fixtureMedia: [] }
for (const path of ['EventFeed.tsx', 'FeedImage.tsx', 'FeedGlyph.tsx', 'presentation.ts', 'feed.css', 'controller.ts', 'contract.ts', 'runtime.ts']) report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(frontend, 'src/features/event-feed', path))).digest('hex')
const images = ['parade-scene.png', 'revenge-scene.png']
for (const image of images) report.fixtureMedia.push({ path: `frontend/public/world/illustrated/${image}`, sha256: createHash('sha256').update(await readFile(resolve(frontend, 'public/world/illustrated', image))).digest('hex') })
await build({ root: frontend, configFile: false, plugins: [react()], publicDir: false, cacheDir: '/tmp/mcr-fe-ref-feed-vite-cache', build: { outDir, emptyOutDir: true, target: 'es2022', rollupOptions: { input: resolve(import.meta.dirname, 'preview.html') } } })
const server = await preview({ root: frontend, configFile: false, build: { outDir }, preview: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    const page = await context.newPage(); page.on('pageerror', e => report.errors.push(e.message))
    await page.route('**/api/concert/events/*/media/*/content', route => route.fulfill({ contentType: 'image/png', path: resolve(frontend, 'public/world/illustrated', route.request().url().includes('000000000075') ? images[0] : images[1]) }))
    const base = `http://127.0.0.1:${server.httpServer.address().port}/src/features/event-feed/__tests__/reference/preview.html`
    await page.goto(base); await expect(page.locator('.feed-post')).toHaveCount(2)
    await expect(page.locator('.feed-media img')).toHaveCount(2)
    await page.locator('.feed-media img').evaluateAll(images => Promise.all(images.map(image => image.decode())))
    assert.equal(await page.locator('.feed-browse h1,.feed-browse .concert-window').count(), 0)
    const geometry = await page.locator('.feed-browse').evaluate(el => {
      const rows = [...el.querySelectorAll('.feed-post')].map(row => { const r = row.getBoundingClientRect(); return { top: r.top, bottom: r.bottom, height: r.height } })
      const controls = [...el.querySelectorAll('button,summary')].filter(e => e.checkVisibility()).map(e => ({ label: e.getAttribute('aria-label') || e.textContent, width: e.getBoundingClientRect().width, height: e.getBoundingClientRect().height }))
      return { scrollWidth: document.documentElement.scrollWidth, width: innerWidth, rows, controls, footerBottom: el.querySelector('.feed-footer').getBoundingClientRect().bottom }
    })
    assert(geometry.scrollWidth <= width, JSON.stringify(geometry)); assert(geometry.rows[1].bottom < 620, 'two dense articles visible')
    assert(geometry.controls.every(c => c.width >= 44 && c.height >= 44), JSON.stringify(geometry.controls))
    await page.screenshot({ path: resolve(evidence, `feed-${width}.png`), fullPage: true })
    await page.getByRole('tab', { name: 'ALL', exact: true }).focus(); await page.keyboard.press('ArrowRight')
    await expect(page.getByRole('tab', { name: 'MEETUP' })).toBeFocused(); await expect(page.getByText('No posts yet.', { exact: true })).toBeVisible()
    await page.keyboard.press('End'); await expect(page.getByRole('tab', { name: 'NOTICE' })).toBeFocused()
    await page.getByRole('button', { name: 'Fixture notice' }).click(); await expect(page.getByRole('link', { name: 'Source' })).toBeVisible()
    await page.keyboard.press('Tab'); await page.getByRole('tab', { name: 'ALL', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect(page.locator('.feed-post')).toHaveCount(2)
    const calls = await page.evaluate(() => [...window.__REFERENCE_FEED__.calls])
    await page.getByRole('button', { name: 'Open thread · 7 comments' }).click(); await expect(page.locator('#destination')).toContainText('Fixture thread')
    await page.getByRole('button', { name: 'Write a post' }).focus(); await page.keyboard.press('Enter'); await expect(page.locator('#destination')).toHaveText('Fixture compose destination')
    await page.screenshot({ path: resolve(evidence, `feed-focus-${width}.png`), fullPage: true })
    const beforeScrub = await page.evaluate(() => window.__REFERENCE_FEED__.resources()); assert.equal(beforeScrub, 2)
    await page.evaluate(() => window.__REFERENCE_FEED__.invalidate())
    await expect(page.locator('.feed-media img')).toHaveCount(0); assert.equal(await page.evaluate(() => window.__REFERENCE_FEED__.resources()), 0)
    await expect(page.getByRole('button', { name: 'Write a post' })).toHaveCount(0)
    for (const mode of ['empty', 'readonly', 'unavailable', 'pending', 'error']) {
      await page.goto(`${base}?mode=${mode}`)
      if (mode === 'readonly') { await expect(page.locator('.feed-post')).toHaveCount(2); await expect(page.getByRole('button', { name: 'Write a post' })).toHaveCount(0) }
      else if (mode === 'error') await expect(page.getByRole('alert')).toBeVisible()
      else await expect(page.locator('.feed-browse [role=status]').first()).toBeVisible()
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
      if (width === 390) await page.screenshot({ path: resolve(evidence, `feed-${mode}-390.png`), fullPage: true })
    }
    report.widths.push({ width, ...geometry, checks: ['two articles/media', 'four tabs and keyboard', 'notice source', 'thread/compose callbacks', '44px targets', 'no overflow', 'privacy removes images/revokes blobs', 'empty/read-only/unavailable/pending/error'], calls })
    await page.evaluate(() => window.__REFERENCE_FEED__.dispose()); await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} finally {
  await browser?.close(); await server.close()
  await writeFile(resolve(evidence, 'browser-report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(`FE-REF-FEED ${report.result}: 360/390/430`)
