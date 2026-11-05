/** Execute only inside a separately accepted finite browser lease and dual guards. */
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'

assert(process.env.MCR_VENUE_BROWSER_ACCEPTED, 'A real finite browser offer is required')
const front = resolve(import.meta.dirname, '../..'), root = resolve(front, '..')
const output = process.env.MCR_VENUE_BROWSER_OUTPUT, scratch = process.env.MCR_VENUE_BROWSER_SCRATCH
const spatial = process.env.MCR_VENUE_SPATIAL_INPUT, executablePath = process.env.MCR_VENUE_BROWSER_EXECUTABLE
assert(output && scratch && spatial && executablePath)
assert(resolve(output).startsWith(resolve(root, 'docs/mcr/evidence/venue-community') + '/'))
await mkdir(output, { recursive: true })
const report = { result: 'FAIL', classification: 'CONTROLLED_COMPONENT_FIXTURE',
  limitation: 'Real newly authored components with normalized adapter fixture. Not production shell/HTTP, backend, two-principal, device GPS or product acceptance.',
  sourceCommit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  fixtureClockMs: 1_800_000_000_000, originalPhotoSHA256: 'f7d841c815dfbf85b48b7e61a06106b9edab195e7bfbd4d37b78d8b23e610ac1',
  spatialSHA256: createHash('sha256').update(await readFile(spatial)).digest('hex'), widths: [], errors: [] }
const server = await createServer({ configFile: false, root: front, cacheDir: resolve(scratch, 'vite-cache'), logLevel: 'error',
  resolve: { alias: [{ find: /^.*\/features\/paradise-city-map\/spatial\/contract$/, replacement: spatial }] },
  server: { host: '127.0.0.1', port: 0, strictPort: false, watch: null, ws: false } })
let browser
async function capture(page, name, locator) {
  const path = resolve(output, name + '.png')
  await locator.screenshot({ path })
  return { path, capturedAt: new Date().toISOString(), viewport: page.viewportSize(),
    dpr: await page.evaluate(() => devicePixelRatio), componentBounds: await locator.boundingBox() }
}
try {
  await server.listen()
  const address = server.httpServer.address(); assert(address && typeof address !== 'string')
  browser = await chromium.launch({ executablePath, headless: true }); report.browserVersion = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, hasTouch: true, reducedMotion: 'reduce' })
    const page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(`http://127.0.0.1:${address.port}/e2e/venue-community/preview.html`)
    await expect(page.getByRole('button', { name: 'Accept', exact: true })).toBeVisible()
    assert.equal((await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.state())).writes, 0)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    const invitation = await capture(page, `invitation-${width}`, page.locator('.venue-meeting-invitation'))
    await page.getByRole('button', { name: 'Accept', exact: true }).click()
    await expect(page.locator('.venue-meeting-status-accepted')).toBeVisible()
    await expect(page.locator('.venue-meeting-status-accepted')).toContainText('Meeting accepted')
    await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.conversation())
    await expect(page.getByText('On my way.', { exact: true })).toBeVisible()
    assert.equal((await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.state())).writes, 1)
    const conversation = await capture(page, `conversation-${width}`, page.locator('.venue-meeting-conversation'))
    await page.getByRole('button', { name: 'View on map', exact: true }).click()
    await expect.poll(() => page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.state().focuses)).toBe(1)
    await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.conversation())
    await expect(page.getByText('On my way.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Focus meeting place on map', exact: true }).click()
    await expect.poll(() => page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.state().focuses)).toBe(2)
    await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.conversation())
    await expect(page.getByText('On my way.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Message', exact: true }).click()
    await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toBeFocused()
    await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Private fixture draft')
    await page.getByRole('button', { name: 'Uncertain next action', exact: true }).click()
    await page.getByRole('button', { name: 'Send message', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Check original action' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Close', exact: true })).toBeDisabled()
    const uncertain = await capture(page, `uncertain-${width}`, page.locator('.venue-meeting'))
    await page.getByRole('button', { name: 'Check original action' }).focus()
    await page.keyboard.press('Enter')
    await expect(page.getByText('Private fixture draft', { exact: true })).toBeVisible()
    const commands = await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.state())
    assert.equal(commands.writes, 2); assert.equal(commands.statuses, 1)
    for (const button of await page.locator('.venue-meeting button').all()) {
      const box = await button.boundingBox(); assert(box && box.width >= 44 && box.height >= 44)
    }
    let shortHeight = null
    if (width === 390) {
      await page.setViewportSize({ width, height: 520 })
      await page.getByRole('textbox', { name: 'Message', exact: true }).focus()
      await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Short-height private draft')
      await page.getByRole('button', { name: 'Send message', exact: true }).scrollIntoViewIfNeeded()
      await expect(page.getByRole('button', { name: 'Send message', exact: true })).toBeInViewport()
      shortHeight = await capture(page, 'short-height-390', page.locator('.venue-meeting'))
    }
    await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.revoke())
    await expect(page.getByText(/unavailable until current participant/)).toBeVisible()
    await expect(page.getByText('Private fixture draft', { exact: true })).toHaveCount(0)
    await expect(page.getByText('inkparade', { exact: true })).toHaveCount(0)
    assert.deepEqual((await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.state())).subscriptions, { clock: 0, invalidation: 0 })
    await page.evaluate(() => window.__VENUE_MEETING_FIXTURE__.dispose())
    report.widths.push({ width, result: 'PASS', invitation, conversation, uncertain, shortHeight,
      mapFocus: 'Controlled sole Map focus port; no real position, GPS or production shell', minimumTouchTarget: 44, focus: 'MESSAGE focuses composer; status recovery works by keyboard',
      keyboardLimit: 'Desktop headless viewport resize, not a physical mobile software keyboard' })
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { name: error.name, message: error.message }; process.exitCode = 1 }
finally { await browser?.close(); await server.close(); await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify({ result: report.result, failure: report.failure, widths: report.widths.length }))
