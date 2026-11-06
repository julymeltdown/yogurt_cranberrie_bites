/** Execute only inside a separately accepted finite browser lease and dual guards. */
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'

assert(process.env.MCR_VENUE_BROWSER_ACCEPTED, 'A real finite browser offer is required')
const front = resolve(import.meta.dirname, '../..'), root = resolve(front, '..')
const output = process.env.MCR_VENUE_BROWSER_OUTPUT ?? resolve(root, 'docs/mcr/evidence/task-26-invitations')
const executablePath = process.env.MCR_VENUE_BROWSER_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
await mkdir(output, { recursive: true })
const report = { result: 'FAIL', classification: 'CONTROLLED_COMPONENT_FIXTURE',
  limitation: 'Real newly authored invitation list component with normalized adapter fixture. Not production shell/HTTP, backend, two-principal, device GPS or product acceptance.',
  sourceCommit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  fixtureClockMs: 1_800_000_000_000, widths: [], errors: [] }
const server = await createServer({ configFile: false, root: front, logLevel: 'error',
  server: { host: '127.0.0.1', port: 0, strictPort: false, watch: null, ws: false } })
let browser
async function capture(page, name, locator) {
  const path = resolve(output, name + '.png')
  await locator.screenshot({ path })
  return { path, capturedAt: new Date().toISOString(), viewport: page.viewportSize(),
    dpr: await page.evaluate(() => devicePixelRatio), componentBounds: await locator.boundingBox() }
}
async function open(page, port) {
  await page.goto(`http://127.0.0.1:${port}/e2e/venue-community/inbox-preview.html?run=${Date.now()}`)
  await expect(page.getByRole('heading', { name: 'Invitations', exact: true })).toBeVisible()
  await expect(page.getByRole('tab', { name: 'Incoming', exact: true })).toBeVisible()
  await expect.poll(() => page.evaluate(() => window.__VENUE_INBOX_FIXTURE__.state().inboxReads)).toBeGreaterThan(0)
}
try {
  await server.listen()
  const address = server.httpServer.address(); assert(address && typeof address !== 'string')
  browser = await chromium.launch({ executablePath, headless: true }); report.browserVersion = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, hasTouch: true, reducedMotion: 'reduce' })
    const page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    const port = address.port
    await open(page, port)
    assert.equal((await page.evaluate(() => window.__VENUE_INBOX_FIXTURE__.state())).writes, 0)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    const list = await capture(page, `list-${width}`, page.locator('.venue-invitation-inbox'))
    await page.locator('[data-invitation-id="00000000-0000-4000-8000-000000000005"]').getByRole('button', { name: 'Accept', exact: true }).click()
    await expect(page.locator('[data-invitation-id="00000000-0000-4000-8000-000000000005"]')).toHaveAttribute('data-display-status', 'accepted')
    const accept = await capture(page, `accept-${width}`, page.locator('.venue-invitation-inbox'))
    await open(page, port)
    await page.locator('[data-invitation-id="00000000-0000-4000-8000-000000000006"]').getByRole('button', { name: 'Decline', exact: true }).click()
    await expect(page.locator('[data-invitation-id="00000000-0000-4000-8000-000000000006"]')).toHaveAttribute('data-display-status', 'declined')
    const decline = await capture(page, `decline-${width}`, page.locator('.venue-invitation-inbox'))
    await open(page, port)
    await expect(page.locator('[data-server-status="expired"]')).toBeVisible()
    const expired = await capture(page, `expired-${width}`, page.locator('[data-server-status="expired"]'))
    await open(page, port)
    await page.evaluate(() => window.__VENUE_INBOX_FIXTURE__.setFull())
    await expect(page.getByText('8 of 8 party places · Full')).toBeVisible()
    const full = await capture(page, `full-${width}`, page.locator('.venue-invitation-inbox'))
    await open(page, port)
    await page.evaluate(() => window.__VENUE_INBOX_FIXTURE__.deny())
    await expect(page.getByText('The server denied this action. Current permissions and invitation status were refreshed.')).toBeVisible()
    const denied = await capture(page, `denied-${width}`, page.locator('.venue-invitation-inbox'))
    for (const control of await page.locator('.venue-invitation-inbox button, .venue-invitation-inbox a').all()) {
      const box = await control.boundingBox(); assert(box && box.width >= 44 && box.height >= 44)
    }
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    const state = await page.evaluate(() => window.__VENUE_INBOX_FIXTURE__.state())
    assert.equal(state.writes, 1); assert.equal(state.notice, 'denied')
    await page.evaluate(() => window.__VENUE_INBOX_FIXTURE__.dispose())
    report.widths.push({ width, list, accept, decline, expired, full, denied, openWrites: 0,
      acceptWrites: 1, declineWrites: 1, expiredWrites: 0, fullWrites: 0, deniedWrites: 1,
      deniedNotice: 'denied', overflow: 'PASS', minimumTouchTarget: 44 })
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { name: error.name, message: error.message }; process.exitCode = 1 }
finally { await browser?.close(); await server.close(); await writeFile(resolve(output, 'inbox-native.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify({ result: report.result, failure: report.failure, widths: report.widths.length }))
