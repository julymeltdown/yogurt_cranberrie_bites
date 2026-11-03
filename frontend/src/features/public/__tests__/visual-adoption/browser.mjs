// Owned ephemeral production/fixture builds; actual installed Chrome, no user profile.
import assert from 'node:assert/strict'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { mkdtemp, readFile, writeFile, rm, readdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { build, preview } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { chromium, expect } from '@playwright/test'

const dir = path.dirname(fileURLToPath(import.meta.url)), root = path.resolve(dir, '../../../../../..')
const frontend = path.join(root, 'frontend'), authDir = path.join(frontend, 'src/features/auth/__tests__/visual-adoption')
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const temp = await mkdtemp(path.join(tmpdir(), 'mcr-uiv02-'))
const errors = [], screenshots = [], results = [], requests = [], servers = []
let browser
async function serve(outDir, entries) {
  await build({ configFile: false, root: frontend, plugins: [react(), tailwindcss()],
    build: { outDir, emptyOutDir: true, rollupOptions: { input: entries } } })
  const server = await preview({ configFile: false, root: frontend, build: { outDir }, preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  servers.push(server)
  return `http://127.0.0.1:${server.httpServer.address().port}`
}
async function capture(page, lane, name, width) {
  if (!name.includes('keyboard')) await page.evaluate(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur()
  })
  await page.evaluate(() => scrollTo(0, 0))
  const folder = lane === 'public' ? dir : authDir, file = `${name}-${width}.png`
  await page.screenshot({ path: path.join(folder, 'evidence', file), fullPage: true })
  screenshots.push({ lane, file, width, sha256: hash(await readFile(path.join(folder, 'evidence', file))) })
}
async function check(page, scope) {
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
  for (const element of await page.locator(`${scope} a, ${scope} button, ${scope} input`).all()) {
    if (!await element.isVisible() || await element.getAttribute('class') === 'public-skip') continue
    const box = await element.boundingBox()
    assert(box && box.width >= 44 && box.height >= 44, `44px: ${await element.textContent()}`)
  }
  const failures = await page.locator(scope).evaluate(container => {
    const lum = color => color.match(/[\d.]+/g).slice(0, 3).map(Number).map(v => {
      const c = v / 255; return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4
    }).reduce((a, n, i) => a + n * [.2126, .7152, .0722][i], 0)
    return [...container.querySelectorAll('h1,h2,p,label,input,button,a,dt,dd')].filter(el => {
      if (!el.getClientRects().length || el.disabled || !el.textContent.trim() && el.tagName !== 'INPUT') return false
      let node = el, background = 'rgba(0, 0, 0, 0)'
      while (node && background === 'rgba(0, 0, 0, 0)') { background = getComputedStyle(node).backgroundColor; node = node.parentElement }
      const style = getComputedStyle(el), a = lum(style.color), b = lum(background)
      return (Math.max(a, b) + .05) / (Math.min(a, b) + .05) < (parseFloat(style.fontSize) >= 24 ? 3 : 4.5)
    }).map(el => el.textContent || el.name)
  })
  assert.deepEqual(failures, [], 'text contrast on solid backgrounds (grain separately visually inspected)')
}
try {
  const production = await serve(path.join(temp, 'production'), [path.join(frontend, 'index.html')])
  const fixture = await serve(path.join(temp, 'fixtures'), [path.join(dir, 'preview.html'), path.join(authDir, 'preview.html')])
  const caps = JSON.parse(await readFile(path.join(root, 'contracts/fixtures/core/auth-capabilities-disabled.json')))
  caps.providers[0] = { provider: 'EMAIL', enabled: true, disabled_reason: null }
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, reducedMotion: 'reduce' })
    const page = await context.newPage(), world = [], unexpected = []
    page.on('pageerror', error => errors.push(error.message))
    page.on('websocket', socket => world.push(socket.url()))
    page.on('request', req => {
      const url = new URL(req.url())
      if (url.origin !== production && url.origin !== fixture) unexpected.push(url.origin)
      if (/phaser|\/world\/avatars\//i.test(url.pathname)) world.push(url.pathname)
    })
    await page.goto(production)
    await expect(page.getByText('World entry is not open yet.', { exact: true })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Enter the world', exact: true })).toBeDisabled()
    await page.keyboard.press('Tab')
    await expect(page.getByRole('link', { name: 'Skip to content' })).toBeFocused()
    await page.keyboard.press('Enter')
    await expect(page.locator('#public-main')).toBeFocused()
    await check(page, '.public-entry')
    await capture(page, 'public', 'production-home', width)
    await page.getByRole('link', { name: 'Sign in', exact: true }).click()
    await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible()
    await expect(page.getByText('Event entry is not open yet. Please check back later.')).toBeVisible()
    await check(page, '.auth-sign-in')
    await capture(page, 'auth', 'production-login-unavailable', width)
    assert.equal(await page.getByRole('form').count(), 0)
    await page.goBack()
    await expect(page.locator('.public-poster')).toBeVisible()
    const nav = page.getByRole('navigation', { name: 'Public information' })
    for (const label of ['Terms', 'Privacy', 'Safety', 'Contact']) {
      await nav.getByRole('link', { name: label, exact: true }).click()
      await expect(page.getByRole('heading', { name: label, exact: true })).toBeFocused()
      await check(page, '.public-entry')
      assert.equal(await page.locator('.public-entry--poster, .public-document .concert-window').count(), 0)
      if (label === 'Terms') await capture(page, 'public', 'production-document-unavailable', width)
    }
    await page.goto(production + '/signup')
    await expect(page.locator('.auth-enrollment')).toBeVisible()
    assert.equal(await page.locator('.auth-sign-in--concert').count(), 0)
    await capture(page, 'auth', 'production-signup-regression', width)
    await page.goto(production + '/reset-password')
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
    assert.equal(await page.locator('.auth-sign-in--concert').count(), 0)
    await capture(page, 'auth', 'production-reset-regression', width)
    const publicPreview = fixture + '/src/features/public/__tests__/visual-adoption/preview.html'
    await page.goto(publicPreview)
    await expect(page.getByText('Synthetic public fixture · no live event or approved policy')).toBeVisible()
    const entry = page.getByRole('link', { name: 'Sign in to enter' })
    await expect(entry).toHaveAttribute('href', '/login?returnTo=%2Fworld')
    await entry.focus()
    assert.notEqual(await entry.evaluate(e => getComputedStyle(e).outlineStyle), 'none')
    await check(page, '.public-entry')
    await capture(page, 'public', 'fixture-anonymous', width)
    await page.setViewportSize({ width, height: 420 })
    await entry.scrollIntoViewIfNeeded()
    await expect(entry).toBeInViewport()
    await page.setViewportSize({ width, height: 844 })
    for (const status of ['checking', 'error']) {
      await page.goto(publicPreview + `?entry=${status}`)
      await expect(page.getByRole('button', { name: status === 'checking' ? 'Checking entry…' : 'Enter the world', exact: true })).toBeDisabled()
      await capture(page, 'public', `fixture-entry-${status}`, width)
    }
    await page.goto(publicPreview + '?route=%2Flegal%2Fterms')
    await expect(page.getByText('Opening this document…')).toBeVisible()
    await capture(page, 'public', 'fixture-document-loading', width)
    await expect(page.getByText('TEST-2026.11.03')).toBeVisible()
    await check(page, '.public-entry')
    await capture(page, 'public', 'fixture-document-published', width)
    await page.goto(publicPreview + '?route=%2Flegal%2Fterms&document=error')
    await expect(page.getByRole('alert')).toContainText('We could not open this document.')
    await capture(page, 'public', 'fixture-document-error', width)
    // Browser transport fixture: canonical Auth still owns all state and validation.
    let capMode = 'ready', releaseCaps, releaseLogin, loginCount = 0
    await page.route('**/api/auth/capabilities', async route => {
      if (capMode === 'loading') await new Promise(resolve => { releaseCaps = resolve })
      await route.fulfill(capMode === 'error' ? { status: 503, json: { code: 'UNAVAILABLE', message: 'Private diagnostic' } } : { status: 200, json: caps })
    })
    await page.route('**/api/auth/login', async route => {
      loginCount++
      assert.equal(route.request().headers()['x-mcr-intent'], 'browser')
      assert.equal(route.request().headers().origin, fixture)
      requests.push({ width, method: route.request().method(), path: '/api/auth/login', scenario: 'synthetic credential denial' })
      await new Promise(resolve => { releaseLogin = resolve })
      await route.fulfill({ status: 401, json: { code: 'BAD', message: 'PRIVATE_ACCOUNT_DIAGNOSTIC' } })
    })
    const authPreview = fixture + '/src/features/auth/__tests__/visual-adoption/preview.html'
    await page.goto(authPreview)
    await expect(page.getByText('Synthetic Auth fixture · contract responses, no live sign-in')).toBeVisible()
    const submit = page.getByRole('button', { name: 'Sign in with email' })
    await expect(submit).toBeEnabled()
    for (const provider of ['Google', 'Apple', 'X']) await expect(page.getByRole('button', { name: `Continue with ${provider}` })).toBeDisabled()
    assert.equal(await page.getByRole('button', { name: /Discord/ }).count(), 0)
    await check(page, '.auth-sign-in')
    await capture(page, 'auth', 'fixture-populated', width)
    const email = page.getByLabel('Email', { exact: true }), password = page.getByLabel('Password', { exact: true })
    await email.fill('fan@example.com')
    await email.focus()
    await page.keyboard.press('Tab')
    await expect(password).toBeFocused()
    assert.notEqual(await password.evaluate(e => getComputedStyle(e).outlineStyle), 'none')
    await capture(page, 'auth', 'fixture-keyboard', width)
    await password.fill('synthetic-wrong-password')
    await submit.focus()
    await page.keyboard.press('Enter')
    await expect(page.getByRole('button', { name: 'Signing in…' })).toBeDisabled()
    await expect(password).toHaveValue('')
    await capture(page, 'auth', 'fixture-pending', width)
    await expect.poll(() => Boolean(releaseLogin)).toBe(true)
    releaseLogin()
    await expect(page.getByText('We could not sign you in. Check your details or try again later.')).toBeVisible()
    assert.equal(loginCount, 1)
    assert(!(await page.locator('body').innerText()).includes('PRIVATE_ACCOUNT_DIAGNOSTIC'))
    await check(page, '.auth-sign-in')
    await capture(page, 'auth', 'fixture-credentials-error', width)
    await page.setViewportSize({ width, height: 420 })
    await password.scrollIntoViewIfNeeded()
    await expect(password).toBeInViewport()
    await submit.scrollIntoViewIfNeeded()
    await expect(submit).toBeInViewport()
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    await page.setViewportSize({ width, height: 844 })
    await page.getByRole('link', { name: 'Forgot password?' }).click()
    await expect(page.getByRole('heading', { name: /Reset password/i })).toBeVisible()
    await page.goBack()
    await expect(submit).toBeEnabled()
    await page.getByRole('link', { name: 'Create an account' }).click()
    await expect(page.locator('.auth-enrollment')).toBeVisible()
    await page.goBack()
    await expect(submit).toBeEnabled()
    capMode = 'loading'
    await page.goto(authPreview)
    await expect(page.getByText('Checking sign-in options…')).toBeVisible()
    await expect(submit).toBeDisabled()
    await capture(page, 'auth', 'fixture-capabilities-loading', width)
    capMode = 'ready'
    await expect.poll(() => Boolean(releaseCaps)).toBe(true)
    releaseCaps()
    await expect(submit).toBeEnabled()
    capMode = 'error'
    await page.goto(authPreview)
    await expect(page.getByText('Sign-in options are unavailable.')).toBeVisible()
    await capture(page, 'auth', 'fixture-capabilities-error', width)
    capMode = 'ready'
    await page.getByRole('button', { name: 'Check options again' }).click()
    await expect(submit).toBeEnabled()
    await page.getByRole('button', { name: 'Close sign-in and return home' }).click()
    await expect(page.locator('.public-poster')).toBeVisible()
    await page.goBack()
    await expect(submit).toBeEnabled()
    assert.deepEqual(world, [], 'No public Phaser, world asset, game or socket')
    assert.deepEqual(unexpected, [])
    results.push({ width, height: 844, productionRoutes: 'PASS', fixtureStates: 'PASS', keyboard44pxContrast: 'PASS', documentsOtherAuth: 'PASS', browserBack: 'PASS', noWorldOrExternal: 'PASS' })
    await context.close()
  }
  assert.deepEqual(errors, [])
  for (const file of await readdir(path.join(temp, 'production/assets'))) if (file.endsWith('.js')) {
    const source = await readFile(path.join(temp, 'production/assets', file), 'utf8')
    for (const marker of ['Synthetic Auth fixture', 'Synthetic public fixture', 'PRIVATE_ACCOUNT_DIAGNOSTIC', 'TEST-2026.11.03']) assert(!source.includes(marker), `fixture leaked ${file}`)
  }
  const sources = ['frontend/src/features/public/ui/Landing.tsx', 'frontend/src/features/public/ui/PublicFrame.tsx', 'frontend/src/features/public/ui/public.css',
    'frontend/src/features/auth/ui/SignIn.tsx', 'frontend/src/features/auth/ui/sign-in.css',
    'frontend/src/features/public/__tests__/visual-adoption/browser.mjs', 'frontend/src/features/public/__tests__/visual-adoption/preview.html', 'frontend/src/features/public/__tests__/visual-adoption/preview.tsx',
    'frontend/src/features/auth/__tests__/visual-adoption/preview.html', 'frontend/src/features/auth/__tests__/visual-adoption/preview.tsx']
  const sourceHashes = {}
  for (const p of sources) sourceHashes[p] = hash(await readFile(path.join(root, p)))
  const report = { result: 'PASS', task: 'UIV02 public/auth actual adoption', assignmentBase: '453312121ae7539c027b9deb19009f9f569d4040',
    testedCheckoutHead: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), node: process.version, browser: browser.version(),
    sourceHashes, screenshots, results, requests, errors,
    scope: 'Actual production build routes with no configured event, plus clearly labeled synthetic canonical-Auth/public-document fixtures. No live login, approved artwork, native device or release acceptance.',
    reference: { path: 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg', sha256: hash(await readFile(path.join(root, 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg'))) } }
  await writeFile(path.join(dir, 'evidence/report.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(`PASS UIV02: actual production + labeled fixture at 360/390/430; ${screenshots.length} captures; keyboard, 44px, solid contrast, back, states, documents/auth consumers, no world/external/production fixture leaks.`)
} finally {
  await browser?.close()
  for (const server of servers) await new Promise(resolve => server.httpServer.close(resolve))
  await rm(temp, { recursive: true, force: true })
}
