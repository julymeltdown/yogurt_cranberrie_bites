import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../../..'), out = resolve(import.meta.dirname, 'evidence/browser')
await mkdir(out, { recursive: true })
const report = { result: 'FAIL', base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  scope: 'FE39 deterministic authenticated C04 HTTP fixture through actual canonical client. No liveBE, IN04, native-device or production-root acceptance. Closure mutation retry awaits pinned amendment; progress refresh never counts as job retry.',
  widths: [], sourceHashes: {}, checks: [] }
for (const directory of ['frontend/src/features/admin/mural', 'frontend/src/features/admin/closure']) {
  for (const name of await readdir(resolve(root, directory))) {
    if (!/\.(ts|tsx|css)$/.test(name)) continue
    const path = `${directory}/${name}`; report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')
  }
}
for (const name of ['fixture.ts','preview.tsx','preview.css','preview.html','browser-smoke.mjs']) { const path = `frontend/src/features/admin/mural/__tests__/${name}`; report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex') }
for (const name of ['mural.schema.json','admin.schema.json','operations.json','policy.json']) { const path = `contracts/domain/v1/${name}`; report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex') }
const server = await createServer({ root: resolve(root, 'frontend'), logLevel: 'error', server: { host: '127.0.0.1', port: 5239, strictPort: true } })
let browser
try {
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360,390,430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' }), errors = []
    page.on('pageerror', e => errors.push(e.message))
    await page.goto('http://127.0.0.1:5239/src/features/admin/mural/__tests__/preview.html')
    const status = page.getByRole('status', { name: 'Wall control status' }), select = page.getByLabel('Wall action', { exact: true })
    await expect(status).toContainText('Server wall state loaded')
    async function layout() {
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
      assert(await page.locator('button,input,textarea,select').evaluateAll(elements => elements.every(e => { const r = e.getBoundingClientRect(); return r.width >= 44 && r.height >= 44 })), 'target below44px')
    }
    await layout()
    async function review(action, reason = 'Reviewed fixture operation') {
      await select.selectOption(action); await page.getByLabel('Reason', { exact: true }).fill(reason)
      await page.getByRole('button', { name: 'Review wall action', exact: true }).click()
      await expect(page.getByRole('group', { name: 'Confirm wall action', exact: true })).toBeFocused()
    }
    async function confirm() { await page.getByRole('button', { name: 'Confirm wall action', exact: true }).focus(); await page.keyboard.press('Enter'); await expect(status).toContainText('Server confirmed') }
    await review('PAUSE'); assert.equal((await page.evaluate(() => window.__FE39__.counts())).posts, 0)
    await page.getByRole('button', { name: 'Cancel wall action' }).click(); assert.equal((await page.evaluate(() => window.__FE39__.counts())).posts, 0)
    await review('PAUSE'); await confirm()
    await select.selectOption('RESTORE'); await page.getByLabel('Snapshot to restore').fill('00000000-0000-4000-8000-000000000004'); await page.getByLabel('Reason', { exact: true }).fill('Restore this reviewed snapshot')
    await page.getByRole('button', { name: 'Review wall action', exact: true }).click()
    await expect(page.getByRole('group', { name: 'Confirm wall action' })).toContainText('00000000-0000-4000-8000-000000000004')
    await expect(page.getByRole('group', { name: 'Confirm wall action' })).toContainText('Expected wall revision 11')
    await layout(); await page.screenshot({ path: resolve(out, `${width}-restore-confirmation.png`), fullPage: true }); await confirm()
    await select.selectOption('LOCK')
    await page.getByLabel('X start', { exact: true }).fill('2'); await page.getByLabel('Y start', { exact: true }).fill('3')
    await page.getByLabel('X end exclusive', { exact: true }).fill('8'); await page.getByLabel('Y end exclusive', { exact: true }).fill('9')
    await page.getByLabel('Reason', { exact: true }).fill('Protect reviewed region'); await page.getByRole('button', { name: 'Review wall action' }).click()
    await page.evaluate(() => window.__FE39__.mode('lost')); await page.getByRole('button', { name: 'Confirm wall action' }).click()
    await expect(status).toContainText('unresolved'); const posts = (await page.evaluate(() => window.__FE39__.counts())).posts
    await page.getByRole('button', { name: 'Refresh wall controls' }).click(); await expect(status).toContainText('Original command remains unresolved')
    await layout(); await page.screenshot({ path: resolve(out, `${width}-unknown.png`), fullPage: true })
    await page.getByRole('button', { name: 'Check original wall command' }).click(); await expect(status).toContainText('Original action succeeded')
    assert.equal((await page.evaluate(() => window.__FE39__.counts())).posts, posts)
    await page.evaluate(() => window.__FE39__.mode('ack')); await review('RESUME'); await confirm(); await review('FREEZE')
    await expect(page.getByRole('group', { name: 'Confirm wall action' })).toContainText('Freezing is permanent'); await confirm()
    await expect(select).toHaveCount(0)
    await page.getByRole('button', { name: 'Event closure', exact: true }).click()
    const closureStatus = page.getByRole('status', { name: 'Event closure status' })
    await page.getByLabel('Closure reason').fill('Close after the final show'); await page.getByRole('button', { name: 'Review event closure' }).click()
    await expect(page.getByRole('group', { name: 'Confirm event closure' })).toBeFocused()
    await page.getByRole('button', { name: 'Confirm close event' }).click(); await expect(page.getByLabel('Closure job state')).toContainText('PARTIAL_FAILURE')
    await expect(page.getByRole('list', { name: 'Closure steps' })).toContainText('Export permitted archive — FAILED')
    await expect(page.getByRole('button', { name: 'Retry failed closure steps' })).toBeDisabled()
    await layout(); await page.screenshot({ path: resolve(out, `${width}-partial-closure.png`), fullPage: true })
    const closurePosts = (await page.evaluate(() => window.__FE39__.counts())).posts
    await page.evaluate(() => window.__FE39__.progress('COMPLETED')); await page.getByRole('button', { name: 'Refresh closure progress' }).click(); await expect(closureStatus).toContainText('all closure steps complete')
    assert.equal((await page.evaluate(() => window.__FE39__.counts())).posts, closurePosts)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
    assert(await page.locator('button,input,textarea,select').evaluateAll(elements => elements.every(e => { const r = e.getBoundingClientRect(); return r.width >= 44 && r.height >= 44 })), 'target below44px')
    const headings = await page.locator('h1').allTextContents(); assert.deepEqual(headings, ['CLOSE THE EVENT'])
    await page.evaluate(() => window.__FE39__.logout()); await expect(page.getByRole('list', { name: 'Closure steps' })).toHaveCount(0); await expect(page.getByText('00000000-0000-4000-8000-000000000050', { exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'Wall control', exact: true }).click(); await expect(page.getByLabel('Wall action', { exact: true })).toHaveCount(0)
    assert.deepEqual(errors, []); report.widths.push(width); await page.evaluate(() => window.__FE39__.dispose()); await page.close()
  }
  report.result = 'PASS'; report.checks = ['preview/cancel never mutate', 'keyboard explicit confirmation', 'restore names exact snapshot and revision', 'half-open lock rectangle', 'unknown wall action recovers only through verified status and current read without replay',
    'pause/resume/permanentfreeze', 'partial closure failed step visible; independent authorized progress refresh never retries mutation', 'privacy scrubs both retained controllers', '360/390/430 targets>=44px/no overflow; labeled native DOM and focused confirmation', 'no browser runtime errors']
} finally { await browser?.close(); await server.close(); await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report, null, 2))
