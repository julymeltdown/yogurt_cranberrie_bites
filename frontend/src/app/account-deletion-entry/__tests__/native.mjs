import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_AUTH_DELETION_EVIDENCE ?? '/tmp/mcr-fe36-auth-deletion-root-evidence'
await mkdir(evidence, { recursive: true })
const core = JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/core/bootstrap-open.json', import.meta.url)))
const login = JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/core/auth-success.json', import.meta.url)))
const capabilities = JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/core/auth-capabilities-disabled.json', import.meta.url)))
const contract = JSON.parse(await readFile(new URL('../../../../../contracts/generated/mcr-domain.openapi.json', import.meta.url)))
const targets = contract.components.schemas.UserDeletionStep.properties.target.enum
capabilities.providers = capabilities.providers.map(p => p.provider === 'EMAIL' ? { ...p, enabled: true, disabled_reason: null } : p)
const other = '00000000-0000-4000-8000-000000000999', sagaId = '88888888-8888-4888-8888-888888888888'
let mode = 'pending', actor = login.userId, browser, release = () => {}
const requests = [], errors = [], results = []
function saga(complete = false) {
 const now = Date.now()
 return { id: sagaId, userId: actor, status: complete ? 'COMPLETED' : mode === 'partial' ? 'DEAD_LETTERED' : 'IN_PROGRESS', retentionUntil: new Date(now + 86400000).toISOString(),
  createdAt: new Date(now - 60000).toISOString(), updatedAt: new Date(now - 1000).toISOString(), completedAt: complete ? new Date(now - 2000).toISOString() : null,
  steps: targets.map((target, i) => ({ target, status: complete ? 'SUCCEEDED' : mode === 'partial' && i === 7 ? 'DEAD_LETTERED' : 'PENDING', attemptCount: 1,
   nextAttemptAt: new Date(now + 60000).toISOString(), lastError: 'PRIVATE provider diagnostic', completedAt: complete ? new Date(now - 3000).toISOString() : null })) }
}
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'synthetic-auth-deletion-production-routes', configureServer(server) {
 server.middlewares.use(async (req, res, next) => {
  if (!req.url?.startsWith('/api/')) return next()
  let text = ''; for await (const chunk of req) text += chunk
  requests.push({ path: req.url, method: req.method, body: text, intent: req.headers['x-mcr-intent'] ?? null, key: req.headers['idempotency-key'] ?? null, bearer: Boolean(req.headers.authorization) })
  res.setHeader('Cache-Control', 'no-store'); res.setHeader('Content-Type', 'application/json')
  let value
  if (req.url === '/api/auth/capabilities') value = capabilities
  else if (req.url === '/api/auth/login') value = { ...login, expiresIn: 60 }
  else if (req.url === '/api/auth/logout') { res.statusCode = 204; return res.end() }
  else if (req.url.endsWith('/bootstrap') || req.url.endsWith('/me')) {
   const b = structuredClone(core); b.participant.user_id = actor; b.participant.profile.user_id = actor; b.capabilities.world = { available: true, reason: null }
   value = req.url.endsWith('/me') ? b.participant : b
  } else if (req.url === '/api/auth/account' && req.method === 'DELETE') {
   if (mode === 'uncertain' || mode === 'unauthorized') { res.statusCode = mode === 'uncertain' ? 503 : 401; return res.end(JSON.stringify({ code: 'UNAUTHENTICATED', message: 'PRIVATE provider failure' })) }
   const s = saga(mode === 'legacy' || mode === 'malformed' || mode === 'slow')
   if (mode === 'legacy') s.steps = s.steps.slice(0, 4)
   if (mode === 'malformed') s.steps[7].target = 'PROFILE'
   value = { userId: actor, deleted: true, sagaId, saga: s }
   res.statusCode = 202
   if (mode === 'slow') await new Promise(yes => { release = yes })
  } else if (req.url === '/api/auth/account/deletion' && req.method === 'GET') value = saga(mode === 'complete')
  else { res.statusCode = 503; value = { code: 'UNAVAILABLE', message: 'Synthetic service unavailable' } }
  res.end(JSON.stringify(value))
 })
} }] })
try {
 await dev.listen(); browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
 const url = `http://127.0.0.1:${dev.httpServer.address().port}/src/app/account-deletion-entry/__tests__/preview.html`
 for (const width of [360, 390, 430]) {
  const context = await browser.newContext({ viewport: { width, height: 900 }, reducedMotion: 'reduce' }), page = await context.newPage()
  page.on('pageerror', e => errors.push(e.message))
  const open = async (route = '/me/account', anonymous = false) => { actor = login.userId; await page.goto(`${url}?route=${route}${anonymous ? '&anonymous=1' : ''}`) }
  const request = async () => { await page.getByRole('button', { name: 'Request account deletion' }).click(); await expect(page.getByRole('button', { name: 'Keep account' })).toBeFocused(); await page.getByLabel('Deletion confirmation').fill('DELETE_MY_ACCOUNT'); await page.getByRole('button', { name: 'Confirm deletion request' }).click() }
  const progress = page.getByRole('button', { name: 'Check deletion progress' })
  const counts = start => requests.slice(start).filter(r => r.path === '/api/auth/account' || r.path === '/api/auth/account/deletion' || r.path === '/api/auth/refresh')
  mode = 'pending'; await open('/me/settings'); await expect(page.getByRole('link', { name: 'Account controls', exact: true })).toBeVisible()
  const mounts = await page.evaluate(() => window.__ACCOUNT_DELETION_ROOT__.stats.mounts)
  await page.evaluate(() => { window.__DELETION_HOST__ = document.querySelector('[data-world-host]') })
  await page.getByRole('link', { name: 'Account controls', exact: true }).focus(); await page.keyboard.press('Enter'); await expect(page.getByRole('heading', { name: 'Account controls' })).toBeVisible()
  await page.getByRole('button', { name: 'Back', exact: true }).click(); await expect(page.getByRole('link', { name: 'Account controls', exact: true })).toBeFocused()
  assert(await page.evaluate(() => document.querySelector('[data-world-host]') === window.__DELETION_HOST__)); assert.equal(await page.evaluate(() => window.__ACCOUNT_DELETION_ROOT__.stats.mounts), mounts)
  await page.getByRole('link', { name: 'Account controls', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Account controls' })).toBeVisible()
  let start = requests.length
  await page.evaluate(() => window.__ACCOUNT_DELETION_ROOT__.seedPrivate()); await request()
  await expect(page.getByRole('heading', { name: 'Account deletion status' })).toBeFocused(); await expect(page.getByRole('status').filter({ hasText: 'Deletion is pending' })).toBeVisible()
  await expect(page.getByRole('list', { name: 'Deletion progress' }).getByRole('listitem')).toHaveCount(8)
  assert.equal(await page.locator('[data-world-host],canvas').count(), 0); assert.equal(await page.evaluate(() => window.__ACCOUNT_DELETION_ROOT__.stats.games), 0); assert(await page.evaluate(() => window.__ACCOUNT_DELETION_ROOT__.privateCleared()))
  assert.deepEqual(counts(start).map(r => [r.method, r.path]), [['DELETE', '/api/auth/account']])
  const sent = counts(start)[0]; assert.equal(sent.body, ''); assert.equal(sent.key, null); assert.equal(sent.intent, 'browser'); assert(sent.bearer)
  mode = 'partial'; await progress.click(); await expect(page.getByRole('status').filter({ hasText: 'Deletion needs recovery' })).toBeVisible(); assert(!(await page.locator('body').innerText()).includes('PRIVATE provider'))
  await page.screenshot({ path: `${evidence}/public-partial-${width}.png`, fullPage: true }); assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
  mode = 'complete'; await progress.click(); await expect(page.getByText('Account deletion confirmed by the Auth owner.', { exact: true })).toBeVisible()
  assert.equal(await page.evaluate(() => window.__ACCOUNT_DELETION_ROOT__.snapshot().userId), null); assert.equal(counts(start).filter(r => r.method === 'DELETE').length, 1); assert.equal(counts(start).filter(r => r.method === 'GET').length, 2)
  for (const kind of ['uncertain', 'legacy', 'malformed']) {
   mode = kind; await open(); await expect(page.getByRole('heading', { name: 'Account controls' })).toBeVisible(); start = requests.length; await request()
   await expect(page.getByRole('heading', { name: 'Account deletion status' })).toBeVisible(); await expect(page.getByRole('status').filter({ hasText: 'Deletion is not confirmed' })).toBeVisible()
   assert.equal(counts(start).length, 1); await expect(page.getByRole('list', { name: 'Deletion progress' })).toHaveCount(0)
   mode = 'complete'; await progress.click(); await expect(page.getByText('Account deletion confirmed by the Auth owner.', { exact: true })).toBeVisible()
   assert.deepEqual(counts(start).map(r => r.method), ['DELETE', 'GET'])
  }
  mode = 'pending'; await open('/account/deleted', true); await expect(page.getByText(/No deletion completion is confirmed/)).toBeVisible(); assert.equal(await page.locator('[data-world-host],canvas').count(), 0); await expect(progress).toHaveCount(0)
  mode = 'unauthorized'; await open(); await expect(page.getByRole('heading', { name: 'Account controls' })).toBeVisible(); start = requests.length; await request(); await expect(page.getByRole('heading', { name: 'Sign in to continue' })).toBeVisible(); assert.equal(counts(start).length, 1)
  mode = 'slow'; await open(); await expect(page.getByRole('heading', { name: 'Account controls' })).toBeVisible(); start = requests.length; await request(); await expect.poll(() => counts(start).length).toBe(1)
  actor = other; await page.evaluate(id => window.__ACCOUNT_DELETION_ROOT__.changeIdentity(id), other); release(); await expect(page.getByText('Account deletion confirmed by the Auth owner.', { exact: true })).toHaveCount(0)
  await expect.poll(() => page.evaluate(() => window.__ACCOUNT_DELETION_ROOT__.snapshot().userId)).toBe(other)
  mode = 'pending'; await page.clock.install(); await open(); await expect(page.getByRole('heading', { name: 'Account controls' })).toBeVisible(); await request(); await expect(page.getByRole('heading', { name: 'Account deletion status' })).toBeVisible()
  start = requests.length; await page.clock.fastForward(61000); await expect(page.getByText(/No deletion completion is confirmed/)).toBeVisible(); await expect(progress).toHaveCount(0); assert.equal(counts(start).length, 0)
  const stats = await page.evaluate(() => window.__ACCOUNT_DELETION_ROOT__.stats); assert(stats.peakGames <= 1 && stats.peakConnections <= 1)
  results.push({ width, defaultSettingsKeyboardBackFocus: true, canonicalDeleteOnce: true, noBodyKeyRefreshReplayPoll: true, eightStepPublicRecovery: true, partialFailure: true, unknownLegacyMalformed: true,
   worldDisposedRecoveryRetained: true, privateCacheScrubbed: true, completionLogout: true, directEntryTruthful: true, unauthorizedClears: true, lateIdentityFenced: true, expiryScrubs: true, instrumentedPeakWorlds: stats.peakGames })
  await context.close()
 }
 assert.deepEqual(errors, [])
 const paths = ['src/features/auth/model/account-deletion.ts', 'src/features/auth/model/session.ts', 'src/app/router.tsx', 'src/app/PublicBoundary.tsx', 'src/features/settings/SettingsPage.tsx',
  'src/app/account-deletion-entry/owner.ts', 'src/app/account-deletion-entry/context.tsx', 'src/app/account-deletion-entry/context.tsx', 'src/app/account-deletion-entry/panel.tsx', 'src/app/account-deletion-entry/recovery.tsx', 'src/app/account-deletion-entry/link.tsx',
  'src/app/account-deletion-entry/__tests__/preview.tsx', 'src/app/account-deletion-entry/__tests__/native.mjs']
 const hashes = Object.fromEntries(await Promise.all(paths.map(async path => [path, createHash('sha256').update(await readFile(path)).digest('hex')])))
 await writeFile(`${evidence}/result.json`, JSON.stringify({ status: 'PASS', syntheticHTTP: true, liveDeletion: false, cleanup: 'NOT_RUN', rootDefaults: true, instrumentedWorld: true, results, requests, errors, hashes }, null, 2) + '\n')
 console.log(JSON.stringify({ status: 'PASS', evidence, widths: results.length, requests: requests.length, errors }))
} finally { await browser?.close(); await dev.close() }
