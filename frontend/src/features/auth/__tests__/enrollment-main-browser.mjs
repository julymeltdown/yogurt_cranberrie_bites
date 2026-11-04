// Actual main-entry Auth/Gateway/mail pairing. This runner starts no DB/service/build.
// World acceptance remains open until a coherent real concert producer is paired.
import assert from 'node:assert/strict'
import { mkdir, readFile, readdir, writeFile, link as linkFile, unlink } from 'node:fs/promises'
import path from 'node:path'
import { createServer } from 'vite'
import { chromium } from '@playwright/test'
const evidence = process.env.MCR_FE10_EVIDENCE, serving = process.env.MCR_FE10_BACKEND_OUTPUT
assert(evidence && serving, 'Allocated private evidence and independently owned backend output required')
const ready = JSON.parse(await readFile(path.join(serving, 'browser-ready.json'), 'utf8'))
assert.equal(ready.frontendOrigin, 'http://localhost:3000'); assert.equal(ready.syntheticPolicyOnly, true)
assert.equal(ready.authOnly, true, 'No fixture foundation/bootstrap success may substitute for real World producers')
assert.match(ready.gatewayOrigin, /^http:\/\/localhost:\d+$/)
assert.match(ready.gatewayLoopbackOrigin, /^http:\/\/127\.0\.0\.1:\d+$/)
assert.equal(new URL(ready.gatewayOrigin).port, new URL(ready.gatewayLoopbackOrigin).port)
assert.match(ready.binarySha256, /^[a-f0-9]{64}$/); assert.match(ready.sourceRevision, /^[a-f0-9]{40}$/)
assert.equal(ready.binarySha256, process.env.MCR_FE10_BACKEND_BINARY_SHA, 'Fresh paired binary pin required')
assert.equal(ready.sourceRevision, process.env.MCR_FE10_BACKEND_SOURCE, 'Fresh paired source pin required')
for (const service of ['auth-service', 'api-gateway']) {
  assert(path.isAbsolute(ready.runtimeMigrationPaths[service]))
  assert(Object.keys(ready.runtimeMigrationSha256[service]).length > 0)
  for (const digest of Object.values(ready.runtimeMigrationSha256[service])) assert.match(digest, /^[a-f0-9]{64}$/)
}
assert.match(ready.policyRevision, /^[a-f0-9-]{36}$/)
assert.equal(ready.verdict, 'SERVING_FIXTURE_ONLY_BROWSER_ACCEPTANCE_REMAINS_FE_OWNED')
const eventId = process.env.MCR_FE10_EVENT_ID
assert.match(eventId ?? '', /^[a-f0-9-]{36}$/, 'Explicit event only; does not fabricate a concert producer')
await mkdir(evidence, { recursive: true })
const suffix = crypto.randomUUID(), email = `fe10-${suffix}@example.invalid`, password = `  Synthetic-${suffix}  `
const calls = [], decisions = [], wireOutcomes = [], errors = [], results = []
let browser, dev, sockets = 0
const boundedEnd = Date.now() + 150_000
async function mailFor(attemptId, afterCapture = -1) {
  while (Date.now() < boundedEnd) {
    const captures = (await readdir(serving)).flatMap(file => {
      const match = /^browser-mail-(\d+)\.json$/.exec(file)
      return match && Number.isSafeInteger(Number(match[1])) && Number(match[1]) > afterCapture ? [{ file, index: Number(match[1]) }] : []
    }).sort((a, b) => b.index - a.index)
    for (const { file, index } of captures) {
      const mail = JSON.parse(await readFile(path.join(serving, file), 'utf8'))
      if (mail.attemptId === attemptId) return { mail, index }
    }
    await new Promise(resolve => setTimeout(resolve, 100))
  }
  throw new Error('Real mail-worker capture was not available inside the original bound')
}
async function control(name, body) {
  assert(['browser-expire.json', 'browser-rotate.json'].includes(name))
  const temporary = path.join(serving, `.fe204-${name}-${crypto.randomUUID()}.tmp`)
  await writeFile(temporary, JSON.stringify(body) + '\n', { flag: 'wx', mode: 0o600 })
  // Atomic no-replace publication. Only these exact private test inputs are writable.
  try { await linkFile(temporary, path.join(serving, name)) } finally { await unlink(temporary) }
}
async function receipt(name) {
  assert(['browser-challenge-expired.json', 'browser-policy-rotated.json'].includes(name))
  while (Date.now() < boundedEnd) {
    try { return JSON.parse(await readFile(path.join(serving, name), 'utf8')) }
    catch (error) { if (error.code !== 'ENOENT' && !(error instanceof SyntaxError)) throw error }
    await new Promise(resolve => setTimeout(resolve, 100))
  }
  throw new Error(`Missing bounded private fixture receipt: ${name}`)
}
async function submitVerification(page, expectedStatus) {
  const pending = page.waitForResponse(r => new URL(r.url()).pathname === '/api/auth/enrollment/verify' && r.request().method() === 'POST')
  await page.getByRole('button', { name: 'Verify email', exact: true }).click()
  const response = await pending; assert.equal(response.status(), expectedStatus)
  return response.json()
}
async function resendDisabled(page) {
  await page.waitForFunction(() => [...document.querySelectorAll('button')].some(button => button.textContent === 'Request another code' && button.disabled))
}
async function resend(page) {
  const pending = page.waitForResponse(r => new URL(r.url()).pathname === '/api/auth/enrollment/resend' && r.request().method() === 'POST')
  await page.getByRole('button', { name: 'Request another code' }).click()
  const response = await pending; assert.equal(response.status(), 202)
  return response.json()
}

function observe(page) {
  page.on('pageerror', error => errors.push(String(error)))
  page.on('websocket', () => { sockets += 1 })
  page.on('request', request => {
    const url = new URL(request.url())
    if (url.pathname.startsWith('/api/')) calls.push({ path: url.pathname, method: request.method() })
  })
  page.on('response', async response => {
    const url = new URL(response.url())
    if (!url.pathname.startsWith('/api/')) return
    wireOutcomes.push({ path: url.pathname, method: response.request().method(), status: response.status() })
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
      proxy: { '/api': { target: ready.gatewayLoopbackOrigin, changeOrigin: false } } } })
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
  const signupResponse = page.waitForResponse(r => new URL(r.url()).pathname === '/api/auth/enrollment/signup' && r.request().method() === 'POST')
  await page.getByRole('form', { name: 'Create account' }).evaluate(form => {
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
  })
  await page.getByText(/Account creation and email delivery are not confirmed/).waitFor()
  const signupReceipt = await (await signupResponse).json()
  assert.equal(calls.filter(c => c.path.endsWith('/enrollment/signup')).length, 1)
  assert.equal(calls.filter(c => c.path.endsWith('/enrollment/attempts')).length, 1)
  await clean(page); await page.screenshot({ path: path.join(evidence, 'main-signup-accepted-390.png') })
  await page.getByRole('link', { name: 'Continue to email verification' }).click()
  const attemptId = await page.getByLabel('Signup attempt').inputValue(); assert.match(attemptId, /^[a-f0-9-]{36}$/)
  const captured = await mailFor(attemptId); let mail = captured.mail, link = new URL(mail.verificationLink)
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
  // The private fixture may only expire a current challenge produced by this real signup.
  await control('browser-expire.json', { attemptId })
  const expired = await receipt('browser-challenge-expired.json')
  assert.equal(expired.attemptId, attemptId); assert.equal(expired.negativeOnly, true)
  assert(Number.isFinite(Date.parse(expired.actualChallengeExpiresAt)) && Date.parse(expired.actualChallengeExpiresAt) < Date.parse(expired.serverTime))
  await page.goto(link.href); await page.waitForURL('http://localhost:3000/verify-email')
  assert.equal(await page.getByLabel('Signup attempt').inputValue(), attemptId)
  assert.equal(await page.getByLabel('Verification code').inputValue(), mail.verificationCode)
  const rejection = await submitVerification(page, 422)
  assert.equal(rejection.details.reason, 'VERIFICATION_FAILED')
  await page.getByText(/It may be invalid, expired or already used/).waitFor()
  assert.equal(await page.getByText('Email verified. Sign in to continue.').count(), 0)
  await clean(page, mail.verificationCode)
  await page.screenshot({ path: path.join(evidence, 'main-expired-code-rejected-390.png') })
  results.push({ case: 'actual-expired-challenge-cannot-verify-negative-only-fixture', passed: true })
  const beforeResend = calls.filter(c => c.path.endsWith('/enrollment/resend')).length
  const firstResend = await resend(page)
  assert.equal(firstResend.attemptId, attemptId)
  assert(Date.parse(firstResend.expiresAt) <= Date.parse(signupReceipt.expiresAt))
  assert(Date.parse(firstResend.resendAvailableAt) > Date.parse(firstResend.serverTime))
  assert.equal(firstResend.resendAvailableAt, signupReceipt.resendAvailableAt, 'This case requires a real predeadline neutral resend')
  await resendDisabled(page)
  // Predeadline is neutral202 without new mail. Wait the actual displayed deadline;
  // no automatic POST or artificial production TTL change.
  await page.waitForFunction(() => [...document.querySelectorAll('button')].some(b => b.textContent === 'Request another code' && !b.disabled), undefined, { timeout: 70_000 })
  await new Promise(resolve => setTimeout(resolve, 350))
  assert.equal(calls.filter(c => c.path.endsWith('/enrollment/resend')).length, beforeResend + 1)
  for (const file of await readdir(serving)) {
    const match = /^browser-mail-(\d+)\.json$/.exec(file)
    if (match && Number(match[1]) > captured.index) {
      const extra = JSON.parse(await readFile(path.join(serving, file), 'utf8'))
      assert.notEqual(extra.attemptId, attemptId, 'Predeadline resend unexpectedly dispatched mail')
    }
  }
  const fresh = await resend(page)
  assert(Date.parse(fresh.resendAvailableAt) > Date.parse(firstResend.resendAvailableAt))
  assert(Date.parse(fresh.expiresAt) <= Date.parse(signupReceipt.expiresAt))
  await resendDisabled(page)
  results.push({ case: 'real-server-resend-deadline-without-automatic-replay', passed: true })
  const latest = await mailFor(attemptId, captured.index); mail = latest.mail; link = new URL(mail.verificationLink)
  assert.equal(link.origin, 'http://localhost:3000'); assert.equal(link.pathname, '/verify-email'); assert.equal(link.hash, '')
  assert.match(mail.verificationCode, /^\d{6}$/)
  assert(Date.parse(mail.expiresAt) > Date.parse(expired.serverTime))
  assert.deepEqual([...link.searchParams.keys()].sort(), ['attemptId', 'code'])
  assert.equal(link.searchParams.get('attemptId'), attemptId); assert.equal(link.searchParams.get('code'), mail.verificationCode)
  await page.goto(link.href); await page.waitForURL('http://localhost:3000/verify-email')
  const verified = await submitVerification(page, 200); assert.equal(verified.status, 'VERIFIED')
  await page.getByText('Email verified. Sign in to continue.').waitFor(); await clean(page, mail.verificationCode)
  await page.screenshot({ path: path.join(evidence, 'main-mail-link-verified-390.png') })
  results.push({ case: 'actual-main-signup-mailworker-latest-bound-link-verification', passed: true })
  await control('browser-rotate.json', { expectedRevision: ready.policyRevision })
  const rotated = await receipt('browser-policy-rotated.json')
  assert.equal(rotated.previousRevision, ready.policyRevision); assert.equal(rotated.syntheticPolicyOnly, true)
  assert.match(rotated.policyRevision, /^[a-f0-9-]{36}$/); assert.notEqual(rotated.policyRevision, ready.policyRevision)
  assert(Array.isArray(rotated.requiredAcknowledgementIds) && rotated.requiredAcknowledgementIds.length > 0)
  await page.getByRole('link', { name: 'Sign in', exact: true }).click()
  await page.getByLabel('Email', { exact: true }).fill(email); await page.getByLabel('Password', { exact: true }).fill(password)
  await page.getByRole('form', { name: 'Email sign-in' }).evaluate(form => {
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
  })
  await page.getByRole('heading', { name: 'Review current agreements' }).waitFor()
  assert(decisions.some(d => d.status === 200 && d.decision === 'CONSENT_REQUIRED' && d.policyRevision === rotated.policyRevision))
  assert(!calls.some(c => c.path.endsWith('/bootstrap')), 'Bootstrap dispatched before required current consent')
  await page.getByRole('button', { name: 'Load current agreements' }).click()
  await page.getByRole('checkbox').first().waitFor()
  assert((await page.getByRole('group', { name: 'Required agreements' }).innerText()).includes(rotated.policyRevision))
  assert.equal(await page.getByRole('checkbox').count(), rotated.requiredAcknowledgementIds.length)
  for (const checkbox of await page.getByRole('checkbox').all()) await checkbox.check()
  await clean(page, mail.verificationCode)
  await page.screenshot({ path: path.join(evidence, 'main-current-consent-required-390.png') })
  const consentResponse = page.waitForResponse(r => new URL(r.url()).pathname === '/api/auth/enrollment/consent' && r.request().method() === 'POST')
  await page.getByRole('button', { name: 'Agree and continue' }).evaluate(button => { button.click(); button.click() })
  const acceptedConsent = await consentResponse
  assert.equal(acceptedConsent.status(), 200)
  const consentCommand = acceptedConsent.request().postDataJSON()
  assert.equal(consentCommand.policyRevision, rotated.policyRevision)
  assert.deepEqual([...consentCommand.acknowledgementIds].sort(), [...rotated.requiredAcknowledgementIds].sort())
  await page.getByRole('link', { name: 'Continue', exact: true }).waitFor()
  assert.equal(calls.filter(c => c.path.endsWith('/enrollment/consent')).length, 1)
  assert(!calls.some(c => c.path.endsWith('/bootstrap')), 'Private destination opened before explicit continuation')
  await page.getByRole('link', { name: 'Continue', exact: true }).click()
  await page.waitForURL('http://localhost:3000/world')
  await page.getByRole('heading', { name: 'Your event is unavailable', exact: true }).waitFor()
  assert(wireOutcomes.some(r => r.path === `/api/concert/events/${eventId}/bootstrap` && r.status === 503), 'Auth-only fixture must deny real bootstrap')
  assert.equal(await page.locator('canvas').count(), 0); assert.equal(sockets, 0)
  assert.equal(calls.filter(c => c.path === '/api/auth/login').length, 1)
  assert(decisions.some(d => d.status === 200 && d.decision === 'ADMITTED' && d.policyRevision === rotated.policyRevision && d.acceptedRevision === rotated.policyRevision))
  await clean(page, mail.verificationCode); await page.screenshot({ path: path.join(evidence, 'main-authorized-world-continuation-pending-390.png') })
  results.push({ case: 'canonical-login-renewed-explicit-consent-and-real-current-admission', passed: true })
  assert.deepEqual(errors, [])
  await writeFile(path.join(evidence, 'main-browser-report.json'), JSON.stringify({
    qualification: 'Actual main entry; real Auth RPC/Gateway/private DB and canonical mail-worker local capture; explicit synthetic test policy only',
    frontendOrigin: ready.frontendOrigin, backendOrigin: ready.gatewayOrigin, upstream: ready.gatewayLoopbackOrigin,
    backendBinarySha256: ready.binarySha256, backendSourceRevision: ready.sourceRevision, runtimeMigrationSha256: ready.runtimeMigrationSha256, initialPolicyRevision: ready.policyRevision, policyRevision: rotated.policyRevision,
    node: process.version, browser: browser.version(), results, calls, wireOutcomes, decisions, errors, sockets,
    worldContinuation: 'OPEN: Auth-only fixture does not contain coherent Concert/Profile/Friend producers; no World/canvas/socket success claimed',
    externalMail: 'NOT_RUN', productionLegalPublication: 'ABSENT_NOT_CLAIMED'
  }, null, 2) + '\n')
  await context.close()
} finally { await browser?.close(); await dev?.close() }
