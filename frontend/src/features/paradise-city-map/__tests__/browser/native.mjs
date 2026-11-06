import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const frontend = resolve(import.meta.dirname, '../../../../..')
const evidence = resolve(import.meta.dirname, '../../../../../../.omo/evidence/mcr-finish-project/task-25-map-friends')
const temporary = process.env.MCR_TASK25_TMP ?? '/tmp/mcr-task25-map-friends'
await mkdir(evidence, { recursive: true })
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'vite-cache'),
  server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const report = { result: 'FAIL', qualification: 'SUBSTITUTED_FIXTURE',
  note: 'Actual VenueShellFeatures, production L01 adapter, controller and Map panel. Deterministic session/community clock/Location/landmark/friend inputs; no live backend, device GPS or physical venue.',
  cases: [], errors: [] }
let browser, page, stage = 'start'

function denyGeolocation() {
  Object.defineProperty(navigator, 'geolocation', { configurable: true, value: {
    watchPosition(_success, error) { setTimeout(() => error?.({ code: 1, message: 'denied', PERMISSION_DENIED: 1, POSITION_UNAVAILABLE: 2, TIMEOUT: 3 }), 0); return 7 },
    clearWatch() {},
  } })
}
async function targets(host) {
  return host.evaluate(() => [...document.querySelectorAll('.pc-map button')].flatMap(button => {
    const rect = button.getBoundingClientRect()
    if (rect.width === 0 || rect.height === 0 || getComputedStyle(button).visibility === 'hidden') return []
    return rect.width < 43.5 || rect.height < 43.5
      ? [{ label: button.getAttribute('aria-label') ?? button.textContent?.trim(), width: Math.round(rect.width), height: Math.round(rect.height) }]
      : []
  }))
}
async function noOverflow(host) {
  return host.evaluate(() => document.documentElement.scrollWidth <= innerWidth)
}
async function open(width, height, mode, geolocationDenied) {
  const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce' })
  if (geolocationDenied) await context.addInitScript(denyGeolocation)
  page = await context.newPage(); page.on('pageerror', error => report.errors.push(`${mode}/${width}: ${error.message}`))
  await page.goto(`${origin}/src/features/paradise-city-map/__tests__/browser/fixture.html?mode=${mode}`)
  await expect(page.getByRole('tab', { name: 'MAP' })).toBeVisible({ timeout: 15_000 })
  return context
}
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[360, 800], [390, 844], [430, 932]]) {
    stage = `${width}/cluster`
    let context = await open(width, height, 'cluster', false)
    await expect(page.getByRole('button', { name: 'View cluster of 3 friends' })).toBeVisible()
    assert.equal(await noOverflow(page), true)
    assert.deepEqual(await targets(page), [])
    await page.screenshot({ path: resolve(evidence, `clustered-friends-${width}.png`), fullPage: true })
    await context.close()

    stage = `${width}/denied`
    context = await open(width, height, 'denied', true)
    await page.getByRole('button', { name: 'SHARE LOCATION' }).click()
    await page.getByRole('checkbox').check()
    await page.getByRole('button', { name: 'Allow phone location' }).click()
    await expect(page.getByText('Phone permission denied')).toBeVisible({ timeout: 10_000 })
    await page.getByRole('tab', { name: 'FRIENDS' }).click()
    assert.equal(await noOverflow(page), true)
    assert.deepEqual(await targets(page), [])
    await page.screenshot({ path: resolve(evidence, `permission-denied-${width}.png`), fullPage: true })
    await context.close()

    stage = `${width}/paused`
    context = await open(width, height, 'paused', false)
    await expect(page.locator('[aria-label="Your current shared position"]')).toBeVisible()
    assert.equal(await noOverflow(page), true)
    assert.deepEqual(await targets(page), [])
    await page.screenshot({ path: resolve(evidence, `self-marker-${width}.png`), fullPage: true })
    await page.getByRole('button', { name: 'PAUSE' }).click()
    await expect(page.getByText('Location sharing paused', { exact: true })).toBeVisible()
    assert.equal(await page.locator('[aria-label="Your current shared position"]').count(), 0)
    assert.equal(await noOverflow(page), true)
    assert.deepEqual(await targets(page), [])
    await page.screenshot({ path: resolve(evidence, `paused-${width}.png`), fullPage: true })
    await context.close()

    stage = `${width}/revoked`
    context = await open(width, height, 'revoked', false)
    await page.getByRole('button', { name: 'REVOKE' }).click()
    await page.getByRole('button', { name: 'STOP SHARING' }).click()
    await expect(page.getByText('Location sharing revoked', { exact: true })).toBeVisible()
    assert.equal(await page.locator('[aria-label="Your current shared position"]').count(), 0)
    assert.equal(await noOverflow(page), true)
    assert.deepEqual(await targets(page), [])
    await page.screenshot({ path: resolve(evidence, `revoked-${width}.png`), fullPage: true })
    await context.close()

    stage = `${width}/stale`
    context = await open(width, height, 'stale', false)
    await page.getByRole('tab', { name: 'FRIENDS' }).click()
    await page.evaluate(() => window.__VENUE_CLOCK__.advance(20000))
    await expect(page.getByText('Snapshot 20s ago')).toBeVisible()
    await expect(page.getByText(/Last update/).first()).toBeVisible()
    assert.equal(await noOverflow(page), true)
    assert.deepEqual(await targets(page), [])
    await page.screenshot({ path: resolve(evidence, `stale-${width}.png`), fullPage: true })
    await page.getByRole('button', { name: 'REFRESH' }).click()
    await expect(page.getByText('Snapshot 20s ago')).toHaveCount(0)
    await expect(page.getByText('Friend locations refreshed.')).toBeVisible()
    await page.screenshot({ path: resolve(evidence, `stale-recovered-${width}.png`), fullPage: true })
    await context.close()

    stage = `${width}/unavailable`
    context = await open(width, height, 'unavailable', false)
    await page.getByRole('tab', { name: 'FRIENDS' }).click()
    await expect(page.getByText('Friends unavailable')).toBeVisible()
    await expect(page.getByText(/friendship authority is unavailable \(503\)/)).toBeVisible()
    assert.equal(await page.getByText('No friend location is currently authorized.').count(), 0)
    assert.equal(await noOverflow(page), true)
    assert.deepEqual(await targets(page), [])
    await page.screenshot({ path: resolve(evidence, `unavailable-503-${width}.png`), fullPage: true })
    await context.close()

    report.cases.push({ width, height, cluster: true, permissionDenied: true, paused: true, revoked: true, stale: true, staleRecovered: true, unavailable503: true })
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
