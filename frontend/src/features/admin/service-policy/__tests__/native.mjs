// Owned actual-component browser gate. Requires a separately accepted finite native allocation.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES, scratch = process.env.MCR_FE38_SCRATCH, output = process.env.MCR_FE38_OUTPUT
assert(dependencies && scratch && output)
const require = createRequire(resolve(dependencies, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')))
const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
const { chromium, expect } = require('@playwright/test')
const { aliases } = await import('./vitest.config.mjs')
const frontend = resolve(import.meta.dirname, '../../../../..')
const fixture = '/src/features/admin/service-policy/__tests__/preview.html'
await mkdir(output, { recursive: true })
const report = { result: 'FAIL', qualification: 'ACTUAL_LEAF_CANONICAL_AUTH_OPERATIONS_ENTRY_CONTROLLED_HTTP_AND_TIME_NOT_DEFAULT_ADMIN_NOT_LIVE',
  widths: [], errors: [], screenshots: [], checks: {} }
const server = await createServer({ root: frontend, configFile: false, cacheDir: resolve(scratch, 'vite'), logLevel: 'warn', resolve: { alias: aliases }, plugins: [react(), {
  name: 'fe38-policy-preview', configureServer(instance) { instance.middlewares.use(async (request, response, next) => {
    if (!request.headers.accept?.includes('text/html')) return next()
    response.setHeader('Content-Type', 'text/html'); response.end(await instance.transformIndexHtml(request.url, await readFile(resolve(frontend, '.' + fixture), 'utf8')))
  }) },
}], server: { host: '127.0.0.1', port: 0, watch: null, fs: { allow: [frontend, resolve(frontend, '..'), dependencies] } } })
let browser, page
const save = async name => { await page.screenshot({ path: resolve(output, name + '.png') }); report.screenshots.push(name + '.png') }
try {
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    page = await browser.newPage({ viewport: { width, height: 844 }, deviceScaleFactor: 1 }); page.setDefaultTimeout(8000)
    page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/`)
    await page.getByLabel('Email', { exact: true }).fill('operator@example.invalid'); await page.getByLabel('Password', { exact: true }).fill('controlled-password')
    await page.getByRole('button', { name: 'Sign in to controlled fixture' }).click()
    await expect.poll(() => page.evaluate(() => window.__FE38_POLICY__.state().controller.decision?.policy_revision)).toBe(4)
    await page.getByLabel('Minimum client version', { exact: true }).fill('2.0.0')
    await page.getByLabel('Publication expiry (UTC)', { exact: true }).fill('2026-11-05T00:05:00Z')
    await page.getByLabel('Reason', { exact: true }).fill('Scheduled service work')
    await page.getByRole('button', { name: 'Review policy change' }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('heading', { name: 'Confirm service policy change' })).toBeFocused()
    await expect(page.getByRole('button', { name: 'Confirm policy change' })).toBeVisible()
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Horizontal overflow')
    const buttons = await page.getByRole('button').evaluateAll(elements => elements.filter(e => !e.disabled).map(e => ({ name: e.textContent, h: e.getBoundingClientRect().height })))
    assert(buttons.every(button => button.h >= 44), 'Control target below44px')
    await save('confirmation-' + width); report.widths.push({ width, keyboardReviewFocus: true, noHorizontalOverflow: true, controlsAtLeast44: true })
    await page.getByRole('button', { name: 'Cancel confirmation' }).click(); await expect(page.getByRole('button', { name: 'Review policy change' })).toBeFocused()
    if (width === 390) {
      await page.evaluate(() => window.__FE38_POLICY__.mode('conflict'))
      await page.getByRole('button', { name: 'Review policy change' }).click(); await page.getByRole('button', { name: 'Confirm policy change' }).click()
      await expect.poll(() => page.evaluate(() => window.__FE38_POLICY__.state().controller.command?.stage)).toBe('conflict')
      await save('conflict-390'); await page.getByRole('button', { name: 'Refresh service policy' }).click()
      await expect(page.getByRole('button', { name: 'Return to retained draft' })).toBeEnabled(); await page.getByRole('button', { name: 'Return to retained draft' }).click()
      await expect(page.getByLabel('Reason', { exact: true })).toHaveValue('Scheduled service work')
      await page.evaluate(() => window.__FE38_POLICY__.mode('pending'))
      await page.getByRole('button', { name: 'Review policy change' }).click(); await page.getByRole('button', { name: 'Confirm policy change' }).click()
      await expect.poll(() => page.evaluate(() => window.__FE38_POLICY__.state().controller.command?.stage)).toBe('pending')
      await expect(page.getByLabel('Reason', { exact: true })).toBeDisabled(); await save('pending-390')
      await page.getByRole('button', { name: 'Check policy command status' }).click()
      await expect.poll(() => page.evaluate(() => window.__FE38_POLICY__.state().controller.command?.stage)).toBe('pending')
      await page.evaluate(() => window.__FE38_POLICY__.settle()); await page.getByRole('button', { name: 'Check policy command status' }).click()
      await expect.poll(() => page.evaluate(() => window.__FE38_POLICY__.state().controller.command?.stage)).toBe('succeeded')
      const beforeWithdrawal = await page.evaluate(() => window.__FE38_POLICY__.state())
      assert.equal(beforeWithdrawal.requests.filter(r => r.method === 'PUT').length, 2)
      assert.equal(beforeWithdrawal.controller.command.receipt.resource_revision, 23)
      report.checks = { terminalConflictDraftRetained: true, synthetic202StatusRecovery: true, puts: 2, eventRevisionDistinctFromPolicyCAS: true }
      await page.evaluate(() => window.__FE38_POLICY__.withdraw())
      await expect(page.getByRole('heading', { name: 'Service policy unavailable' })).toBeVisible(); await expect(page.getByLabel('Reason', { exact: true })).toHaveCount(0)
      await save('withdrawn-390'); report.checks.authorityWithdrawalScrubs = true
    }
    await page.evaluate(() => window.__FE38_POLICY__.dispose()); await page.close(); page = null
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = String(error); if (page) await save('failure') }
finally { await page?.close(); await browser?.close(); await server.close(); report.resourcesClosed = true; await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report)); if (report.result !== 'PASS') process.exitCode = 1
