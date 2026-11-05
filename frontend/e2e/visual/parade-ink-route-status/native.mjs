// PI12 COMPOSITE_SOURCE_FIXTURE. Must run only beneath a fresh finite dual-guard offer.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile, mkdir, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createRequire } from 'node:module'
import { pathToFileURL } from 'node:url'
import { foundationOverlay } from './foundation-overlay.mjs'

const root = resolve(import.meta.dirname, '../../../..'), front = resolve(root, 'frontend')
const require = createRequire(resolve(process.env.MCR_READONLY_FRONTEND ?? front, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')).href)
// Vite setup follows actual qualified PI01 producer 77cfe94; cache remains private.
const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')).href)
const { chromium, expect } = require('@playwright/test')
const output = process.env.MCR_PI12_OUTPUT, temporary = process.env.MCR_PI12_TMP
const deadline = Date.parse(process.env.MCR_PI12_WORK_DEADLINE ?? ''), port = Number(process.env.MCR_PI12_PORT)
const group = process.env.MCR_PI12_GROUP
const retainedImages = JSON.parse(process.env.MCR_PI12_RETAINED_IMAGES ?? '{}')
assert(output?.startsWith(root + '/docs/mcr/evidence/parade-ink-report-error-network/'))
assert(temporary?.startsWith('/private/tmp/mcr-pi12-') && deadline > Date.now() && port > 1023)
assert(['network', 'report-errors', 'both'].includes(group))
const report = { qualification: 'COMPOSITE_SOURCE_FIXTURE; controlled HTTP/C05/service; not live/device/QA05', group, status: 'IN_PROGRESS', checks: [], retainedChecks: [] }
const state = page => page.evaluate(() => window.__PI12__.state())
const act = (page, method, value) => page.evaluate(([m, v]) => window.__PI12__[m](v), [method, value])
let server, browser, context
async function guard() { const s = await statfs(root); assert(Date.now() < deadline && s.bavail * s.bsize >= 8 * 1024 ** 3) }
async function capture(page, result, label) {
  await guard(); await page.evaluate(() => document.fonts.ready)
  const geometry = await page.evaluate(() => {
    const controls = [...document.querySelectorAll('.connection-status :is(button,a),.pi-report :is(button,select,textarea),.pi-route-state :is(button,a),.pi-report-entry > section > button')]
      .filter(e => e.getClientRects().length).map(e => { const r = e.getBoundingClientRect(); return { name: e.textContent?.trim() || e.getAttribute('aria-label'), width: r.width, height: r.height } })
    return { overflow: document.documentElement.scrollWidth > innerWidth, controls,
      contract: getComputedStyle(document.documentElement).getPropertyValue('--pi-contract').trim(),
      raw: /PRIVATE_HTTP_PAYLOAD|PRIVATE_PAYLOAD_MUST|PRIVATE_REPORT_TRANSPORT/.test(document.body.textContent) }
  })
  assert.equal(geometry.contract, '"pi01-photo2-v1"'); assert(!geometry.overflow && !geometry.raw)
  for (const c of geometry.controls) assert(c.width >= 43.9 && c.height >= 43.9, `44px target: ${c.name}`)
  const proof = await state(page); assert(proof.peak <= 1 && proof.games <= 1 && proof.activeSockets <= 1 && proof.canvases <= 1)
  // All frozen cases retain geometry/state proof; select 39 PNGs across both groups.
  const selected = !result.scenario.startsWith('report-') && !result.scenario.startsWith('http-')
    ? result.width === 390 && result.height === 844 || ['offline', 'version'].includes(label)
    : result.width === 390 && result.height === 844 || ['report-ready', 'report-pending', 'http-401', 'http-503'].includes(label)
  const screenshot = selected ? `${result.width}-${result.height}-${label}.png` : null
  const retainedImage = screenshot ? retainedImages[screenshot] : null
  if (retainedImage) {
    const source = resolve(root, retainedImage.path)
    assert(source.startsWith(root + '/docs/mcr/evidence/parade-ink-report-error-network/'))
    assert.equal(createHash('sha256').update(await readFile(source)).digest('hex'), retainedImage.sha256)
  } else if (screenshot) await page.screenshot({ path: resolve(output, screenshot), fullPage: true })
  result.captures.push({ label, geometry, proof, screenshot, retainedImage })
}
try {
  await mkdir(output, { recursive: true })
  let retainedNative = null
  const retained = process.env.MCR_PI12_RETAINED_NATIVE ? JSON.parse(process.env.MCR_PI12_RETAINED_NATIVE) : null
  if (retained) {
    const source = resolve(root, retained.path)
    assert(source.startsWith(root + '/docs/mcr/evidence/parade-ink-report-error-network/'))
    const bytes = await readFile(source)
    assert.equal(createHash('sha256').update(bytes).digest('hex'), retained.sha256)
    retainedNative = JSON.parse(bytes.toString())
    report.retainedQualification = retained
  }
  const fixture = resolve(front, 'src/app/__tests__/parade-ink-route-status/preview.html')
  server = await createServer({ root: front, configFile: false, cacheDir: resolve(temporary, 'vite'), logLevel: 'error',
    // TEST ONLY known catalog-shaped metadata, never production recognition/release proof.
    define: { 'import.meta.env.VITE_MCR_APPLICATION_RELEASE': JSON.stringify({ version: 1, applicationVersion: '0.1.0', buildId: 'a'.repeat(64), sourceRevision: 'b'.repeat(40) }) },
    plugins: [await foundationOverlay(root), react(), { name: 'pi12-real-app-fixture', configureServer(s) {
      s.middlewares.use(async (req, res, next) => {
        if (!req.headers.accept?.includes('text/html')) return next()
        res.setHeader('Content-Type', 'text/html'); res.end(await s.transformIndexHtml(req.url, await readFile(fixture, 'utf8')))
      })
    } }], optimizeDeps: { entries: [fixture] }, server: { host: '127.0.0.1', port, strictPort: true, watch: null, fs: { allow: [root] } } })
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true })
  for (const [width, height] of [[360, 844], [390, 844], [430, 844], [390, 420]]) {
    const networkScenarios = ['offline', 'reconnecting', 'resync', 'suspended', 'unavailable', 'maintenance', 'version', 'default-version']
    const reportScenarios = ['report-ready', 'report-pending', 'report-unknown', 'report-failed', 'report-permission', 'report-unavailable', 'http-401', 'http-403', 'http-404', 'http-410', 'http-429', 'http-503']
    const scenarios = group === 'both' ? [...networkScenarios, ...reportScenarios] : group === 'network' ? networkScenarios : reportScenarios
    for (const scenario of scenarios) {
      const prior = retainedNative && [...(retainedNative.retainedChecks ?? []), ...retainedNative.checks].find(c => c.width === width && c.height === height && c.scenario === scenario && (c.completed === true || retained.legacyCompletedScenarios?.includes(c.scenario) && c.width === 360 && c.height === 844) && c.captures.length === (scenario === 'reconnecting' ? 2 : 1) && c.errors.length === 0)
      if (prior) { report.retainedChecks.push(prior); continue }
      await guard(); context = await browser.newContext({ viewport: { width, height }, isMobile: true, hasTouch: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
      const page = await context.newPage(); page.setDefaultTimeout(10000)
      const result = { width, height, scenario, captures: [], errors: [] }; report.checks.push(result)
      page.on('pageerror', e => { if (!scenario.startsWith('http-')) result.errors.push(e.message) })
      const query = new URLSearchParams({ path: '/world' })
      if (scenario === 'default-version') query.set('connectionOwner', 'default')
      if (scenario.startsWith('report-')) query.set('reportMode', scenario.slice(7))
      if (scenario.startsWith('http-')) query.set('httpStatus', scenario.slice(5))
      await page.goto(`http://127.0.0.1:${port}/?${query}`)
      await page.waitForFunction(() => Boolean(window.__PI12__), undefined, { timeout: 15_000 })
      try { await expect.poll(async () => (await state(page)).connection.kind).toBe('ready') }
      catch (error) { result.startup = await state(page); throw error }
      await expect(page.locator('.shell-world-host canvas')).toHaveCount(1)
      if (networkScenarios.includes(scenario)) {
        await act(page, 'connection', scenario)
        await expect.poll(async () => (await state(page)).displayConnection.kind).toBe(scenario === 'default-version' ? 'version' : scenario)
        if (scenario === 'default-version') await expect.poll(async () => (await state(page)).defaultConnection.kind).toBe('version')
        const view = (await state(page)).displayConnection
        assert(view.stale, 'Previous ready baseline must be marked stale')
        if (scenario === 'reconnecting') assert(view.pending)
        await capture(page, result, scenario)
        if (scenario === 'reconnecting') { await act(page, 'connection', 'expire'); await capture(page, result, 'reconnecting-expired') }
      } else if (scenario.startsWith('http-')) {
        await act(page, 'navigate', '/world/help')
        await expect(page.getByRole('heading', { name: 'This page is unavailable' })).toBeVisible()
        await capture(page, result, scenario)
        await page.getByRole('link', { name: 'Back to world', exact: true }).click()
        await expect.poll(async () => (await state(page)).connection.kind).toBe('ready')
        assert.equal((await state(page)).activeSockets, 1)
      } else {
        const path = await page.evaluate(() => window.__PI12__.reportPath())
        await act(page, 'navigate', path)
        if (['report-permission', 'report-unavailable'].includes(scenario)) {
          await expect(page.getByText('This safety service is unavailable. No action has been sent.')).toBeVisible()
          await expect(page.getByRole('button', { name: 'Submit report', exact: true })).toHaveCount(0)
        } else {
          const dialog = page.getByRole('dialog', { name: 'Report a concern' })
          await expect(dialog.getByRole('button', { name: 'Submit report', exact: true })).toBeVisible()
          await expect(page.getByRole('button', { name: 'Close report', exact: true })).toBeFocused()
          await expect(page.getByText('Evidence attachments are unavailable in this connection. You can submit a text report when the target is authorized.')).toBeAttached()
          await page.getByRole('textbox').fill('Controlled private report draft')
          if (scenario !== 'report-ready') {
            await dialog.getByRole('button', { name: 'Submit report', exact: true }).click()
            await expect(page.getByRole('button', { name: 'Check original request status' })).toBeVisible()
            await page.getByRole('button', { name: 'Check original request status' }).click()
            await expect.poll(async () => (await state(page)).requests.filter(r => r.path.endsWith('/commands')).length).toBe(1)
            assert.equal((await state(page)).requests.filter(r => r.method === 'POST' && r.path.endsWith('/reports')).length, 1)
          }
          await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab')
          assert(await dialog.evaluate(d => d.contains(document.activeElement)), 'Keyboard focus remains inside native dialog')
        }
        await capture(page, result, scenario)
        if (scenario === 'report-ready') {
          await page.keyboard.press('Escape'); await expect(page.getByRole('dialog')).toHaveCount(0)
          assert.equal((await state(page)).requests.filter(r => r.method === 'POST' && r.path.endsWith('/reports')).length, 0)
        }
      }
      assert.equal(result.errors.length, 0, JSON.stringify(result.errors))
      await act(page, 'dispose'); await context.close(); context = null
      result.completed = true
      await writeFile(resolve(output, 'native-progress.json'), JSON.stringify(report, null, 2) + '\n')
    }
  }
  const qualified = [...report.checks, ...report.retainedChecks]
  assert.equal(qualified.length, group === 'both' ? 80 : group === 'network' ? 32 : 48)
  assert.equal(new Set(qualified.map(c => `${c.width}/${c.height}/${c.scenario}`)).size, qualified.length)
  report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.failure = String(error); process.exitCode = 1 }
finally { await context?.close(); await browser?.close(); await server?.close(); report.finishedUTC = new Date().toISOString(); await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n') }
