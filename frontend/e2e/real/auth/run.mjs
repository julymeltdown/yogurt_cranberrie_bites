// Real local services only. No route.fulfill, MSW, response fixtures, tracing or credential reports.
import assert from 'node:assert/strict'
import { createHash, randomBytes } from 'node:crypto'
import { spawn, execFileSync } from 'node:child_process'
import { request as httpRequest } from 'node:http'
import { createServer as createPortReservation } from 'node:net'
import { readFile, writeFile, mkdir, readdir, lstat } from 'node:fs/promises'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const directory = dirname(fileURLToPath(import.meta.url)), root = resolve(directory, '../../../..'), frontend = resolve(root, 'frontend')
const { values } = parseArgs({ options: { 'bin-dir': { type: 'string' }, receipt: { type: 'string' }, evidence: { type: 'string' }, 'service-source-root': { type: 'string' }, historical: { type: 'boolean', default: false } } })
if (!values['bin-dir'] || !values.receipt || !values.evidence) throw new Error('Explicit binary directory, provenance receipt and new evidence directory are required')
const evidence = resolve(values.evidence), binDir = resolve(values['bin-dir']), serviceSourceRoot = resolve(values['service-source-root'] ?? root)
await mkdir(evidence, { mode: 0o700 })
const digest = bytes => createHash('sha256').update(bytes).digest('hex')
const delay = ms => new Promise(resolve => setTimeout(resolve, ms))
let stage = 'preflight', browser, server, stack, exitCode, gateway = null, descriptor = null, interrupted = false
const checks = {}, requests = [], pageErrors = []
const report = { scope: 'Actual isolated Auth/frontend interoperability; test-only enrollment acknowledgements', result: 'FAIL',
  qualificationMode: values.historical ? 'HISTORICAL_EXPLORATORY' : 'CURRENT_SOURCE_REQUESTED', currentCandidateGate: 'NOT_RUN', checks, requests, productionConsent: 'NOT_RUN: approved eligibility/consent persistence contract missing',
  world: 'NOT_RUN: actual bootstrap remains unavailable', productionTLSCookies: 'NOT_RUN: local HTTP only', secretsRecorded: false }
const readyFile = resolve(evidence, 'ready.json'), stackReport = resolve(evidence, 'stack.json')
const launcher = resolve(root, 'backend/scripts/mcr/dev_stack.py')
function command(args) { return new Promise(resolve => {
  const child = spawn('python3', [launcher, ...args], { cwd: root, stdio: ['ignore', 'ignore', 'ignore'] })
  child.once('error', () => resolve(127)); child.once('exit', code => resolve(code ?? 1))
}) }
async function until(check, timeout = 15000, ignoreInterrupt = false) {
  const end = Date.now() + timeout
  while (Date.now() < end) { if (interrupted && !ignoreInterrupt) throw new Error('interrupted'); if (await check()) return; await delay(100) }
  throw new Error('bounded wait failed')
}
function privateFile(info) { assert(info.isFile() && !info.isSymbolicLink() && info.uid === process.getuid() && !(info.mode & 0o077)) }
async function mail(kind, email) {
  let found
  await until(async () => {
    for (const name of await readdir(descriptor.mail_directory)) {
      if (!/^[0-9a-f-]{36}\.json$/.test(name)) continue
      const path = resolve(descriptor.mail_directory, name); privateFile(await lstat(path))
      const value = JSON.parse(await readFile(path, 'utf8'))
      if (value.kind === kind && value.payload?.email === email) { found = value.payload; return true }
    }
    return false
  })
  return found
}
const stopSignal = () => { interrupted = true; void browser?.close(); stack?.kill('SIGTERM') }
process.once('SIGINT', stopSignal); process.once('SIGTERM', stopSignal)
try {
  report.frontendRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()
  report.frontendSourcesSha256 = {}
  async function sources(path) {
    for (const item of await readdir(path, { withFileTypes: true })) {
      const child = resolve(path, item.name)
      if (item.isDirectory()) await sources(child)
      else if (item.isFile() && /\.(?:ts|tsx|css|json)$/.test(item.name)) report.frontendSourcesSha256[child.slice(root.length + 1)] = digest(await readFile(child))
    }
  }
  await sources(resolve(frontend, 'src'))
  report.harnessHashes = {}
  for (const name of ['entry.html', 'entry.tsx', 'run.mjs']) report.harnessHashes[name] = digest(await readFile(resolve(directory, name)))
  report.lockfileSha256 = digest(await readFile(resolve(frontend, 'pnpm-lock.yaml')))
  const sourceReceipt = JSON.parse(await readFile(resolve(values.receipt), 'utf8'))
  assert(sourceReceipt.result === 'PASS' || sourceReceipt.exit_code === 0)
  const services = sourceReceipt.owned_stack?.services ?? sourceReceipt.services ?? Object.entries(sourceReceipt.binary_sha256 ?? {}).map(([service, binary_sha256]) => ({ service, binary_sha256 }))
  assert(Array.isArray(services) && services.length === 6)
  report.serviceSourceRevision = sourceReceipt.source_revision
  report.serviceSourceRoot = serviceSourceRoot
  report.binaryReceiptSha256 = digest(await readFile(resolve(values.receipt)))
  report.binaryHashes = {}
  for (const row of services) {
    assert(/^[a-z-]+$/.test(row.service)); const path = resolve(binDir, row.service), info = await lstat(path)
    assert(info.isFile() && !info.isSymbolicLink() && (info.mode & 0o111))
    const hash = digest(await readFile(path)); assert.equal(hash, row.binary_sha256); report.binaryHashes[row.service] = hash
  }
  assert(sourceReceipt.source_files_sha256 && Object.keys(sourceReceipt.source_files_sha256).length)
  const differences = []
  for (const [name, expected] of Object.entries(sourceReceipt.source_files_sha256)) {
    assert(name.startsWith('backend/') && !name.split('/').includes('..'))
    if (digest(await readFile(resolve(serviceSourceRoot, name))) !== expected) differences.push(name)
  }
  report.sourceDifferences = differences
  if (!values.historical) assert.equal(differences.length, 0)
  const eventId = '00000000-0000-4000-8000-000000000001'
  const entry = await readFile(resolve(directory, 'entry.html'), 'utf8'), production = await readFile(resolve(frontend, 'index.html'), 'utf8')
  const reservation = createPortReservation()
  await new Promise((yes, no) => { reservation.once('error', no); reservation.listen(0, '127.0.0.1', yes) })
  const appPort = reservation.address().port
  await new Promise((yes, no) => reservation.close(error => error ? no(error) : yes()))
  server = await createServer({ root: frontend, define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(eventId) },
    server: { host: '127.0.0.1', port: appPort, strictPort: true }, plugins: [{ name: 'actual-local-auth-only', configureServer(vite) {
      vite.middlewares.use(async (req, res, next) => {
        const path = (req.url ?? '').split('?')[0]
        if (path.startsWith('/api/')) {
          if (!gateway) { res.statusCode = 503; res.end(); return }
          // Forward original Origin, cookies and body unchanged. All status/body/cookie headers are actual Gateway output.
          const upstream = httpRequest(new URL(req.url, gateway), { method: req.method, headers: { ...req.headers, host: new URL(gateway).host } }, response => {
            requests.push({ method: req.method, path, status: response.statusCode })
            res.writeHead(response.statusCode, response.headers); response.pipe(res)
          })
          upstream.on('error', () => { if (!res.headersSent) res.statusCode = 502; res.end() })
          req.once('aborted', () => upstream.destroy()); res.once('close', () => { if (!res.writableEnded) upstream.destroy() }); req.pipe(upstream); return
        }
        if (['/', '/login', '/signup', '/verify-email', '/reset-password', '/world', '/__production'].includes(path)) {
          const html = path === '/__production' ? production.replace('<head>', '<head><script>history.replaceState(null,"","/signup")</script>') : entry
          res.setHeader('Content-Type', 'text/html'); res.end(await vite.transformIndexHtml(req.url, html)); return
        }
        next()
      })
    } }] })
  await server.listen(); const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  stage = 'owned-stack-start'
  stack = spawn('python3', [launcher, 'start', '--bin-dir', binDir, '--ready-file', readyFile, '--report', stackReport,
    '--browser-origin', origin, '--duration-seconds', '900'], { cwd: root, stdio: ['ignore', 'ignore', 'ignore'] })
  stack.once('exit', code => { exitCode = code ?? 1 }); stack.once('error', () => { exitCode = 127 })
  await until(async () => {
    if (exitCode !== undefined) throw new Error('owned stack stopped before readiness')
    try { privateFile(await lstat(readyFile)); descriptor = JSON.parse(await readFile(readyFile, 'utf8')); return descriptor.kind === 'mcr-owned-dev-stack-v1' } catch { return false }
  }, 120000)
  assert.equal(await command(['status', '--ready-file', readyFile]), 0)
  gateway = descriptor.gateway_url; assert(/^http:\/\/127\.0\.0\.1:\d+$/.test(gateway)); checks.stackReady = 'PASS'
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = await browser.version()
  const context = await browser.newContext({ viewport: { width: 390, height: 844 } }), page = await context.newPage()
  context.on('page', p => p.on('pageerror', () => pageErrors.push('browser-page-error')))
  page.on('pageerror', () => pageErrors.push('browser-page-error'))
  const go = async (p, path) => p.evaluate(path => window.__MCR_REAL_EMAIL__.go(path), path)
  const snapshot = p => p.evaluate(() => window.__MCR_REAL_EMAIL__.snapshot())
  const capture = async name => page.screenshot({ path: resolve(evidence, name + '.png'), fullPage: true, mask: [page.locator('input')] })
  stage = 'production-policy-gate'
  await page.goto(origin + '/__production'); await expect(page.getByText('Account creation is not open yet. We are preparing the signup requirements and notices.')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Create account', exact: true })).toBeDisabled(); checks.productionSignupBlocked = 'PASS'
  stage = 'signup-delivery-verification'
  await page.goto(origin + '/signup')
  const suffix = randomBytes(12).toString('hex'), email = `local-${suffix}@example.invalid`, password = randomBytes(24).toString('base64url'), replacement = randomBytes(24).toString('base64url')
  await expect(page.getByLabel('Email', { exact: true })).toBeEnabled()
  await page.getByLabel('Email', { exact: true }).fill(email); await page.getByLabel('Password', { exact: true }).fill(password)
  await page.getByRole('checkbox').nth(0).check(); await page.getByRole('checkbox').nth(1).check()
  await page.getByRole('button', { name: 'Create account', exact: true }).click()
  await expect(page.getByRole('link', { name: 'Continue to email verification' })).toBeVisible(); checks.signup = 'PASS'
  const verification = await mail('VERIFY_EMAIL', email); assert(/^\d{6}$/.test(verification.verification_code)); checks.actualVerificationDelivery = 'PASS'
  await page.getByRole('link', { name: 'Continue to email verification' }).click()
  await page.getByLabel('Email', { exact: true }).fill(email)
  await page.getByLabel('Verification code', { exact: true }).fill(verification.verification_code === '000000' ? '000001' : '000000')
  await page.getByRole('button', { name: 'Verify email', exact: true }).click()
  await expect(page.getByText('We could not verify this code. It may be invalid, expired or already used. You can request another code.')).toBeVisible()
  checks.wrongVerificationRejected = 'PASS'
  await page.getByLabel('Verification code', { exact: true }).fill(verification.verification_code)
  await page.getByRole('button', { name: 'Verify email', exact: true }).click(); await expect(page.getByText('Email verified. Sign in to continue.')).toBeVisible()
  checks.verification = 'PASS'; await capture('verified-mobile')
  async function login(p, candidate = password, accepted = true) {
    await go(p, '/login'); await expect(p.getByRole('button', { name: 'Sign in with email' })).toBeEnabled()
    await p.getByLabel('Email', { exact: true }).fill(email); await p.getByLabel('Password', { exact: true }).fill(candidate)
    await p.getByRole('button', { name: 'Sign in with email' }).click()
    if (accepted) await expect.poll(async () => (await snapshot(p)).hasAccess).toBe(true)
    else await expect(p.getByText('We could not sign you in. Check your details or try again later.')).toBeVisible()
  }
  stage = 'sign-in-world-unavailable'
  await login(page); const identity = (await snapshot(page)).userId; assert(identity)
  await expect.poll(() => requests.some(r => r.path.endsWith('/bootstrap') && r.status >= 400)).toBe(true)
  await expect(page.locator('canvas')).toHaveCount(0); checks.actualBootstrapUnavailable = 'PASS: actual unsuccessful backend response; no world opened'; await capture('signed-in-bootstrap-unavailable'); checks.signIn = 'PASS'
  const cookie = (await context.cookies()).find(c => c.name === 'refresh_token'); assert(cookie?.httpOnly && cookie.sameSite === 'Lax' && !cookie.secure)
  checks.localHttpCookie = 'PASS'; assert.equal(await page.evaluate(() => localStorage.length + sessionStorage.length), 0)
  async function rejectOldSession(cookies) {
    const stale = await browser.newContext({ storageState: { cookies, origins: [] } }), stalePage = await stale.newPage()
    try {
      await stalePage.goto(origin + '/login')
      const rejected = await stalePage.evaluate(async () => (await fetch('/api/auth/refresh', { method: 'POST', credentials: 'include', headers: { 'Content-Type': 'application/json', 'X-MCR-Intent': 'browser' }, body: '{}' })).status)
      assert.equal(rejected, 401)
    } finally { await stale.close() }
  }
  stage = 'two-tab-invalidation-logout'
  const peer = await context.newPage(); await peer.goto(origin + '/login'); await login(peer)
  await expect.poll(async () => (await snapshot(page)).hasAccess).toBe(false)
  await go(peer, '/login'); await expect(peer.getByRole('button', { name: 'Sign out', exact: true })).toBeVisible()
  await go(page, '/login'); await page.getByLabel('Email', { exact: true }).fill(email); await page.getByLabel('Password', { exact: true }).fill(password)
  const peerGeneration = (await snapshot(page)).generation, loggedOutCookies = await context.cookies()
  await peer.getByRole('button', { name: 'Sign out', exact: true }).click(); await expect(peer.getByText('You’re signed out.')).toBeVisible()
  await expect.poll(async () => (await snapshot(page)).generation).toBeGreaterThan(peerGeneration)
  await expect(page.getByLabel('Email', { exact: true })).toHaveValue(''); await expect(page.getByLabel('Password', { exact: true })).toHaveValue('')
  checks.logoutPeerDraftInvalidation = 'PASS'
  await rejectOldSession(loggedOutCookies); checks.logoutRevokesRefreshSession = 'PASS'
  assert.equal((await snapshot(peer)).hasAccess, false); assert.equal((await snapshot(page)).hasAccess, false); checks.twoTabInvalidationAndLogout = 'PASS'
  stage = 'password-reset-delivery'
  await login(page); const oldCookies = await context.cookies()
  await go(peer, '/reset-password'); await peer.getByLabel('Email', { exact: true }).fill(email)
  await peer.getByRole('button', { name: 'Request reset', exact: true }).click()
  await expect(peer.getByText('Request received. If password recovery is available for this address, instructions may arrive. Account details and email delivery are not confirmed here.')).toBeVisible()
  const reset = await mail('PASSWORD_RESET', email), link = new URL(reset.reset_link)
  assert(link.pathname === '/reset-password' && !link.search && /^#token=[A-Za-z0-9_-]{43}$/.test(link.hash))
  assert.equal(link.origin, origin); await peer.goto(link.href); report.resetEntry = 'ACTUAL_DELIVERED_APP_LINK'
  await expect.poll(() => peer.url()).toBe(origin + '/reset-password')
  await expect(peer.locator('#reset-token')).toHaveCount(0); checks.resetLinkScrubbed = 'PASS'
  await peer.getByLabel('New password', { exact: true }).fill(replacement); await peer.getByRole('button', { name: 'Reset password', exact: true }).click()
  await expect(peer.getByText('Password reset completed. Sign in again with your new password.')).toBeVisible()
  await expect.poll(async () => (await snapshot(page)).hasAccess).toBe(false); checks.resetAndPeerInvalidation = 'PASS'
  stage = 'old-session-and-password-rejected'
  await rejectOldSession(oldCookies); checks.oldRefreshSessionRejected = 'PASS'
  await login(peer, password, false); await login(peer, replacement)
  assert.equal((await snapshot(peer)).userId, identity); checks.newPasswordAndCanonicalIdentity = 'PASS'
  await go(peer, '/login'); await peer.getByRole('button', { name: 'Sign out', exact: true }).click(); await expect(peer.getByText('You’re signed out.')).toBeVisible()
  assert.equal(await peer.evaluate(() => localStorage.length + sessionStorage.length), 0)
  assert.deepEqual(pageErrors, []); checks.noCredentialStorageOrPageErrors = 'PASS'
  report.result = 'PASS'
  report.currentCandidateGate = values.historical ? 'NOT_RUN: historical binaries' : 'PASS: declared source inputs and executed binary hashes match'
} catch { report.failureStage = stage }
finally {
  await browser?.close(); await server?.close()
  if (stack && exitCode === undefined) {
    const stopped = descriptor ? await command(['stop', '--ready-file', readyFile]) : 1
    if (stopped !== 0) stack.kill('SIGTERM')
    try { await until(() => exitCode !== undefined, 20000, true) } catch { report.result = 'FAIL'; report.cleanup = 'FAIL: launcher did not terminate' }
  }
  if (stack) {
    try { const owned = JSON.parse(await readFile(stackReport, 'utf8')); report.cleanup = owned.cleanup; report.stackReportSha256 = digest(await readFile(stackReport)); assert.equal(owned.cleanup, 'PASS'); assert.equal(owned.result, 'PASS'); assert.equal(exitCode, 0)
      assert.equal(owned.services.length, 6)
      for (const row of owned.services) {
        assert.equal(row.binary_sha256, report.binaryHashes[row.service]); assert.equal(row.migrate, 'PASS'); assert.equal(row.verify, 'PASS')
        assert.equal(row.ledger_before, row.expected_migrations); assert.equal(row.ledger_after, row.expected_migrations)
      } }
    catch { report.result = 'FAIL'; report.cleanup = 'FAIL: owned stack cleanup not proved' }
  }
  process.removeListener('SIGINT', stopSignal); process.removeListener('SIGTERM', stopSignal)
  if (report.result !== 'PASS') report.currentCandidateGate = 'NOT_PROVEN'
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n', { mode: 0o600 })
}
console.log(JSON.stringify({ result: report.result, failureStage: report.failureStage, historical: values.historical, checks: Object.keys(checks).length, cleanup: report.cleanup }))
process.exitCode = report.result === 'PASS' ? 0 : 1
