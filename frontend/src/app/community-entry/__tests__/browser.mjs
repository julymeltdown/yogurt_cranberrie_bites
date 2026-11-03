import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { relative, resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'

const frontend = resolve(import.meta.dirname, '../../../..'), root = resolve(frontend, '..'), evidence = resolve(import.meta.dirname, 'evidence/browser')
await mkdir(evidence, { recursive: true })
const report = { result: 'FAIL', scope: 'MOCK_READY scoped community adapters only. Native Chrome; synthetic same-origin HTTP/privacy fixtures. Production router, live backend, devices and IN02 NOT_RUN.',
  baseCommit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourceHashes: {}, widths: [], errors: [] }
async function collect(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.name === 'evidence') continue
    const file = resolve(dir, entry.name)
    if (entry.isDirectory()) await collect(file)
    else if (/\.(ts|tsx|mjs|css|html)$/.test(file)) report.sourceHashes[relative(root, file)] = createHash('sha256').update(await readFile(file)).digest('hex')
  }
}
for (const path of ['app/community-entry', 'features/party-authoring', 'features/party-management', 'features/giveaway-authoring', 'features/gift-pickup']) await collect(resolve(frontend, 'src', path))
const server = await createServer({ root: frontend, logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser
try {
  await server.listen(); const port = server.httpServer.address().port
  const url = `http://127.0.0.1:${port}/src/app/community-entry/__tests__/preview.html`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430, 1280]) {
    const context = await browser.newContext({ viewport: { width, height: 900 }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => report.errors.push(e.message))
    await page.goto(url)
    const title = page.getByLabel('Party title'); await title.fill('Retained adapter draft')
    await page.getByRole('button', { name: 'Save session draft' }).click()
    await page.getByRole('button', { name: 'giveaway', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Start a giveaway' })).toBeVisible()
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    await page.getByRole('button', { name: 'party', exact: true }).click(); await expect(title).toHaveValue('Retained adapter draft')
    await page.getByRole('button', { name: 'manageParty', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Applications', exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'rally', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Rendezvous' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'HERE', exact: true })).toBeDisabled()
    await page.getByRole('button', { name: 'reserve', exact: true }).click(); await expect(page.getByRole('button', { name: 'Confirm reservation' })).toBeEnabled()
    await page.evaluate(() => window.__COMMUNITY_ENTRY__.lost())
    await page.getByRole('button', { name: 'Confirm reservation' }).click(); await expect(page.getByText(/Outcome unknown/)).toBeVisible()
    assert.equal((await page.evaluate(() => window.__COMMUNITY_ENTRY__.state())).writes, 1)
    await page.getByRole('button', { name: 'pickup', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Finish the original pickup command' })).toBeVisible()
    await page.getByRole('button', { name: 'Check original command' }).click(); await expect(page.getByText(/still pending/)).toBeVisible()
    await page.getByRole('button', { name: 'reserve', exact: true }).click(); await expect(page.getByText(/still pending/)).toBeVisible()
    if (width === 390) await page.screenshot({ path: resolve(evidence, '390-pending.png'), fullPage: true })
    await page.goto(`${url}?start=reserve`)
    await expect(page.getByRole('button', { name: 'Check original command' })).toBeVisible()
    const restored = await page.evaluate(() => window.__COMMUNITY_ENTRY__.state())
    assert.equal(restored.writes, 0); assert(restored.journal); assert(!/token|code|instructions/.test(JSON.stringify(restored.journal)))
    await page.getByRole('button', { name: 'Check original command' }).click(); await expect(page.getByText(/still pending/)).toBeVisible()
    await page.evaluate(() => window.__COMMUNITY_ENTRY__.revoke()); await expect(page.getByRole('heading', { name: 'Community action unavailable' })).toBeVisible()
    assert.equal((await page.evaluate(() => window.__COMMUNITY_ENTRY__.state())).journal, null)
    assert.equal((await page.evaluate(() => window.__COMMUNITY_ENTRY__.state())).subscriptions.clock, 0)
    await page.goto(`${url}?owner=1&start=manage`)
    await page.getByRole('button', { name: 'Open pickup 1' }).click(); await expect(page.getByLabel('Recipient’s six-digit code')).toBeVisible()
    assert.equal((await page.evaluate(() => window.__COMMUNITY_ENTRY__.state())).writes, 0)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    if (width === 390) await page.screenshot({ path: resolve(evidence, '390-owner.png'), fullPage: true })
    await page.evaluate(() => window.__COMMUNITY_ENTRY__.clock()); await expect(page.getByRole('heading', { name: 'Community action unavailable' })).toBeVisible()
    await page.evaluate(() => window.__COMMUNITY_ENTRY__.dispose()); assert.equal(await page.locator('main').count(), 0)
    report.widths.push({ width, result: 'PASS', strictMode: true, draftNavigation: 'PASS', unknownRecovery: 'PASS', reloadNoReplay: 'PASS', ownerBoundPickup: 'PASS', withdrawal: 'PASS' }); await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (e) { report.failure = { name: e.name, message: e.message }; process.exitCode = 1 }
finally { await browser?.close(); await server.close(); report.cleanup = 'PASS'; await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify({ ...report, sourceHashes: `${Object.keys(report.sourceHashes).length} source files hashed in report.json` }))
