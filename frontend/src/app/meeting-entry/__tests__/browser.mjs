import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { relative, resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'

const frontend = resolve(import.meta.dirname, '../../../..'), root = resolve(frontend, '..'), evidence = resolve(import.meta.dirname, 'evidence/browser')
await mkdir(evidence, { recursive: true })
const report = { result: 'FAIL', scope: 'Synthetic meeting-entry provider/panels, canonical HTTP/privacy and FE19; no production router, live server, venue or IN02 acceptance',
  baseCommit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourceHashes: {}, widths: [], errors: [] }
async function collect(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.name === 'evidence') continue
    const file = resolve(dir, entry.name)
    if (entry.isDirectory()) await collect(file)
    else if (/\.(ts|tsx|mjs|css|html)$/.test(file)) report.sourceHashes[relative(root, file)] = createHash('sha256').update(await readFile(file)).digest('hex')
  }
}
await collect(resolve(import.meta.dirname, '..'))
const server = await createServer({ root: frontend, logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser
try {
  await server.listen()
  const port = server.httpServer.address().port
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => report.errors.push(e.message))
    await page.goto(`http://127.0.0.1:${port}/src/app/meeting-entry/__tests__/preview.html`)
    await expect(page.getByRole('heading', { name: 'Current meeting' })).toBeVisible()
    assert.deepEqual((await page.evaluate(() => window.__MEETING_ENTRY__.state())).subscriptions, { clock: 1, authority: 1, places: 1, invalidations: 1 })
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    await page.evaluate(() => window.__MEETING_ENTRY__.clock(false))
    await expect(page.getByText(/Real-world meeting guidance is unavailable/)).toBeVisible()
    await expect(page.getByText('SYNTHETIC red scarf')).toHaveCount(0)
    await page.evaluate(() => window.__MEETING_ENTRY__.clock(true))
    await expect(page.getByText('SYNTHETIC red scarf')).toBeVisible()
    const checkIn = page.getByRole('button', { name: 'Open check-in fixture' }); await checkIn.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('heading', { name: 'Real-world check-in' })).toBeVisible()
    await expect(page.getByText('SYNTHETIC red scarf')).toHaveCount(0)
    await page.getByLabel('Reviewed place').selectOption('00000000-0000-4000-8000-000000000002')
    await page.getByLabel('Who can see this?').selectOption('synthetic-party')
    await page.getByLabel('How to find me').fill('SYNTHETIC marker')
    await page.getByLabel('Identification', { exact: true }).fill('SYNTHETIC private scarf')
    await page.getByRole('checkbox').check(); await page.evaluate(() => window.__MEETING_ENTRY__.mode('lost'))
    await page.getByRole('button', { name: 'Share check-in' }).click()
    await expect(page.getByRole('button', { name: 'Share check-in' })).toBeDisabled()
    await page.getByRole('button', { name: 'Open beacon fixture' }).click()
    await expect(page.getByRole('heading', { name: 'Current meeting' })).toBeVisible()
    await page.getByRole('button', { name: 'Open check-in fixture' }).click()
    await expect(page.getByRole('button', { name: 'Share check-in' })).toBeDisabled()
    assert.equal((await page.evaluate(() => window.__MEETING_ENTRY__.state())).writes, 1)
    assert.deepEqual((await page.evaluate(() => window.__MEETING_ENTRY__.state())).subscriptions, { clock: 1, authority: 1, places: 1, invalidations: 1 })
    for (const target of await page.locator('button,input,select,textarea').all()) {
      const box = await target.boundingBox(); assert(box && box.width >= 44 && box.height >= 44)
    }
    if (width === 390) { await page.evaluate(() => scrollTo(0, 0)); await page.screenshot({ path: resolve(evidence, 'unknown-command-390.png'), fullPage: true }) }
    await page.evaluate(() => window.__MEETING_ENTRY__.mode('privacy'))
    await expect(page.getByText(/Real-world meeting guidance is unavailable/)).toBeVisible()
    assert.deepEqual((await page.evaluate(() => window.__MEETING_ENTRY__.state())).subscriptions, { clock: 0, authority: 0, places: 0, invalidations: 0 })
    await page.evaluate(() => window.__MEETING_ENTRY__.dispose()); assert.equal(await page.locator('main').count(), 0)
    report.widths.push({ width, result: 'PASS', strictMode: true, minimumTouchTarget: 44, cleanup: 'PASS' }); await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (e) { report.failure = { name: e.name, message: e.message }; process.exitCode = 1 }
finally { await browser?.close(); await server.close(); report.cleanup = 'PASS'; await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report))
