// Uses installed Chrome and only owned ephemeral servers/contexts, never a user profile.
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile, readdir } from 'node:fs/promises'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_FE08_EVIDENCE ?? '/tmp/mcr-fe08-evidence'
await mkdir(evidence, { recursive: true })
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser, production
const errors = [], results = []
try {
  await dev.listen()
  const base = `http://127.0.0.1:${dev.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const fixture = (query = '') => `${base}/src/features/public/__tests__/fixtures/preview.html${query}`
  async function noOverflow(page) { assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow') }
  async function targetsAndContrast(page) {
    for (const link of await page.locator('.public-entry a:not(.public-skip), .public-entry button').all()) {
      const box = await link.boundingBox(); assert(box && box.width >= 44 && box.height >= 44, 'public target below 44px')
    }
    const failures = await page.evaluate(() => {
      const rgb = color => color.match(/[0-9.]+/g).slice(0, 3).map(Number)
      const lum = color => rgb(color).map(x => { const c = x / 255; return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4 }).reduce((sum, x, i) => sum + x * [.2126, .7152, .0722][i], 0)
      return [...document.querySelectorAll('.public-entry h1, .public-entry h2, .public-entry p, .public-entry a, .public-entry dt, .public-entry dd, .public-entry button')].filter(el => {
        if (el.disabled || !el.textContent.trim()) return false
        let node = el, background = 'rgba(0, 0, 0, 0)'
        while (node && background === 'rgba(0, 0, 0, 0)') { background = getComputedStyle(node).backgroundColor; node = node.parentElement }
        const style = getComputedStyle(el), a = lum(style.color), b = lum(background), ratio = (Math.max(a, b) + .05) / (Math.min(a, b) + .05)
        return ratio < (parseFloat(style.fontSize) >= 24 ? 3 : 4.5)
      }).map(el => el.textContent)
    })
    assert.deepEqual(failures, [], 'enabled public text contrast')
  }
  for (const [width, height] of [[360, 800], [390, 844], [430, 932], [1280, 800]]) {
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' }), page = await context.newPage()
    const unwanted = []
    page.on('pageerror', error => errors.push(error.message))
    page.on('request', req => { const url = req.url(); if (/\/src\/world\/|\/phaser(?:\.|\/)|\/world\/avatars\//i.test(url)) unwanted.push(url) })
    page.on('websocket', socket => { if (new URL(socket.url()).pathname === '/world-ws') unwanted.push(socket.url()) })
    await page.goto(fixture())
    await expect(page.getByRole('heading', { name: /One world.*Every era/ })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Sign in to enter' })).toHaveAttribute('href', '/login?returnTo=%2Fworld')
    assert.equal(await page.evaluate(() => window.__FE08_TEST__.stats.mounts), 0)
    await page.keyboard.press('Tab')
    await expect(page.getByRole('link', { name: 'Skip to content' })).toBeFocused()
    await page.keyboard.press('Enter')
    await expect(page.locator('#public-main')).toBeFocused()
    await page.getByRole('link', { name: 'Sign in to enter' }).focus()
    assert.notEqual(await page.getByRole('link', { name: 'Sign in to enter' }).evaluate(el => getComputedStyle(el).outlineStyle), 'none')
    await expect(page.getByRole('link', { name: 'Skip to content' })).not.toBeInViewport()
    await targetsAndContrast(page); await noOverflow(page)
    // Chrome full-page capture offsets fixed elements by current scroll; capture from the top.
    await page.evaluate(() => window.scrollTo(0, 0))
    await page.screenshot({ path: `${evidence}/landing-${width}.png`, fullPage: true })
    await page.getByRole('navigation', { name: 'Public information' }).getByRole('link', { name: 'Terms', exact: true }).click()
    await expect(page.getByText('Opening this document…')).toBeVisible()
    await expect(page.getByText('TEST-2026.11.03')).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Terms', exact: true })).toBeFocused()
    await expect(page.getByText('Synthetic document preview for UI testing. This is not approved policy or operator contact information.')).toBeVisible()
    await expect(page.locator('time').first()).toHaveAttribute('datetime', '2026-11-03T00:00:00Z')
    await targetsAndContrast(page); await noOverflow(page)
    await page.screenshot({ path: `${evidence}/synthetic-document-${width}.png`, fullPage: true })
    await page.getByRole('link', { name: 'Open the synthetic privacy page' }).click()
    await expect(page.getByRole('heading', { name: 'Privacy', exact: true })).toBeFocused()
    await page.goBack()
    await expect(page.getByRole('heading', { name: 'Terms', exact: true })).toBeFocused()
    await page.getByRole('link', { name: 'Back to home' }).click()
    await expect(page.getByRole('heading', { name: /One world.*Every era/ })).toBeVisible()
    await page.setViewportSize({ width, height: 420 })
    await page.getByRole('link', { name: 'Sign in to enter' }).scrollIntoViewIfNeeded()
    await expect(page.getByRole('link', { name: 'Sign in to enter' })).toBeInViewport()
    await noOverflow(page); await page.setViewportSize({ width, height })
    assert.deepEqual(unwanted, [], 'anonymous public page imported/allocated world resources')
    await page.evaluate(() => window.__FE08_TEST__.ready())
    await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
    assert.equal(new URL(page.url()).pathname, '/world')
    assert.equal(await page.evaluate(() => window.__FE08_TEST__.stats.peakGames), 1)
    await page.evaluate(() => window.__FE08_TEST__.revoke())
    await expect.poll(() => page.evaluate(() => window.__FE08_TEST__.stats.games)).toBe(0)
    await page.goto(fixture('?entry=unavailable'))
    await expect(page.getByRole('button', { name: 'Enter the world', exact: true })).toBeDisabled()
    await expect(page.getByText('World entry is not open yet.', { exact: true })).toBeVisible()
    await page.screenshot({ path: `${evidence}/unavailable-entry-${width}.png`, fullPage: true })
    await page.goto(fixture('?route=%2Flegal%2Fterms&mode=error'))
    await expect(page.getByRole('alert')).toHaveText('We could not open this document. Please try again.Try again')
    assert(!(await page.locator('body').innerText()).includes('SYNTHETIC_FE08_RAW_ERROR_MUST_NOT_RENDER'))
    await page.evaluate(() => window.__FE08_TEST__.mode('published'))
    await page.getByRole('button', { name: 'Try again' }).click()
    await expect(page.getByText('TEST-2026.11.03')).toBeVisible()
    await context.close()
    results.push({ width, height, anonymousNoWorld: 'PASS', readyReplace: 'PASS', documents: 'PASS', focusContrastTargets: 'PASS', reducedHeight: 'PASS' })
  }
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const productionBase = `http://127.0.0.1:${production.httpServer.address().port}`
  const context = await browser.newContext({ viewport: { width: 390, height: 844 } }), page = await context.newPage()
  const requests = [], sockets = []
  page.on('request', req => { const path = new URL(req.url()).pathname; if (path.startsWith('/api/') || path.startsWith('/world/') || /phaser/i.test(path)) requests.push(path) })
  page.on('websocket', socket => sockets.push(socket.url()))
  await page.goto(productionBase)
  await expect(page.getByRole('heading', { name: /One world.*Every era/ })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Enter the world', exact: true })).toBeDisabled()
  await page.screenshot({ path: `${evidence}/production-home-390.png`, fullPage: true })
  for (const key of ['terms', 'privacy', 'safety', 'contact']) {
    await page.getByRole('navigation', { name: 'Public information' }).getByRole('link', { name: key[0].toUpperCase() + key.slice(1), exact: true }).click()
    await expect(page.getByText(key === 'contact' ? 'Operator contact details are not available yet.' : 'This document is not available yet.', { exact: true })).toBeVisible()
    assert.equal(await page.locator('time').count(), 0)
    assert(!(await page.locator('body').innerText()).includes('SYNTHETIC_FE08_DOCUMENT'))
  }
  await page.screenshot({ path: `${evidence}/production-contact-390.png`, fullPage: true })
  assert.equal(await page.evaluate(() => typeof window.__FE08_TEST__), 'undefined')
  assert.deepEqual(requests, []); assert.deepEqual(sockets, [])
  assert.equal(await page.locator('canvas,[data-world-host]').count(), 0)
  for (const file of await readdir('dist/assets')) if (file.endsWith('.js')) {
    const source = await readFile(`dist/assets/${file}`, 'utf8')
    for (const marker of ['SYNTHETIC_FE08_DOCUMENT', 'TEST-2026.11.03', '__FE08_TEST__', 'fixture-only-memory-token']) assert(!source.includes(marker), `fixture leaked into ${file}`)
  }
  await context.close(); assert.deepEqual(errors, [])
  await writeFile(`${evidence}/browser-results.json`, JSON.stringify({ status: 'MOCK_READY', browser: await browser.version(), results,
    production: { noDocumentEndpointOrWorldRequests: 'PASS', defaultUnavailable: 'PASS', noSockets: 'PASS', fixtureExclusion: 'PASS' },
    limitations: 'Synthetic document provider and mock world leases only. Approved legal/publication/contact/retention contract, real document delivery, actual Phaser/socket and device/IN01 gates NOT_RUN.' }, null, 2))
  console.log(JSON.stringify({ status: 'PASS', viewports: results.length, evidence }))
} finally {
  await browser?.close(); await dev.close(); if (production) await new Promise(resolve => production.httpServer.close(resolve))
}
