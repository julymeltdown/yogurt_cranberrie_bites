import assert from 'node:assert/strict'
import { build, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const frontend = resolve(import.meta.dirname, '../../../..'), fixture = '/src/app/errors/__tests__/preview.html'
const outDir = '/tmp/mcr-degraded-build', evidence = resolve(frontend, 'src/features/connection-status/__tests__/evidence')
await build({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner', cacheDir: '/tmp/mcr-degraded-cache', logLevel: 'warn',
  build: { outDir, emptyOutDir: true, rollupOptions: { input: [resolve(frontend, 'index.html'), resolve(frontend, `.${fixture}`)] } } })
const server = await preview({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner', build: { outDir },
  plugins: [{ name: 'degraded-fixture-document', configurePreviewServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (new URL(request.url, 'http://127.0.0.1').pathname !== '/world/help') return next()
      response.setHeader('Content-Type', 'text/html'); response.end(await readFile(resolve(outDir, `.${fixture}`), 'utf8'))
    })
  } }], preview: { host: '127.0.0.1', port: 0, strictPort: false } })
const report = { result: 'FAIL', scope: 'Synthetic canonical Auth, existing WorldShell/WorldHost, actual FE14 Phaser and FE17/MockWorldTransport. Synthetic visibilitychange, not OS background or live service/device qualification.', browser: '', widths: [], screenshots: [], errors: [] }
let browser, stage = 'start'
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  const base = `http://127.0.0.1:${server.httpServer.address().port}`
  for (const [width, height] of [[360,800], [390,844], [430,932]]) {
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    const page = await context.newPage(); page.on('pageerror', e => report.errors.push(e.message))
    const state = () => page.evaluate(() => window.__DEGRADED__.state())
    const set = value => page.evaluate(value => window.__DEGRADED__.set(value), value)
    const kind = async value => expect.poll(async () => (await state()).status.kind).toBe(value)
    const open = async () => {
      await page.goto(`${base}${fixture}`)
      await expect.poll(async () => (await state()).runtime).toBe('ready')
      await expect.poll(async () => (await state()).replication).toBe('opening')
      await page.evaluate(() => window.__DEGRADED__.admit()); await kind('ready')
    }
    const shot = async name => {
      await page.locator('.shell-panel-body').evaluate(el => { el.scrollTop = 0 })
      const file = `${name}-${width}.png`; await page.screenshot({ path: resolve(evidence, file), fullPage: true }); report.screenshots.push(file)
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
      const small = await page.locator('.connection-status').evaluateAll(els => els.flatMap(el => [...el.querySelectorAll('button,a')]).filter(el => el.checkVisibility()).map(el => ({ text: el.textContent, width: el.getBoundingClientRect().width, height: el.getBoundingClientRect().height })).filter(r => r.width < 43.9 || r.height < 43.9))
      assert.deepEqual(small, [])
      if (['offline', 'maintenance', '404'].includes(name)) {
        const fallback = page.locator('.connection-status').getByRole('link', { name: 'Back to home', exact: true })
        await fallback.scrollIntoViewIfNeeded(); await expect(fallback).toBeInViewport()
        const actions = `${name}-actions-${width}.png`; await page.screenshot({ path: resolve(evidence, actions), fullPage: true }); report.screenshots.push(actions)
      }
    }
    stage = `${width}/ready`; await open(); await shot('ready')
    assert.equal((await state()).subscriptions, 1); assert.equal((await state()).games, 1)
    stage = `${width}/background`
    await page.evaluate(() => { window.__DEGRADED__.hidden(true); window.__DEGRADED__.hidden(true) }); await kind('suspended')
    const paused = await state(); assert.equal(paused.engineVisible, false); assert.equal(paused.activeSockets, 0); assert.equal(paused.games, 1)
    await shot('paused')
    await page.evaluate(() => { window.__DEGRADED__.hidden(false); window.__DEGRADED__.hidden(false) })
    await expect.poll(async () => (await state()).replication).toBe('opening')
    assert.equal((await state()).sockets, 2); assert.equal((await state()).mounts, 1); assert.equal((await state()).activeSockets, 1)
    await page.evaluate(() => window.__DEGRADED__.admit()); await kind('ready')
    stage = `${width}/offline`; await set('offline'); await kind('offline'); await shot('offline')
    assert.equal((await state()).retryCalls, 0)
    await set('online'); await kind('resync'); assert.equal((await state()).status.stale, true)
    await set('closed'); await kind('unavailable'); await shot('unavailable')
    const retry = page.getByRole('button', { name: 'Reconnect', exact: true }); await retry.focus(); await page.keyboard.press('Enter')
    await expect.poll(async () => (await state()).replication).toBe('opening'); await kind('reconnecting'); await shot('reconnecting')
    assert.equal((await state()).retryCalls, 1)
    await set('timeout'); await kind('unavailable'); await shot('timed-out')
    assert.equal((await state()).status.pending, false)
    await page.evaluate(() => window.__DEGRADED__.admit()); await kind('ready')
    stage = `${width}/resync`; await set('resync'); await kind('resync'); await shot('resync')
    assert.equal((await state()).resyncs, 1); assert.equal((await state()).status.stale, true)
    await page.evaluate(() => window.__DEGRADED__.resnapshot()); await kind('ready'); assert.equal((await state()).status.stale, false)
    stage = `${width}/map-version`; await set('map-version'); await kind('version'); await shot('map-version')
    assert.equal(await page.getByRole('button', { name: 'Reconnect' }).count(), 0)
    stage = `${width}/reload`
    await page.getByRole('button', { name: 'Reload page', exact: true }).focus()
    await Promise.all([page.waitForEvent('load'), page.keyboard.press('Enter')])
    await expect.poll(async () => (await state()).runtime).toBe('ready')
    await expect.poll(async () => (await state()).replication).toBe('opening')
    await page.evaluate(() => window.__DEGRADED__.admit()); await kind('ready')
    assert.equal((await state()).mounts, 1); assert.equal((await state()).games, 1)
    stage = `${width}/maintenance`; await set('maintenance'); await kind('maintenance'); await shot('maintenance')
    await set('version'); await kind('version'); await shot('version')
    stage = `${width}/404`; await set('404'); await expect(page.getByRole('heading', { name: 'Page not found' })).toBeVisible(); await shot('404')
    await page.getByRole('link', { name: 'Back to world', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('navigation', { name: 'World tools' })).toBeVisible()
    assert.equal((await state()).mounts, 1); assert.equal((await state()).peakGames, 1); assert.equal((await state()).subscriptions, 1)
    await page.evaluate(() => window.__DEGRADED__.navigate('/world/help')); await expect(page.getByRole('heading', { name: 'Page not found' })).toBeVisible()
    await page.getByRole('link', { name: 'Back to home', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect.poll(async () => (await state()).games).toBe(0); assert.equal((await state()).activeSockets, 0); assert.equal((await state()).subscriptions, 0)
    await page.evaluate(() => window.__DEGRADED__.dispose()); assert.equal((await state()).canvases, 0)
    report.widths.push({ width, height, result: 'PASS', oneGame: true, oneActiveMockSocket: true, oneObserverSubscription: true, boundedWait: true, keyboardReconnectReloadFallback: true, foregroundOwnedByFE17: true, resyncRequests: 1, cleanup: true })
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.errors.push(`${stage}: ${error.stack ?? error}`); throw error }
finally { await browser?.close(); await server.close(); await writeFile(resolve(evidence, 'native.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report))
