// Installed Chrome; only task-owned ephemeral servers, profiles and synthetic inputs.
import assert from 'node:assert/strict'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_FE10_EVIDENCE ?? '/tmp/mcr-fe10-evidence'
await mkdir(evidence, { recursive: true })
const capabilities = JSON.parse(await readFile('../contracts/fixtures/core/auth-capabilities-disabled.json', 'utf8'))
capabilities.providers[0] = { provider: 'EMAIL', enabled: true, disabled_reason: null }
const userId = '00000000-0000-4000-8000-000000000001', email = 'synthetic-fe10@example.com', password = '  SYNTHETIC_FE10_PASSWORD  ', code = '938271'
const calls = { signup: 0, verify: 0, resend: 0 }, results = [], errors = []
let mode = 'success', browser, production
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'fe10-owned-synthetic-http', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/api/')) return next()
    res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store')
    const send = (status, body) => { if (!res.destroyed) { res.statusCode = status; res.end(JSON.stringify(body)) } }
    if (req.url === '/api/auth/capabilities') return send(200, capabilities)
    assert.equal(req.headers.origin, `http://${req.headers.host}`); assert.equal(req.headers['x-mcr-intent'], 'browser'); assert.equal(req.headers.referer, undefined)
    assert.equal(req.headers.authorization, undefined)
    const kind = Object.keys(calls).find(k => req.url === `/api/auth/email/${k}`)
    assert(kind, `unexpected API ${req.url}`); calls[kind]++
    const chunks = []; for await (const chunk of req) chunks.push(chunk)
    const body = JSON.parse(Buffer.concat(chunks).toString('utf8'))
    assert.deepEqual(Object.keys(body).sort(), kind === 'signup' ? ['email', 'password'] : kind === 'verify' ? ['code', 'email'] : ['email'])
    assert.equal(body.email, email)
    if (kind === 'signup') assert.equal(body.password, password)
    if (kind === 'verify') assert.equal(body.code, code)
    const selected = mode
    await new Promise(resolve => setTimeout(resolve, selected === 'delay' ? 500 : 120))
    if (selected === 'throttle') { res.setHeader('Retry-After', '1'); return send(429, { code: 'RATE', message: 'PRIVATE_FE10_SERVER_TEXT' }) }
    if (selected === 'duplicate') return send(409, { code: 'CONFLICT', message: 'PRIVATE_FE10_SERVER_TEXT' })
    if (selected === 'expired') return send(400, { code: 'BAD', message: 'PRIVATE_FE10_SERVER_TEXT' })
    if (selected === 'uncertain') return send(503, { code: 'DOWN', message: 'PRIVATE_FE10_SERVER_TEXT' })
    return kind === 'resend' ? send(202, { status: 'ACCEPTED' }) : send(200, { userId, status: kind === 'signup' ? 'PENDING_VERIFICATION' : 'VERIFIED' })
  })
} }] })
async function hygiene(page) {
  const snapshot = await page.evaluate(() => ({ html: document.body.innerHTML, url: location.href, state: JSON.stringify(history.state),
    local: JSON.stringify(localStorage), session: JSON.stringify(sessionStorage), cookie: document.cookie }))
  for (const [key, value] of Object.entries(snapshot)) for (const marker of [password, code, 'PRIVATE_FE10_SERVER_TEXT', 'PRIVATE_URL_VALUE']) assert(!value.includes(marker), `secret in ${key}`)
  assert.equal(await page.locator('canvas,[data-world-host]').count(), 0)
}
async function layout(page) {
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
  for (const control of await page.locator('.auth-enrollment a,.auth-enrollment button,.auth-enrollment input').all()) {
    const box = await control.boundingBox(); assert(box && box.width >= 44 && box.height >= 44, 'target below44px')
  }
  const bad = await page.evaluate(() => {
    const lum = color => color.match(/[0-9.]+/g).slice(0, 3).map(Number).map(x => { const c = x / 255; return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4 }).reduce((n, x, i) => n + x * [.2126, .7152, .0722][i], 0)
    return [...document.querySelectorAll('.auth-enrollment h1,.auth-enrollment p,.auth-enrollment label,.auth-enrollment a,.auth-enrollment button')].filter(el => {
      if (el.disabled || !el.textContent.trim()) return false
      let node = el, bg = 'rgba(0, 0, 0, 0)'
      while (node && bg === 'rgba(0, 0, 0, 0)') { bg = getComputedStyle(node).backgroundColor; node = node.parentElement }
      const style = getComputedStyle(el), a = lum(style.color), b = lum(bg)
      return (Math.max(a, b) + .05) / (Math.min(a, b) + .05) < (parseFloat(style.fontSize) >= 24 ? 3 : 4.5)
    }).map(el => el.textContent)
  })
  assert.deepEqual(bad, [], 'text contrast')
}
async function fill(page, kind) {
  await page.getByLabel('Email', { exact: true }).fill(email)
  if (kind === 'signup') {
    await page.getByLabel('Password', { exact: true }).fill(password)
    for (const box of await page.getByRole('checkbox').all()) await box.check()
  } else await page.getByLabel('Verification code', { exact: true }).fill(code)
}
try {
  await dev.listen(); const base = `http://127.0.0.1:${dev.httpServer.address().port}`, fixture = `${base}/src/features/auth/__tests__/fixtures/enrollment-preview.html`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[360, 800], [390, 844], [430, 932], [1280, 800]]) {
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(fixture)
    await expect(page.getByRole('heading', { name: 'Create account' })).toBeFocused()
    await expect(page.getByLabel('Email', { exact: true })).toBeEnabled()
    await page.getByLabel('Email', { exact: true }).focus(); await page.keyboard.press('Tab')
    await expect(page.getByLabel('Password', { exact: true })).toBeFocused()
    assert.notEqual(await page.getByLabel('Password', { exact: true }).evaluate(el => getComputedStyle(el).outlineStyle), 'none')
    await layout(page); await hygiene(page); await page.screenshot({ path: `${evidence}/signup-${width}.png`, fullPage: true })
    mode = 'duplicate'; await fill(page, 'signup'); let count = calls.signup
    await page.getByRole('button', { name: 'Create account', exact: true }).click()
    await expect(page.getByText(/We could not create this account/)).toBeVisible(); assert.equal(calls.signup, count + 1)
    assert(await page.getByText(/We could not create this account/).evaluate(el => el.closest('[role="status"]') === document.activeElement), 'feedback not focused')
    await hygiene(page)
    mode = 'success'; await page.getByLabel('Password', { exact: true }).fill(password); count = calls.signup
    await page.getByRole('button', { name: 'Create account', exact: true }).click()
    await page.getByRole('form', { name: 'Create account', exact: true }).evaluate(form => form.requestSubmit())
    await expect(page.getByText(/Account created. Verify your email/)).toBeVisible(); assert.equal(calls.signup, count + 1)
    assert.equal(await page.evaluate(() => window.__FE10_TEST__.hasAccess()), false)
    await hygiene(page); await layout(page); await page.screenshot({ path: `${evidence}/created-${width}.png`, fullPage: true })
    await page.getByRole('link', { name: 'Continue to email verification' }).click()
    await expect(page.getByRole('heading', { name: 'Verify email' })).toBeFocused()
    await expect(page.getByLabel('Email', { exact: true })).toHaveValue('')
    mode = 'expired'; await fill(page, 'verify'); count = calls.verify
    await page.setViewportSize({ width, height: 420 }); await page.getByRole('button', { name: 'Verify email', exact: true }).scrollIntoViewIfNeeded()
    await expect(page.getByRole('button', { name: 'Verify email', exact: true })).toBeInViewport()
    await layout(page); await page.setViewportSize({ width, height })
    await page.getByRole('button', { name: 'Verify email', exact: true }).click()
    await expect(page.getByText(/It may be invalid, expired or already used/)).toBeVisible(); assert.equal(calls.verify, count + 1)
    await expect(page.getByLabel('Verification code')).toHaveValue(''); await hygiene(page)
    mode = 'throttle'; count = calls.resend
    await page.getByRole('button', { name: 'Request another code' }).click()
    await expect(page.getByText(/Server requested a wait/)).toBeVisible()
    await expect(page.getByRole('button', { name: 'Verify email', exact: true })).toBeDisabled()
    await page.screenshot({ path: `${evidence}/cooldown-${width}.png`, fullPage: true })
    await expect(page.getByRole('button', { name: 'Request another code' })).toBeEnabled({ timeout: 2500 }); assert.equal(calls.resend, count + 1)
    mode = 'success'; await page.getByRole('button', { name: 'Request another code' }).click()
    await expect(page.getByText(/Account details and email delivery are not confirmed here/)).toBeVisible()
    await expect(page.getByText(/Server requested a wait/)).toHaveCount(0)
    mode = 'uncertain'; await page.getByRole('button', { name: 'Request another code' }).click()
    await expect(page.getByText(/result and email delivery are not confirmed/)).toBeVisible(); await hygiene(page)
    await page.getByLabel('Verification code').fill(code); count = calls.verify
    const before = await page.evaluate(() => window.__FE10_TEST__.dispatches().verify)
    await page.getByRole('button', { name: 'Verify email', exact: true }).click()
    await expect(page.getByText(/Your action may have completed/)).toBeVisible()
    assert.equal(calls.verify, count + 1); assert.equal(await page.evaluate(() => window.__FE10_TEST__.dispatches().verify), before + 1)
    await hygiene(page); await page.screenshot({ path: `${evidence}/uncertain-${width}.png`, fullPage: true })
    mode = 'delay'; await page.getByLabel('Verification code').fill(code)
    await page.getByRole('button', { name: 'Verify email', exact: true }).click()
    await expect(page.getByText('Checking your code…')).toBeVisible()
    await page.evaluate(() => window.__FE10_TEST__.clear())
    await expect(page.getByText(/Your session changed/)).toBeVisible(); await page.waitForTimeout(550)
    await expect(page.getByText(/Email verified/)).toHaveCount(0); await hygiene(page)
    // Route exit also cancels a dispatched operation; its late success cannot project onto signup.
    await fill(page, 'verify'); await page.getByRole('button', { name: 'Verify email', exact: true }).click()
    await expect(page.getByText('Checking your code…')).toBeVisible()
    await page.evaluate(() => window.__FE10_TEST__.go('/signup'))
    await expect(page.getByRole('heading', { name: 'Create account' })).toBeFocused()
    await page.waitForTimeout(550); await expect(page.getByText(/Email verified/)).toHaveCount(0); await hygiene(page)
    await page.evaluate(() => window.__FE10_TEST__.go('/verify-email?email=PRIVATE_URL_VALUE#code=938271'))
    await expect(page.getByRole('heading', { name: 'Verify email' })).toBeFocused(); await hygiene(page)
    mode = 'success'; await fill(page, 'verify'); count = calls.verify
    await page.getByRole('button', { name: 'Verify email', exact: true }).click()
    await page.getByRole('form', { name: 'Verify email', exact: true }).evaluate(form => form.requestSubmit())
    await expect(page.getByText('Email verified. Sign in to continue.')).toBeVisible(); assert.equal(calls.verify, count + 1)
    assert.equal(await page.evaluate(() => window.__FE10_TEST__.hasAccess()), false)
    await hygiene(page); await layout(page); await page.screenshot({ path: `${evidence}/verified-${width}.png`, fullPage: true })
    await context.close(); results.push({ width, height, forms: 'PASS', duplicateAndUncertainty: 'PASS', expiryAndServerCooldown: 'PASS', privacyAndHygiene: 'PASS', focusTargetsContrast: 'PASS', reducedHeight: 'PASS' })
  }
  // Both no-event configurations use the same explicit anonymous discovery, with synthetic signup qualification absent.
  for (const event of ['none', 'malformed']) {
    const context = await browser.newContext({ viewport: { width: 390, height: 844 } }), page = await context.newPage()
    mode = 'success'
    await page.goto(`${fixture}?screen=verify&event=${event}`)
    await expect(page.getByRole('button', { name: 'Verify email', exact: true })).toBeDisabled()
    await page.getByRole('button', { name: 'Check email availability' }).click()
    await expect(page.getByLabel('Email', { exact: true })).toBeEnabled()
    await fill(page, 'verify'); await page.getByRole('button', { name: 'Request another code' }).click()
    await expect(page.getByText(/Account details and email delivery are not confirmed here/)).toBeVisible()
    await page.getByLabel('Verification code').fill(code); await page.getByRole('button', { name: 'Verify email', exact: true }).click()
    await expect(page.getByText('Email verified. Sign in to continue.')).toBeVisible()
    assert.equal(await page.evaluate(() => window.__FE10_TEST__.hasAccess()), false); await hygiene(page)
    await page.screenshot({ path: `${evidence}/verify-event-${event}-390.png`, fullPage: true }); await context.close()
  }
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const productionBase = `http://127.0.0.1:${production.httpServer.address().port}`, context = await browser.newContext({ viewport: { width: 390, height: 844 } }), page = await context.newPage(), requests = []
  page.on('request', req => requests.push(req.url())); page.on('pageerror', error => errors.push(error.message))
  for (const path of ['/signup', '/verify-email']) {
    await page.goto(`${productionBase}${path}`); await expect(page.getByRole('heading', { name: path === '/signup' ? 'Create account' : 'Verify email' })).toBeFocused()
    await expect(page.getByRole('button', { name: path === '/signup' ? 'Create account' : 'Verify email', exact: true })).toBeDisabled()
    await expect(page.getByRole('checkbox')).toHaveCount(0); await hygiene(page); await layout(page)
    await page.screenshot({ path: `${evidence}/production-${path.slice(1)}-390.png`, fullPage: true })
  }
  for (const width of [360, 430, 1280]) { await page.setViewportSize({ width, height: 800 }); await page.goto(`${productionBase}/signup`); await expect(page.getByRole('heading')).toHaveText('Create account'); await layout(page) }
  assert(!requests.some(url => url.includes('/api/') || url.includes('WorldHost') || url.includes('world-adapter') || url.includes('phaser') || url.includes('__tests__')), 'production loaded API, world or fixture')
  // Production bundle, ordinary public route: explicitly requested capability GET then anonymous commands.
  const publicCalls = []
  await page.route(`${productionBase}/api/**`, async route => {
    const request = route.request(), path = new URL(request.url()).pathname
    publicCalls.push(path)
    if (path === '/api/auth/capabilities') return route.fulfill({ json: capabilities })
    assert(['/api/auth/email/resend', '/api/auth/email/verify'].includes(path), `unexpected public API ${path}`)
    assert.equal(request.headers().authorization, undefined)
    const body = request.postDataJSON()
    assert.deepEqual(body, path.endsWith('/resend') ? { email } : { email, code })
    return route.fulfill({ status: path.endsWith('/resend') ? 202 : 200, json: path.endsWith('/resend') ? { status: 'ACCEPTED' } : { userId, status: 'VERIFIED' } })
  })
  await page.goto(`${productionBase}/verify-email`)
  await expect(page.getByRole('button', { name: 'Verify email', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: 'Check email availability' }).click()
  await expect(page.getByLabel('Email', { exact: true })).toBeEnabled(); await fill(page, 'verify')
  await page.getByRole('button', { name: 'Request another code' }).click()
  await expect(page.getByText(/Account details and email delivery are not confirmed here/)).toBeVisible()
  await page.getByLabel('Verification code').fill(code); await page.getByRole('button', { name: 'Verify email', exact: true }).click()
  await expect(page.getByText('Email verified. Sign in to continue.')).toBeVisible()
  assert.deepEqual(publicCalls, ['/api/auth/capabilities', '/api/auth/email/resend', '/api/auth/email/verify'])
  assert.equal(await page.evaluate(() => typeof window.__FE10_TEST__), 'undefined'); await hygiene(page); await layout(page)
  await page.screenshot({ path: `${evidence}/production-event-independent-verified-1280.png`, fullPage: true })
  for (const file of await readdir('dist/assets')) if (file.endsWith('.js')) {
    const text = await readFile(`dist/assets/${file}`, 'utf8'); assert(!text.includes('SYNTHETIC_FE10') && !text.includes('__FE10_TEST__'), 'fixture in production bundle')
  }
  assert.deepEqual(errors, [])
  const report = { browser: await browser.version(), results, production: 'PASS: cold signup/verify no API; explicit verify capability discovery and anonymous resend/verify work without event; no world/fixture entry', eventIndependentDiscovery: 'PASS: null, malformed and production no-event', publicCalls, calls, pageErrors: errors,
    limits: ['Synthetic HTTP only; no real account, mail, eligibility or consent persistence.', 'Application dispatch count is not physical TCP arrival proof.', 'No response challenge expiry/cooldown metadata; countdown uses Retry-After only.'] }
  await writeFile(`${evidence}/report.json`, JSON.stringify(report, null, 2)); console.log(JSON.stringify(report, null, 2)); await context.close()
} finally { await browser?.close(); await new Promise(resolve => production ? production.httpServer.close(resolve) : resolve()); await dev.close() }
