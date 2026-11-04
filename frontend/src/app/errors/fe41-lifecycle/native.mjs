// One changed 390px default-root admission/version journey. Reuses the exact
// retained default-entry fixture with only its ticket-response branch replaced.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createRequire } from 'node:module'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
const root = resolve(import.meta.dirname, '../../../..')
const require = createRequire(resolve(process.env.MCR_FRONTEND_DEPENDENCIES, 'package.json'))
const { createServer } = await import(require.resolve('vite'))
const react = (await import(require.resolve('@vitejs/plugin-react'))).default
const tailwind = (await import(require.resolve('@tailwindcss/vite'))).default
const { chromium, expect } = require('@playwright/test')
const packages = require('./package.json')
const browserPackages = Object.keys(packages.dependencies).filter(name => !['@tailwindcss/vite', 'tailwindcss'].includes(name))
const aliases = Object.keys(packages.dependencies).sort((a, b) => b.length - a.length).map(find => ({ find, replacement: resolve(process.env.MCR_FRONTEND_DEPENDENCIES, 'node_modules', find) }))
const output = process.env.MCR_FE41_OUTPUT, scratch = process.env.MCR_FE41_CACHE
assert(output?.startsWith('/') && scratch?.startsWith('/'))
await mkdir(output)
const fixture = resolve(root, 'src/app/errors/__tests__/default-entry/preview.ts')
const original = await readFile(fixture, 'utf8')
const needle = "if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, ++serial))"
assert.equal(original.split(needle).length, 2)
const replacement = "if (path.endsWith('/world-tickets')) return Response.json({ code: 'GATEWAY_409', message: 'Untrusted remote text', details: { reason: 'MAP_VERSION_MISMATCH' } }, { status: 409 })"
const input = { revision: 'TEST_ONLY_FE41_not_C05_authority', reviewedBy: 'Synthetic admission fixture',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
const fixtureHtml = resolve(root, 'src/app/errors/__tests__/default-entry/preview.html')
const port = Number(process.env.MCR_FE41_PORT); assert(port > 1024)
const server = await createServer({ root, configFile: false, cacheDir: scratch,
  resolve: { alias: aliases }, optimizeDeps: { noDiscovery: true, include: [...browserPackages, 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client'] },
  plugins: [react(), tailwind(), { name: 'exact-retained-fixture-ticket-mismatch-only', enforce: 'pre',
    transform(source, id) { if (id.split('?')[0] === fixture) { assert.equal(source, original); return { code: source.replace(needle, replacement), map: null } } },
    configureServer(dev) { dev.middlewares.use(async (request, response, next) => {
      if (!request.headers.accept?.includes('text/html')) return next()
      response.setHeader('Content-Type', 'text/html')
      response.end(await dev.transformIndexHtml(request.url, await readFile(fixtureHtml, 'utf8')))
    }) },
  }], logLevel: 'error', define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
    'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)) },
  server: { host: '127.0.0.1', port, strictPort: true, watch: null } })
const report = { result: 'FAIL', revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), port,
  fixtureSHA256: createHash('sha256').update(original).digest('hex'), fixtureTransform: { needle, replacement },
  scope: 'Actual main/default production/HTTP decoder/transport/composition/status and native keyboard reload; synthetic409fixture, no backend/device/fullvisual acceptance', checks: [], errors: [] }
let browser, page
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  page = await browser.newPage({ viewport: { width: 390, height: 844 }, hasTouch: true, reducedMotion: 'reduce', timezoneId: 'Asia/Seoul' })
  page.on('pageerror', error => report.errors.push(error.message))
  await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
  await page.goto(`http://127.0.0.1:${port}/login`)
  await expect(page.getByRole('button', { name: 'Sign in with email' })).toBeEnabled()
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  const region = page.getByRole('region', { name: 'World connection status' })
  await expect(region).toContainText('Reload to update the world')
  await expect(region.getByRole('button', { name: 'Reconnect', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Try opening world again' })).not.toBeVisible()
  const state = () => page.evaluate(() => window.__FE41_ENTRY__.state())
  report.beforeReload = await state()
  assert.equal(report.beforeReload.canvases, 0); assert.equal(report.beforeReload.active, 0); assert.equal(report.beforeReload.sockets, 0)
  const ticketCount = report.beforeReload.requests.filter(row => row.path.endsWith('/world-tickets')).length
  assert.equal(ticketCount, 1)
  await page.evaluate(() => { window.__FE41_ENTRY__.offline(true); window.__FE41_ENTRY__.offline(false) })
  await expect(region).toContainText('Reload to update the world')
  assert.equal((await state()).requests.filter(row => row.path.endsWith('/world-tickets')).length, 1)
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
  await page.screenshot({ path: resolve(output, '390-map-version.png'), fullPage: true })
  report.checks.push('Canonical409 produces visible reload-only version; withdrawn canvas/socket; browser hints do not replay ticket')
  const reload = region.getByRole('button', { name: 'Reload page', exact: true })
  await reload.scrollIntoViewIfNeeded(); await expect(reload).toBeInViewport()
  const rect = await reload.boundingBox(); assert(rect.width >= 44 && rect.height >= 44)
  await reload.focus(); await Promise.all([page.waitForEvent('load'), page.keyboard.press('Enter')])
  await expect(page.getByRole('link', { name: 'Sign in', exact: true })).toBeVisible()
  report.afterReload = await state(); assert.equal(report.afterReload.active, 0); assert.equal(report.afterReload.canvases, 0)
  assert.equal(report.afterReload.requests.filter(row => row.path.endsWith('/world-tickets')).length, 0)
  report.checks.push('Actual keyboard Enter reload reaches access gate; no stale credential/admission replay')
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} finally {
  if (page && !page.isClosed()) report.finalState = await page.evaluate(() => ({ state: window.__FE41_ENTRY__?.state(), body: document.body.innerText })).catch(error => ({ diagnosticError: error.message }))
  await browser?.close(); await server.close()
  report.cleanup = 'Browser and Vite awaited closed; independent guardian owns final PID/TMP/socket census'
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
