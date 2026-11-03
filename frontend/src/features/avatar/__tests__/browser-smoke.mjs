// Actual native Chrome, canonical Auth/shared HTTP, actual shared C06 compositor.
// HTTP responses and geometric atlas are explicitly test-only fixtures, never product art.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { syntheticAvatarPng } from '../../../world/avatar/__tests__/fixtures/png.mjs'
const root = fileURLToPath(new URL('../../../../', import.meta.url))
const evidence = resolve(root, 'src/features/avatar/__tests__/evidence/native-01')
await mkdir(evidence, { recursive: true })
const fixture = async name => JSON.parse(await readFile(resolve(root, `../contracts/fixtures/core/${name}.json`)))
const auth = await fixture('auth-success'), participant = await fixture('private-participant'), conflict = await fixture('error-version-conflict'), caps = await fixture('auth-capabilities-disabled')
caps.providers[0] = { ...caps.providers[0], enabled: true, disabled_reason: null }
let mode = 'ok', saved = structuredClone(participant.profile.avatar), browser
const errors = [], requests = [], assetRequests = [], results = [], png = syntheticAvatarPng()
const respond = (res, status, body) => { res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(body === undefined ? '' : JSON.stringify(body)) }
const dev = await createServer({ root, logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'fe13-test-http', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (req.url === '/world/fe13-test/avatar.png') {
      assetRequests.push({ authorization: req.headers.authorization, cookie: req.headers.cookie, referer: req.headers.referer })
      res.writeHead(200, { 'Content-Type': 'image/png', 'Cache-Control': 'no-store' }); res.end(png); return
    }
    if (!req.url?.startsWith('/api/')) return next()
    try {
      if (req.url === '/api/auth/capabilities') return respond(res, 200, caps)
      if (req.url === '/api/auth/login') return respond(res, 200, auth)
      if (req.url === '/api/auth/logout') return respond(res, 204)
      assert.equal(req.headers.authorization, `Bearer ${auth.accessToken}`)
      if (req.url.endsWith('/me') && req.method === 'GET') {
        requests.push({ method: 'GET', revision: saved.revision })
        return respond(res, 200, { ...participant, profile: { ...participant.profile, avatar: saved } })
      }
      if (req.url.endsWith('/me/avatar') && req.method === 'PUT') {
        assert.equal(req.headers['x-mcr-intent'], 'browser'); assert.equal(req.headers['idempotency-key'], undefined)
        const chunks = []; for await (const chunk of req) chunks.push(chunk)
        const body = JSON.parse(Buffer.concat(chunks).toString()); requests.push({ method: 'PUT', body })
        if (mode === 'conflict' || body.expected_revision !== saved.revision) return respond(res, 409, conflict)
        if (mode === 'denied') return respond(res, 403, { code: 'GATEWAY_403', message: 'private server details', details: { reason: 'POLICY_DENIED' } })
        saved = { revision: saved.revision + 1, config: body.config }
        if (mode === 'uncertain') return respond(res, 503, { code: 'GATEWAY_503', message: 'Fixture upstream failed after commit', details: { reason: 'DEPENDENCY_UNAVAILABLE' } })
        return respond(res, 200, saved)
      }
      return respond(res, 404, {})
    } catch (error) { errors.push(String(error)); if (!res.destroyed) respond(res, 500, {}) }
  })
} }] })
const snapshot = page => page.evaluate(() => window.__FE13_TEST_ONLY__.state())
const puts = () => requests.filter(r => r.method === 'PUT').length
const ready = async page => { await expect(page.getByRole('img', { name: 'Draft outfit preview' })).toBeVisible(); assert(await page.evaluate(() => window.__FE13_TEST_ONLY__.sharedPixels())) }
try {
  await dev.listen(); const origin = `http://127.0.0.1:${dev.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    saved = structuredClone(participant.profile.avatar); mode = 'ok'; const initialWrites = puts()
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(`${origin}/src/features/avatar/__tests__/fixtures/preview.html`); await ready(page)
    await expect(page.getByRole('button', { name: 'fixture unsupported' })).toBeDisabled()
    assert.deepEqual(await page.evaluate(() => window.__FE13_TEST_ONLY__.pixels()), [180, 70, 210, 255])
    // Keyboard-activate an actual preset button; native selects exercise the same draft path below.
    const accessory = page.getByLabel('accessory', { exact: true })
    await page.getByRole('button', { name: 'fixture no accessory', exact: true }).focus(); await page.keyboard.press('Space')
    await expect(accessory).toHaveValue(''); await ready(page)
    assert.notDeepEqual(await page.evaluate(() => window.__FE13_TEST_ONLY__.pixels()), [180, 70, 210, 255])
    await page.getByRole('button', { name: 'Cancel changes' }).click(); await expect(accessory).toHaveValue('accessory_fixture'); await ready(page)
    assert.equal(puts(), initialWrites); assert.deepEqual(await page.evaluate(() => window.__FE13_TEST_ONLY__.pixels()), [180, 70, 210, 255])
    await page.getByRole('button', { name: 'fixture no accessory', exact: true }).click(); await ready(page)
    await page.evaluate(() => window.__FE13_TEST_ONLY__.panel(false)); await expect.poll(async () => (await snapshot(page)).editor.phase).toBe('idle')
    assert.equal((await snapshot(page)).canvases, 0)
    await page.evaluate(() => window.__FE13_TEST_ONLY__.panel(true)); await page.getByRole('button', { name: 'Keep my draft' }).click()
    await ready(page); await expect(accessory).toHaveValue('')
    await page.getByRole('button', { name: 'Save outfit' }).click(); await expect(page.getByRole('status')).toHaveText('Outfit saved.')
    assert.equal(saved.revision, 2); assert.equal(saved.config.item_ids.accessory, null)
    assert.equal((await snapshot(page)).commits.at(-1).config.item_ids.accessory, null)
    // Failed save leaves draft, conflict never auto-replays and explicit read/rebase chooses next revision.
    await page.getByRole('button', { name: 'preset fixture', exact: true }).click(); mode = 'denied'
    await page.getByRole('button', { name: 'Save outfit' }).click(); await expect(page.getByRole('status')).toContainText('Your draft is still here')
    await expect(accessory).toHaveValue('accessory_fixture'); mode = 'conflict'
    await page.getByRole('button', { name: 'Save outfit' }).click(); await expect(page.getByRole('button', { name: 'Check latest saved outfit' })).toBeVisible()
    const afterConflict = puts(); await expect(page.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
    mode = 'ok'; saved = { revision: 7, config: saved.config }
    await page.getByRole('button', { name: 'Check latest saved outfit' }).click(); await expect(page.getByRole('heading', { name: 'Saved outfit · revision 7' })).toBeVisible()
    await expect(accessory).toHaveValue('accessory_fixture'); await page.getByRole('button', { name: 'Keep my draft' }).click()
    assert.equal(puts(), afterConflict)
    await page.getByRole('button', { name: 'Save outfit' }).click(); await expect(page.getByRole('status')).toHaveText('Outfit saved.')
    assert.equal(requests.filter(r => r.method === 'PUT').at(-1).body.expected_revision, 7)
    // Simulated upstream503 after server commit must reconcile before another write.
    await accessory.selectOption(''); mode = 'uncertain'; await page.getByRole('button', { name: 'Save outfit' }).click()
    await expect(page.getByRole('status')).toContainText('The save result is unknown')
    const uncertainWrites = puts(); await expect(page.getByRole('button', { name: 'Save outfit' })).toBeDisabled(); mode = 'ok'
    await page.getByRole('button', { name: 'Check latest saved outfit' }).click(); await page.getByRole('button', { name: 'Use saved outfit' }).click()
    assert.equal(puts(), uncertainWrites); await expect(page.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
    // Actual selection churn: aborted thumbnail pixels cannot reappear after cancel.
    await accessory.selectOption('accessory_fixture'); await page.getByRole('button', { name: 'Save outfit' }).click(); await expect(page.getByRole('status')).toHaveText('Outfit saved.')
    await page.evaluate(() => window.__FE13_TEST_ONLY__.selectionChurn(true)); await accessory.selectOption('')
    await page.getByRole('button', { name: 'Cancel changes' }).click(); await ready(page)
    await page.waitForTimeout(400); await expect(accessory).toHaveValue('accessory_fixture'); await ready(page)
    assert.deepEqual(await page.evaluate(() => window.__FE13_TEST_ONLY__.pixels()), [180, 70, 210, 255])
    assert.equal((await snapshot(page)).canvases, 1)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal clipping')
    for (const element of await page.locator('button,select').all()) { const box = await element.boundingBox(); if (box) assert(box.height >= 44, 'touch control below44px') }
    await page.getByRole('button', { name: 'Cancel changes' }).focus(); await page.keyboard.press('Shift+Tab'); await page.keyboard.press('Tab')
    assert(await page.getByRole('button', { name: 'Cancel changes' }).evaluate(el => getComputedStyle(el).outlineStyle !== 'none'))
    await page.screenshot({ path: `${evidence}/avatar-${width}.png`, fullPage: true })
    // Product entry must refuse synthetic manifest/binding rather than show these fixture pixels.
    await page.evaluate(() => window.__FE13_TEST_ONLY__.production())
    await expect(page.getByRole('status')).toContainText('Reviewed avatar artwork is unavailable')
    assert.equal((await snapshot(page)).canvases, 0); assert((await snapshot(page)).caches.every(cache => cache.disposed && cache.entries === 0 && cache.references === 0))
    await page.evaluate(() => window.__FE13_TEST_ONLY__.suspend()); await expect(page.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
    await page.evaluate(() => window.__FE13_TEST_ONLY__.dispose()); assert.equal(await page.locator('canvas').count(), 0)
    results.push({ width, canonicalAuthHttp: 'PASS', sharedCompositorRgba: 'PASS', keyboardTouchAndClipping: 'PASS', draftSaveCancelAndPanelResume: 'PASS', conflictAndUncertainReconciliation: 'PASS', churnAndTeardown: 'PASS', productEntryUnavailable: 'PASS' })
    await context.close()
  }
  assert(assetRequests.every(r => !r.authorization && !r.cookie && !r.referer), 'asset credential leak')
  assert.deepEqual(errors, [])
  const paths = execFileSync('rg', ['--files', 'src/features/avatar', 'src/entities/avatar/runtime'], { cwd: root, encoding: 'utf8' }).trim().split('\n').filter(path => !path.includes('/evidence/')).sort()
  const hashes = Object.fromEntries(await Promise.all(paths.map(async path => [path, createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')])))
  const report = { status: 'MOCK_READY', browser: await browser.version(), node: process.version, revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
    dependency: 'c2149dae63d21324179be3dca7df4bc79a003881 (local duplicate c321279)', results, pageErrors: errors, assetRequests: assetRequests.length, sourceHashes: hashes,
    limits: ['Synthetic geometric atlas/test HTTP only; no production review receipt or character art.', 'Callback observes shared verified saved state; real world/profile root wiring and two-account propagation NOT_RUN.', 'Actual iOS/Android, backend persistence, AR03 and IN01 NOT_RUN.'] }
  await writeFile(`${evidence}/report.json`, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify({ ...report, sourceHashes: `${paths.length} files pinned` }, null, 2))
} finally { await browser?.close(); await dev.close() }
