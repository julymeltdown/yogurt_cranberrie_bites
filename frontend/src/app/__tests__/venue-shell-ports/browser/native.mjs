import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const frontend = resolve(import.meta.dirname, '../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
await mkdir(evidence, { recursive: true })
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: '/tmp/mcr-venue-shell-ports-cache',
  server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
await server.listen()
const report = { result: 'FAIL', qualification: 'SUBSTITUTED_FIXTURE', note: 'Actual VenueShellFeatures/production ports/siteplan loader/Map panel. Deterministic session, community clock, Location, landmark and reviewed friend inputs; no live backend, device or physical venue.', cases: [], errors: [] }
let browser, page, stage = 'start'
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[360, 800], [390, 844], [430, 932]]) {
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce' })
    page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    const origin = `http://127.0.0.1:${server.httpServer.address().port}`
    stage = `${width}/real`; await page.goto(`${origin}/src/app/__tests__/venue-shell-ports/browser/fixture.html?mode=real`)
    await expect(page.getByRole('tab', { name: 'MAP' })).toBeVisible({ timeout: 15_000 })
    await page.getByRole('tab', { name: 'FRIENDS' }).click()
    await expect(page.getByRole('button', { name: 'Fixture friend' })).toBeVisible()
    const fallback = page.locator('.pc-portrait-fallback')
    await expect(fallback).toHaveCount(1)
    const fit = await fallback.evaluate(element => {
      const frame = element.parentElement, box = frame?.getBoundingClientRect()
      if (!frame || !box) return { clipped: true, reason: 'missing portrait frame' }
      const style = getComputedStyle(frame)
      const inner = { left: box.left + parseFloat(style.borderLeftWidth), right: box.right - parseFloat(style.borderRightWidth),
        top: box.top + parseFloat(style.borderTopWidth), bottom: box.bottom - parseFloat(style.borderBottomWidth) }
      const range = document.createRange(); range.selectNodeContents(element)
      const text = range.getBoundingClientRect()
      return { clipped: text.left < inner.left - 0.5 || text.right > inner.right + 0.5 || text.top < inner.top - 0.5 || text.bottom > inner.bottom + 0.5
        || frame.scrollWidth > frame.clientWidth || frame.scrollHeight > frame.clientHeight,
        text: { left: text.left, right: text.right, top: text.top, bottom: text.bottom }, inner }
    })
    assert.equal(fit.clipped, false, `${width}: fallback label clipped ${JSON.stringify(fit)}`)
    await page.screenshot({ path: resolve(evidence, `real-friends-${width}.png`), fullPage: true })
    await page.getByRole('tab', { name: 'MAP' }).click()
    await page.screenshot({ path: resolve(evidence, `real-capability-${width}.png`), fullPage: true })
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    stage = `${width}/removed`; await page.goto(`${origin}/src/app/__tests__/venue-shell-ports/browser/fixture.html?mode=removed`)
    await expect(page.getByRole('status')).toBeVisible({ timeout: 15_000 })
    await expect(page.getByRole('tab', { name: 'MAP' })).toHaveCount(0)
    await page.screenshot({ path: resolve(evidence, `removed-port-${width}.png`), fullPage: true })
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    report.cases.push({ width, height, realCapability: true, removedPortUnavailable: true })
  }
  assert.deepEqual(report.errors, [])
  report.result = 'PASS'
} catch (error) {
  report.errors.push(`${stage}: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await browser?.close()
  await server.close()
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2))
}
if (report.result !== 'PASS') { console.error(JSON.stringify(report, null, 2)); process.exit(1) }
console.log(JSON.stringify(report))
