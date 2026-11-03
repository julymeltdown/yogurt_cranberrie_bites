// Installed Chrome only; own ephemeral servers and browser contexts. No user profile.
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile, readdir } from 'node:fs/promises'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_FE09_EVIDENCE ?? '/tmp/mcr-fe09-evidence'
await mkdir(evidence, { recursive: true })
const contractFixture = JSON.parse(await readFile('../contracts/fixtures/core/bootstrap-open.json', 'utf8'))
contractFixture.capabilities.world = { available: true, reason: null }
const authentication = JSON.parse(await readFile('../contracts/fixtures/core/auth-success.json', 'utf8'))
const capabilities = JSON.parse(await readFile('../contracts/fixtures/core/auth-capabilities-disabled.json', 'utf8'))
capabilities.providers[0] = { provider: 'EMAIL', enabled: true, disabled_reason: null }
const stats = { login: 0, logout: 0, refresh: 0, active: 0, peak: 0, calls: [] }
let failLogout = false
const dev = await createServer({ plugins: [{ name: 'fe09-owned-native-cookie-fixture', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/api/')) return next()
    res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store')
    const send = (status, value) => { res.statusCode = status; res.end(value === undefined ? undefined : JSON.stringify(value)) }
    if (req.url === '/api/auth/capabilities') return send(200, capabilities)
    if (req.url.endsWith('/bootstrap')) return send(200, contractFixture)
    assert.equal(req.headers['x-mcr-intent'], 'browser')
    assert.equal(req.headers.origin, `http://${req.headers.host}`)
    stats.active++; stats.peak = Math.max(stats.peak, stats.active); stats.calls.push(req.url)
    try {
      if (req.url === '/api/auth/login') {
        stats.login++
        const chunks = []; for await (const chunk of req) chunks.push(chunk)
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8'))
        await new Promise(resolve => setTimeout(resolve, 150))
        if (body.email === 'throttle@example.com') return send(429, { code: 'TOO_MANY', message: 'raw throttle detail' })
        if (body.password === 'wrong') return send(401, { code: 'BAD', message: 'Account found: raw-private-error' })
        res.setHeader('Set-Cookie', 'fe09_fixture=rotated; HttpOnly; SameSite=Lax; Path=/')
        return send(200, authentication)
      }
      if (req.url === '/api/auth/logout') {
        stats.logout++
        await new Promise(resolve => setTimeout(resolve, 100))
        if (failLogout) return send(503, { code: 'OFFLINE', message: 'raw outage' })
        res.setHeader('Set-Cookie', 'fe09_fixture=; Max-Age=0; HttpOnly; SameSite=Lax; Path=/')
        return send(204)
      }
      if (req.url === '/api/auth/refresh') { stats.refresh++; return send(200, { ...authentication, accessToken: 'browser-rotated-token' }) }
      return send(404, {})
    } finally { stats.active-- }
  })
} }], server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser, production
const errors = [], results = []
try {
  await dev.listen()
  const base = `http://127.0.0.1:${dev.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const fixtureUrl = `${base}/src/features/auth/__tests__/fixtures/preview.html`
  async function fill(page, email = 'fan@example.com', password = 'test-password') {
    await expect(page.getByRole('button', { name: 'Sign in with email' })).toBeEnabled()
    await page.getByLabel('Email', { exact: true }).fill(email)
    await page.getByLabel('Password', { exact: true }).fill(password)
  }
  for (const [width, height] of [[360, 800], [390, 844], [430, 932], [1280, 800]]) {
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(fixtureUrl)
    await fill(page)
    for (const provider of ['Google', 'Apple', 'X']) await expect(page.getByRole('button', { name: `Continue with ${provider}` })).toBeDisabled()
    await page.getByLabel('Email', { exact: true }).focus(); await page.keyboard.press('Tab')
    await expect(page.getByLabel('Password', { exact: true })).toBeFocused()
    assert.notEqual(await page.getByLabel('Password', { exact: true }).evaluate(el => getComputedStyle(el).outlineStyle), 'none')
    const contrastFailures = await page.evaluate(() => {
      const rgb = color => color.match(/[0-9.]+/g).slice(0, 3).map(Number)
      const lum = color => rgb(color).map(x => { const c = x / 255; return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4 }).reduce((sum, x, i) => sum + x * [.2126, .7152, .0722][i], 0)
      return [...document.querySelectorAll('.auth-sign-in h1, .auth-sign-in p, .auth-sign-in label, .auth-sign-in input, .auth-sign-in button, .auth-sign-in a')].filter(el => {
        if (el.disabled || !el.textContent.trim() && el.tagName !== 'INPUT') return false
        let node = el, background = 'rgba(0, 0, 0, 0)'
        while (node && background === 'rgba(0, 0, 0, 0)') { background = getComputedStyle(node).backgroundColor; node = node.parentElement }
        const style = getComputedStyle(el), a = lum(style.color), b = lum(background)
        const ratio = (Math.max(a, b) + .05) / (Math.min(a, b) + .05)
        return ratio < (parseFloat(style.fontSize) >= 24 ? 3 : 4.5)
      }).map(el => el.textContent || el.name)
    })
    assert.deepEqual(contrastFailures, [], 'enabled sign-in text contrast')
    await page.screenshot({ path: `${evidence}/sign-in-${width}.png`, fullPage: true })
    await fill(page, 'fan@example.com', 'wrong')
    const count = stats.login
    await page.getByRole('button', { name: 'Sign in with email' }).click()
    await expect(page.getByRole('button', { name: 'Signing in…' })).toBeDisabled()
    await expect(page.getByLabel('Password', { exact: true })).toHaveValue('')
    await expect(page.getByText('We could not sign you in. Check your details or try again later.')).toBeVisible()
    assert.equal(stats.login, count + 1)
    assert(!(await page.locator('body').innerText()).includes('raw-private-error'))
    await page.screenshot({ path: `${evidence}/failed-sign-in-${width}.png`, fullPage: true })
    await fill(page, 'throttle@example.com')
    await page.getByRole('button', { name: 'Sign in with email' }).click()
    await expect(page.getByText('Too many attempts. Wait a while before trying again.')).toBeVisible()
    await fill(page)
    await page.getByRole('button', { name: 'Sign in with email' }).click()
    await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
    assert.equal(new URL(page.url()).pathname, '/feed')
    assert.equal(await page.evaluate(() => window.__FE09_TEST__.counters.peak), 1)
    const cookies = await context.cookies(); assert(cookies.some(c => c.name === 'fe09_fixture' && c.httpOnly))
    assert.equal(await page.evaluate(() => localStorage.length + sessionStorage.length), 0)
    assert(!(await page.locator('body').innerText()).includes(authentication.accessToken))
    failLogout = true
    await page.evaluate(() => { void window.__FE09_TEST__.logout() })
    await expect(page.getByRole('navigation', { name: 'Main navigation' })).toHaveCount(0)
    await expect.poll(() => page.evaluate(() => window.__FE09_TEST__.counters.live)).toBe(0)
    await page.evaluate(() => window.__FE09_TEST__.openLogin())
    await expect(page.getByText('Local access is cleared. Server sign-out could not be confirmed.')).toBeVisible()
    await page.screenshot({ path: `${evidence}/unconfirmed-logout-${width}.png`, fullPage: true })
    failLogout = false
    await page.getByRole('button', { name: 'Try server sign-out again' }).click()
    await expect(page.getByText('You’re signed out.')).toBeVisible()
    assert(!(await context.cookies()).some(c => c.name === 'fe09_fixture'))
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    for (const control of await page.locator('.auth-sign-in input, .auth-sign-in button, .auth-sign-in a').all()) {
      const box = await control.boundingBox(); assert(box && box.height >= 44)
    }
    await page.setViewportSize({ width, height: 420 })
    await expect(page.getByLabel('Password', { exact: true })).toBeVisible()
    results.push({ width, height, signIn: 'PASS', invalidCredentials: 'PASS', throttling: 'PASS', logoutFailureAndRetry: 'PASS', peakWorldLease: 1 })
    await context.close()
  }
  // One real browser context shares the HttpOnly fixture cookie, native Web Locks and BroadcastChannel.
  const context = await browser.newContext(), a = await context.newPage(), b = await context.newPage()
  await a.goto(fixtureUrl); await fill(a); await a.getByRole('button', { name: 'Sign in with email' }).click()
  await expect(a.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
  await b.goto(fixtureUrl); await fill(b)
  await b.getByRole('button', { name: 'Sign in with email' }).click()
  await expect(a.getByRole('navigation', { name: 'Main navigation' })).toHaveCount(0)
  await expect(b.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
  await a.evaluate(() => window.__FE09_TEST__.openLogin()); await fill(a)
  await a.getByRole('button', { name: 'Sign in with email' }).click()
  const result = b.evaluate(() => window.__FE09_TEST__.logout())
  await result
  await expect.poll(() => a.evaluate(() => window.__FE09_TEST__.counters.live)).toBe(0)
  await expect.poll(() => b.evaluate(() => window.__FE09_TEST__.counters.live)).toBe(0)
  assert.equal(stats.peak, 1, 'native cross-tab cookie writers overlapped')
  assert.equal(stats.refresh, 0, 'production-style controller restored/refreshed automatically')
  await context.close()
  // Production is built without an event ID. There is no fixture identity/default/discovery.
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const productionBase = `http://127.0.0.1:${production.httpServer.address().port}`
  const prodContext = await browser.newContext(), page = await prodContext.newPage(), requests = []
  page.on('request', req => { if (new URL(req.url()).pathname.startsWith('/api/')) requests.push(req.url()) })
  await page.goto(`${productionBase}/login`)
  await expect(page.getByText('Event entry is not open yet. Please check back later.')).toBeVisible()
  assert.equal(requests.length, 0)
  assert.equal(await page.evaluate(() => typeof window.__FE09_TEST__), 'undefined')
  for (const file of await readdir('dist/assets')) if (file.endsWith('.js')) {
    const source = await readFile(`dist/assets/${file}`, 'utf8')
    for (const forbidden of ['fixture-not-a-real-credential', 'Synthetic contract fixture', '__FE09_TEST__', '00000000-0000-4000-8000-000000000201']) assert(!source.includes(forbidden), `${forbidden} leaked into ${file}`)
  }
  await page.screenshot({ path: `${evidence}/production-unavailable.png`, fullPage: true })
  await prodContext.close()
  assert.deepEqual(errors, [])
  await writeFile(`${evidence}/browser-results.json`, JSON.stringify({ status: 'MOCK_READY', browser: await browser.version(), results,
    crossTab: { nativeLocks: 'PASS', nativeBroadcast: 'PASS', httpOnlyFixtureCookie: 'PASS', peakCookieWriters: stats.peak, automaticRefreshes: stats.refresh },
    production: { noDefaultEvent: 'PASS', noRequestsWithoutEvent: 'PASS', fixtureExclusion: 'PASS' },
    limitations: 'Synthetic same-origin HTTP server. Secure production cookie/domain, BE03, revoked sessions, resource bytes, real Phaser/socket and two-user flow NOT_RUN.' }, null, 2))
  console.log(JSON.stringify({ status: 'PASS', viewports: results.length, peakCookieWriters: stats.peak, automaticRefreshes: stats.refresh, evidence }))
} finally {
  await browser?.close(); await dev.close(); if (production) await new Promise(resolve => production.httpServer.close(resolve))
}
