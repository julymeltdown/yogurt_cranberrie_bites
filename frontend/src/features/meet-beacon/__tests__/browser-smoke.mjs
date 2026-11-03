import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const frontend = resolve(import.meta.dirname, '../../../..'), root = resolve(frontend, '..')
const evidence = resolve(import.meta.dirname, 'evidence/browser')
await mkdir(evidence, { recursive: true })
const report = { scope: 'FE20 synthetic fixtures using canonical HTTP/privacy and actual FE19 controller; no real venue, server, device or multiplayer acceptance', result: 'FAIL', widths: [], checks: [], errors: [] }
report.baseCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()
report.sourceHashes = {}
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
  const address = server.httpServer.address(), port = address.port
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(`http://127.0.0.1:${port}/src/features/meet-beacon/__tests__/fixtures/preview.html`)
    await expect(page.getByRole('heading', { name: 'Current meeting' })).toBeVisible()
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `overflow at ${width}`)
    for (const target of await page.locator('button,input,select,textarea,a').all()) {
      const box = await target.boundingBox(); assert(box && box.width >= 44 && box.height >= 44, 'touch target smaller than 44px')
    }
    const here = page.getByRole('button', { name: 'I am here' }), check = page.getByRole('checkbox', { name: 'I am reporting my own real-world status for this meeting revision.' })
    await expect(here).toBeDisabled()
    await page.getByRole('button', { name: 'Simulate virtual movement (fixture only)' }).click()
    assert.equal((await page.evaluate(() => window.__FE20__.state())).reports, 0)
    await check.focus(); await page.keyboard.press('Space'); await here.focus(); await page.keyboard.press('Enter')
    assert.equal((await page.evaluate(() => window.__FE20__.state())).reports, 1); await expect(here).toBeDisabled()
    await page.evaluate(() => window.__FE20__.mode('moved'))
    await expect(page.getByRole('heading', { name: 'Meeting moved or changed' })).toBeVisible(); await expect(here).toBeDisabled()
    await page.getByRole('button', { name: 'I have reviewed this revision' }).click()
    await expect(page.getByRole('heading', { name: 'Current meeting' })).toBeVisible()
    if (width === 390) { await page.evaluate(() => scrollTo(0, 0)); await page.screenshot({ path: resolve(evidence, 'current-390.png'), fullPage: true }) }
    await page.evaluate(() => window.__FE20__.mode('expired'))
    await expect(page.getByRole('heading', { name: 'Meeting expired' })).toBeVisible()
    await expect(page.getByText('SYNTHETIC red scarf', { exact: true })).toHaveCount(0); await expect(here).toBeDisabled()
    await page.evaluate(() => window.__FE20__.mode('ok'))
    await page.evaluate(() => window.__FE20__.mode('revoked'))
    await expect(page.getByRole('heading', { name: 'Meeting revoked' })).toBeVisible()
    await expect(page.getByText('SYNTHETIC red scarf', { exact: true })).toHaveCount(0)
    await page.evaluate(() => window.__FE20__.mode('ok'))
    await page.evaluate(() => window.__FE20__.mode('denied'))
    await expect(page.getByRole('heading', { name: 'Meeting unavailable' })).toBeVisible()
    if (width === 390) {
      await page.evaluate(() => window.__FE20__.mode('ok')); await page.evaluate(() => window.__FE20__.mode('lost'))
      await page.getByLabel('Reviewed place').selectOption('00000000-0000-4000-8000-000000000002')
      await page.getByLabel('Who can see this?').selectOption('synthetic-party')
      await page.getByLabel('How to find me').fill('SYNTHETIC marker')
      await page.getByLabel('Identification', { exact: true }).fill('SYNTHETIC private draft')
      await page.getByRole('checkbox', { name: 'I explicitly confirm this real-world description and its audience.' }).check()
      await page.getByRole('button', { name: 'Share check-in' }).click()
      await expect(page.getByText('The command outcome is unconfirmed. Check its status before making another change.').first()).toBeVisible()
      await expect(page.getByRole('button', { name: 'Share check-in' })).toBeDisabled()
      assert.equal((await page.evaluate(() => window.__FE20__.state())).writes, 1)
      await page.getByRole('button', { name: 'Check command status' }).last().click()
      assert.equal((await page.evaluate(() => window.__FE20__.state())).writes, 1)
      await page.evaluate(() => window.__FE20__.mode('privacy'))
      await expect(page.getByLabel('Identification', { exact: true })).toHaveValue('')
      await expect(page.getByText('SYNTHETIC red scarf', { exact: true })).toHaveCount(0)
      await page.evaluate(() => scrollTo(0, 0)); await page.screenshot({ path: resolve(evidence, 'privacy-cleared-390.png'), fullPage: true })
    }
    report.widths.push({ width, result: 'PASS', minimumTouchTarget: 44, horizontalOverflow: false })
    await page.evaluate(() => window.__FE20__.dispose()); assert.equal(await page.locator('main').count(), 0)
    await context.close()
  }
  assert.deepEqual(report.errors, [])
  report.checks = ['Keyboard acknowledgement and explicit self-report only', 'Moved revision confirmation', 'Expiry/revocation/denied scrubbing', 'Unknown write status check without mutation replay', 'Canonical privacy draft cleanup', '360/390/430 widths, >=44px controls, no page errors, owner disposal']
  report.contractHashes = Object.fromEntries(await Promise.all(['contracts/domain/v1/beacon.schema.json', 'contracts/domain/v1/common.schema.json', 'contracts/domain/v1/party.schema.json', 'frontend/src/shared/api/domain-operations.generated.ts'].map(async file => [file, createHash('sha256').update(await readFile(resolve(root, file))).digest('hex')])))
  report.result = 'PASS'
} catch (error) { report.failure = { name: error.name, message: error.message }; process.exitCode = 1 }
finally { await browser?.close(); await server.close(); report.cleanup = 'PASS'; await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report))
