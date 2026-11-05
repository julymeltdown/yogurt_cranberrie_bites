// Actual App/Auth/Phaser, synthetic canonical HTTP/C05. No physical/location claim.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createRequire } from 'node:module'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { resolve, extname } from 'node:path'
import { pathToFileURL } from 'node:url'
import { root, front, canonical, paths, bytes, gitSourcePlugin } from './git-inputs.mjs'
const require = createRequire(resolve(front, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')).href)
const react = (await import(pathToFileURL(require.resolve('@vitejs/plugin-react')).href)).default
const { chromium, expect } = require('@playwright/test')
const output = process.env.MCR_PI_OUTPUT, tmp = process.env.MCR_PI_TMP
const port = Number(process.env.MCR_PI_PORT), deadline = Date.parse(process.env.MCR_PI_WORK_DEADLINE)
assert(output.startsWith(root + '/docs/mcr/evidence/parade-ink-map-places/rooms-virtual-help-2026-11-05/') && tmp.startsWith('/private/tmp/mcr-pi11-rooms-') && port > 1023)
const input = JSON.parse(bytes('frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'))
const local = JSON.parse(bytes('frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/local-input.json'))
const fixture = resolve(front, 'src/app/world-entry/__tests__/parade-ink-map-places/rooms-virtual-help.html')
const sha = b => createHash('sha256').update(b).digest('hex')
// Exact canonical TEST_ONLY catalog pair; never a production release claim.
const policyRelease = { version: 1, applicationVersion: '0.1.0', buildId: 'a'.repeat(64), sourceRevision: 'b'.repeat(40) }
const report = { canonical, status: 'IN_PROGRESS', qualification: 'ACTUAL_APP_CONTROLLED_HTTP_C05_NOT_LIVE_OR_DEVICE',
  producerBoundary: 'Immutable source-qualified PI07 leaves published at ecb2c4adeaed06bc5b7ceb1e1f21d6b9a97e19c5; exact bytes retained',
  openCriterion: 'W04 party invitation UNIMPLEMENTED owners20f/Root1fc', checks: [] }
const state = page => page.evaluate(() => window.__ART06_DEFAULT__.state())
async function settle(page) { assert(Date.now() < deadline); await page.evaluate(() => document.fonts.ready); await page.waitForTimeout(160) }
async function capture(page, row, name) {
  await settle(page)
  const file = `${row.width}x${row.height}-${name}.png`
  await page.screenshot({ path: resolve(output, file) })
  row.images.push({ file, sha256: sha(await readFile(resolve(output, file))) })
}
async function targets(page, selector) {
  const rows = await page.locator(selector).evaluateAll(elements => elements.map(e => {
    const r = e.getBoundingClientRect(), s = getComputedStyle(e)
    return { name: e.getAttribute('aria-label') || e.textContent, width: r.width, height: r.height, display: s.display }
  }).filter(r => r.display !== 'none' && r.width > 0))
  assert(rows.length > 0)
  for (const row of rows) assert(row.width >= 44 && row.height >= 44, JSON.stringify(row))
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  return rows
}
async function focus(page, locator) {
  await locator.scrollIntoViewIfNeeded(); await locator.focus(); await page.keyboard.press('Tab'); await locator.focus()
  const r = await locator.evaluate(e => { const r = e.getBoundingClientRect(), s = getComputedStyle(e), hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)
    return { focused: document.activeElement === e, width: r.width, height: r.height, outline: s.outlineStyle, outlineWidth: s.outlineWidth, visible: r.top >= 0 && r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth, hit: hit === e || e.contains(hit) } })
  assert(r.focused && r.width >= 44 && r.height >= 44 && r.visible && r.hit && r.outline !== 'none' && parseFloat(r.outlineWidth) >= 2, JSON.stringify(r))
  return r
}
let server, browser, context, currentPage
await mkdir(output, { recursive: true })
try {
  server = await createServer({ root: front, configFile: false, cacheDir: resolve(tmp, 'vite'), logLevel: 'error',
    plugins: [gitSourcePlugin(), react(), { name: 'pi11-frozen-public-and-entry', configureServer(s) {
      s.middlewares.use(async (req, res, next) => {
        const pathname = new URL(req.url, 'http://localhost').pathname
        const path = pathname.startsWith('/__pi11_git/') ? decodeURIComponent(pathname.slice('/__pi11_git/'.length)) : 'frontend/public' + decodeURIComponent(pathname)
        if (paths.has(path)) { const value = bytes(path); const mime = { '.png': 'image/png', '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.json': 'application/json' }[extname(path)]; if (mime) res.setHeader('Content-Type', mime); res.end(value); return }
        if (!req.headers.accept?.includes('text/html')) return next()
        res.setHeader('Content-Type', 'text/html'); res.end(await s.transformIndexHtml(req.url, bytes(fixture).toString()))
      })
    } }], resolve: { dedupe: ['react', 'react-dom', 'zod'] },
    define: { 'import.meta.env.VITE_MCR_APPLICATION_RELEASE': JSON.stringify(policyRelease), 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(input.event.event_id), 'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(local)) },
    server: { host: '127.0.0.1', port, strictPort: true, watch: null }, optimizeDeps: { entries: [], noDiscovery: false, include: ['react', 'react-dom/client', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-router-dom', 'phaser', 'zod'] } })
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const [width, height] of [[360, 844], [390, 844], [430, 844], [390, 420]]) {
    const row = { width, height, images: [], errors: [], focus: [], targets: {} }; report.checks.push(row)
    context = await browser.newContext({ viewport: { width, height }, isMobile: true, hasTouch: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
    const page = await context.newPage(); currentPage = page; page.setDefaultTimeout(12000); page.on('pageerror', e => row.errors.push(e.message))
    await page.goto(`http://127.0.0.1:${port}/login?scenario=eligible-attendee-contract-fixture&era=PARADE`)
    await page.getByLabel('Email', { exact: true }).fill('map-fixture@example.test'); await page.getByLabel('Password', { exact: true }).fill('fixture-password')
    await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
    await expect.poll(async () => (await state(page)).activeSockets).toBe(1)
    row.policy = await page.evaluate(() => window.__PI11_ROOMS__.policyState())
    assert(row.policy.length > 0 && row.policy.every(read => read.authorized && read.recognized), 'Canonical protected policy gate must actually run')
    await expect(page.locator('.shell-world-host canvas')).toHaveCount(1); await settle(page)
    assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--pi-contract').trim()), '"pi01-photo2-v1"')
    await page.locator('.shell-world-host canvas').evaluate(e => Reflect.set(e, '__pi11Incumbent', true))
    const open = async name => { const tools = page.getByRole('button', { name: 'World destinations', exact: true }); if (await tools.getAttribute('aria-expanded') !== 'true') await tools.click(); await page.getByRole('navigation', { name: 'World tools', exact: true }).getByRole('link', { name, exact: true }).click(); await settle(page) }
    const back = async () => { await page.getByRole('button', { name: 'Back to previous panel', exact: true }).click(); await expect(page).toHaveURL(/\/world(?:\?|$)/); await settle(page) }
    await open('Rooms'); await expect(page.getByRole('heading', { name: 'Choose your room', exact: true })).toBeVisible()
    row.targets.rooms = await targets(page, '.world-rooms :is(button,a)')
    row.focus.push(await focus(page, page.getByRole('button', { name: 'Refresh rooms', exact: true })))
    await capture(page, row, 'rooms')
    const switchButton = page.locator('.world-access-list button:enabled').first()
    if (width === 390 && height === 844 && await switchButton.count()) {
      await switchButton.click(); await expect(page.getByText('Room switch confirmed.', { exact: true })).toBeVisible()
      row.switchConfirmed = true; await capture(page, row, 'room-confirmed')
    }
    await back(); await open('Nearby'); await expect(page.getByRole('heading', { name: 'Nearby in world', exact: true })).toBeVisible()
    await expect(page.getByText('People currently visible to you in this virtual room. This list does not show physical location or distance.', { exact: true })).toBeVisible()
    row.targets.nearby = await targets(page, '.world-nearby :is(button,a)'); await capture(page, row, 'nearby')
    const actor = page.locator('.world-nearby-person').first(), link = actor.locator('h3 a'), name = await link.innerText(), id = await link.getAttribute('data-nearby-user-id')
    const trigger = actor.getByRole('button', { name: `View ${name} avatar`, exact: true }), count = (await state(page)).requests.length
    row.focus.push(await focus(page, trigger)); await page.keyboard.press('Enter')
    const dialog = page.getByRole('dialog', { name, exact: true }); await expect(dialog).toBeVisible()
    await expect(dialog.getByText('Character unavailable', { exact: true })).toBeVisible()
    await expect(dialog.getByRole('link', { name: 'View profile', exact: true })).toHaveAttribute('href', `/people/${id}`)
    await expect(dialog.getByRole('button', { name: 'Close avatar details', exact: true })).toBeFocused()
    row.targets.popover = await targets(page, '.world-nearby-popover :is(button,a)'); await capture(page, row, 'nearby-popover')
    await page.keyboard.press('Escape'); await expect(dialog).toHaveCount(0); await expect(trigger).toBeFocused()
    assert.equal((await state(page)).requests.length, count, 'Opening/dismissing portraits adds no HTTP reads')
    if (width === 390 && height === 844) {
      await page.keyboard.press('Enter'); await expect(dialog).toBeVisible(); await page.evaluate(id => window.__PI11_ROOMS__.removePerson(id), id)
      await expect(dialog).toHaveCount(0); await expect(page.locator(`[data-nearby-user-id="${id}"]`)).toHaveCount(0)
      await expect(page.getByRole('region', { name: 'Other people in this world', exact: true })).toBeFocused(); row.privateWithdrawal = true
    }
    await back(); await open('Help'); await expect(page.getByRole('heading', { name: 'One world, at your pace', exact: true })).toBeVisible()
    row.targets.help = await targets(page, '.world-help a'); await capture(page, row, 'help')
    const end = page.getByRole('link', { name: 'Return to world', exact: true }); row.focus.push(await focus(page, end)); await capture(page, row, 'help-end-focus')
    await page.keyboard.press('Enter'); await expect(page).toHaveURL(/\/world(?:\?|$)/)
    assert(await page.locator('.shell-world-host canvas').evaluate(e => Reflect.get(e, '__pi11Incumbent')))
    const after = await state(page); assert.equal(after.activeSockets, 1); assert.equal(after.canvases, 1)
    assert.deepEqual(row.errors, []); row.passed = true
    await context.close(); context = null
  }
  report.status = 'PASS'
} catch (error) {
  report.status = 'FAIL'; report.failure = error.stack
  try { report.failureState = { path: new URL(currentPage.url()).pathname,
    retained: await state(currentPage), policy: await currentPage.evaluate(() => window.__PI11_ROOMS__.policyState()),
    body: (await currentPage.locator('body').innerText()).slice(0, 6000) } }
  catch (diagnostic) { report.failureState = { unavailable: diagnostic.message } }
  throw error
}
finally {
  await context?.close(); await browser?.close(); await server?.close()
  await writeFile(resolve(output, 'native-report.json'), JSON.stringify(report, null, 2) + '\n')
}
