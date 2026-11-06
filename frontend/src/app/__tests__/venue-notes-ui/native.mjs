import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const frontend = resolve(import.meta.dirname, '../../../..'), evidence = resolve(import.meta.dirname, '../../../../../.omo/evidence/mcr-finish-project/task-27-notes')
await mkdir(evidence, { recursive: true })
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: '/tmp/mcr-venue-notes-ui-cache',
  server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
await server.listen()
const report = { result: 'FAIL', qualification: 'SUBSTITUTED_FIXTURE',
  note: 'Actual VenueShellFeatures production sources, siteplan loader and Map/Notes panel. Deterministic session, community clock, Location, landmark and Note producer rows; no live backend, device or physical venue.',
  cases: [], errors: [] }
let browser, page, stage = 'start'
const origin = () => `http://127.0.0.1:${server.httpServer.address().port}`
function track(next) {
  next.on('pageerror', error => report.errors.push(error.message))
  next.on('console', message => { if (message.type() === 'error') report.errors.push(`console: ${message.text()}`) })
  return next
}
async function ready(target) {
  await expect(target.getByRole('button', { name: 'Share location' })).toBeEnabled({ timeout: 15_000 })
}
async function openNotes(target, state = 'normal') {
  await target.goto(`${origin()}/src/app/__tests__/venue-notes-ui/fixture.html?state=${state}`)
  await ready(target)
  await target.getByRole('tab', { name: 'NOTES' }).click()
}
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[360, 800], [390, 844], [430, 932]]) {
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce' })
    page = track(await context.newPage())
    stage = `${width}/clusters`
    await openNotes(page)
    await expect(page.getByLabel('Places with notes').getByRole('button', { name: 'Main Gate, 2 notes' })).toBeVisible({ timeout: 10_000 })
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    await page.screenshot({ path: resolve(evidence, `clusters-${width}.png`), fullPage: true })

    stage = `${width}/selected`
    await page.getByLabel('Places with notes').getByRole('button', { name: 'Main Gate, 2 notes' }).click()
    await expect(page.getByRole('heading', { name: 'MAIN GATE / 2 NOTES' })).toBeVisible({ timeout: 10_000 })
    await expect(page.getByText('See you by the main gate after the show.')).toBeVisible()
    await page.screenshot({ path: resolve(evidence, `selected-${width}.png`), fullPage: true })

    stage = `${width}/reply`
    await page.getByRole('button', { name: 'READ REPLIES' }).first().click()
    await expect(page.getByText('Bring the light sticks.')).toBeVisible({ timeout: 10_000 })
    const card = page.locator('.pc-note-card').filter({ hasText: 'See you by the main gate after the show.' })
    await card.getByRole('button', { name: 'REPLY' }).click()
    await page.getByRole('textbox', { name: 'Reply text' }).fill('Reply from the notes client.')
    await page.getByRole('button', { name: 'POST REPLY' }).click()
    await expect(page.getByText('Reply from the notes client.')).toBeVisible({ timeout: 10_000 })
    await page.screenshot({ path: resolve(evidence, `reply-thread-${width}.png`), fullPage: true })

    stage = `${width}/meet`
    await page.locator('.pc-note-card').filter({ hasText: 'See you by the main gate after the show.' })
      .getByRole('button', { name: 'MEET HERE' }).click()
    await expect(page.getByRole('heading', { name: 'Meet at Main Gate?' })).toBeVisible({ timeout: 10_000 })
    await page.screenshot({ path: resolve(evidence, `meet-here-${width}.png`), fullPage: true })
    await page.getByRole('button', { name: 'Close' }).click()

    stage = `${width}/composer`
    await page.getByRole('button', { name: '+ NEW NOTE' }).click()
    await expect(page.getByRole('group', { name: 'Visible to' })).toBeVisible()
    await expect(page.getByRole('group', { name: 'Expires after' })).toBeVisible()
    await page.getByRole('textbox', { name: 'Note text' }).fill('Browser authored place note.')
    await page.getByRole('button', { name: 'EVERYONE' }).click()
    await page.getByRole('button', { name: 'END OF DAY' }).click()
    await page.screenshot({ path: resolve(evidence, `composer-${width}.png`), fullPage: true })
    await page.getByRole('button', { name: '+ POST NOTE' }).click()
    await expect(page.getByText('Browser authored place note.')).toBeVisible({ timeout: 10_000 })
    await page.screenshot({ path: resolve(evidence, `created-${width}.png`), fullPage: true })

    stage = `${width}/withdraw`
    const own = page.locator('.pc-note-card').filter({ hasText: 'This notice expires shortly.' })
    await own.getByRole('button', { name: 'WITHDRAW' }).click()
    await expect(page.getByText('This notice expires shortly.')).toHaveCount(0, { timeout: 10_000 })
    await page.screenshot({ path: resolve(evidence, `withdrawn-${width}.png`), fullPage: true })

    stage = `${width}/unreviewed`
    await page.getByRole('tab', { name: 'MAP' }).click()
    await page.getByRole('button', { name: 'Select Paradise City Station' }).click()
    await expect(page.getByText('Illustrated reference · real-place approval unavailable.')).toBeVisible()
    await expect(page.getByRole('button', { name: '+ LEAVE A NOTE' })).toBeDisabled()
    await page.screenshot({ path: resolve(evidence, `unreviewed-${width}.png`), fullPage: true })

    stage = `${width}/expired`
    await openNotes(page, 'expired')
    await expect(page.getByLabel('Places with notes').getByRole('button', { name: 'Main Gate, 2 notes' })).toBeVisible({ timeout: 10_000 })
    await page.getByLabel('Places with notes').getByRole('button', { name: 'Main Gate, 2 notes' }).click()
    await expect(page.getByText('This notice expires shortly.')).toBeVisible({ timeout: 10_000 })
    await page.evaluate(() => window.__VENUE_NOTES__.advance(3_000))
    await expect(page.getByText('This notice expires shortly.')).toHaveCount(0, { timeout: 10_000 })
    await page.screenshot({ path: resolve(evidence, `expired-${width}.png`), fullPage: true })

    stage = `${width}/denied`
    await openNotes(page, 'denied')
    await expect(page.getByText('Note counts are unavailable for this place.')).toBeVisible({ timeout: 10_000 })
    await expect(page.getByLabel('Places with notes').getByRole('button', { name: /Main Gate, .* note/ })).toHaveCount(0)
    await page.getByRole('combobox', { name: 'Read notes at place' }).selectOption('main-gate')
    await page.getByRole('button', { name: 'READ NOTES' }).click()
    await expect(page.getByText('Notes are unavailable for this place.')).toBeVisible({ timeout: 10_000 })
    await page.screenshot({ path: resolve(evidence, `denied-${width}.png`), fullPage: true })

    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    report.cases.push({ width, height, clusters: true, selected: true, composer: true, replyThread: true,
      meetHere: true, created: true, expired: true, denied: true, unreviewedDenied: true, withdrawn: true })
    await context.close()
  }
  assert.deepEqual(report.errors, [])
  report.result = 'PASS'
} catch (error) {
  report.errors.push(`${stage}: ${error instanceof Error ? error.message : String(error)}`)
  if (page) {
    report.body = await page.locator('body').innerText().catch(() => '')
    report.notesLog = await page.evaluate(() => window.__VENUE_NOTES__.log.slice(-20)).catch(() => [])
  }
} finally {
  await browser?.close()
  await server.close()
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2))
}
if (report.result !== 'PASS') { console.error(JSON.stringify(report, null, 2)); process.exit(1) }
console.log(JSON.stringify(report))
