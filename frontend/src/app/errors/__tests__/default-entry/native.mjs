import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const frontend = resolve(import.meta.dirname, '../../../../..')
const evidence = resolve(process.env.FE41_OUTPUT ?? resolve(frontend, '../docs/mcr/evidence/fe41-default-entry'))
const scratch = process.env.FE41_SCRATCH
if (!scratch) throw Error('FE41_SCRATCH must name the allocated private scratch directory')
await mkdir(evidence, { recursive: true })
const fixture = '/src/app/errors/__tests__/default-entry/preview.html'
const run = new Date().toISOString().replaceAll(/[:.]/g, '-')
const input = { revision: 'TEST_ONLY_FE41_not_C05_authority', reviewedBy: 'Synthetic default-entry fixture',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
const server = await createServer({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner',
  cacheDir: resolve(scratch, 'vite'), logLevel: 'warn',
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
    'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)) },
  plugins: [{ name: 'fe41-actual-main-fixture', configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (!request.headers.accept?.includes('text/html')) return next()
      const html = await readFile(resolve(frontend, `.${fixture}`), 'utf8')
      response.setHeader('Content-Type', 'text/html'); response.end(await server.transformIndexHtml(request.url, html))
    })
  } }], server: { host: '127.0.0.1', port: 0, strictPort: false, watch: null } })
const report = { run, result: 'FAIL', scope: 'Actual main.tsx/default production composition, UI login, synthetic HTTP/socket/map admission, selected source characters; no live backend/device acceptance.', widths: [], screenshots: [], errors: [] }
let browser, stage = 'start'
try {
  await server.listen(); const base = `http://127.0.0.1:${server.httpServer.address().port}`; report.server = base
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const [width, height] of [[360,800], [390,844], [430,932]]) {
    stage = `${width}/login`
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    const page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
    const state = () => page.evaluate(() => window.__FE41_ENTRY__.state())
    const region = page.getByRole('region', { name: 'World connection status' })
    const login = async () => {
      await expect(page.getByRole('button', { name: 'Sign in with email' })).toBeEnabled()
      await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
      await page.getByLabel('Password', { exact: true }).fill('fixture-password')
      await page.getByRole('button', { name: 'Sign in with email' }).click()
      await expect(page.getByRole('button', { name: 'World tools' })).toBeVisible()
      await expect.poll(async () => (await state()).canvases).toBe(1)
      await expect(region).toHaveCount(0)
    }
    const shot = async name => {
      const file = `${run}-${name}-${width}.png`; await page.screenshot({ path: resolve(evidence, file), fullPage: true }); report.screenshots.push(file)
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
      assert.deepEqual(await region.locator('a,button').evaluateAll(els => els.filter(el => el.checkVisibility()).map(el => ({ text: el.textContent, w: el.getBoundingClientRect().width, h: el.getBoundingClientRect().height })).filter(r => r.w < 43.9 || r.h < 43.9)), [])
    }
    await page.goto(`${base}/login`); await login()
    await shot('ready')
    const initial = await state(); assert.equal(initial.active, 1); assert.equal(initial.peak, 1)
    // Same root controller and WorldHost survive actual panel navigation/coverage.
    await page.getByRole('button', { name: 'World tools' }).click()
    await page.getByRole('navigation', { name: 'World tools' }).getByRole('link', { name: 'Rooms' }).click()
    await expect(region).toHaveCount(0); assert.equal((await state()).canvases, 1)
    assert.equal((await state()).sockets, initial.sockets, 'Covering the engine retains live presence without another connection')
    assert.equal((await state()).active, 1)
    await shot('covered-panel')
    await page.getByRole('link', { name: 'Back to world', exact: true }).click()
    await expect(region).toHaveCount(0); assert.equal((await state()).canvases, 1)
    const recovered = await state()
    stage = `${width}/offline`; await page.evaluate(() => window.__FE41_ENTRY__.offline(true))
    await expect(region).toContainText('You are offline'); await expect(region).toContainText('may be out of date'); await shot('offline')
    await page.evaluate(() => window.__FE41_ENTRY__.offline(false))
    await expect(region).toContainText('Updating the world'); assert.equal((await state()).sockets, recovered.sockets)
    stage = `${width}/manual-reconnect`; await page.evaluate(() => { window.__FE41_ENTRY__.hold(true); window.__FE41_ENTRY__.close() })
    await expect(region.getByRole('button', { name: 'Reconnect', exact: true })).toBeVisible(); await shot('unavailable')
    await region.getByRole('button', { name: 'Reconnect', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect(region).toContainText('Connecting to the world'); await expect.poll(async () => (await state()).active).toBe(1)
    assert.equal((await state()).sockets, recovered.sockets + 1); assert.equal((await state()).canvases, 1)
    await page.evaluate(() => window.__FE41_ENTRY__.admit()); await expect(region).toHaveCount(0)
    stage = `${width}/short-viewport`; await page.setViewportSize({ width, height: 480 })
    await page.evaluate(() => window.__FE41_ENTRY__.close()); await expect(region).toContainText('World connection unavailable')
    const home = region.getByRole('link', { name: 'Back to home', exact: true }); await home.scrollIntoViewIfNeeded(); await expect(home).toBeInViewport(); await shot('short-actions')
    stage = `${width}/reload`; await region.getByRole('button', { name: 'Reload page', exact: true }).focus()
    await Promise.all([page.waitForEvent('load'), page.keyboard.press('Enter')])
    // The fixture keeps Auth tokens in memory; reload correctly returns to the access gate.
    await page.getByRole('link', { name: 'Sign in', exact: true }).click(); await login()
    await page.setViewportSize({ width, height })
    stage = `${width}/shell-404-and-home-cleanup`
    await page.getByRole('button', { name: 'World tools' }).click()
    await page.getByRole('navigation', { name: 'World tools' }).getByRole('link', { name: 'Help' }).click()
    // Help is a registered placeholder. Exercise an actual unknown in-shell URL via browser history.
    await page.evaluate(() => { history.pushState(null, '', '/world/missing-fixture-route'); dispatchEvent(new PopStateEvent('popstate')) })
    // Embedded ConcertWindow titlebars are deliberately hidden by the outer panel.
    await expect(page.locator('#panel-title')).toHaveText('Unavailable')
    await expect(page.getByRole('region', { name: 'Page not found' })).toBeVisible()
    await expect(page.getByRole('status')).toContainText('This page is unavailable or no longer exists.')
    assert.equal((await state()).canvases, 1); await expect(region).toHaveCount(0)
    const route = page.locator('.connection-status').filter({ has: page.getByRole('heading', { name: 'Page not found' }) })
    await route.getByRole('link', { name: 'Back to world', exact: true }).click()
    await expect(region).toHaveCount(0)
    await page.evaluate(() => window.__FE41_ENTRY__.offline(true))
    await region.getByRole('link', { name: 'Back to home', exact: true }).click()
    await expect.poll(async () => (await state()).canvases).toBe(0)
    assert.equal((await state()).active, 0); await expect(region).toHaveCount(0)
    stage = `${width}/safe-404`; await page.goto(`${base}/missing-private-fixture-token?access_token=do-not-render`)
    await expect(page.getByRole('heading', { name: 'Page not found' })).toBeVisible()
    await expect(page.locator('body')).not.toContainText('do-not-render'); await expect(page.locator('body')).not.toContainText('missing-private-fixture-token')
    await shot('safe-404'); await page.getByRole('link', { name: 'Back to home', exact: true }).click()
    await expect.poll(async () => (await state()).canvases).toBe(0); assert.equal((await state()).active, 0)
    report.widths.push({ width, height, result: 'PASS', actualMain: true, uiLogin: true, oneCanvasAndActiveSocket: true, onlineDoesNotRetry: true, keyboardReconnectReload: true, shortActionsReachable: true, safe404: true })
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.errors.push(`${stage}: ${error.stack ?? error}`); throw error }
finally { await browser?.close(); await server.close(); await writeFile(resolve(evidence, `${run}-native.json`), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report))
