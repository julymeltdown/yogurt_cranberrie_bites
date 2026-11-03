import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../../../..'), out = resolve(import.meta.dirname, 'evidence/browser')
await mkdir(out, { recursive: true })
const report = { result: 'FAIL', base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  scope: 'FE39 explicit RESUME_CLOSURE frontend through canonical HttpClient and authenticated deterministic C04 fixture. No live backend, original-actor server proof, real cleanup, IN04, physical-device or production-root acceptance.',
  widths: [], sourceHashes: {}, checks: [] }
async function pin(path) { report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex') }
for (const directory of ['frontend/src/features/admin/closure','frontend/src/features/admin/mural']) {
  for (const entry of await readdir(resolve(root, directory), { withFileTypes: true })) if (entry.isFile() && /\.(ts|tsx|css)$/.test(entry.name)) await pin(`${directory}/${entry.name}`)
}
for (const name of ['fixture.ts','preview.tsx','preview.css','preview.html','browser-smoke.mjs']) await pin(`frontend/src/features/admin/closure/__tests__/resume/${name}`)
for (const path of ['frontend/src/shared/api/http.ts','frontend/src/shared/query/privacy.ts','contracts/domain/v1/admin.schema.json','contracts/domain/v1/operations.json','contracts/domain/v1/policy.json','docs/mcr/contracts/event-closure-resume.md','frontend/src/shared/api/domain-operations.generated.ts']) await pin(path)
const server = await createServer({ root: resolve(root, 'frontend'), logLevel: 'error', server: { host: '127.0.0.1', port: 5249, strictPort: true } })
let browser
try {
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360,390,430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' }), errors = []
    page.setDefaultTimeout(5000)
    page.on('pageerror', e => errors.push(e.message))
    const status = page.getByRole('status', { name: 'Event closure status' }), review = page.getByRole('group', { name: 'Confirm closure resume' })
    const counts = () => page.evaluate(() => window.__CLOSURE_RESUME__.counts())
    async function open() {
      await page.goto('http://127.0.0.1:5249/src/features/admin/closure/__tests__/resume/preview.html')
      await expect(page.getByLabel('Closure job state')).toContainText('PARTIAL_FAILURE · revision 8')
    }
    async function layout() {
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
      assert(await page.locator('button,input,textarea,select').evaluateAll(elements => elements.every(e => { const r = e.getBoundingClientRect(); return r.width >= 44 && r.height >= 44 })), 'target below44px')
    }
    async function prepare() {
      await page.getByLabel('Resume reason', { exact: true }).fill('Retry the failed permitted archive export')
      await page.getByRole('button', { name: 'Retry failed closure steps', exact: true }).click()
      await expect(review).toBeFocused(); await expect(review).toContainText('00000000-0000-4000-8000-000000000123')
      await expect(review).toContainText('expected job revision 8'); await expect(review).toContainText('Acceptance does not mean cleanup is finished')
    }
    async function confirm() {
      await page.getByRole('button', { name: 'Confirm resume closure', exact: true }).focus(); await page.keyboard.press('Enter')
    }
    await open(); await layout(); await prepare(); assert.equal((await counts()).posts, 0)
    await page.getByRole('button', { name: 'Cancel closure resume' }).click(); assert.equal((await counts()).posts, 0)
    await expect(review).toHaveCount(0)
    await prepare(); await layout(); await page.screenshot({ path: resolve(out, `${width}-resume-confirmation.png`), fullPage: true })
    await confirm(); await expect(page.getByLabel('Closure job state')).toContainText('RUNNING · revision 9')
    assert.equal((await counts()).posts, 1); assert.equal((await counts()).admissions, 1)
    assert.equal(await page.getByRole('list', { name: 'Closure steps' }).locator('li').filter({ hasText: 'SUCCEEDED' }).count(), 5)
    await page.getByRole('button', { name: 'Refresh closure progress' }).click(); await expect(status).toContainText('Closure is in progress')
    assert.equal((await counts()).posts, 1)
    await page.evaluate(() => window.__CLOSURE_RESUME__.complete())
    await page.getByRole('button', { name: 'Refresh closure progress' }).click(); await expect(status).toContainText('all closure steps complete')
    assert.equal((await counts()).posts, 1); assert(Object.values((await counts()).effects).every(value => value === 1))
    await layout(); await page.screenshot({ path: resolve(out, `${width}-completed.png`), fullPage: true })

    await open(); await prepare(); await page.evaluate(() => window.__CLOSURE_RESUME__.mode('lost')); await confirm()
    await expect(status).toContainText('unresolved'); const original = await page.evaluate(() => window.__CLOSURE_RESUME__.command())
    await expect(page.getByRole('form', { name: 'Prepare closure resume' })).toHaveCount(0)
    await page.getByRole('button', { name: 'Refresh closure progress' }).click(); await expect(page.getByLabel('Closure job state')).toContainText('RUNNING')
    await expect(page.getByRole('button', { name: 'Check original closure command' })).toBeVisible()
    assert.equal((await page.evaluate(() => window.__CLOSURE_RESUME__.command())).key, original.key)
    await layout(); await page.screenshot({ path: resolve(out, `${width}-unknown.png`), fullPage: true })
    await page.getByRole('button', { name: 'Check original closure command' }).click(); await expect(page.getByRole('button', { name: 'Check original closure command' })).toHaveCount(0)
    assert.equal((await counts()).posts, 1); assert.equal((await counts()).admissions, 1)
    await page.evaluate(() => { window.__CLOSURE_RESUME__.fail(); window.__CLOSURE_RESUME__.mode('ack') })
    await page.getByRole('button', { name: 'Refresh closure progress' }).click(); await page.getByLabel('Resume reason', { exact: true }).fill('Explicitly retry the later failed boundary')
    await page.getByRole('button', { name: 'Retry failed closure steps', exact: true }).click(); await expect(review).toContainText('expected job revision 10')
    await confirm(); await expect(page.getByLabel('Closure job state')).toContainText('RUNNING · revision 11')
    assert.equal((await counts()).admissions, 2); assert.equal((await counts()).posts, 2)

    await open(); await prepare(); await page.evaluate(() => window.__CLOSURE_RESUME__.mode('pending')); await confirm()
    await expect(status).toContainText('unresolved'); assert.equal((await counts()).admissions, 0)
    await page.getByRole('button', { name: 'Check original closure command' }).click(); await expect(status).toContainText('pending or unavailable')
    await page.evaluate(() => window.__CLOSURE_RESUME__.pending()); await page.getByRole('button', { name: 'Check original closure command' }).click()
    await expect(page.getByLabel('Closure job state')).toContainText('RUNNING'); assert.equal((await counts()).posts, 1); assert.equal((await counts()).admissions, 1)

    await open(); await prepare(); await page.evaluate(() => window.__CLOSURE_RESUME__.conflict()); await confirm()
    await expect(status).toContainText('VERSION_CONFLICT'); await expect(page.getByRole('list', { name: 'Closure steps' })).toHaveCount(0)
    await expect(review).toHaveCount(0); assert.equal((await counts()).admissions, 0)
    await layout(); await page.screenshot({ path: resolve(out, `${width}-conflict.png`), fullPage: true })
    await page.getByRole('button', { name: 'Refresh closure progress' }).click(); await expect(page.getByLabel('Closure job state')).toContainText('revision 9')
    await page.getByLabel('Resume reason', { exact: true }).fill('Review current failed revision after conflict')
    await page.getByRole('button', { name: 'Retry failed closure steps', exact: true }).click(); await expect(review).toContainText('expected job revision 9')
    await page.evaluate(() => window.__CLOSURE_RESUME__.revoke()); await expect(review).toHaveCount(0)
    await expect(page.getByRole('list', { name: 'Closure steps' })).toHaveCount(0); await expect(page.getByLabel('Resume reason', { exact: true })).toHaveCount(0)
    assert.equal((await counts()).posts, 1)
    await page.evaluate(() => window.__CLOSURE_RESUME__.dispose())
    await open(); await prepare(); await page.evaluate(() => window.__CLOSURE_RESUME__.logout())
    await expect(review).toHaveCount(0); await expect(page.getByRole('list', { name: 'Closure steps' })).toHaveCount(0)
    assert.equal((await counts()).posts, 0); await layout(); assert.deepEqual(errors, [])
    report.widths.push(width); await page.evaluate(() => window.__CLOSURE_RESUME__.dispose()); await page.close()
  }
  report.result = 'PASS'; report.checks = ['exact job/revision/reason preview; cancel sends no POST; focused review and keyboard confirmation',
    'one explicit resume admission preserves completed steps; RUNNING is not completion; progress GET never resumes',
    'lost response preserves command identity across job GET; original receipt plus authorized job GET resolves without replay',
    'later failed boundary requires explicit fresh intent; pending202 status remains blocked until settled',
    'stale job revision409 withdraws data; current authorized read and new preview required',
    'role revocation and logout scrub draft/job/command before another POST',
    '360/390/430 touch widths, targets>=44px, no overflow, no page errors, reduced motion']
} finally { await browser?.close(); await server.close(); await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify({ result: report.result, widths: report.widths, browser: report.browser, checks: report.checks }, null, 2))
