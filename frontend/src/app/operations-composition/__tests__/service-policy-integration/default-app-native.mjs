// Actual main.tsx, router, AdminBoundary, default ports, canonical Auth and current composed owner/panel.
// Only HTTP is controlled. No injected Auth/controller/clock/router or rendering substitute.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { mkdir, writeFile, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { createPolicyServer, wire } from './default-app-server.mjs'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES, scratch = process.env.MCR_FE38_SCRATCH, output = process.env.MCR_FE38_OUTPUT
assert(dependencies && scratch && output)
const require = createRequire(resolve(dependencies, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')))
const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
const { default: tailwindcss } = await import(pathToFileURL(require.resolve('@tailwindcss/vite')))
const { chromium, expect } = require('@playwright/test')
const { aliases } = await import('./vitest.config.mjs')
const nativeAliases = [{ find: /^@hookform\/resolvers\/zod$/, replacement: require.resolve('@hookform/resolvers/zod') }, ...aliases]
const frontend = resolve(import.meta.dirname, '../../../../..')
await mkdir(output, { recursive: true })
const report = { status: 'FAIL', qualification: 'ACTUAL_MAIN_ROUTER_ADMINBOUNDARY_DEFAULTPORTS_CANONICAL_AUTH_CURRENT_ADMIN_COMPOSITION_CONTROLLED_HTTP',
  limits: 'Real backend, measured production-clock cohort, World ticket, physical device and Parade Ink visual acceptance NOT_RUN.',
  retained: { width360: 'Native05 actual-main confirmation/cancel/layout PASS; unchanged inputs; no replay' },
  cases: [], captures: [], errors: [], requests: [], pngBytes: 0 }
const server = await createServer({ root: frontend, configFile: false, cacheDir: resolve(scratch, 'default-app-vite'), logLevel: 'error',
  resolve: { alias: nativeAliases }, plugins: [react(), tailwindcss()], define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(wire.event) },
  optimizeDeps: { include: ['react', 'react-dom/client', 'react/jsx-runtime', 'react-router-dom', '@tanstack/react-query', 'zod', 'ky', 'phaser'] },
  server: { host: '127.0.0.1', port: 5384, strictPort: true, watch: null, fs: { allow: [frontend, resolve(frontend, '..'), dependencies] } } })
let browser, context, page, stage = 'startup'
const policy = () => page.getByRole('region', { name: 'Service policy', exact: true })
const navigate = path => page.evaluate(path => { history.pushState(null, '', path); dispatchEvent(new PopStateEvent('popstate')) }, path)
async function capture(name) {
  assert(report.captures.length < 40)
  const path = resolve(output, name + '.png'); await page.screenshot({ path, fullPage: true })
  report.pngBytes += (await stat(path)).size; assert(report.pngBytes <= 16 * 1024**2)
  report.captures.push(name + '.png')
}
async function start(width) {
  const controlled = createPolicyServer()
  context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, reducedMotion: 'reduce', isMobile: true, hasTouch: true })
  page = await context.newPage(); page.setDefaultTimeout(10_000)
  page.on('pageerror', error => report.errors.push(error.message))
  await page.route(url => url.pathname.startsWith('/api/'), async route => {
    const request = route.request()
    try {
      const response = await controlled.handle(new Request(request.url(), { method: request.method(), headers: request.headers(),
        ...(request.postData() ? { body: request.postData() } : {}) }))
      await route.fulfill({ status: response.status, headers: Object.fromEntries(response.headers), body: await response.text() })
    } catch { await route.abort('failed') }
  })
  await page.goto('http://127.0.0.1:5384/login')
  await page.getByLabel('Email', { exact: true }).fill('operator@example.invalid')
  await page.getByLabel('Password', { exact: true }).fill('TEST_ONLY_controlled_password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  await page.waitForURL('**/world') // Canonical sign-in's real safe-return destination; no World-readiness claim.
  await navigate('/admin/event')
  await expect(page.getByRole('heading', { name: 'Event settings', exact: true })).toBeVisible()
  await expect(policy().getByRole('button', { name: 'Review policy change', exact: true })).toBeEnabled()
  assert.equal(controlled.requests.filter(request => new URL(request.url).pathname === '/api/auth/login').length, 1)
  return controlled
}
async function review() {
  await policy().getByRole('button', { name: 'Review policy change', exact: true }).focus()
  await page.keyboard.press('Enter')
  await expect(policy().getByRole('heading', { name: 'Confirm service policy change', exact: true })).toBeFocused()
}
const confirm = () => policy().getByRole('button', { name: 'Confirm policy change', exact: true }).click()
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    args: ['--disable-background-networking', '--disable-component-update'] })
  report.browser = browser.version()
  for (const width of [390, 430]) {
    stage = `${width}/actual-main-sign-in`; const controlled = await start(width)
    await policy().getByLabel('Service mode', { exact: true }).selectOption('MAINTENANCE')
    await policy().getByLabel('Minimum client version', { exact: true }).fill('2.0.0')
    await policy().getByLabel('Publication expiry (UTC)', { exact: true }).fill(new Date(Date.now() + 300_000).toISOString())
    await policy().getByLabel('Reason', { exact: true }).fill('Scheduled service work')
    await review()
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, 'Horizontal overflow')
    const sizes = await policy().getByRole('button').evaluateAll(elements => elements.filter(element => !element.disabled).map(element => {
      const r = element.getBoundingClientRect(); return { width: r.width, height: r.height, text: element.textContent }
    }))
    assert(sizes.every(r => r.width >= 44 && r.height >= 44), JSON.stringify(sizes))
    await capture(`actual-default-admin-confirmation-${width}`)
    await policy().getByRole('button', { name: 'Cancel confirmation', exact: true }).click()
    await expect(policy().getByRole('button', { name: 'Review policy change', exact: true })).toBeFocused()
    assert.equal(controlled.writes().length, 0)
    const checks = ['actual-main-canonical-email-sign-in', 'default-admin-event-and-policy-mounted', 'keyboard-review-cancel-focus', 'cancel-no-write', 'no-horizontal-overflow', 'policy-targets-at-least44']
    if (width === 390) {
      stage = '390/CAS-conflict'; controlled.service.mode = 'conflict'; await review(); await confirm()
      await expect(policy().getByText('conflict', { exact: true })).toBeVisible(); await capture('actual-default-admin-conflict-390')
      await policy().getByRole('button', { name: 'Refresh service policy', exact: true }).click()
      await expect(policy().getByRole('button', { name: 'Return to retained draft', exact: true })).toBeEnabled()
      await policy().getByRole('button', { name: 'Return to retained draft', exact: true }).click()
      await expect(policy().getByLabel('Reason', { exact: true })).toHaveValue('Scheduled service work')
      stage = '390/pending'; controlled.service.mode = 'pending'; await review(); await confirm()
      await expect(policy().getByText('pending', { exact: true })).toBeVisible()
      await expect(policy().getByLabel('Reason', { exact: true })).toBeDisabled(); await capture('actual-default-admin-pending-390')
      const key = controlled.writes()[1].headers.get('Idempotency-Key')
      await page.getByRole('link', { name: 'Reports', exact: true }).click()
      await expect(page.getByRole('heading', { name: 'Report queue', exact: true })).toBeVisible()
      await expect(policy()).toHaveCount(0)
      await navigate('/admin/content')
      await expect(page.getByRole('heading', { name: 'Content review', exact: true })).toBeVisible()
      await capture('actual-default-admin-member-content-sibling-390')
      await page.getByRole('link', { name: 'Event', exact: true }).click()
      await expect(policy().getByRole('button', { name: 'Check policy command status', exact: true })).toBeVisible()
      controlled.service.statusHttp = 404
      await policy().getByRole('button', { name: 'Check policy command status', exact: true }).click()
      await expect(policy().getByText('pending', { exact: true })).toBeVisible()
      await expect(policy().getByText('Status remains unresolved. Current policy information is unavailable. No publication has been assumed.', { exact: true })).toBeVisible()
      await expect(policy().getByLabel('Reason', { exact: true })).toBeDisabled()
      assert.equal(controlled.writes().length, 2)
      await capture('actual-default-admin-pending-status-unavailable-390')
      controlled.service.statusHttp = 200
      await policy().getByRole('button', { name: 'Check policy command status', exact: true }).click()
      await expect(policy().getByText('pending', { exact: true })).toBeVisible()
      controlled.settle(); await policy().getByRole('button', { name: 'Check policy command status', exact: true }).click()
      await expect(policy().getByText('succeeded', { exact: true })).toBeVisible()
      assert.equal(controlled.writes().length, 2); assert.equal(controlled.service.receipt.resource_revision, 23)
      const statusGets = controlled.requests.filter(request => new URL(request.url).searchParams.get('operation_id') === 'setConcertServicePolicy')
      assert.equal(statusGets.length, 3); assert(statusGets.every(request => new URL(request.url).searchParams.get('idempotency_key') === key))
      await capture('actual-default-admin-recovered-390')
      stage = '390/withdrawal'; controlled.state.role = null; controlled.replaceDecision()
      await page.getByRole('button', { name: 'Check current access', exact: true }).click()
      await expect(policy()).toHaveCount(0)
      await expect(page.getByRole('heading', { name: 'Administration unavailable', exact: true })).toBeVisible()
      await expect(page.getByLabel('Reason', { exact: true })).toHaveCount(0)
      assert.equal(controlled.writes().length, 2); await capture('actual-default-admin-withdrawn-390')
      checks.push('terminal-CAS-conflict-retained-draft', '202-pending', 'Reports-and-MemberContent-sibling-navigation', 'same-original-status-key', '404-retains-known-pending-receipt-no-replay', 'explicit-status-success-event-revision23', 'withdrawal-hides-policy-and-draft')
    }
    if (width === 430) {
      stage = '430/lost-response'; controlled.service.mode = 'lost'; await review(); await confirm()
      await expect(policy().getByText('unknown', { exact: true })).toBeVisible()
      assert.equal(controlled.writes().length, 1)
      const originalKey = controlled.writes()[0].headers.get('Idempotency-Key')
      await expect(policy().getByLabel('Reason', { exact: true })).toBeDisabled()
      await capture('actual-default-admin-unknown-lost-response-430')
      await policy().getByRole('button', { name: 'Check policy command status', exact: true }).click()
      await expect(policy().getByText('succeeded', { exact: true })).toBeVisible()
      assert.equal(controlled.writes().length, 1)
      const status = controlled.requests.filter(request => new URL(request.url).searchParams.get('operation_id') === 'setConcertServicePolicy')
      assert.equal(status.length, 1); assert.equal(new URL(status[0].url).searchParams.get('idempotency_key'), originalKey)
      assert.equal(controlled.service.receipt.resource_revision, 22)
      await capture('actual-default-admin-lost-response-recovered-430')
      stage = '430/moderator'; controlled.state.role = 'MODERATOR'; controlled.replaceDecision()
      await page.getByRole('button', { name: 'Check current access', exact: true }).click()
      await expect(policy()).toHaveCount(0)
      await expect(page.getByRole('heading', { name: 'Administration unavailable', exact: true })).toBeVisible()
      await page.getByRole('link', { name: 'Reports', exact: true }).click()
      await expect(page.getByRole('heading', { name: 'Report queue', exact: true })).toBeVisible()
      assert.equal(controlled.writes().length, 1); await capture('actual-default-admin-moderator-reports-430')
      checks.push('lost-response-unknown-no-replay', 'explicit-original-key-recovery-event-revision22', 'Moderator-never-policy', 'Moderator-Reports-preserved')
    }
    report.cases.push({ width, status: 'PASS', checks })
    report.requests.push({ width, requests: controlled.requests.map(request => ({ method: request.method, path: new URL(request.url).pathname,
      operation: new URL(request.url).searchParams.get('operation_id'), key: request.headers.get('Idempotency-Key') ?? new URL(request.url).searchParams.get('idempotency_key') })) })
    await context.close(); context = null; page = null
  }
  assert.deepEqual(report.errors, []); report.status = 'PASS'
} catch (error) {
  report.failure = { stage, message: String(error) }
  if (page && !page.isClosed()) { try { await capture('failure') } catch (captureError) { report.captureFailure = String(captureError) } }
} finally {
  await context?.close(); await browser?.close(); await server.close()
  report.resourcesClosed = true
  await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(JSON.stringify(report)); if (report.status !== 'PASS') process.exitCode = 1
