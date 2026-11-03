import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const evidence = process.env.MCR_DELETION_TARGETS_EVIDENCE ?? '/tmp/mcr-fe36-deletion-targets-evidence'
await mkdir(evidence, { recursive: true })
const results = [], errors = [], logoutRequests = []
let browser
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{
  name: 'synthetic-deletion-canonical-logout-only',
  configureServer(server) { server.middlewares.use((req, res, next) => {
    if (!req.url?.startsWith('/api/')) return next()
    if (req.url !== '/api/auth/logout' || req.method !== 'POST') throw new Error('Unexpected fixture HTTP route ' + req.url)
    logoutRequests.push({ path: req.url, method: req.method })
    res.setHeader('Cache-Control', 'no-store'); res.statusCode = 204; res.end()
  }) },
}] })
try {
  await dev.listen()
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const url = `http://127.0.0.1:${dev.httpServer.address().port}/src/features/account/__tests__/deletion-preview.html`
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 900 }, reducedMotion: 'reduce' })
    const page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    const open = async (query = '') => { await page.goto(url + query); await expect(page.getByRole('heading', { name: 'Account controls' })).toBeVisible() }
    const snapshot = () => page.evaluate(() => window.__DELETION_TARGETS__.snapshot())
    const requestDeletion = async () => {
      await page.getByRole('button', { name: 'Request account deletion' }).click()
      await expect(page.getByRole('button', { name: 'Keep account' })).toBeFocused()
      await expect(page.getByRole('button', { name: 'Confirm deletion request' })).toBeDisabled()
      await page.getByLabel('Deletion confirmation').fill('DELETE_MY_ACCOUNT')
      await page.getByRole('button', { name: 'Confirm deletion request' }).click()
    }
    const check = () => page.getByRole('button', { name: 'Check deletion progress' }).click()
    const status = text => page.getByRole('status').filter({ hasText: text })
    await open('?unavailable')
    await expect(page.getByRole('button', { name: 'Request account deletion' })).toBeDisabled()
    assert.equal((await snapshot()).requests, 0)

    await open()
    await page.getByRole('button', { name: 'Request account deletion' }).click()
    await expect(page.getByRole('button', { name: 'Keep account' })).toBeFocused()
    await page.keyboard.press('Shift+Tab')
    assert(await page.evaluate(() => document.querySelector('dialog').contains(document.activeElement)))
    await page.keyboard.press('Escape')
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect(page.getByRole('button', { name: 'Request account deletion' })).toBeFocused()
    await requestDeletion()
    await expect(status('Deletion is pending')).toBeVisible()
    await expect(page.getByRole('list', { name: 'Deletion progress' }).getByRole('listitem')).toHaveCount(8)
    for (const label of ['Profile', 'Friends', 'Posts', 'Notifications', 'Concert tickets', 'Concert sessions', 'Concert sharing', 'Concert private data']) await expect(page.getByText(label + ': pending', { exact: true })).toBeVisible()
    let state = await snapshot()
    assert.equal(state.gate, 'suspended'); assert(state.privateCleared); assert.equal(state.requests, 1); assert.equal(state.reads, 0); assert.equal(state.completed, 0); assert.equal(state.restricted, 1)
    await expect(page.getByRole('button', { name: 'Request account deletion' })).toBeDisabled()
    await page.screenshot({ path: `${evidence}/pending-${width}.png`, fullPage: true })
    await page.evaluate(() => window.__DELETION_TARGETS__.mode('retry')); await check()
    await expect(page.getByText('Concert private data: retry', { exact: true })).toBeVisible()
    await page.evaluate(() => window.__DELETION_TARGETS__.mode('failure')); await check()
    await expect(status('Deletion needs recovery')).toBeVisible()
    await expect(page.getByText('Concert private data: dead lettered', { exact: true })).toBeVisible()
    assert(!(await page.locator('body').innerText()).includes('PRIVATE provider'))
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    await page.screenshot({ path: `${evidence}/partial-${width}.png`, fullPage: true })
    await page.evaluate(() => window.__DELETION_TARGETS__.mode('complete')); await check()
    await expect(status('Account deletion confirmed')).toBeVisible()
    state = await snapshot(); assert.equal(state.requests, 1); assert.equal(state.reads, 3); assert.equal(state.completed, 1); assert.equal(state.userId, null)

    for (const mode of ['legacy', 'duplicate', 'unknown', 'uncertain']) {
      await open('?mode=' + mode); await requestDeletion()
      await expect(status('Deletion is not confirmed')).toBeVisible()
      state = await snapshot(); assert.equal(state.completed, 0); assert.equal(state.requests, 1); assert.equal(state.reads, 0); assert(state.privateCleared); assert.equal(state.gate, 'suspended')
      await expect(page.getByRole('list', { name: 'Deletion progress' })).toHaveCount(0)
      assert(!(await page.locator('body').innerText()).includes('PRIVATE uncertain'))
      await page.evaluate(() => window.__DELETION_TARGETS__.mode('complete')); await check()
      await expect(status('Account deletion confirmed')).toBeVisible()
      state = await snapshot(); assert.equal(state.requests, 1); assert.equal(state.reads, 1); assert.equal(state.completed, 1)
    }
    await open('?mode=slow'); await requestDeletion()
    await page.evaluate(() => { window.__DELETION_TARGETS__.changeIdentity(); window.__DELETION_TARGETS__.mode('complete'); window.__DELETION_TARGETS__.release() })
    await expect(status('Private account views were cleared')).toBeVisible()
    state = await snapshot(); assert.equal(state.completed, 0); assert.equal(state.userId, '33333333-3333-4333-8333-333333333333'); assert.equal(state.progress, null)
    results.push({ width, eightStepPresentation: true, nativeModalFocusEscapeRestore: true, unavailable: true, pendingRetryDeadLetter: true,
      privateDiagnosticHidden: true, canonicalPrivacyScrub: true, completionLogout: true, legacyDuplicateUnknownRejected: true,
      uncertainDeleteManualGetOnly: true, requestExactlyOnce: true, lateIdentityFenced: true, horizontalOverflow: false })
    await context.close()
  }
  assert.deepEqual(errors, [])
  const paths = ['src/features/account/contract.ts', 'src/features/account/controller.ts', 'src/features/account/panel.tsx',
    'src/features/account/__tests__/deletion-fixtures.ts', 'src/features/account/__tests__/deletion-preview.html',
    'src/features/account/__tests__/deletion-preview.tsx', 'src/features/account/__tests__/deletion-browser-smoke.mjs']
  const hashes = Object.fromEntries(await Promise.all(paths.map(async path => [path, createHash('sha256').update(await readFile(path)).digest('hex')])))
  await writeFile(`${evidence}/result.json`, JSON.stringify({ status: 'PASS', synthetic: true, liveAuthPort: false, realCleanup: 'NOT_RUN', rootIntegrated: false,
    results, errors, logoutRequests, hashes }, null, 2) + '\n')
  console.log(JSON.stringify({ status: 'PASS', evidence, viewports: results.length, canonicalLogoutRequests: logoutRequests.length, errors }))
} finally { await browser?.close(); await dev.close() }
