import assert from 'node:assert/strict'
import { createServer as createNetServer } from 'node:net'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(root, 'frontend')
const evidence = process.env.MCR_FE21_EVIDENCE
assert(evidence?.startsWith('/'), 'Set MCR_FE21_EVIDENCE to a new absolute directory')
await mkdir(evidence, { mode: 0o700 })
const hash = value => createHash('sha256').update(value).digest('hex'), hashes = {}
async function collect(dir) { for (const entry of await readdir(dir, { withFileTypes: true })) { const path = resolve(dir, entry.name); if (entry.isDirectory()) await collect(path); else if (/\.(ts|tsx|mjs|html|css|json)$/.test(path)) hashes[relative(root, path)] = hash(await readFile(path)) } }
await collect(resolve(frontend, 'src'))
const report = { scope: 'FE21 deterministic C04 consumer fixtures through actual canonical Auth/FE02, FE07 and Phaser/FE14/FE15; no real backend/WebSocket/multiuser acceptance',
  result: 'FAIL', revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourceHashes: hashes,
  contracts: Object.fromEntries(await Promise.all(['contracts/generated/mcr-domain.openapi.json', 'contracts/domain/v1/operations.json', 'contracts/domain/v1/party.schema.json', 'contracts/domain/v1/common.schema.json'].map(async p => [p, hash(await readFile(resolve(root, p)))]))),
  widths: [], checks: [], cleanup: 'NOT_RUN' }
const reservation = createNetServer(); await new Promise(done => reservation.listen(0, '127.0.0.1', done)); const port = reservation.address().port; await new Promise(done => reservation.close(done))
const server = await createServer({ root: frontend, logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
let browser, stage = 'start'
try {
  await server.listen(); browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const [width, height] of [[390, 844], [1280, 900]]) {
    stage = `start-${width}`
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce', hasTouch: width < 600 }), page = await context.newPage(), errors = []
    page.on('pageerror', error => errors.push(error.message))
    const state = () => page.evaluate(() => window.__FE21__.state())
    const mode = value => page.evaluate(value => window.__FE21__.mode(value), value)
    const shot = async (name, keepScroll = false) => { if (!keepScroll) await page.locator('.shell-panel-body').evaluate(el => { el.scrollTop = 0 }); await page.screenshot({ path: resolve(evidence, `${width}-${name}.png`), fullPage: true }) }
    const apply = () => page.getByRole('button', { name: 'Apply to party', exact: true })
    const main = () => page.getByRole('button', { name: 'Parties', exact: true })
    const goPlan = async () => { await page.getByRole('link', { name: 'MY PLAN', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Your applications' })).toBeVisible() }
    const openParty = async () => { await main().click(); await page.getByRole('link', { name: 'Before the encore', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Before the encore', exact: true })).toBeVisible() }
    await page.goto(`http://127.0.0.1:${port}/src/features/party-discovery/__tests__/fixtures/preview.html`)
    await expect.poll(async () => (await state()).world?.phase).toBe('ready'); await expect(page.getByRole('link', { name: 'Before the encore', exact: true })).toBeVisible()
    const expand = page.getByRole('button', { name: 'Expand panel' }); if (await expand.isVisible()) await expand.click()
    assert.equal((await state()).mounts, 1); assert.equal((await state()).canvases, 1); assert.equal((await state()).connections, 1)
    await shot('discovery')
    stage = `filters-${width}`
    await page.getByText('Filter parties', { exact: true }).click(); await page.getByRole('textbox', { name: 'Ticket section' }).fill('No match'); await page.getByRole('button', { name: 'Show parties' }).click(); await expect(page.getByText(/No parties match this page/)).toBeVisible()
    await page.getByRole('textbox', { name: 'Ticket section' }).fill(''); await page.getByRole('button', { name: 'Show parties' }).click(); await expect(page.getByRole('link', { name: 'Before the encore', exact: true })).toBeVisible()
    const title = page.getByRole('link', { name: 'Before the encore', exact: true }); await title.focus(); await page.keyboard.press('Enter')
    await expect(page.locator('#panel-title')).toBeFocused(); await expect(apply()).toBeEnabled(); await shot('detail'); await apply().scrollIntoViewIfNeeded(); await shot('detail-apply', true)
    const foot = (await state()).foot; await page.keyboard.press('ArrowRight'); assert.deepEqual((await state()).foot, foot, 'blocking panel moved avatar')
    stage = `partial-${width}`
    await mode('partial'); await expect(apply()).toBeDisabled(); await expect(page.getByText(/Load all remaining MY PLAN pages/)).toBeVisible(); await goPlan()
    await page.getByRole('button', { name: 'Load more owned parties' }).click(); await expect(page.getByRole('button', { name: 'Load more owned parties' })).toHaveCount(0)
    await expect(page.getByRole('button', { name: 'Load more memberships' })).toBeVisible(); await page.getByRole('button', { name: 'Load more memberships' }).click()
    await page.getByRole('button', { name: 'Load more applications' }).click(); await expect(page.getByRole('button', { name: 'Load more applications' })).toHaveCount(0)
    await openParty(); await expect(apply()).toBeEnabled()
    // Server fixture now returns complete initial pages for later current-state refreshes.
    await mode('ok'); await expect(apply()).toBeEnabled()
    stage = `apply-${width}`
    await page.getByRole('textbox', { name: /Note to the host/ }).fill('Looking forward to the encore.')
    await apply().evaluate(button => { button.click(); button.click() })
    await expect.poll(async () => (await state()).party.command.phase).toBe('succeeded'); assert.equal((await state()).requests.filter(r => r.method === 'POST').length, 1)
    await goPlan(); await expect(page.getByRole('button', { name: 'Cancel application' })).toBeEnabled()
    await page.evaluate(() => window.__FE21__.readOnly()); await page.getByRole('button', { name: 'Cancel application' }).click()
    await expect.poll(async () => (await state()).party.command.currentState).toBe('CANCELLED'); await expect(page.getByRole('button', { name: 'Cancel application' })).toHaveCount(0); await shot('my-plan-cancelled')
    const deletion = (await state()).requests.find(r => r.method === 'DELETE'); assert(deletion.query.includes('expected_revision=1')); assert.equal(deletion.body, '')
    stage = `pending-${width}`
    await page.evaluate(() => window.__FE21__.open()); await mode('pending'); await openParty(); await expect(apply()).toBeEnabled(); await apply().click()
    await expect.poll(async () => (await state()).party.command.phase).toBe('pending'); await shot('pending')
    const pendingPosts = (await state()).requests.filter(r => r.method === 'POST').length
    await page.getByRole('button', { name: 'Check original command status' }).click(); await expect.poll(async () => (await state()).party.command.phase).toBe('pending')
    assert.equal((await state()).requests.filter(r => r.method === 'POST').length, pendingPosts)
    await page.evaluate(() => window.__FE21__.status('SUCCEEDED')); await page.getByRole('button', { name: 'Check original command status' }).click(); await expect.poll(async () => (await state()).party.command.phase).toBe('succeeded')
    await mode('ok'); await goPlan(); await page.getByRole('button', { name: 'Cancel application' }).click(); await expect.poll(async () => (await state()).party.command.currentState).toBe('CANCELLED')
    stage = `lost-${width}`
    // Restore a fresh synthetic OPEN capability release without restamping a revoked scope.
    await page.evaluate(() => window.__FE21__.open()); await mode('lost'); await openParty(); await expect(apply()).toBeEnabled(); await apply().click()
    await expect.poll(async () => (await state()).party.command.phase).toBe('uncertain'); const sent = (await state()).requests.filter(r => r.method === 'POST').length
    await page.evaluate(() => window.__FE21__.status('404')); await page.getByRole('button', { name: 'Check original command status' }).click(); await expect(page.getByText(/does not prove whether it ran/)).toBeVisible()
    await goPlan(); await page.evaluate(() => window.__FE21__.status('outage')); await page.getByRole('button', { name: 'Check original command status' }).click(); await expect.poll(async () => (await state()).party.command.phase).toBe('unavailable'); await shot('uncertain')
    await page.evaluate(() => window.__FE21__.status('SUCCEEDED')); await page.getByRole('button', { name: 'Check original command status' }).click(); await expect.poll(async () => (await state()).party.command.phase).toBe('succeeded')
    assert.equal((await state()).requests.filter(r => r.method === 'POST').length, sent)
    const checks = (await state()).requests.filter(r => r.path.endsWith('/commands')).slice(-3); assert(checks.length >= 3); assert(new Set(checks.map(r => new URLSearchParams(r.query).get('idempotency_key'))).size === 1)
    stage = `states-${width}`
    await mode('full'); await openParty(); await expect(page.getByText('This party is full.', { exact: true })).toBeVisible(); await expect(apply()).toBeDisabled()
    await mode('cancelled'); await expect(page.getByText('This party is cancelled.', { exact: true })).toBeVisible(); await shot('cancelled-party')
    await mode('private'); await expect(page.getByText('Your current access does not allow this view.')).toBeVisible(); assert.equal(await page.getByRole('heading', { name: 'Before the encore', exact: true }).count(), 0); await shot('private')
    await mode('ok'); await expect(page.getByRole('heading', { name: 'Before the encore', exact: true })).toBeVisible()
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal page overflow')
    assert(await page.locator('.party-view').evaluate(el => el.scrollWidth <= el.clientWidth + 1), 'feature horizontal overflow')
    for (const element of await page.locator('.party-view button:visible,.party-view a:visible,.party-view input:not([type=checkbox]):visible,.party-view summary:visible').all()) {
      const box = await element.boundingBox(); assert(box && box.width >= 44 && box.height >= 44, 'feature touch target smaller than 44px')
    }
    assert.equal(await page.locator('.party-view img,.party-view canvas').count(), 0)
    const before = await state(); assert.equal(before.mounts, 1); assert.equal(before.disposals, 0); assert.equal(before.canvases, 1); assert.equal(before.peakConnections, 1)
    await page.goBack(); await expect(page.getByRole('link', { name: 'Before the encore', exact: true })).toBeFocused(); assert.equal((await state()).mounts, 1)
    stage = `privacy-${width}`
    await page.evaluate(() => window.__FE21__.revoke()); await expect.poll(async () => (await state()).party.activity.data).toBeNull(); assert.equal((await state()).party.command.phase, 'idle'); assert.equal((await state()).party.note, '')
    assert.equal(await page.locator('body').getByText('PRIVATE RAW SERVER MESSAGE MUST NEVER RENDER').count(), 0)
    await page.evaluate(() => window.__FE21__.dispose()); const after = await state(); assert.equal(after.canvases, 0); assert.equal(after.connections, 0); assert.equal(after.disposals, 1); assert.deepEqual(errors, [])
    report.widths.push({ width, height, checks: ['filters', 'keyboard detail', 'partial independent pages', 'single apply', 'READ_ONLY safety cancel', '202 PENDING explicit status and fresh reconciliation', 'lost/404/outage original-key recovery', 'fresh success reconciliation', 'full/cancelled/private', 'no overflow', '44px controls', 'Back navigation', 'single persistent Phaser/fixture connection', 'privacy cleanup', 'disposal'], pageErrors: errors }); await context.close()
  }
  report.result = 'PASS'; report.cleanup = 'PASS'; report.checks.push('All native browser assertions passed')
} catch (error) { report.failure = { stage, message: error instanceof Error ? error.message : 'unknown' }; throw error }
finally { await browser?.close(); await server.close(); await writeFile(resolve(evidence, 'report.json'), `${JSON.stringify(report, null, 2)}\n`); console.log(JSON.stringify({ result: report.result, stage, evidence, widths: report.widths, cleanup: report.cleanup })) }
