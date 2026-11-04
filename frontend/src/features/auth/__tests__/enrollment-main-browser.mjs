// Actual main-entry Auth/Gateway/mail pairing. This runner starts no DB/service/build.
// World acceptance remains open until a coherent real concert producer is paired.
import assert from 'node:assert/strict'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { createServer } from 'vite'
import { chromium } from '@playwright/test'
const evidence = process.env.MCR_FE10_EVIDENCE, serving = process.env.MCR_FE10_BACKEND_OUTPUT
assert(evidence && serving, 'Allocated private evidence and independently owned backend output required')
const ready = JSON.parse(await readFile(path.join(serving, 'browser-ready.json'), 'utf8'))
assert.equal(ready.frontendOrigin, 'http://localhost:3000'); assert.equal(ready.syntheticPolicyOnly, true)
assert.match(ready.gatewayOrigin, /^http:\/\/localhost:\d+$/); assert.match(ready.policyRevision, /^[a-f0-9-]{36}$/)
assert.equal(ready.verdict, 'SERVING_FIXTURE_ONLY_BROWSER_ACCEPTANCE_REMAINS_FE_OWNED')
const eventId = process.env.MCR_FE10_EVENT_ID
assert.match(eventId ?? '', /^[a-f0-9-]{36}$/, 'Explicit event only; does not fabricate a concert producer')
await mkdir(evidence, { recursive: true })
const suffix = crypto.randomUUID(), email = `fe10-${suffix}@example.invalid`, password = `  Synthetic-${suffix}  `
const calls = [], decisions = [], errors = [], results = []
let browser, dev
const boundedEnd = Date.now() + 150_000
async function mailFor(attemptId) {
  while (Date.now() < boundedEnd) {
    for (const file of (await readdir(serving)).filter(f => /^browser-mail-\d+\.json$/.test(f))) {
      const mail = JSON.parse(await readFile(path.join(serving, file), 'utf8'))
      if (mail.attemptId === attemptId) return mail
    }
    await new Promise(resolve => setTimeout(resolve, 100))
  }
  throw new Error('Real mail-worker capture was not available inside the original bound')
}
function observe(page) {
  page.on('pageerror', error => errors.push(String(error)))
  page.on('request', request => {
    const url = new URL(request.url())
    if (url.pathname.startsWith('/api/')) calls.push({ path: url.pathname, method: request.method() })
  })
  page.on('response', async response => {
    const url = new URL(response.url())
    if (!url.pathname.endsWith('/api/auth/enrollment/admission')) return
    try {
      const receipt = await response.json()
      decisions.push({ status: response.status(), decision: receipt.decision, policyRevision: receipt.policyRevision, acceptedRevision: receipt.acceptedRevision })
    } catch { errors.push('Malformed real admission response') }
  })
}
async function clean(page, code = '') {
  const snapshot = await page.evaluate(() => ({ html: document.body.innerHTML, url: location.href,
    state: JSON.stringify(history.state), local: JSON.stringify(localStorage), session: JSON.stringify(sessionStorage), cookie: document.cookie }))
  for (const [field, value] of Object.entries(snapshot)) {
    for (const secret of [password, ...(code ? [code] : [])]) assert(!value.includes(secret), `Secret retained in ${field}`)
    assert(!/accessToken|refreshToken/.test(value), `Credential serialization in ${field}`)
  }
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Horizontal overflow')
}
try {
  dev = await createServer({ configLoader: 'runner', cacheDir: process.env.MCR_FE10_CACHE,
    define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(eventId) },
    server: { host: 'localhost', port: 3000, strictPort: true,
      proxy: { '/api': { target: ready.gatewayOrigin, changeOrigin: false } } } })
  await dev.listen()
  browser = await chromium.launch({ channel: 'chrome', headless: true,
    args: ['--disable-background-networking', '--disable-component-update'] })
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage()
  observe(page); await page.goto('http://localhost:3000/signup')
  await page.getByRole('heading', { name: 'Create account' }).waitFor()
  assert(await page.getByRole('button', { name: 'Create account' }).isDisabled())
  await page.getByRole('button', { name: 'Load current agreements' }).click()
  await page.getByRole('checkbox').first().waitFor()
  for (const checkbox of await page.getByRole('checkbox').all()) await checkbox.check()
  await page.getByLabel('Email', { exact: true }).fill(email); await page.getByLabel('Password', { exact: true }).fill(password)
  await page.getByRole('form', { name: 'Create account' }).evaluate(form => {
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
  })
  await page.getByText(/Account creation and email delivery are not confirmed/).waitFor()
  assert.equal(calls.filter(c => c.path.endsWith('/enrollment/signup')).length, 1)
  assert.equal(calls.filter(c => c.path.endsWith('/enrollment/attempts')).length, 1)
  await clean(page); await page.screenshot({ path: path.join(evidence, 'main-signup-accepted-390.png') })
  await page.getByRole('link', { name: 'Continue to email verification' }).click()
  const attemptId = await page.getByLabel('Signup attempt').inputValue(); assert.match(attemptId, /^[a-f0-9-]{36}$/)
  const mail = await mailFor(attemptId), link = new URL(mail.verificationLink)
  assert.equal(link.origin, 'http://localhost:3000'); assert.equal(link.pathname, '/verify-email'); assert.equal(link.hash, '')
  assert.deepEqual([...link.searchParams.keys()].sort(), ['attemptId', 'code'])
  assert.equal(link.searchParams.get('attemptId'), attemptId); assert.equal(link.searchParams.get('code'), mail.verificationCode)
  assert.match(mail.verificationCode, /^\d{6}$/)
  const cookies = await context.cookies('http://localhost:3000/api/auth/enrollment/verify')
  const binding = cookies.find(cookie => cookie.path === '/api/auth/enrollment' && cookie.name.includes('enrollment'))
  assert(binding?.httpOnly); assert.equal(binding.sameSite, 'Strict')
  // A second browser has no bound capability. Its real denial must not verify the account.
  const other = await browser.newContext({ viewport: { width: 360, height: 844 } }), foreign = await other.newPage()
  await foreign.goto(link.href); await foreign.waitForURL('http://localhost:3000/verify-email')
  await foreign.getByRole('button', { name: 'Verify email', exact: true }).click()
  await foreign.getByText(/This attempt is unavailable in this browser/).waitFor(); await clean(foreign, mail.verificationCode)
  assert.equal(await foreign.getByText('Email verified. Sign in to continue.').count(), 0); await other.close()
  results.push({ case: 'real-bound-cookie-rejects-second-browser', passed: true })
  await page.goto(link.href); await page.waitForURL('http://localhost:3000/verify-email')
  assert.equal(await page.getByLabel('Signup attempt').inputValue(), attemptId)
  assert.equal(await page.getByLabel('Verification code').inputValue(), mail.verificationCode)
  await page.getByRole('button', { name: 'Verify email', exact: true }).click()
  await page.getByText('Email verified. Sign in to continue.').waitFor(); await clean(page, mail.verificationCode)
  await page.screenshot({ path: path.join(evidence, 'main-mail-link-verified-390.png') })
  results.push({ case: 'actual-main-signup-mailworker-bound-link-verification', passed: true })
  await page.getByRole('link', { name: 'Sign in', exact: true }).click()
  await page.getByLabel('Email', { exact: true }).fill(email); await page.getByLabel('Password', { exact: true }).fill(password)
  await page.getByRole('form', { name: 'Email sign-in' }).evaluate(form => {
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
  })
  await page.waitForURL('http://localhost:3000/world')
  await page.waitForFunction(() => ![...document.querySelectorAll('[role="status"],h1')].some(e => /Opening your event|Checking account entry/.test(e.textContent)))
  assert.equal(calls.filter(c => c.path === '/api/auth/login').length, 1)
  assert(decisions.some(d => d.status === 200 && d.decision === 'ADMITTED' && d.policyRevision === ready.policyRevision && d.acceptedRevision === ready.policyRevision))
  await clean(page, mail.verificationCode); await page.screenshot({ path: path.join(evidence, 'main-authorized-world-continuation-pending-390.png') })
  results.push({ case: 'canonical-sign-in-and-real-current-admission', passed: true })
  assert.deepEqual(errors, [])
  await writeFile(path.join(evidence, 'main-browser-report.json'), JSON.stringify({
    qualification: 'Actual main entry; real Auth RPC/Gateway/private DB and canonical mail-worker local capture; explicit synthetic test policy only',
    frontendOrigin: ready.frontendOrigin, backendOrigin: ready.gatewayOrigin, policyRevision: ready.policyRevision,
    node: process.version, browser: browser.version(), results, calls, decisions, errors,
    worldContinuation: 'OPEN: Auth-only fixture does not contain coherent Concert/Profile/Friend producers; no World/canvas/socket success claimed',
    externalMail: 'NOT_RUN', productionLegalPublication: 'ABSENT_NOT_CLAIMED'
  }, null, 2) + '\n')
  await context.close()
} finally { await browser?.close(); await dev?.close() }
