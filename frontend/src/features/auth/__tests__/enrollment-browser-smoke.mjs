// Controlled feature qualification through canonical Auth/HTTP; not real Auth/mail/legal evidence.
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { createServer } from 'vite'
import { chromium } from '@playwright/test'
const evidence = process.env.MCR_FE10_EVIDENCE
assert(evidence, 'Explicit private evidence directory required')
await mkdir(evidence, { recursive: true })
const capabilities = JSON.parse(await readFile('../contracts/fixtures/core/auth-capabilities-disabled.json', 'utf8'))
capabilities.providers[0] = { provider: 'EMAIL', enabled: true, disabled_reason: null }
const revision = '11111111-1111-4111-8111-111111111111', attemptId = '22222222-2222-4222-8222-222222222222'
const email = 'synthetic-fe10@example.invalid', password = '  SYNTHETIC_FE10_PASSWORD  ', code = '938271'
const calls = { policy: 0, attempts: 0, signup: 0, verify: 0, resend: 0 }, results = [], errors = [], signupBodies = []
let mode = 'success', browser, attemptExpiry = new Date(Date.now() + 600_000).toISOString()
const stamp = () => { const time = Date.now(); return { serverTime: new Date(time).toISOString(), expiresAt: new Date(time + 600_000).toISOString() } }
const accepted = () => { const t = stamp(); return { status: 'ACCEPTED', attemptId, ...t, expiresAt: attemptExpiry, resendAvailableAt: new Date(Date.parse(t.serverTime) + 3000).toISOString() } }
const dev = await createServer({ cacheDir: path.join(evidence, 'vite-cache'), server: { host: '127.0.0.1', port: 0, strictPort: false },
  plugins: [{ name: 'fe10-explicit-synthetic-http', configureServer(server) {
    server.middlewares.use(async (req, res, next) => {
      if (!req.url?.startsWith('/api/')) return next()
      res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store')
      const send = (status, body) => { if (!res.destroyed) { res.statusCode = status; res.end(JSON.stringify(body)) } }
      try {
        if (req.url === '/api/auth/capabilities') return send(200, capabilities)
        const key = Object.keys(calls).find(key => req.url === `/api/auth/enrollment/${key}`)
        assert(key, `unexpected API ${req.url}`); calls[key]++
        assert.equal(req.headers.authorization, undefined); assert.equal(req.headers['idempotency-key'], undefined)
        assert.equal(req.headers.referer, undefined)
        if (req.method === 'POST') { assert.equal(req.headers.origin, `http://${req.headers.host}`); assert.equal(req.headers['x-mcr-intent'], 'browser') }
        if (key === 'policy') {
          if (mode === 'no-policy') return send(503, { code: 'UNAVAILABLE', message: 'PRIVATE_SERVER_TEXT', details: { reason: 'POLICY_UNAVAILABLE' } })
          const time = stamp(); return send(200, { contractVersion: '1.0', revision,
            documents: [{ id: 'test-terms', title: 'Synthetic test agreements', url: 'https://example.invalid/test-terms', sha256: 'a'.repeat(64), requiredAcknowledgementId: 'test-terms-v1' }],
            publishedAt: new Date(Date.parse(time.serverTime) - 1000).toISOString(), ...time })
        }
        const chunks = []; for await (const chunk of req) chunks.push(chunk)
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8'))
        if (key === 'attempts') {
          assert.deepEqual(body, { policyRevision: revision }); res.setHeader('Set-Cookie', 'mcr-test-binding=synthetic; HttpOnly; SameSite=Strict; Path=/api/auth/enrollment')
          const t = stamp(); attemptExpiry = t.expiresAt; return send(201, { attemptId, policyRevision: revision, ...t })
        }
        assert(req.headers.cookie?.includes('mcr-test-binding=synthetic'), 'same-browser attempt cookie required')
        assert.equal(body.attemptId, attemptId)
        if (key === 'signup') {
          assert.deepEqual(Object.keys(body).sort(), ['acknowledgementIds','attemptId','email','idempotencyKey','password','policyRevision'])
          assert.equal(body.email, email); assert.equal(body.password, password); assert.equal(body.policyRevision, revision)
          assert.deepEqual(body.acknowledgementIds, ['test-terms-v1']); assert.match(body.idempotencyKey, /^[0-9a-f-]{36}$/)
          signupBodies.push(body)
          await new Promise(resolve => setTimeout(resolve, 120))
          if (mode === 'unknown') return send(503, { code: 'DOWN', message: 'PRIVATE_SERVER_TEXT', details: { reason: 'DEPENDENCY_UNAVAILABLE' } })
          return send(202, accepted())
        }
        if (key === 'verify') {
          assert.deepEqual(Object.keys(body).sort(), ['attemptId','code']); assert.equal(body.code, code)
          if (mode === 'expired') return send(410, { code: 'EXPIRED', message: 'PRIVATE_SERVER_TEXT', details: { reason: 'ATTEMPT_EXPIRED' } })
          return send(200, { status: 'VERIFIED', attemptId, serverTime: stamp().serverTime })
        }
        assert.equal(key, 'resend'); assert.deepEqual(Object.keys(body).sort(), ['attemptId','idempotencyKey']); return send(202, accepted())
      } catch (error) { errors.push(String(error)); return send(500, { code: 'TEST_FAILURE', message: 'controlled test failed' }) }
    })
  } }] })
async function hygiene(page) {
  const snapshot = await page.evaluate(() => ({ html: document.body.innerHTML, url: location.href, state: JSON.stringify(history.state),
    local: JSON.stringify(localStorage), session: JSON.stringify(sessionStorage), cookie: document.cookie }))
  for (const [key, value] of Object.entries(snapshot)) for (const marker of [password, code, 'PRIVATE_SERVER_TEXT', 'mcr-test-binding']) assert(!value.includes(marker), `secret in ${key}`)
  assert.equal(await page.locator('canvas,[data-world-host]').count(), 0)
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
}
async function signup(page) {
  await page.getByRole('button', { name: 'Load current agreements' }).click()
  await page.getByRole('checkbox').check(); await page.getByLabel('Email', { exact: true }).fill(email); await page.getByLabel('Password', { exact: true }).fill(password)
  const before = calls.signup
  await page.getByRole('form', { name: 'Create account' }).evaluate(form => { form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })) })
  await page.waitForFunction(() => document.querySelector('#enrollment-password')?.value !== '  SYNTHETIC_FE10_PASSWORD  ')
  return before
}
try {
  await dev.listen(); const base = dev.resolvedUrls.local[0]
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--disable-background-networking', '--disable-component-update'] })
  for (const width of [360,390,430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => errors.push(String(error)))
    mode = 'success'; await page.goto(`${base}src/features/auth/__tests__/fixtures/enrollment-preview.html`)
    await page.getByRole('button', { name: 'Create account' }).waitFor()
    assert(await page.getByRole('button', { name: 'Create account' }).isDisabled())
    const before = await signup(page)
    await page.getByText(/Account creation and email delivery are not confirmed/).waitFor(); assert.equal(calls.signup, before + 1)
    await hygiene(page); await page.screenshot({ path: path.join(evidence, `accepted-${width}.png`) })
    await page.getByRole('link', { name: 'Continue to email verification' }).click()
    assert.equal(await page.getByLabel('Signup attempt').inputValue(), attemptId)
    assert.equal(await page.getByLabel('Email', { exact: true }).count(), 0)
    if (width === 390) {
      assert(await page.getByRole('button', { name: 'Request another code' }).isDisabled())
      await page.waitForFunction(() => [...document.querySelectorAll('button')].some(button => button.textContent === 'Request another code' && !button.disabled))
      const resendBefore = calls.resend; await page.getByRole('button', { name: 'Request another code' }).click()
      await page.getByText('Request accepted. Email delivery is not confirmed. Use the latest code if instructions arrive.').waitFor()
      assert.equal(calls.resend, resendBefore + 1); results.push({ case: 'authoritative-resend-countdown', passed: true })
    }
    await page.getByLabel('Verification code').fill(code); await page.getByRole('button', { name: 'Verify email', exact: true }).click()
    await page.getByText('Email verified. Sign in to continue.').waitFor(); await hygiene(page)
    assert.equal(await page.evaluate(() => window.__FE10_TEST__.hasAccess()), false)
    await page.screenshot({ path: path.join(evidence, `verified-${width}.png`) })
    results.push({ width, case: 'bound signup-double-submit-verification', passed: true }); await context.close()
  }
  const context = await browser.newContext({ viewport: { width:390,height:844 } }), page = await context.newPage()
  mode = 'unknown'; await page.goto(`${base}src/features/auth/__tests__/fixtures/enrollment-preview.html`); await signup(page)
  await page.getByRole('button', { name: 'Retry original request' }).waitFor(); const original = signupBodies.at(-1)
  mode = 'success'; await page.getByRole('button', { name: 'Retry original request' }).click(); await page.getByText(/Account creation and email delivery are not confirmed/).waitFor()
  assert.deepEqual(signupBodies.at(-1), original); await hygiene(page); results.push({ case: 'unknown-explicit-identical-retry', passed:true })
  mode = 'expired'; await page.getByRole('link', { name: 'Continue to email verification' }).click(); await page.getByLabel('Verification code').fill(code)
  await page.getByRole('button', { name:'Verify email',exact:true }).click(); await page.getByText('This signup attempt expired. Start again with the current published agreements.').waitFor()
  await hygiene(page); results.push({ case:'expired-attempt-no-completion',passed:true })
  mode = 'no-policy'; await page.goto(`${base}src/features/auth/__tests__/fixtures/enrollment-preview.html`)
  await page.getByRole('button', { name:'Load current agreements' }).click(); await page.getByText('Signup agreements are unavailable. Account creation cannot continue until the approved documents are published.').first().waitFor()
  assert.equal(await page.getByRole('checkbox').count(),0); assert(await page.getByRole('button',{name:'Create account'}).isDisabled()); await hygiene(page)
  results.push({ case:'unpublished-policy-fail-closed',passed:true }); await context.close()
  assert.deepEqual(errors,[])
  await writeFile(path.join(evidence,'report.json'),JSON.stringify({ qualification:'controlled fixtures through canonical client; no real Auth/mail/legal publication',results,calls,errors },null,2)+'\n')
} finally { await browser?.close(); await dev.close() }
