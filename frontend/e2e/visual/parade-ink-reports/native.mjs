// Changed O04 presentation in actual default App. Controlled HTTP, no real backend claim.
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { root, frontend, requireDependency, aliases } from './config.mjs'
import { foundationOverlay, foundationPin } from './foundation.mjs'
import { createReportsServer, wire } from '../../../src/app/operations-composition/__tests__/reports-production/server.ts'
const { createServer } = await import(requireDependency.resolve('vite'))
const { default: react } = await import(requireDependency.resolve('@vitejs/plugin-react'))
const { chromium, expect } = requireDependency('@playwright/test')
const out = process.env.MCR_O04_OUTPUT, temporary = process.env.MCR_O04_TMP, port = Number(process.env.MCR_OPS_PORT)
assert(out && temporary && Number.isInteger(port), 'Fresh finite guarded browser offer required')
const foundation = await foundationOverlay()
await mkdir(out, { recursive: true })
execFileSync(process.env.MCR_O04_PYTHON || 'python3', [resolve(import.meta.dirname, 'reference.py')], { cwd: root, env: process.env })
const report = { status: 'FAIL', source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  qualification: 'Actual index/main/router/AdminBoundary/default production ports/current controller and registry; controlled strict HTTP and role fixtures. Exact readonly PI01 source/API overlay, not published/native PI01 acceptance. No real backend cohort/protected evidence bytes/EOF/physical device.',
  foundation: { commit: foundationPin, postimages: foundation.pins }, cases: [], captures: [], errors: [], cleanup: {} }
const server = await createServer({ root: frontend, configFile: false, logLevel: 'error', plugins: [foundation.plugin, react()],
  cacheDir: resolve(temporary, 'vite-native'), resolve: { alias: aliases },
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(wire.event) },
  server: { host: '127.0.0.1', port, strictPort: true, fs: { allow: [root, '/Users/lhs/Desktop/mcr-worktrees/fe-runtime-preferences', process.env.MCR_O04_DEPS_FRONTEND] } },
  optimizeDeps: { include: ['react', 'react-dom/client', 'react/jsx-runtime', 'react-router-dom', '@tanstack/react-query', 'zod', 'ky', 'phaser'] } })
let browser, context, page, stage = 'start'
const capture = async (name, element) => {
  const file = `${name}.png`
  if (element) await element.screenshot({ path: resolve(out, file) })
  else await page.screenshot({ path: resolve(out, file), fullPage: !name.includes('short-') })
  report.captures.push(file)
}
async function bounds(width) {
  const metrics = await page.locator('.reports-ink').evaluate(element => {
    const title = element.querySelector('.reports-ink__title'), heading = title.querySelector('h1')
    const style = getComputedStyle(title), font = getComputedStyle(heading)
    return { overflow: document.documentElement.scrollWidth > innerWidth, width: element.getBoundingClientRect().width,
      titleHeight: title.getBoundingClientRect().height, titleColor: style.backgroundColor,
      titleText: font.color, titleFont: font.fontFamily, titleSize: font.fontSize,
      fontsLoaded: document.fonts.check('26px "Parade Ink Display"') && document.fonts.check('18px "Parade Ink Text"'),
      controls: [...element.querySelectorAll('button,select,textarea')].map(e => {
        const r = e.getBoundingClientRect(); return { label: e.getAttribute('aria-label') || e.textContent, width: r.width, height: r.height }
      }) }
  })
  assert.equal(metrics.overflow, false); assert(metrics.width <= width)
  assert.equal(metrics.titleHeight, 48); assert.equal(metrics.titleColor, 'rgb(23, 23, 23)')
  assert.equal(metrics.titleText, 'rgb(255, 255, 255)'); assert.equal(metrics.titleSize, '26px'); assert.equal(metrics.fontsLoaded, true)
  for (const c of metrics.controls) assert(c.width >= 44 && c.height >= 44, JSON.stringify(c))
  return metrics
}
async function start(width, options = {}) {
  const controlled = createReportsServer()
  controlled.state.role = options.denied ? null : 'MODERATOR'
  if (options.empty) controlled.reports.list = { items: [], next_cursor: null }
  context = await browser.newContext({ viewport: { width, height: 844 }, isMobile: true, hasTouch: true, reducedMotion: 'reduce', deviceScaleFactor: 1 })
  page = await context.newPage(); page.on('pageerror', e => report.errors.push(e.message))
  await page.route(url => url.pathname.startsWith('/api/'), async route => {
    const r = route.request()
    try { const response = await controlled.handle(new Request(r.url(), { method: r.method(), headers: r.headers(), ...(r.postData() ? { body: r.postData() } : {}) }))
      await route.fulfill({ status: response.status, headers: Object.fromEntries(response.headers), body: await response.text() })
    } catch { await route.abort('failed') }
  })
  await page.goto(`http://127.0.0.1:${port}/login`)
  await page.getByLabel('Email', { exact: true }).fill('o04@example.com'); await page.getByLabel('Password', { exact: true }).fill('TEST_ONLY_O04_password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click(); await page.waitForURL('**/world')
  await page.evaluate(() => { history.pushState(null, '', '/admin/reports'); dispatchEvent(new PopStateEvent('popstate')) })
  if (!options.denied) await expect(page.locator('.reports-ink')).toBeVisible()
  await page.evaluate(() => document.fonts.ready)
  return controlled
}
const decisionWrites = controlled => controlled.requests.filter(r => r.method === 'POST' && /\/admin\/reports\/[^/]+\/decision$/.test(new URL(r.url).pathname)).length
async function select() {
  await page.getByRole('button', { name: /^Review 00000000/ }).click()
  await expect(page.getByText('Private test statement', { exact: true })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Review POST report', exact: true })).toBeFocused()
}
async function prepare() {
  await page.getByLabel('Action', { exact: true }).selectOption('RESOLVED')
  await page.getByLabel('Reason for this action', { exact: true }).fill('Reviewed the factual report without unrelated personal data.')
  await page.keyboard.press('Tab')
  const review = page.getByRole('button', { name: 'Review action', exact: true })
  await expect(review).toBeFocused()
  const outline = await review.evaluate(e => getComputedStyle(e).outlineWidth)
  assert.equal(outline, '3px')
  if (page.viewportSize().width === 390 && !report.pressed) {
    const box = await review.boundingBox(); assert(box)
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2); await page.mouse.down()
    assert.equal(await review.evaluate(e => e.matches(':active')), true)
    await capture('app-pressed-review-390'); await page.mouse.up(); report.pressed = true
  } else await page.keyboard.press('Enter')
  await expect(page.getByRole('button', { name: 'Confirm action', exact: true })).toBeFocused()
}
try {
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    args: ['--disable-background-networking', '--disable-component-update'] })
  report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    stage = `${width}/queue`; const controlled = await start(width)
    await expect(page.getByRole('button', { name: /^Review 00000000/ })).toBeEnabled()
    const item = { width, metrics: await bounds(width), states: [] }; report.cases.push(item)
    await capture(`app-queue-${width}`); if (width === 390) await capture('own-390-queue-region', page.locator('.reports-ink'))
    await select(); item.states.push('selected-detail-focus')
    const count = controlled.requests.length
    await page.getByRole('button', { name: 'Open private evidence 1', exact: true }).click()
    await expect(page.getByText('Evidence is unavailable. No public fallback is used.', { exact: true })).toBeVisible()
    assert.equal(controlled.requests.length, count)
    await capture(`app-evidence-unavailable-${width}`)
    stage = `${width}/confirmation`; await prepare(); await bounds(width)
    await capture(`app-confirmation-${width}`)
    await page.getByRole('button', { name: 'Cancel confirmation', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Review action', exact: true })).toBeFocused()
    item.cancelDecisionWrites = decisionWrites(controlled); assert.equal(item.cancelDecisionWrites, 0)
    item.states.push('confirmation-cancel-focus-no-write')
    stage = `${width}/unknown`; await prepare(); controlled.reports.mode = 'pending'
    await page.getByRole('button', { name: 'Confirm action', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Check original command', exact: true })).toBeVisible()
    await capture(`app-pending-${width}`)
    controlled.reports.statusHttp = 404
    await page.getByRole('button', { name: 'Check original command', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText('outcome remains unknown')
    await expect(page.getByLabel('Reason for this action', { exact: true })).toBeDisabled()
    await expect(page.getByRole('button', { name: 'Refresh records', exact: true })).toBeDisabled()
    await capture(`app-unknown-${width}`); item.states.push('pending-unknown-disabled-original-command')
    await page.setViewportSize({ width, height: 430 })
    if (width === 390) {
      const cdp = await context.newCDPSession(page)
      await cdp.send('Emulation.setSafeAreaInsetsOverride', { insets: { top: 20, bottom: 34, left: 0, right: 0 } })
      const padding = await page.locator('.reports-ink__body').evaluate(e => parseFloat(getComputedStyle(e).paddingBottom))
      assert.equal(padding, 34); report.safeArea = { emulation: 'Chrome CDP, not physical device', top: 20, bottom: 34, ownBottomPadding: padding }
    }
    await page.getByRole('button', { name: 'Check original command', exact: true }).scrollIntoViewIfNeeded()
    await expect(page.getByRole('button', { name: 'Check original command', exact: true })).toBeInViewport()
    await bounds(width); await capture(`app-short-status-${width}`); item.states.push('short-height-status-reachable')
    await page.setViewportSize({ width, height: 844 })
    stage = `${width}/terminal`; controlled.reports.statusHttp = 200; controlled.reports.status = width === 390 ? 'FAILED' : 'SUCCEEDED'
    if (controlled.reports.status === 'SUCCEEDED') controlled.reports.report = { ...controlled.reports.report, state: 'RESOLVED', revision: 2 }
    await page.getByRole('button', { name: 'Check original command', exact: true }).click()
    await expect(page.getByText(width === 390 ? 'The server recorded failure. Refresh before starting a new action.' : 'The current server record confirms this action.', { exact: true })).toBeVisible()
    await capture(`app-terminal-${width}`); item.states.push(controlled.reports.status)
    item.commandPosts = decisionWrites(controlled); assert.equal(item.commandPosts, 1)
    assert(!controlled.requests.some(r => /\/evidence\/|\/media\//.test(new URL(r.url).pathname)))
    await context.close(); context = undefined
  }
  stage = '390/conflict'; const conflict = await start(390); await select(); await prepare(); conflict.reports.mode = 'conflict'
  await page.getByRole('button', { name: 'Confirm action', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('record changed')
  await expect(page.getByLabel('Action', { exact: true })).toBeDisabled(); await capture('app-conflict-390')
  await context.close(); context = undefined
  stage = '390/empty'; await start(390, { empty: true }); await expect(page.getByRole('heading', { name: 'No reports on this page' })).toBeVisible(); await capture('app-empty-390')
  await context.close(); context = undefined
  stage = '390/no-assignment'; const denied = await start(390, { denied: true })
  await expect(page.getByRole('heading', { name: 'Administration unavailable' })).toBeVisible()
  assert(!denied.requests.some(r => new URL(r.url).pathname.includes('/admin/'))); await capture('app-denied-390')
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
  await context.close(); context = undefined
  assert.deepEqual(report.errors, []); report.status = 'PASS'
} catch (error) {
  report.failure = { stage, error: String(error) }; if (page && !page.isClosed()) await capture('failure'); throw error
} finally {
  await context?.close(); await browser?.close(); report.cleanup.browserClosed = true
  await server.close(); report.cleanup.serverClosed = true
  await writeFile(resolve(out, 'native.json'), JSON.stringify(report, null, 2) + '\n')
  // Source bytes are read solely for an immutable publication/qualification readset.
  const own = await readFile(resolve(frontend, 'src/features/admin/reports/ui/parade-ink/ReportsReview.tsx'))
  report.cleanup.sourceBytes = own.length
}
