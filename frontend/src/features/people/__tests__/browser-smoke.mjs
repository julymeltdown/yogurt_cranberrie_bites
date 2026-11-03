// Native Chrome with canonical session/shared HTTP and explicit test-only server fixtures.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = fileURLToPath(new URL('../../../../', import.meta.url)), evidence = resolve(root, 'src/features/people/__tests__/evidence/native-01')
await mkdir(evidence, { recursive: true })
const fixture = async (family, name) => JSON.parse(await readFile(resolve(root, `../contracts/fixtures/${family}/${name}.json`)))
const auth = await fixture('core', 'auth-success'), caps = await fixture('core', 'auth-capabilities-disabled'), publicIntro = await fixture('domain', 'public-intro')
caps.providers[0] = { ...caps.providers[0], enabled: true, disabled_reason: null }
const initial = () => ({ intro_id: publicIntro.intro_id, event_id: publicIntro.event_id, owner_id: auth.userId, text: 'I enjoy meeting the community.', age_bounds: { minimum_age: 20, maximum_age_exclusive: 30 }, expires_at: null, created_at: '2026-11-04T08:00:00Z', state: 'DRAFT', revision: 1 })
let saved = initial(), uncertain = false, denied = false, command = null, browser
const errors = [], requests = [], results = []
const respond = (res, status, value) => { res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(value === undefined ? '' : JSON.stringify(value)) }
const failure = (res, status, reason) => respond(res, status, { code: 'GATEWAY', message: 'private detail must not appear', details: { reason } })
const dev = await createServer({ root, logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'fe2930-test-http', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/api/')) return next()
    try {
      if (req.url === '/api/auth/capabilities') return respond(res, 200, caps)
      if (req.url === '/api/auth/login') return respond(res, 200, auth)
      if (req.url === '/api/auth/logout') return respond(res, 204)
      assert.equal(req.headers.authorization, `Bearer ${auth.accessToken}`)
      const url = new URL(req.url, 'http://fixture'), path = url.pathname, chunks = []
      for await (const chunk of req) chunks.push(chunk)
      const body = chunks.length ? JSON.parse(Buffer.concat(chunks).toString()) : null
      requests.push({ method: req.method, path, query: Object.fromEntries(url.searchParams), body, key: req.headers['idempotency-key'] })
      if (path.endsWith('/intros')) return respond(res, 200, { items: [publicIntro], next_cursor: null })
      if (path.includes('/people/')) return denied ? failure(res, 404, 'RESOURCE_UNAVAILABLE') : respond(res, 200, publicIntro.author)
      if (path.endsWith('/me/age-status')) return respond(res, 200, { user_id: auth.userId, event_id: publicIntro.event_id, status: 'PROVIDER_UNAVAILABLE', verified_at: null, expires_at: null, verification_available: false })
      if (path.endsWith('/commands')) { assert.equal(url.searchParams.get('idempotency_key'), command?.idempotency_key); return respond(res, 200, command) }
      if (path.endsWith('/me/intro') && req.method === 'GET') return respond(res, 200, saved)
      assert.match(req.headers['idempotency-key'] ?? '', /^[0-9a-f-]{36}$/); assert.equal(body.expected_revision, saved.revision)
      if (path.endsWith('/me/intro') && req.method === 'PUT') {
        saved = { ...saved, text: body.text, age_bounds: body.age_bounds, expires_at: body.expires_at, revision: saved.revision + 1, state: saved.state === 'PUBLISHED' ? 'PUBLISHED' : 'DRAFT' }
        command = { receipt_id: '00000000-0000-4000-8000-000000000046', event_id: saved.event_id, actor_id: auth.userId, operation_id: 'setConcertIntro', idempotency_key: req.headers['idempotency-key'], state: 'SUCCEEDED', resource_id: saved.intro_id, resource_revision: saved.revision, failure_reason: null, result_status: 200, created_at: new Date().toISOString(), expires_at: new Date(Date.now() + 3_600_000).toISOString() }
        return uncertain ? failure(res, 503, 'DEPENDENCY_UNAVAILABLE') : respond(res, 200, saved)
      }
      if (path.endsWith('/publish') || path.endsWith('/close')) { saved = { ...saved, revision: saved.revision + 1, state: path.endsWith('/close') ? 'CLOSED' : 'PUBLISHED' }; return respond(res, 200, saved) }
      return failure(res, 404, 'RESOURCE_UNAVAILABLE')
    } catch (error) { errors.push(String(error)); if (!res.destroyed) respond(res, 500, {}) }
  })
} }] })
const writes = () => requests.filter(r => r.method !== 'GET').length
const layout = async page => { assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal clipping'); for (const control of await page.locator('button,input,textarea').all()) { const box = await control.boundingBox(); if (box) assert(box.height >= 44, 'control below 44px') } }
try {
  await dev.listen(); const origin = `http://127.0.0.1:${dev.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    saved = initial(); uncertain = denied = false; command = null
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message)); await page.goto(`${origin}/src/features/people/__tests__/fixtures/preview.html`)
    await expect(page.getByRole('link', { name: publicIntro.author.display_name, exact: true })).toBeVisible(); await expect(page.getByRole('img')).toHaveAccessibleName(`${publicIntro.author.display_name}'s avatar: avatar unavailable`)
    await expect.poll(() => page.evaluate(() => window.__FE2930_TEST_ONLY__.state().resolves)).toBeGreaterThan(0)
    assert.equal(await page.locator('canvas,img').count(), 0); assert(!(await page.locator('body').innerText()).includes('Minimum age'))
    await page.getByLabel('Find introductions').fill('community'); await page.getByRole('button', { name: 'Search', exact: true }).focus(); await page.keyboard.press('Enter'); await expect.poll(() => requests.at(-1)?.query.q).toBe('community'); await layout(page); await page.screenshot({ path: `${evidence}/people-${width}.png`, fullPage: true })
    await page.getByRole('link', { name: publicIntro.author.display_name, exact: true }).click(); await expect(page.getByRole('heading', { name: publicIntro.author.display_name })).toBeVisible(); await expect(page.getByRole('link', { name: 'Report profile' })).toHaveAttribute('href', `/report?targetType=USER&targetId=${publicIntro.author.user_id}`); await expect(page.getByRole('button', { name: 'Request sharing' })).toBeDisabled(); await layout(page)
    await page.getByRole('link', { name: 'Back to people' }).click(); denied = true; await page.getByRole('link', { name: publicIntro.author.display_name, exact: true }).click(); await expect(page.getByRole('status')).toContainText('This profile is unavailable.'); assert(!(await page.locator('body').innerText()).includes('private detail')); await page.getByRole('link', { name: 'Back to people' }).click(); await expect(page.getByRole('link', { name: publicIntro.author.display_name, exact: true })).toHaveCount(0); denied = false
    await page.getByRole('link', { name: 'Editor fixture' }).click(); await expect(page.getByLabel('Introduction', { exact: true })).toHaveValue(initial().text)
    await page.getByLabel('Introduction', { exact: true }).fill('My private local draft'); await page.getByRole('link', { name: 'Age fixture' }).click(); await expect(page.getByRole('heading', { name: 'Verification provider unavailable' })).toBeVisible(); await expect(page.getByRole('button', { name: 'Start verification' })).toBeDisabled(); assert.equal(await page.locator('input').count(), 0); await layout(page)
    await page.getByRole('link', { name: 'Editor fixture' }).click(); await expect(page.getByLabel('Introduction', { exact: true })).toHaveValue('My private local draft')
    await page.evaluate(() => window.__FE2930_TEST_ONLY__.readOnly(true)); const before = writes(); await expect(page.getByRole('button', { name: 'Save introduction' })).toBeDisabled(); await page.getByLabel('Introduction', { exact: true }).fill('Retained read-only draft'); assert.equal(writes(), before)
    await page.evaluate(() => window.__FE2930_TEST_ONLY__.readOnly(false)); uncertain = true; await page.getByRole('button', { name: 'Save introduction' }).click(); await expect(page.getByRole('button', { name: 'Check original command' })).toBeVisible(); await expect(page.getByRole('button', { name: 'Save introduction' })).toBeDisabled()
    const after = writes(); uncertain = false; await page.getByRole('button', { name: 'Check original command' }).click(); await expect(page.getByRole('status')).toContainText('Current server state verified.'); assert.equal(writes(), after); assert.equal(saved.text, 'Retained read-only draft')
    await page.getByRole('button', { name: 'Publish introduction' }).click(); await expect.poll(() => saved.state).toBe('PUBLISHED'); await expect(page.getByRole('button', { name: 'Publish introduction' })).toBeDisabled()
    await page.getByLabel('Introduction', { exact: true }).fill('Unfinished new draft'); await page.evaluate(() => window.__FE2930_TEST_ONLY__.readOnly(true)); await page.getByRole('button', { name: 'Close introduction' }).click(); await expect.poll(() => saved.state).toBe('CLOSED'); await expect(page.getByLabel('Introduction', { exact: true })).toHaveValue('Unfinished new draft')
    await expect(page.getByRole('button', { name: 'Cancel changes' })).toBeEnabled(); await layout(page); await page.getByRole('button', { name: 'Cancel changes' }).focus(); await page.keyboard.press('Shift+Tab'); await page.keyboard.press('Tab'); assert(await page.getByRole('button', { name: 'Cancel changes' }).evaluate(el => getComputedStyle(el).outlineStyle !== 'none')); await page.screenshot({ path: `${evidence}/intro-${width}.png`, fullPage: true })
    await page.evaluate(() => window.__FE2930_TEST_ONLY__.suspend()); await expect(page.getByLabel('Introduction', { exact: true })).toHaveValue(''); assert.equal(await page.locator('canvas,img').count(), 0); await page.evaluate(() => window.__FE2930_TEST_ONLY__.dispose()); await context.close()
    results.push({ width, discoveryNoXAndPrivateProjection: 'PASS', searchAndProfileNavigation: 'PASS', deniedDirectIdListConsistency: 'PASS', sharedProductionEntryRejectsUnreviewedArt: 'PASS', draftNavigationAndReadOnly: 'PASS', uncertainReceiptResourceReadNoReplay: 'PASS', publishAndSafetyClose: 'PASS', unknownProviderNoSelfDeclaration: 'PASS', privacyCleanup: 'PASS', keyboardTouchAndClipping: 'PASS' })
  }
  assert.deepEqual(errors, [])
  const paths = execFileSync('rg', ['--files', 'src/features/people', 'src/features/intro', 'src/features/profile', 'src/features/age-status'], { cwd: root, encoding: 'utf8' }).trim().split('\n').filter(path => !path.includes('/evidence/')).sort()
  const hashes = Object.fromEntries(await Promise.all(paths.map(async path => [path, createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')])))
  const report = { status: 'MOCK_READY', browser: await browser.version(), node: process.version, base: 'e6fef53d9f0841b3e16ce3baa4db01a834043957', candidateParent: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), results, pageErrors: errors, protectedRequests: requests.length, mutationCount: writes(), sourceHashes: hashes, limits: ['Test-only HTTP fixtures with canonical Auth/shared transport; real backend and persistence NOT_RUN.', 'C06 synthetic metadata is deliberately rejected by shared production compositor. No product artwork rendered or approved.', 'Root routes/access/bootstrap/provider callback composition, two-account IN03, physical iOS/Android and production verification NOT_RUN.'] }
  await writeFile(`${evidence}/report.json`, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify({ ...report, sourceHashes: `${paths.length} files pinned` }, null, 2))
} finally { await browser?.close(); await dev.close() }
