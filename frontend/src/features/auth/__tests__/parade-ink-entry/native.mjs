import assert from 'node:assert/strict'
import { pathToFileURL } from 'node:url'
import { mkdir, writeFile, readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { compareHeaders } from './compare.mjs'
import { resolve } from 'node:path'
import { aliases, retained, frontend, compositePlugin } from './runtime.mjs'
assert.equal(process.env.PI10_GUARDED, '1', 'Finite admission/guard required')
const { createServer } = await import(pathToFileURL(retained.resolve('vite')).href)
const { chromium, expect } = retained('@playwright/test')
const output = process.env.PI10_OUTPUT, cache = process.env.PI10_CACHE
assert(output && cache && process.env.PI10_OVERLAY)
await mkdir(output, { recursive: true })
const server = await createServer({ root: frontend, configFile: false, cacheDir: cache,
  plugins: [compositePlugin()], resolve: { alias: aliases },
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('11111111-1111-4111-8111-111111111111') },
  optimizeDeps: { include: ['react', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client', 'react-router-dom', 'react-hook-form', '@hookform/resolvers/zod', 'browser-image-compression', '@tanstack/react-query', 'zod', 'zustand', 'ky', 'phaser'] },
  server: { host: '127.0.0.1', port: Number(process.env.PI10_PORT), strictPort: true,
    fs: { allow: [resolve(frontend, '..'), '/Users/lhs/Desktop/mcr/frontend', process.env.PI10_OVERLAY] } }, logLevel: 'warn' })
let browser, stage = 'server'
const headers = []
const report = { result: 'FAIL', kind: 'COMPOSITE_SOURCE_FIXTURE', shared: '77cfe94af06cac6b292ad7528352e4750e929e71', sharedReadset: 'eb80d722f513ce794fe35317a87702dd2878321a2d76f313d36265b3d593d5ef',
  limits: 'PI10 postimages plus exact PI01 source. Synthetic public/Auth ports; desktop Chromium, no live Auth, real World, physical iOS/Android or release acceptance.',
  screenshots: [], metrics: [], errors: [], states: [] }
async function requireFonts(page) {
  const fonts = await page.evaluate(async () => {
    const requests = ['400 18px "Parade Ink Text"', '700 26px "Parade Ink Display"']
    const loaded = await Promise.all(requests.map(request => document.fonts.load(request)))
    await document.fonts.ready
    return requests.map((request, index) => ({ request, loaded: loaded[index].length, ready: document.fonts.check(request) }))
  })
  assert(fonts.every(font => font.loaded > 0 && font.ready), `Bundled fonts not loaded: ${JSON.stringify(fonts)}`)
  report.fonts = fonts
}
try {
  await server.listen()
  const base = `http://127.0.0.1:${process.env.PI10_PORT}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, reducedMotion: 'reduce' })
    page.on('pageerror', error => report.errors.push(error.message))
    const normalProof = new Set(['landing', 'sign-in', 'signup-policy', 'verification', 'reset', 'callback', 'legal-unavailable', 'deletion-no-retained'])
    // Native03 consumes corrected shared base/chrome. Capture required primary entries at all widths.
    // Twelve failed-input historical PNGs survive;36 current-input PNGs =>48 family, within52/24MiB.
    // No completed full native run exists; changed shared CSS requires fresh geometry/state assertions.
    const image = async label => { if (width !== 390 && !normalProof.has(label)) return; const name = `${label}-${width}.png`; await page.screenshot({ path: resolve(output, name), fullPage: true }); report.screenshots.push(name); await writeFile(resolve(output, 'native-progress.json'), JSON.stringify({ ...report, stage }, null, 2) + '\n') }
    const go = async (path, mode = 'normal', extra = '') => {
      stage = `${path}:${mode}:${width}`
      await page.goto(`${base}/src/features/auth/__tests__/parade-ink-entry/preview.html?path=${encodeURIComponent(path)}&mode=${mode}${extra}`)
      await page.locator('main h1').waitFor(); await requireFonts(page)
    }
    const metrics = async label => {
      const result = await page.evaluate(() => {
        const frame = document.querySelector('.public-entry,.auth-sign-in,.parade-auth-entry')
        const heading = document.querySelector('h1'), style = getComputedStyle(heading)
        const badTargets = [...document.querySelectorAll('a,button,input,summary')].filter(e => {
          const b = e.getBoundingClientRect(); return b.width && b.height && b.bottom > 0 && getComputedStyle(e).visibility !== 'hidden' && (b.width < 43.9 || b.height < 43.9)
        }).map(e => ({ tag: e.tagName, text: e.textContent?.trim().slice(0, 50), id: e.id, width: e.getBoundingClientRect().width, height: e.getBoundingClientRect().height }))
        return { overflow: document.documentElement.scrollWidth > innerWidth, frame: frame.getBoundingClientRect().toJSON(),
          heading: { size: style.fontSize, leading: style.lineHeight, family: style.fontFamily }, badTargets,
          mountedWorld: Boolean(document.querySelector('canvas,[data-world-host]')) }
      })
      report.metrics.push({ label, width, ...result }); assert.equal(result.overflow, false, `Horizontal overflow ${label}`)
      assert.deepEqual(result.badTargets, [], `Small targets ${label}`); assert.equal(result.mountedWorld, false)
    }
    await go('/'); await expect(page.getByRole('link', { name: 'Sign in to enter', exact: true })).toBeVisible()
    await expect(page.locator('.landing-crowd')).toHaveAttribute('data-art-state', 'ready'); await expect(page.locator('.public-entry')).toHaveCSS('background-color', 'rgb(249, 249, 248)'); await expect(page.locator('.public-poster')).toHaveCSS('background-image', 'none'); await metrics('landing'); await image('landing'); headers.push({ name: 'landing', width, bytes: await page.locator('.public-header').screenshot() })
    await page.getByRole('link', { name: 'Sign in to enter', exact: true }).click()
    await expect(page.getByLabel('Email', { exact: true })).toBeVisible(); await requireFonts(page)
    await expect(page.locator('.concert-window')).toHaveCSS('background-image', 'none'); await metrics('sign-in'); await image('sign-in'); headers.push({ name: 'sign-in', width, bytes: await page.locator('.concert-window__titlebar').screenshot() })
    await page.getByLabel('Email', { exact: true }).focus(); await expect(page.getByLabel('Email', { exact: true })).toBeFocused(); assert.equal(await page.getByLabel('Email', { exact: true }).evaluate(e => getComputedStyle(e).outlineWidth), '3px'); await image('sign-in-focus')
    await page.getByLabel('Email', { exact: true }).fill('synthetic@example.invalid'); await page.getByLabel('Password', { exact: true }).fill('synthetic-password')
    const signIn = page.getByRole('button', { name: 'Sign in with email', exact: true })
    await expect(signIn).toHaveCSS('background-color', 'rgb(146, 37, 50)')
    await expect(signIn).toHaveCSS('font-family', /Parade Ink Display/)
    if (width === 390) { const b = await signIn.boundingBox(); await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2); await page.mouse.down(); assert.equal(await signIn.evaluate(e => e.matches(':active')), true); await page.mouse.up() }
    else await signIn.click()
    await expect(page.getByText('Sign-in could not be completed. Try again when the service is available.', { exact: true })).toBeVisible()
    await expect(page.getByLabel('Password', { exact: true })).toHaveValue(''); await image('sign-in-error')
    await go('/signup'); await page.getByRole('button', { name: 'Load current agreements', exact: true }).click()
    await page.getByRole('checkbox').waitFor(); await metrics('signup-policy'); await image('signup-policy')
    await page.getByRole('checkbox').check(); await expect(page.getByRole('button', { name: 'Create account', exact: true })).toBeEnabled();
    await go('/verify-email'); await metrics('verification'); await image('verification')
    await go('/reset-password'); await metrics('reset'); await image('reset')
    await page.getByLabel('Email', { exact: true }).fill('synthetic@example.invalid'); await page.getByRole('button', { name: 'Request reset', exact: true }).click()
    await expect(page.getByRole('status')).toContainText('Account details and email delivery are not confirmed here.'); await image('reset-neutral-result')
    await go('/auth/callback?error=access_denied&returnTo=%2Ffeed'); await expect(page.getByRole('heading', { name: 'Sign-in cancelled', exact: true })).toBeFocused()
    await metrics('callback'); await image('callback')
    await go('/legal/terms'); await expect(page.getByText('This document is not available yet.', { exact: true })).toBeVisible(); await metrics('legal-unavailable'); await image('legal-unavailable')
    await go('/legal/privacy', 'normal', '&documents=published'); await expect(page.getByRole('note')).toContainText('not approved policy'); await metrics('legal-synthetic'); await image('legal-synthetic')
    await go('/account/deleted'); await expect(page.getByRole('status')).toContainText('No deletion completion is confirmed'); await metrics('deletion-no-retained'); await image('deletion-no-retained')
    await go('/login', 'consent'); await expect(page.getByRole('heading', { name: 'Review current agreements', exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Load current agreements', exact: true }).click(); await page.getByRole('checkbox').waitFor(); await metrics('consent'); await image('consent')
    await go('/login', 'inactive'); await expect(page.getByRole('heading', { name: 'Account entry unavailable', exact: true })).toBeVisible(); await metrics('admission-inactive'); await image('admission-inactive')
    await go('/login', 'rate-limited'); await page.getByLabel('Email', { exact: true }).fill('synthetic@example.invalid'); await page.getByLabel('Password', { exact: true }).fill('synthetic-password')
    await page.getByRole('button', { name: 'Sign in with email', exact: true }).click(); await expect(page.getByRole('status')).toContainText('Too many attempts.'); await image('sign-in-rate-limited')
    await go('/login', 'pending'); await page.getByLabel('Email', { exact: true }).fill('synthetic@example.invalid'); await page.getByLabel('Password', { exact: true }).fill('synthetic-password')
    await page.getByRole('button', { name: 'Sign in with email', exact: true }).click(); await expect(page.getByRole('button', { name: 'Signing in…', exact: true })).toBeDisabled(); await image('sign-in-pending')
    await go('/reset-password'); await page.setViewportSize({ width, height: 430 }); await page.getByRole('button', { name: 'I have a reset token', exact: true }).click()
    await page.getByLabel('New password', { exact: true }).focus(); await page.getByRole('button', { name: 'Reset password', exact: true }).scrollIntoViewIfNeeded()
    await expect(page.getByRole('button', { name: 'Reset password', exact: true })).toBeInViewport(); await metrics('short-reset'); await image('short-reset')
    if (width === 390) {
      const cdp = await page.context().newCDPSession(page)
      await cdp.send('Emulation.setSafeAreaInsetsOverride', { insets: { top: 20, bottom: 34, left: 0, right: 0 } })
      const safe = await page.locator('.parade-auth-entry').evaluate(e => ({ top: getComputedStyle(e).marginTop, bottom: getComputedStyle(e).marginBottom }))
      assert.equal(safe.top, '20px'); assert.equal(safe.bottom, '34px'); report.safeArea = { simulated: true, ...safe }; await image('short-safe-area')
      await cdp.detach()
    }
    await page.close()
    report.states.push({ width, checked: true, focus: true, pending: true, disabled: true, error: true, shortHeight: true, truthfulUnavailable: true })
  }
  // Actual index.html -> main.tsx -> default App, with explicit HTTP fixtures only.
  stage = 'actual-default-entry'
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, reducedMotion: 'reduce' })
    page.on('pageerror', error => report.errors.push(error.message))
    await page.route(`${base}/api/**`, route => route.fulfill({ status: 503, contentType: 'application/json', body: '{}' }))
    await page.goto(base); await expect(page.getByRole('heading', { name: 'One world. Every era.', exact: true })).toBeVisible(); await requireFonts(page)
    await expect(page.getByRole('link', { name: 'Sign in to enter', exact: true })).toBeVisible()

    if (width === 390) { const name = `actual-default-entry-${width}.png`; await page.screenshot({ path: resolve(output, name), fullPage: true }); report.screenshots.push(name) }
    assert.equal(await page.locator('canvas,[data-world-host]').count(), 0); await page.close()
  }
  stage = 'fixed-reference-comparison'
  const reference = await readFile(resolve(frontend, '../docs/mcr/design/parade-ink-2026-11-05/parade-ink.jpg'))
  assert.equal(createHash('sha256').update(reference).digest('hex'), '9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27')
  report.comparisons = await compareHeaders(browser, reference, headers, output)
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { stage, message: String(error), stack: error.stack }; throw error }
finally { await browser?.close(); await server.close(); await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n') }
