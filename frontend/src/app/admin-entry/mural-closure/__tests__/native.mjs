import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile, readdir } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../../..'), out = process.env.FE39_OUTPUT
if (!out || !process.env.FE39_PORT || !process.env.FE39_TMP) throw Error('Accepted bounded round environment required')
await mkdir(out, { recursive: true })
const report = { result: 'FAIL', base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  scope: 'New FE39 retained entry/panel adapter through canonical SessionController login and protected C04 HTTP; synthetic server/authority only. No production AdminBoundary, live backend, actual authority/cleanup, IN04 or physical-device claim.',
  contexts: [], errors: [], sourceHashes: {} }
for (const directory of ['frontend/src/app/admin-entry/mural-closure', 'frontend/src/features/admin/mural', 'frontend/src/features/admin/closure']) {
  for (const name of await readdir(resolve(root, directory))) {
    if (!/\.(ts|tsx|css)$/.test(name)) continue
    const path = `${directory}/${name}`; report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')
  }
}
for (const path of ['frontend/src/features/auth/model/session.ts', 'frontend/src/shared/api/http.ts', 'frontend/src/shared/query/privacy.ts', 'frontend/src/shared/api/domain-operations.generated.ts',
  'frontend/src/app/admin-entry/mural-closure/__tests__/fixture.ts', 'frontend/src/app/admin-entry/mural-closure/__tests__/preview.tsx', 'frontend/src/app/admin-entry/mural-closure/__tests__/native.mjs']) report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')
const server = await createServer({ root: resolve(root, 'frontend'), configFile: false, cacheDir: resolve(process.env.FE39_TMP, 'vite-cache'), logLevel: 'error',
  server: { host: '127.0.0.1', port: Number(process.env.FE39_PORT), strictPort: true }, optimizeDeps: { include: ['react', 'react-dom/client', 'react/jsx-runtime', 'zod', '@tanstack/react-query', 'ky'] } })
let browser
const cleanup = { browserClosed: false, serverClosed: false, errors: [] }
try {
  await server.listen()
  const address = server.httpServer.address(), port = address.port
  await writeFile(resolve(out, 'admission.json'), JSON.stringify({ port, node: process.version, base: report.base, sourceHashes: report.sourceHashes }, null, 2))
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true, args: ['--disable-dev-shm-usage'] }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' })
    const page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(`http://127.0.0.1:${port}/src/app/admin-entry/mural-closure/__tests__/preview.html`)
    const status = page.getByRole('status', { name: 'Wall control status' })
    await expect(status).toContainText('Server wall state loaded')
    await page.getByLabel('Wall action', { exact: true }).selectOption('RESTORE')
    const ids = await page.evaluate(() => window.__FE39_CURRENT__.ids)
    await page.getByLabel('Snapshot to restore').fill(ids.restore)
    await page.getByLabel('Reason', { exact: true }).fill('Reviewed adapter snapshot')
    await page.getByRole('button', { name: 'Review wall action', exact: true }).click()
    const review = page.getByRole('group', { name: 'Confirm wall action', exact: true })
    await expect(review).toBeFocused(); await expect(review).toContainText(ids.restore)
    await page.screenshot({ fullPage: true, path: resolve(out, `${width}-restore.png`) })
    await page.getByRole('button', { name: 'Confirm wall action', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect(status).toContainText('Server confirmed')
    await page.evaluate(() => window.__FE39_CURRENT__.readOnly())
    await page.getByRole('button', { name: 'Open closure panel' }).click()
    await page.getByLabel('Existing closure job ID').fill(ids.job)
    await page.getByRole('button', { name: 'Load existing closure job' }).click()
    await expect(page.getByLabel('Closure job state')).toContainText('PARTIAL_FAILURE')
    assert.equal(await page.getByRole('list', { name: 'Closure steps' }).locator('li').count(), 7)
    await page.screenshot({ fullPage: true, path: resolve(out, `${width}-partial.png`) })
    await page.getByLabel('Resume reason').fill('Reviewed adapter failed export')
    await page.getByRole('button', { name: 'Retry failed closure steps' }).click()
    await expect(page.getByRole('group', { name: 'Confirm closure resume' })).toBeFocused()
    await page.evaluate(() => window.__FE39_CURRENT__.mode('lost'))
    await page.getByRole('button', { name: 'Confirm resume closure' }).click()
    const closureStatus = page.getByRole('status', { name: 'Event closure status' })
    await expect(closureStatus).toContainText('unresolved')
    await page.getByRole('button', { name: 'Open wall panel' }).click()
    await page.getByRole('button', { name: 'Open closure panel' }).click()
    await expect(closureStatus).toContainText('unresolved')
    assert.equal((await page.evaluate(() => window.__FE39_CURRENT__.counts())).posts, 2)
    await page.screenshot({ fullPage: true, path: resolve(out, `${width}-retained-unknown.png`) })
    await page.getByRole('button', { name: 'Check original closure command' }).click()
    await expect(page.getByLabel('Closure job state')).toContainText('RUNNING')
    assert.equal((await page.evaluate(() => window.__FE39_CURRENT__.counts())).posts, 2)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Horizontal overflow')
    assert(await page.locator('button,input,textarea,select').evaluateAll(elements => elements.filter(e => e.getBoundingClientRect().width > 0).every(e => { const r = e.getBoundingClientRect(); return r.width >= 44 && r.height >= 44 })), 'Touch target under44px')
    await page.evaluate(() => window.__FE39_CURRENT__.revoke())
    await expect(page.getByRole('heading', { name: 'Event operations unavailable' })).toBeVisible()
    await expect(page.getByRole('list', { name: 'Closure steps' })).toHaveCount(0)
    await page.screenshot({ fullPage: true, path: resolve(out, `${width}-withdrawn.png`) })
    await page.evaluate(() => window.__FE39_CURRENT__.dispose())
    assert.equal((await page.evaluate(() => window.__FE39_CURRENT__.counts())).authorityListeners, 0)
    report.contexts.push({ width, result: 'PASS', posts: 2, retainedUnknownAcrossPanels: true, sevenStepPartialFailure: true, receiptReadOnlyRecovery: true, namedSnapshotRevision: true, withdrawal: true })
    await writeFile(resolve(out, 'progress.json'), JSON.stringify(report, null, 2)); await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = String(error); process.exitCode = 1 }
finally {
  await writeFile(resolve(out, 'native.json'), JSON.stringify(report, null, 2))
  try { await browser?.close(); cleanup.browserClosed = true } catch (error) { cleanup.errors.push(String(error)) }
  try { await server.close(); cleanup.serverClosed = true } catch (error) { cleanup.errors.push(String(error)) }
  await writeFile(resolve(out, 'cleanup.json'), JSON.stringify(cleanup, null, 2))
  if (cleanup.errors.length) process.exitCode = 1
}
