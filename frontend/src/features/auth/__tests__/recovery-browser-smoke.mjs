// Installed Chrome only; owned ephemeral servers and contexts, never a user profile.
import assert from 'node:assert/strict'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_FE11_EVIDENCE ?? '/tmp/mcr-fe11-evidence'
await mkdir(evidence, { recursive: true })
const authentication = JSON.parse(await readFile('../contracts/fixtures/core/auth-success.json', 'utf8'))
const capabilities = JSON.parse(await readFile('../contracts/fixtures/core/auth-capabilities-disabled.json', 'utf8'))
capabilities.providers = capabilities.providers.map(p => p.provider === 'EMAIL' ? { ...p, enabled: true, disabled_reason: null } : p)
const token = 'R'.repeat(43), password = '  SYNTHETIC_FE11_NEW_PASSWORD  '
const calls = { request: 0, confirm: 0, login: 0, logout: 0, refresh: 0, active: 0, peak: 0 }
let mode = 'success', production, browser
const results = [], errors = []
let transportDrop = null
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'fe11-owned-synthetic-http', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/api/')) return next()
    res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store')
    const send = (status, body) => { if (!res.destroyed) { res.statusCode = status; res.end(body === undefined ? undefined : JSON.stringify(body)) } }
    if (req.url === '/api/auth/capabilities') return send(200, capabilities)
    assert.equal(req.headers.origin, `http://${req.headers.host}`); assert.equal(req.headers['x-mcr-intent'], 'browser')
    assert.equal(req.headers.referer, undefined)
    const chunks = []; for await (const chunk of req) chunks.push(chunk)
    const body = chunks.length ? JSON.parse(Buffer.concat(chunks).toString('utf8')) : null
    if (req.url.endsWith('/reset/request')) {
      calls.request++; assert.deepEqual(Object.keys(body), ['email'])
      await new Promise(resolve => setTimeout(resolve, 130))
      if (body.email === 'throttle@example.com') return send(429, { code: 'RATE', message: 'PRIVATE_FE11_RAW_ERROR' })
      return send(200, { status: 'SENT' })
    }
    if (req.url.endsWith('/refresh')) { calls.refresh++; return send(503, {}) }
    calls.active++; calls.peak = Math.max(calls.peak, calls.active)
    try {
      if (req.url.endsWith('/login')) {
        calls.login++; await new Promise(resolve => setTimeout(resolve, 80))
        res.setHeader('Set-Cookie', 'fe11_synthetic=active; HttpOnly; SameSite=Lax; Path=/')
        return send(200, authentication)
      }
      if (req.url.endsWith('/logout')) {
        calls.logout++; res.setHeader('Set-Cookie', 'fe11_synthetic=; Max-Age=0; HttpOnly; SameSite=Lax; Path=/')
        return send(204)
      }
      if (req.url.endsWith('/reset/confirm')) {
        calls.confirm++; assert.deepEqual(Object.keys(body).sort(), ['newPassword', 'token'])
        assert.equal(Buffer.byteLength(body.newPassword, 'utf8') >= 8 && Buffer.byteLength(body.newPassword, 'utf8') <= 128, true)
        const selected = mode
        await new Promise(resolve => setTimeout(resolve, selected === 'delay' ? 450 : 150))
        if (selected === 'network') { req.socket.destroy(); return }
        if (selected === 'server-error') return send(503, { code: 'UNAVAILABLE', message: 'PRIVATE_FE11_RAW_ERROR' })
        if (selected === 'expired') return send(400, { code: 'GATEWAY_400', message: 'PRIVATE_FE11_RAW_ERROR' })
        assert.equal(body.token, token); assert.equal(body.newPassword, password)
        return send(200, { status: 'RESET' })
      }
      return send(404, {})
    } finally { calls.active-- }
  })
} }] })
async function hygiene(page) {
  const snapshot = await page.evaluate(() => ({ html: document.body.innerHTML, url: location.href, state: JSON.stringify(history.state),
    local: JSON.stringify(localStorage), session: JSON.stringify(sessionStorage), cookie: document.cookie }))
  for (const [key, value] of Object.entries(snapshot)) for (const marker of [token, password, 'PRIVATE_FE11_RAW_ERROR', 'SYNTHETIC_FE11_CODE', 'SYNTHETIC_FE11_STATE']) {
    assert(!value.includes(marker), `secret reflected in ${key}`)
  }
  assert.equal(await page.locator('canvas,[data-world-host]').count(), 0)
}
async function layout(page) {
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
  for (const control of await page.locator('.auth-recovery a,.auth-recovery button,.auth-recovery input').all()) {
    const box = await control.boundingBox(); assert(box && box.width >= 44 && box.height >= 44, 'control below44px')
  }
  const failures = await page.evaluate(() => {
    const rgb = value => value.match(/[0-9.]+/g).slice(0, 3).map(Number)
    const lum = value => rgb(value).map(x => { const c = x / 255; return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4 }).reduce((n, x, i) => n + x * [.2126, .7152, .0722][i], 0)
    return [...document.querySelectorAll('.auth-recovery h1,.auth-recovery p,.auth-recovery label,.auth-recovery a,.auth-recovery button')].filter(el => {
      if (el.disabled || !el.textContent.trim()) return false
      let node = el, bg = 'rgba(0, 0, 0, 0)'
      while (node && bg === 'rgba(0, 0, 0, 0)') { bg = getComputedStyle(node).backgroundColor; node = node.parentElement }
      const style = getComputedStyle(el), a = lum(style.color), b = lum(bg)
      return (Math.max(a, b) + .05) / (Math.min(a, b) + .05) < (parseFloat(style.fontSize) >= 24 ? 3 : 4.5)
    }).map(el => el.textContent)
  })
  assert.deepEqual(failures, [], 'enabled text contrast')
}
try {
  await dev.listen(); const base = `http://127.0.0.1:${dev.httpServer.address().port}`
  const fixture = `${base}/src/features/auth/__tests__/fixtures/recovery-preview.html`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[360, 800], [390, 844], [430, 932], [1280, 800]]) {
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(fixture)
    await expect(page.getByRole('heading', { name: 'Reset password' })).toBeFocused()
    await page.getByLabel('Email', { exact: true }).focus(); await page.keyboard.press('Tab')
    await expect(page.getByRole('button', { name: 'Request reset' })).toBeFocused()
    assert.notEqual(await page.getByRole('button', { name: 'Request reset' }).evaluate(el => getComputedStyle(el).outlineStyle), 'none')
    await layout(page); await hygiene(page); await page.screenshot({ path: `${evidence}/request-${width}.png`, fullPage: true })
    await page.getByLabel('Email', { exact: true }).fill('unknown@example.com')
    let count = calls.request
    await page.getByRole('button', { name: 'Request reset' }).click()
    await page.getByRole('form', { name: 'Request password reset' }).evaluate(form => form.requestSubmit())
    await expect(page.getByText(/Account details and email delivery are not confirmed here/)).toBeVisible()
    assert.equal(calls.request, count + 1)
    await page.getByLabel('Email', { exact: true }).fill('throttle@example.com')
    await page.getByRole('button', { name: 'Request reset' }).click()
    await expect(page.getByText('Too many attempts. Wait a while before trying again.')).toBeVisible()
    await page.goto(`${fixture}#token=${token}`)
    await expect(page.getByText('Reset link ready. Enter your new password below.')).toBeVisible()
    assert.equal(new URL(page.url()).hash, ''); assert.equal(new URL(page.url()).search, '')
    await hygiene(page)
    await page.getByLabel('New password').fill(password)
    await page.setViewportSize({ width, height: 420 }); await page.getByRole('button', { name: 'Reset password' }).scrollIntoViewIfNeeded()
    await expect(page.getByRole('button', { name: 'Reset password' })).toBeInViewport()
    await layout(page); await page.setViewportSize({ width, height })
    mode = 'expired'; count = calls.confirm
    await page.getByRole('button', { name: 'Reset password' }).click()
    await expect(page.getByLabel('New password')).toHaveValue('')
    await expect(page.getByText(/It may be invalid, expired or already used/)).toBeVisible()
    assert.equal(calls.confirm, count + 1); await hygiene(page)
    await page.getByLabel('Reset token').fill(token); await page.getByLabel('New password').fill(password)
    mode = 'success'; count = calls.confirm
    await page.getByRole('button', { name: 'Reset password' }).click()
    await page.getByRole('form', { name: 'Confirm password reset' }).evaluate(form => form.requestSubmit())
    await expect(page.getByText('Password reset completed. Sign in again with your new password.')).toBeVisible()
    assert.equal(calls.confirm, count + 1); await hygiene(page); await layout(page)
    await page.screenshot({ path: `${evidence}/confirmed-${width}.png`, fullPage: true })
    await page.goto(`${fixture}?screen=callback`)
    await expect(page.getByRole('heading', { name: 'Sign-in cancelled' })).toBeFocused()
    await expect(page.getByRole('link', { name: 'Back to your destination' })).toHaveAttribute('href', '/feed')
    await hygiene(page); await layout(page); await page.screenshot({ path: `${evidence}/denied-${width}.png`, fullPage: true })
    await page.evaluate(() => window.__FE11_TEST__.go('/auth/callback?code=SYNTHETIC_FE11_CODE&state=SYNTHETIC_FE11_STATE&returnTo=https%3A%2F%2Fevil.example'))
    await expect(page.getByRole('heading', { name: 'Sign-in could not be verified' })).toBeFocused()
    await expect(page.getByRole('link', { name: 'Back to your destination' })).toHaveAttribute('href', '/world')
    await hygiene(page)
    await page.evaluate(() => window.__FE11_TEST__.go('/reset-password'))
    await page.getByRole('button', { name: 'I have a reset token' }).click()
    await page.getByLabel('Reset token').fill(token); await page.getByLabel('New password').fill(password)
    mode = 'server-error'; count = calls.confirm
    const dispatches = await page.evaluate(() => window.__FE11_TEST__.dispatches().confirm)
    await page.getByRole('button', { name: 'Reset password' }).click()
    await expect(page.getByText(/The result could not be confirmed/)).toBeVisible()
    assert.equal(calls.confirm, count + 1); await hygiene(page)
    assert.equal(await page.evaluate(() => window.__FE11_TEST__.dispatches().confirm), dispatches + 1)
    const logins = calls.login
    assert.equal(await page.evaluate(() => window.__FE11_TEST__.login()), false)
    assert.equal(calls.login, logins, 'uncertain recovery must not dispatch a cookie writer')
    await page.screenshot({ path: `${evidence}/uncertain-${width}.png`, fullPage: true })
    await context.close()
    results.push({ width, height, forms: 'PASS', neutralAndUncertain: 'PASS', sourceFragmentAndHygiene: 'PASS', callback: 'PASS', focusTargetsContrast: 'PASS', reducedHeight: 'PASS' })
  }
  // Distinguish a single application fetch from Chromium's opaque connection retries.
  const dropped = await browser.newContext(), probe = await dropped.newPage()
  await probe.goto(`${fixture}#token=${token}`); await probe.getByLabel('New password').fill(password)
  mode = 'network'; const arrivalsBeforeDrop = calls.confirm
  await probe.getByRole('button', { name: 'Reset password' }).click()
  await expect(probe.getByText(/The result could not be confirmed/)).toBeVisible()
  assert.equal(await probe.evaluate(() => window.__FE11_TEST__.dispatches().confirm), 1)
  await expect.poll(() => calls.active).toBe(0)
  transportDrop = { applicationFetches: 1, serverArrivals: calls.confirm - arrivalsBeforeDrop, outcome: 'uncertain',
    limitation: 'Chrome may retransmit a POST after a connection drops before response headers. No application fetch retry occurs; server one-time-token enforcement remains authoritative.' }
  await hygiene(probe); await dropped.close()
  // Native cross-tab recovery confirmation participates in the existing cookie-writer lock.
  mode = 'success'
  const shared = await browser.newContext(), first = await shared.newPage(), second = await shared.newPage()
  await first.goto(fixture)
  await expect.poll(() => first.evaluate(() => window.__FE11_TEST__.login())).toBe(true)
  await expect.poll(() => first.evaluate(() => window.__FE11_TEST__.hasAccess())).toBe(true)
  await second.goto(`${fixture}#token=${token}`)
  await second.getByLabel('New password').fill(password)
  await second.getByRole('button', { name: 'Reset password' }).click()
  await expect(second.getByText('Password reset completed. Sign in again with your new password.')).toBeVisible()
  await expect.poll(() => first.evaluate(() => window.__FE11_TEST__.hasAccess())).toBe(false)
  assert.equal(calls.peak, 1); assert.equal(calls.refresh, 0)
  await hygiene(second); await shared.close()
  // Route exit aborts a queued native Web Lock before it can notify peers or dispatch.
  const queued = await browser.newContext(), waitingPage = await queued.newPage()
  await waitingPage.goto(`${fixture}#token=${token}`)
  await waitingPage.evaluate(async () => {
    let release
    const held = new Promise(resolve => { release = resolve })
    window.__FE11_RELEASE_LOCK__ = release
    await new Promise(resolve => { void navigator.locks.request('mcr-auth-cookie-v1', async () => { resolve(); await held }) })
  })
  await waitingPage.getByLabel('New password').fill(password)
  const queuedBefore = calls.confirm
  await waitingPage.getByRole('button', { name: 'Reset password' }).click()
  await expect(waitingPage.getByText('Confirming your reset…')).toBeVisible()
  await waitingPage.evaluate(() => window.__FE11_TEST__.go('/legal/privacy'))
  await expect(waitingPage.getByRole('heading', { name: 'Privacy', exact: true })).toBeVisible()
  await expect.poll(() => waitingPage.evaluate(async () => (await navigator.locks.query()).pending.length)).toBe(0)
  assert.equal(calls.confirm, queuedBefore)
  await waitingPage.evaluate(() => { window.__FE11_RELEASE_LOCK__(); delete window.__FE11_RELEASE_LOCK__ })
  await queued.close()
  // A pending server mutation may finish after route exit; no late UI success is admitted.
  const cancelled = await browser.newContext(), page = await cancelled.newPage()
  await page.goto(`${fixture}#token=${token}`); await page.getByLabel('New password').fill(password)
  mode = 'delay'; const before = calls.confirm
  await page.getByRole('button', { name: 'Reset password' }).click()
  await expect.poll(() => calls.confirm).toBe(before + 1)
  await page.evaluate(() => window.__FE11_TEST__.go('/legal/privacy'))
  await expect(page.getByRole('heading', { name: 'Privacy', exact: true })).toBeVisible()
  await expect(page.getByText('Password reset completed. Sign in again with your new password.')).toHaveCount(0)
  await expect.poll(() => calls.active).toBe(0); await hygiene(page); await cancelled.close()
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const origin = `http://127.0.0.1:${production.httpServer.address().port}`
  const prod = await browser.newContext({ viewport: { width: 390, height: 844 } }), p = await prod.newPage(), requests = [], referrerLeaks = [], assetReferrers = []
  p.on('pageerror', error => errors.push(error.message))
  p.on('request', req => {
    const url = new URL(req.url())
    if (url.pathname.startsWith('/api/') || /phaser|world-ws/.test(url.pathname)) requests.push(url.pathname)
    if (url.pathname.startsWith('/assets/')) assetReferrers.push({ path: url.pathname, referer: req.headers().referer ?? null })
    if (/SYNTHETIC_FE11_CODE|SYNTHETIC_FE11_STATE/.test(req.headers().referer ?? '')) referrerLeaks.push(url.pathname)
  })
  await p.goto(`${origin}/auth/callback?code=SYNTHETIC_FE11_CODE&state=SYNTHETIC_FE11_STATE`)
  await expect(p.getByRole('heading', { name: 'Sign-in could not be verified' })).toBeVisible()
  await hygiene(p); assert.deepEqual(requests, [])
  assert(assetReferrers.length >= 2, 'cold production load must actually request built assets')
  assert.deepEqual(referrerLeaks, [], 'pre-boot callback asset referrer policy needs integrator qualification')
  await p.goto(`${origin}/reset-password#token=${token}`)
  await expect(p.getByText('Reset link ready. Enter your new password below.')).toBeVisible(); await hygiene(p)
  assert.equal(await p.evaluate(() => typeof window.__FE11_TEST__), 'undefined'); assert.deepEqual(requests, [])
  await p.screenshot({ path: `${evidence}/production-reset-390.png`, fullPage: true })
  for (const name of await readdir('dist/assets')) if (name.endsWith('.js')) {
    const source = await readFile(`dist/assets/${name}`, 'utf8')
    for (const marker of ['__FE11_TEST__', 'SYNTHETIC_FE11_RECOVERY', 'SYNTHETIC_FE11_LOGIN_PASSWORD', 'PRIVATE_FE11_RAW_ERROR']) assert(!source.includes(marker), `test input leaked into ${name}`)
  }
  await prod.close(); assert.deepEqual(errors, [])
  await writeFile(`${evidence}/browser-results.json`, JSON.stringify({ status: 'MOCK_READY', browser: await browser.version(), results,
    crossTab: { peakWriters: calls.peak, automaticRefresh: calls.refresh, peerRevoked: 'PASS', queuedLockCancellation: 'PASS' }, transportDrop, productionIsolation: 'PASS', preBootReferrer: { status: 'PASS', assetReferrers },
    limitations: 'Synthetic HTTP/status/mail inputs and non-Secure test cookie. Real Auth/mail/reset-link BASE deployment, OAuth/cookie handoff, actual devices and release gates NOT_RUN.' }, null, 2))
  console.log(JSON.stringify({ status: 'PASS', viewports: results.length, evidence }))
} finally {
  await browser?.close(); await dev.close(); if (production) await new Promise(resolve => production.httpServer.close(resolve))
}
