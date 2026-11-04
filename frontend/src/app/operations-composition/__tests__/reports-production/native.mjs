// Changed actual-main frontend qualification with controlled HTTP. No backend/three-actor acceptance.
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createRequire } from 'node:module'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { createReportsServer, wire } from './server.ts'
const root = resolve(import.meta.dirname, '../../../../../..'), out = process.env.MCR_OPS_OUTPUT
assert(out && process.env.MCR_OPS_PORT && process.env.MCR_OPS_SOURCE_TMP, 'Fresh allocated private round required')
const { chromium, expect } = createRequire(resolve(root, 'frontend/package.json'))('@playwright/test')
await mkdir(out, { recursive: true })
const report = { status: 'FAIL', source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  qualification: 'Actual index/main/AdminBoundary/default ports, actual generated registry and existing ReportReviewScreen. Controlled strict HTTP fixtures and role responses only. Dedicated evidence operation is absent and must not dispatch. No real actor cohort, real role publication, backend evidence EOF or physical-device claim.',
  hashes: {}, cases: [], captures: [], errors: [], cleanup: {} }
for (const path of ['frontend/index.html', 'frontend/src/main.tsx', 'frontend/src/app/AdminBoundary.tsx', 'frontend/src/app/ports.ts',
  'frontend/src/app/operations-composition/owner.ts', 'frontend/src/app/operations-composition/panel.tsx',
  'frontend/src/app/operations-composition/reports/index.ts', 'frontend/src/app/operations-composition/reports/evidence.ts',
  'frontend/src/features/admin/reports/model/controller.ts', 'frontend/src/features/admin/reports/ui/ModerationReview.tsx',
  'frontend/src/shared/api/domain-operations.generated.ts', 'contracts/generated/mcr-domain.openapi.json']) {
  report.hashes[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')
}
const server = await createServer({ root: resolve(root, 'frontend'), configLoader: 'runner', logLevel: 'error',
  cacheDir: resolve(process.env.MCR_OPS_SOURCE_TMP, 'vite-native'),
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(wire.event) },
  server: { host: '127.0.0.1', port: Number(process.env.MCR_OPS_PORT), strictPort: true },
  optimizeDeps: { include: ['react', 'react-dom/client', 'react/jsx-runtime', 'react-router-dom', '@tanstack/react-query', 'zod', 'ky'] } })
let browser, context
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    args: ['--disable-background-networking', '--disable-component-update'] })
  report.browser = browser.version()
  for (const width of [360, 390, 430]) for (const scenario of ['moderator', 'operator', 'member']) {
    const controlled = createReportsServer(); controlled.state.role = scenario === 'member' ? null : scenario === 'operator' ? 'OPERATOR' : 'MODERATOR'
    const item = { width, scenario, checks: [], requests: [] }; report.cases.push(item)
    context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, reducedMotion: 'reduce', deviceScaleFactor: 1 })
    const page = await context.newPage()
    page.on('pageerror', error => report.errors.push(error.message))
    await page.route(url => url.pathname.startsWith('/api/'), async route => {
      const input = route.request()
      try {
        const response = await controlled.handle(new Request(input.url(), { method: input.method(), headers: input.headers(),
          ...(input.postData() ? { body: input.postData() } : {}) }))
        await route.fulfill({ status: response.status, headers: Object.fromEntries(response.headers), body: await response.text() })
      } catch { await route.abort('failed') }
    })
    await page.goto(`http://127.0.0.1:${process.env.MCR_OPS_PORT}/login`)
    await page.getByLabel('Email', { exact: true }).fill(`${scenario}@example.com`)
    await page.getByLabel('Password', { exact: true }).fill('TEST_ONLY_Reports_password')
    await page.getByRole('button', { name: 'Sign in with email', exact: true }).click(); await page.waitForURL('**/world')
    await page.evaluate(() => { history.pushState(null, '', '/admin/reports'); dispatchEvent(new PopStateEvent('popstate')) })
    const writes = () => controlled.requests.filter(r => r.method === 'POST' && new URL(r.url).pathname.endsWith('/decision'))
    async function capture(state) {
      const name = `${width}-${scenario}-${state}.png`; await page.screenshot({ fullPage: true, path: resolve(out, name) }); report.captures.push(name)
    }
    if (scenario === 'member') {
      await expect(page.getByRole('heading', { name: 'Administration unavailable', exact: true })).toBeVisible()
      await expect(page.getByRole('heading', { name: 'Report queue', exact: true })).toHaveCount(0)
      assert(!controlled.requests.some(r => new URL(r.url).pathname.includes('/admin/')))
      item.checks.push('null_assignment_no_private_reads'); await capture('denied')
    } else {
      await expect(page.getByRole('heading', { name: 'Report queue', exact: true })).toBeVisible()
      await page.getByRole('button', { name: /Review 00000000/ }).click()
      await expect(page.getByText('Private test statement', { exact: true })).toBeVisible()
      await expect(page.getByText('00000000-0000-4000-8000-000000000999', { exact: true })).toHaveCount(0)
      const before = controlled.requests.length
      await page.getByRole('button', { name: 'Open private evidence 1', exact: true }).click()
      await expect(page.getByText('Evidence is unavailable. No public fallback is used.', { exact: true })).toBeVisible()
      await expect(page.getByText('Private test statement', { exact: true })).toBeVisible()
      assert.equal(controlled.requests.length, before)
      await page.getByLabel('Action', { exact: true }).selectOption('RESOLVED')
      await page.getByLabel('Reason for this action', { exact: true }).fill('Reviewed factual report without unrelated personal data.')
      await page.getByRole('button', { name: 'Review action', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Confirm action', exact: true })).toBeFocused()
      await capture('confirmation')
      await page.getByRole('button', { name: 'Cancel confirmation', exact: true }).click(); assert.equal(writes().length, 0)
      await page.getByRole('button', { name: 'Review action', exact: true }).click()
      controlled.reports.mode = scenario === 'operator' ? 'lost' : 'pending'
      await page.getByRole('button', { name: 'Confirm action', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Check original command', exact: true })).toBeVisible()
      assert.equal(writes().length, 1); const key = writes()[0].headers.get('Idempotency-Key')
      if (scenario === 'operator') {
        await page.getByRole('link', { name: 'Event', exact: true }).click()
        await expect(page.getByRole('heading', { name: 'Event settings', exact: true })).toBeVisible()
        await page.getByRole('link', { name: 'Places', exact: true }).click()
        await expect(page.getByRole('heading', { name: 'Venue and places', exact: true })).toBeVisible()
        await page.getByRole('link', { name: 'Wall', exact: true }).click()
        await expect(page.getByRole('heading', { name: 'WALL CONTROL', exact: true })).toBeVisible()
      } else {
        assert(!controlled.requests.some(r => new URL(r.url).pathname.endsWith('/admin/settings')))
        await page.getByRole('link', { name: 'Event', exact: true }).click()
        await expect(page.getByRole('heading', { name: 'Event settings', exact: true })).toHaveCount(0)
        await expect(page.getByText('Moderator access grants report review. Current operator access is required for event controls.', { exact: true })).toBeVisible()
      }
      await page.getByRole('link', { name: 'Reports', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Check original command', exact: true })).toBeVisible()
      assert.equal(writes().length, 1); assert.equal(writes()[0].headers.get('Idempotency-Key'), key)
      controlled.reports.statusHttp = 404
      await page.getByRole('button', { name: 'Check original command', exact: true }).click()
      await expect(page.getByRole('alert')).toContainText('outcome remains unknown')
      await expect(page.getByRole('button', { name: 'Refresh records', exact: true })).toBeDisabled()
      assert.equal(writes().length, 1); await capture('unknown')
      controlled.reports.statusHttp = 200
      controlled.reports.status = scenario === 'operator' ? 'SUCCEEDED' : 'FAILED'
      if (scenario === 'operator') controlled.reports.report = { ...controlled.reports.report, state: 'RESOLVED', revision: 2 }
      await page.getByRole('button', { name: 'Check original command', exact: true }).click()
      if (scenario === 'operator') await expect(page.getByText('The current server record confirms this action.', { exact: true })).toBeVisible()
      else await expect(page.getByRole('alert')).toContainText('server recorded failure')
      assert.equal(writes().length, 1)
      item.checks.push('actual_default_reports_mount', 'reporter_projection_scrubbed', 'missing_evidence_only_no_generic_fallback',
        'focused_confirmation_cancel_no_write', 'single_bound_decision', 'route_retained_command', 'status_404_preserves_unknown', 'qualified_terminal_receipt')
      controlled.state.role = null; controlled.replaceDecision()
      await page.getByRole('button', { name: 'Check current access', exact: true }).click()
      await expect(page.getByRole('heading', { name: 'Report queue', exact: true })).toHaveCount(0)
      await expect(page.getByRole('heading', { name: 'Administration unavailable', exact: true })).toBeVisible()
      item.checks.push('role_withdrawal_scrubs_private_controller')
    }
    await expect(page.getByText('PRIVATE_BACKEND_DIAGNOSTIC', { exact: true })).toHaveCount(0)
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
    item.requests = controlled.requests.map(r => ({ method: r.method, path: new URL(r.url).pathname, protected: !!r.headers.get('Authorization'), key: r.headers.get('Idempotency-Key') }))
    assert(!item.requests.some(r => r.path.includes('/evidence/') || /\/media\//.test(r.path)))
    await context.close(); context = undefined
  }
  assert.equal(report.errors.length, 0); report.status = 'PASS'
} finally {
  try { await context?.close(); await browser?.close(); report.cleanup.browserClosed = true }
  finally { await server.close(); report.cleanup.serverClosed = true; await writeFile(resolve(out, 'native.json'), JSON.stringify(report, null, 2) + '\n') }
}
