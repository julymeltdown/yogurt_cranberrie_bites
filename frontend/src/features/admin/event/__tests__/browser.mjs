import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { relative, resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'

const frontend = resolve(import.meta.dirname, '../../../../..'), root = resolve(frontend, '..')
const evidence = resolve(import.meta.dirname, 'evidence/native'), cacheDir = resolve(import.meta.dirname, '.scratch-cache/browser')
await mkdir(evidence, { recursive: true })
const report = { status: 'FAIL', scope: 'New FE38 components/controller + canonical Auth/privacy/request with deterministic synthetic role/clock/HTTP/place-write ports. No production router, real operator, native device, publication, venue facts or closure acceptance.',
  baseCommit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), browser: '', sources: {}, contracts: {}, reference: {}, captures: {}, scenarios: [], errors: [] }
async function hash(file) { return createHash('sha256').update(await readFile(file)).digest('hex') }
async function collect(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (['evidence', '.scratch-cache'].includes(entry.name)) continue
    const file = resolve(dir, entry.name)
    if (entry.isDirectory()) await collect(file)
    else if (/\.(ts|tsx|mjs|css|html)$/.test(file)) report.sources[relative(root, file)] = await hash(file)
  }
}
for (const lane of ['event', 'places', 'audit']) await collect(resolve(frontend, `src/features/admin/${lane}`))
for (const file of ['contracts/domain/v1/admin.schema.json', 'contracts/domain/v1/feed.schema.json', 'contracts/domain/v1/common.schema.json', 'contracts/domain/v1/operations.json', 'contracts/domain/v1/policy.json', 'contracts/assets/v1/map-manifest.schema.json']) report.contracts[file] = await hash(resolve(root, file))
const reference = 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg'
report.reference[reference] = await hash(resolve(root, reference))
for (const file of ['src/shared/ui/concert-window/index.tsx', 'src/shared/ui/concert-window/concert-window.css', 'src/features/auth/model/session.ts', 'src/shared/api/http.ts', 'src/shared/query/privacy.ts', 'src/features/places/model/contract.ts']) report.sources[`frontend/${file}`] = await hash(resolve(frontend, file))
const server = await createServer({ configFile: false, plugins: [react()], root: frontend, cacheDir, logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  const base = `http://127.0.0.1:${server.httpServer.address().port}/src/features/admin/event/__tests__/fixtures/preview.html`
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => report.errors.push(e.message))
    const state = () => page.evaluate(() => window.__OPERATIONS__.state())
    async function open(surface, role = '') {
      await page.goto(`${base}?surface=${surface}&role=${role}`)
      await expect(page.getByRole('heading', { name: role ? 'Operations unavailable' : { event: 'Event settings', places: 'Venue and places', audit: 'Audit log', summary: 'Control room' }[surface], exact: true })).toBeVisible()
      await expect.poll(async () => (await state()).view.loading).toBe(false)
    }
    async function layout(name) {
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${name}: document overflow`)
      for (const control of await page.locator('button,input,select,textarea').all()) {
        if (!await control.isVisible()) continue
        const box = await control.boundingBox(); assert(box && box.height >= 44 && box.width >= 44, `${name}: control smaller than44px`)
      }
      if (width === 390) { const file = resolve(evidence, `${name}-390.png`); await page.screenshot({ path: file, fullPage: true }); report.captures[relative(root, file)] = await hash(file) }
    }
    for (const surface of ['event', 'places', 'audit']) {
      await open(surface, 'member'); assert.equal((await state()).reads, 0); assert.equal((await state()).writes, 0)
      await layout(`denied-${surface}`)
    }
    await open('event')
    await expect(page.getByText('DEGRADED', { exact: true })).toBeVisible()
    await page.getByLabel('Reason for settings or phase change').fill('SYNTHETIC reviewed flag change')
    await page.getByRole('checkbox', { name: 'mural', exact: true }).uncheck()
    const review = page.getByRole('button', { name: 'Review settings change', exact: true })
    await review.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('heading', { name: 'Confirm operational change' })).toBeFocused(); assert.equal((await state()).writes, 0)
    await layout('event-confirm')
    await page.getByRole('button', { name: 'Cancel confirmation' }).click(); await expect(review).toBeFocused(); assert.equal((await state()).writes, 0)
    await page.evaluate(() => window.__OPERATIONS__.mode('conflict'))
    await review.click(); await page.getByRole('button', { name: 'Confirm this change' }).click()
    await expect(page.getByText('The revision changed. Refresh and review your input before another action.')).toBeVisible()
    await page.getByRole('button', { name: 'Refresh event information' }).click()
    await expect(review).toBeEnabled(); await expect(page.getByLabel('Reason for settings or phase change')).toHaveValue('SYNTHETIC reviewed flag change')
    await page.evaluate(() => window.__OPERATIONS__.mode('pending'))
    await review.click(); await page.getByRole('button', { name: 'Confirm this change' }).click()
    await expect(page.getByRole('button', { name: 'Check command status' })).toBeEnabled(); assert.equal((await state()).view.command.stage, 'pending')
    await page.getByRole('button', { name: 'Refresh event information' }).click(); assert.equal((await state()).writes, 2)
    for (const surface of ['audit', 'places', 'event']) {
      await page.getByRole('navigation', { name: 'Fixture surfaces' }).getByRole('button', { name: surface, exact: true }).click()
      await expect.poll(async () => (await state()).view.loading).toBe(false)
      assert.equal((await state()).view.command.stage, 'pending'); assert.equal((await state()).writes, 2)
    }
    await page.evaluate(() => window.__OPERATIONS__.settle()); await page.getByRole('button', { name: 'Check command status' }).click()
    await expect.poll(async () => (await state()).view.command.stage).toBe('applied')
    await layout('event-applied')
    await open('event')
    await page.getByLabel('Notice title', { exact: true }).fill('SYNTHETIC published bulletin')
    await page.getByLabel('Notice text', { exact: true }).fill('SYNTHETIC safe public information.')
    await page.getByLabel('Reviewed HTTPS source', { exact: true }).fill('https://notice.example.invalid/review')
    await page.getByLabel('Source confirmed at (UTC)', { exact: true }).fill('2026-11-04T03:59:00Z')
    await page.getByLabel('Notice publication state').selectOption('PUBLISHED')
    await page.getByRole('button', { name: 'Review notice change' }).click(); await page.getByRole('button', { name: 'Confirm this change' }).click()
    await expect.poll(async () => (await state()).view.command.stage).toBe('applied')
    await page.getByRole('button', { name: 'Refresh event information' }).click()
    await expect(page.getByRole('button', { name: 'Review notice withdrawal' }).first()).toBeEnabled()
    await page.getByRole('button', { name: 'Review notice withdrawal' }).first().click(); await page.getByRole('button', { name: 'Confirm this change' }).click()
    await expect.poll(async () => (await state()).view.notices[0].state).toBe('WITHDRAWN')
    await layout('notice-withdrawn')
    await page.evaluate(() => window.__OPERATIONS__.revoke()); await expect(page.getByRole('heading', { name: 'Operations unavailable' })).toBeVisible()
    await expect(page.getByText('SYNTHETIC published bulletin')).toHaveCount(0)
    await open('places')
    await page.getByRole('button', { name: 'Review SYNTHETIC landmark' }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('heading', { name: 'Review place source' })).toBeFocused()
    await page.getByLabel('Closure state', { exact: true }).selectOption('CLOSED')
    await page.getByLabel('Reason for place change').fill('SYNTHETIC reviewed closure')
    await layout('place-review')
    await page.getByRole('button', { name: 'Review place change' }).click(); await expect(page.getByRole('heading', { name: 'Confirm operational change' })).toBeFocused()
    await page.getByRole('button', { name: 'Confirm this change' }).click()
    await expect.poll(async () => (await state()).view.command.stage).toBe('applied')
    assert.equal((await state()).view.places[0].place.closure_state, 'CLOSED')
    assert.equal((await state()).view.places[0].place.provenance.source_revision, 'SYNTHETIC release 1')
    await page.evaluate(() => window.__OPERATIONS__.expire()); await expect(page.getByRole('heading', { name: 'Operations unavailable' })).toBeVisible()
    await expect(page.getByLabel('Reason for place change')).toHaveCount(0)
    await open('audit')
    await page.getByRole('button', { name: 'View audit entry' }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('heading', { name: 'Audit entry details' })).toBeFocused()
    assert.equal(await page.locator('img,video,a[href*="private"],iframe').count(), 0)
    await layout('audit-detail')
    await page.evaluate(() => window.__OPERATIONS__.identity()); await expect(page.getByRole('heading', { name: 'Operations unavailable' })).toBeVisible()
    assert.equal((await state()).view.audit.length, 0); assert.equal((await state()).view.auditDetail, null)
    await page.evaluate(() => window.__OPERATIONS__.dispose())
    assert.deepEqual((await state()).subscriptions, { authority: 0, clock: 0, places: 0 })
    report.scenarios.push({ width, state: 'PASS', surfaces: ['O01/O02 event/settings/notices', 'O03 places', 'O08 audit'], strictMode: true,
      deniedReadsWrites: 0, touchMinimum: 44, keyboardFocus: 'PASS', overflow: false, cancel: 'PASS', conflictDraft: 'PASS', pendingNoReplay: 'PASS', noticePublishWithdraw: 'PASS', provenancePreserved: 'PASS', permissionExpiryIdentityCleanup: 'PASS' })
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.status = 'PASS'
} catch (error) { report.failure = { name: error.name, message: error.message }; process.exitCode = 1 }
finally { await browser?.close(); await server.close(); await rm(cacheDir, { recursive: true, force: true }); report.cleanup = 'PASS'; await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify({ status: report.status, scenarios: report.scenarios, errors: report.errors, failure: report.failure }))
