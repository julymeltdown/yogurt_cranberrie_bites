// FE-DIALOG-ESC-01: actual default App; controlled HTTP/C05, never live/device proof.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile, mkdir, statfs } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { createRequire } from 'node:module'
import { pathToFileURL } from 'node:url'
import { foundationOverlay } from './foundation-overlay.mjs'
const root = resolve(import.meta.dirname, '../../../..'), front = resolve(root, 'frontend')
const require = createRequire(resolve(process.env.MCR_READONLY_FRONTEND ?? front, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')).href)
const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')).href)
const { chromium, expect } = require('@playwright/test')
const output = process.env.MCR_PI12_OUTPUT, temporary = process.env.MCR_PI12_TMP
const deadline = Date.parse(process.env.MCR_PI12_WORK_DEADLINE ?? ''), port = Number(process.env.MCR_PI12_PORT)
assert(output?.startsWith(root + '/docs/mcr/evidence/parade-ink-report-error-network/'))
assert(temporary?.startsWith('/private/tmp/mcr-pi12-') && deadline > Date.now() && port === 5381)
const report = { task: 'FE-DIALOG-ESC-01', qualification: 'ACTUAL_MAIN_CANONICAL_AUTH_PRODUCTION_READINESS_CONTROLLED_HTTP_C05_NOT_LIVE_OR_DEVICE', status: 'IN_PROGRESS', checks: [], retainedChecks: [], errors: [] }
const modules = new Map()
let server, browser, context, page, stage = 'bootstrap'
const state = () => page.evaluate(() => window.__PARADE_PRIVATE__.state())
const persist = () => writeFile(resolve(output, 'dialog-native.json'), JSON.stringify(report, null, 2) + '\n')
async function guard() { const s = await statfs(root); assert(Date.now() < deadline && s.bavail * s.bsize >= 8 * 1024 ** 3) }
const reduce = async () => { const button = page.getByRole('button', { name: 'Reduce panel', exact: true }); if (await button.isVisible()) await button.click() }
async function nav(path) {
  await guard(); await page.evaluate(p => window.__PARADE_PRIVATE__.navigate(p), path)
  if (path !== '/world' && !path.startsWith('/report')) { await expect(page.locator('[data-mcr-shell]')).toHaveAttribute('data-panel-size', 'full'); await reduce() }
  await expect.poll(async () => (await state()).path).toBe(path.split('?')[0])
}
async function observe(dialog) {
  await dialog.evaluate(el => { window.__DIALOG_CANCELS__ = 0; el.addEventListener('cancel', () => window.__DIALOG_CANCELS__++) })
}
async function record(name, path, opener, nativeCancel = true) {
  await guard(); await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect.poll(async () => (await state()).path).toBe(path)
  if (opener) await expect(opener).toBeFocused()
  const cancelEvents = await page.evaluate(() => window.__DIALOG_CANCELS__ ?? 0)
  if (nativeCancel) assert.equal(cancelEvents, 1, name + ': native cancel must fire exactly once')
  await expect.poll(async () => (await state()).activeSockets).toBe(1)
  const s = await state(); assert.equal(s.peakCanvases, 1); assert.equal(s.peakSockets, 1)
  assert.equal(s.canvases, 1); assert.equal(s.activeSockets, 1); assert(s.sameHost && s.sameCanvas)
  const mutations = s.requests.filter(r => r.method !== 'GET' && !['/api/auth/login', '/api/auth/enrollment/admission'].includes(r.path) && !r.path.endsWith('/world-tickets'))
  assert.equal(mutations.length, 0, name + ': cancellation must not issue a mutation')
  report.checks.push({ name, status: 'PASS', path, viewport: page.viewportSize(), nativeCancelEvents: cancelEvents,
    focus: await page.evaluate(() => document.activeElement?.textContent?.trim() ?? ''), mutationCount: mutations.length,
    canvases: s.canvases, activeSockets: s.activeSockets, sameHost: s.sameHost, sameCanvas: s.sameCanvas })
  await persist()
}
try {
  await mkdir(output, { recursive: true }); await guard()
  const retained = JSON.parse(await readFile(resolve(root, 'docs/mcr/evidence/parade-ink-report-error-network/dialog-retained04.json'), 'utf8'))
  assert(retained.source.path.startsWith('docs/mcr/evidence/parade-ink-report-error-network/'))
  const raw = await readFile(resolve(root, retained.source.path))
  assert.equal(createHash('sha256').update(raw).digest('hex'), retained.source.sha256)
  for (const [path, digest] of [['frontend/src/features/safety/Dialog.tsx', retained.DialogSha256], ['frontend/src/app/WorldShell.tsx', retained.WorldShellSha256]]) assert.equal(createHash('sha256').update(await readFile(resolve(root, path))).digest('hex'), digest)
  const previous = JSON.parse(raw.toString()); assert.deepEqual(previous.errors, [])
  assert.deepEqual(previous.checks.map(c => c.name), retained.caseNames); assert.equal(previous.checks.length, 7)
  assert(previous.checks.every(c => c.status === 'PASS' && c.mutationCount === 0 && c.canvases === 1 && c.activeSockets === 1))
  report.retainedChecks = previous.checks; report.retainedSource = retained.source
  const fixture = resolve(import.meta.dirname, 'dialog-preview.html')
  server = await createServer({ root: front, configFile: false, cacheDir: resolve(temporary, 'vite-dialog'), logLevel: 'error',
    define: { 'import.meta.env.VITE_MCR_APPLICATION_RELEASE': JSON.stringify({ version: 1, applicationVersion: '0.1.0', buildId: 'a'.repeat(64), sourceRevision: 'b'.repeat(40) }),
      'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
      'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify({ revision: 'test-only', reviewedBy: 'test-only', movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 })) },
    plugins: [await foundationOverlay(root), { name: 'dialog-qualified-module-inputs', enforce: 'pre', transform(code, id) {
      const clean = id.split('?')[0]; if (clean.startsWith(front + '/') && !clean.includes('/node_modules/')) modules.set(relative(root, clean), createHash('sha256').update(code).digest('hex'))
      return null
    } }, react(), { name: 'dialog-actual-main-preview', configureServer(s) {
      s.middlewares.use(async (req, res, next) => {
        if (!req.headers.accept?.includes('text/html')) return next()
        res.setHeader('Content-Type', 'text/html'); res.end(await s.transformIndexHtml(req.url, await readFile(fixture, 'utf8')))
      })
    } }], optimizeDeps: { entries: [fixture] }, server: { host: '127.0.0.1', port, strictPort: true, watch: null, fs: { allow: [root] } } })
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true })
  context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, reducedMotion: 'reduce', deviceScaleFactor: 1, serviceWorkers: 'block' })
  page = await context.newPage(); page.setDefaultTimeout(12000); page.on('pageerror', error => report.errors.push(error.message))
  stage = 'actual login and controlled authority baseline'
  await page.goto(`http://127.0.0.1:${port}/`)
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test'); await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
  await expect.poll(async () => (await state()).canvases).toBe(1)
  await expect.poll(async () => (await state()).activeSockets).toBe(1)
  await expect.poll(async () => (await state()).frames.filter(frame => frame.type === 'snapshot').length).toBeGreaterThan(0)
  await expect(page.getByText('This section is unavailable for the current account or event.', { exact: true })).toBeVisible()
  assert.equal((await state()).requests.filter(r => /\/me\/(social-handles|social-requests|age-status|notifications)$/.test(r.path)).length, 0)
  await page.evaluate(() => window.__PARADE_PRIVATE__.baseline())
  await expect.poll(async () => (await state()).requests.filter(r => r.method === 'GET' && r.path.endsWith('/me/social-handles')).length).toBeGreaterThan(0)
  await expect(page.getByText('This section is unavailable for the current account or event.', { exact: true })).toHaveCount(0)
  if (report.retainedChecks.length === 0) {
  stage = 'actual Blocks modal keyboard containment and native cancellation'
  await nav('/me/blocks'); const unblock = page.getByRole('button', { name: 'Unblock user', exact: true })
  for (const height of [844, 480]) {
    await page.setViewportSize({ width: 390, height }); await unblock.click()
    const dialog = page.getByRole('dialog', { name: 'Unblock this user?', exact: true }); await expect(dialog).toBeVisible(); await observe(dialog)
    await expect(page.getByRole('button', { name: 'Keep blocked', exact: true })).toBeFocused()
    await page.keyboard.press('Shift+Tab'); await expect(page.getByRole('button', { name: 'Confirm unblock', exact: true })).toBeFocused()
    await page.keyboard.press('Tab'); await expect(page.getByRole('button', { name: 'Keep blocked', exact: true })).toBeFocused()
    if (height === 480) await page.screenshot({ path: resolve(output, 'blocks-before-escape-390-480.png') })
    await page.keyboard.press('Escape'); await record('blocks-native-escape-' + height, '/me/blocks', unblock)
    if (height === 480) await page.screenshot({ path: resolve(output, 'blocks-restored-opener-390-480.png') })
  }
  stage = 'shell Escape remains active outside modal'
  await page.keyboard.press('Escape'); await expect.poll(async () => (await state()).path).toBe('/me'); await record('shell-escape-outside-dialog', '/me', null, false)
  stage = 'actual Account cancellation and input reset'
  await nav('/me/account'); const logout = page.getByRole('button', { name: 'Sign out', exact: true })
  await logout.click(); let dialog = page.getByRole('dialog', { name: 'Sign out?', exact: true }); await observe(dialog)
  await expect(page.getByRole('button', { name: 'Stay signed in', exact: true })).toBeFocused()
  await page.keyboard.press('Escape'); await record('signout-native-escape', '/me/account', logout)
  const deletion = page.getByRole('button', { name: 'Request account deletion', exact: true }); await deletion.click()
  dialog = page.getByRole('dialog', { name: 'Request account deletion?', exact: true }); await observe(dialog)
  await expect(page.getByRole('button', { name: 'Keep account', exact: true })).toBeFocused()
  await page.getByLabel('Deletion confirmation', { exact: true }).fill('DELETE_MY_ACCOUNT')
  await page.getByRole('button', { name: 'Confirm deletion request', exact: true }).focus()
  await page.keyboard.press('Escape'); await record('deletion-button-native-escape', '/me/account', deletion)
  await deletion.click(); await expect(page.getByLabel('Deletion confirmation', { exact: true })).toHaveValue('')
  await page.getByLabel('Deletion confirmation', { exact: true }).fill('DELETE_MY_ACCOUNT'); await observe(page.getByRole('dialog'))
  await page.keyboard.press('Escape'); await record('deletion-input-native-escape', '/me/account', deletion)
  await deletion.click(); await expect(page.getByLabel('Deletion confirmation', { exact: true })).toHaveValue('')
  await page.getByRole('button', { name: 'Keep account', exact: true }).click(); await record('deletion-button-cancel-reset', '/me/account', deletion, false)
  }
  stage = 'actual Report native and explicit cancellation'
  const search = await page.evaluate(() => window.__PARADE_PRIVATE__.reportSearch)
  for (const method of ['Escape', 'button']) {
    await nav('/report' + search); const dialog = page.getByRole('dialog', { name: 'Report a concern', exact: true }); await expect(dialog).toBeVisible(); await observe(dialog)
    const close = page.getByRole('button', { name: 'Close report', exact: true }); await expect(close).toBeFocused()
    if (method === 'Escape') await page.keyboard.press('Escape'); else await close.click()
    await record('report-cancel-' + method, '/world', null, method === 'Escape')
  }
  assert.deepEqual(report.errors, []); assert.equal(report.checks.length + report.retainedChecks.length, 9)
  report.status = 'PASS'
} catch (error) {
  report.status = 'FAIL'; report.stage = stage; report.failure = String(error)
  if (page) report.observed = { path: await page.evaluate(() => location.pathname).catch(() => null), dialogs: await page.getByRole('dialog').count().catch(() => null), headings: await page.getByRole('heading').allTextContents().catch(() => []) }
  process.exitCode = 1
} finally {
  if (context) await context.close(); if (browser) await browser.close(); if (server) await server.close()
  report.finishedAt = new Date().toISOString(); await persist()
  await writeFile(resolve(output, 'dialog-module-inputs.json'), JSON.stringify(Object.fromEntries([...modules].sort()), null, 2) + '\n')
}
