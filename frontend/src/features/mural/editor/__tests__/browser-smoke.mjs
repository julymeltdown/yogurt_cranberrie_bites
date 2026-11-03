import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const root = resolve(import.meta.dirname, '../../../../../..'), out = resolve(import.meta.dirname, 'evidence/browser')
await mkdir(out, { recursive: true })
const server = await createServer({ root: resolve(root, 'frontend'), logLevel: 'error', server: { host: '127.0.0.1', port: 5218, strictPort: true } })
const report = { result: 'FAIL', scope: 'FE32 deterministic typed command-port fixture and canonical FE05 privacy. No HTTP/backend, multiuser, native phone, or IN04 acceptance.',
  base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), widths: [], checks: [], sourceHashes: {} }
for (const name of ['contract.ts', 'controller.ts', 'input.ts', 'MuralEditor.tsx', 'editor.css', '__tests__/fixture.ts', '__tests__/preview.tsx', '__tests__/browser-smoke.mjs']) {
  report.sourceHashes[name] = createHash('sha256').update(await readFile(resolve(import.meta.dirname, '..', name))).digest('hex')
}
let browser
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' })
    const page = await context.newPage(), errors = []
    page.on('pageerror', error => errors.push(error.message))
    await page.goto('http://127.0.0.1:5218/src/features/mural/editor/__tests__/preview.html')
    const surface = page.getByRole('group', { name: 'Mural pixel preview' }), confirm = page.getByRole('button', { name: 'Confirm pixel', exact: true })
    await expect(surface).toBeVisible(); await expect(confirm).toBeDisabled()
    await page.getByRole('button', { name: 'Color 2 #A22635', exact: true }).click()
    await page.getByRole('button', { name: 'Zoom in', exact: true }).click(); await page.getByRole('button', { name: 'Zoom in', exact: true }).click()
    const box = await surface.boundingBox(), px = box.x + box.width / 2, py = box.y + box.height / 2
    await page.mouse.move(px, py); await page.mouse.down(); await page.mouse.move(px + 65, py + 20, { steps: 8 }); await page.mouse.move(px, py, { steps: 8 }); await page.mouse.up()
    assert.equal(await page.evaluate(() => window.__MURAL__.read().selection), null)
    assert.equal(await page.evaluate(() => window.__MURAL__.counts().places), 0)
    // Real Chromium multitouch input must invalidate the whole gesture, including its final finger.
    const cdp = await context.newCDPSession(page)
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: px - 15, y: py, id: 1 }, { x: px + 15, y: py, id: 2 }] })
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] })
    assert.equal(await page.evaluate(() => window.__MURAL__.read().selection), null)
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: px, y: py, id: 1 }] })
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchCancel', touchPoints: [] })
    assert.equal(await page.evaluate(() => window.__MURAL__.read().selection), null)
    await surface.click({ position: { x: box.width / 2, y: box.height / 2 } })
    await expect(confirm).toBeEnabled(); assert.equal(await page.evaluate(() => window.__MURAL__.counts().places), 0)
    await surface.press('Escape'); await expect(confirm).toBeDisabled()
    await surface.press('ArrowRight'); await surface.press('ArrowDown'); await surface.press('Enter')
    assert.equal(await page.evaluate(() => window.__MURAL__.counts().places), 0)
    await expect(page.getByRole('spinbutton', { name: 'X', exact: true })).toHaveValue('1')
    await expect(page.getByRole('spinbutton', { name: 'Y', exact: true })).toHaveValue('1')
    await page.screenshot({ path: resolve(out, `${width}-preview.png`), fullPage: true })
    const dimensions = await page.locator('.mcr-mural button, .mcr-mural input').evaluateAll(elements => elements.map(element => {
      const r = element.getBoundingClientRect(); return { width: r.width, height: r.height, label: element.textContent || element.getAttribute('aria-label') }
    }))
    assert(dimensions.every(rect => rect.width >= 44 && rect.height >= 44), JSON.stringify(dimensions))
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
    assert(await page.locator('.mcr-mural *').count() < 130, 'unbounded per-pixel DOM')
    await confirm.focus(); await expect(confirm).toBeFocused(); await page.keyboard.press('Enter')
    await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('Server confirmed'); assert.equal(await page.evaluate(() => window.__MURAL__.counts().places), 1)
    assert.equal(await page.evaluate(() => window.__MURAL__.read().snapshot.revision), 11)
    await expect(confirm).toBeDisabled(); await expect(page.getByLabel('Selection preview')).toContainText('Cooldown:')
    // A fresh verified fixture snapshot resets the server-relative cooldown for independent cases.
    await page.evaluate(() => { window.__MURAL__.refresh(); window.__MURAL__.mode('cooldown') })
    await surface.focus(); await surface.press('ArrowRight'); await confirm.click()
    await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('Server cooldown active'); await expect(confirm).toBeDisabled()
    await page.evaluate(() => { window.__MURAL__.refresh(); window.__MURAL__.mode('rejected') })
    await page.getByRole('spinbutton', { name: 'X', exact: true }).fill('127'); await page.getByRole('spinbutton', { name: 'Y', exact: true }).fill('127')
    await page.getByRole('button', { name: 'Preview pixel', exact: true }).click(); await expect(page.getByLabel('Selection preview')).toContainText('(127, 127)')
    await confirm.click(); await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('server rejected'); await expect(page.getByLabel('Selection preview')).toContainText('No pixel selected')
    await page.evaluate(() => window.__MURAL__.mode('lost'))
    await surface.focus(); await surface.press('ArrowRight'); await confirm.click()
    await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('Outcome unknown'); await expect(confirm).toBeDisabled()
    const places = await page.evaluate(() => window.__MURAL__.counts().places)
    await page.screenshot({ path: resolve(out, `${width}-unknown.png`), fullPage: true })
    await page.getByRole('button', { name: 'Refresh confirmed wall' }).click(); await expect(confirm).toBeDisabled()
    await page.getByRole('button', { name: 'Check original command' }).click(); await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('Server confirmed')
    assert.equal(await page.evaluate(() => window.__MURAL__.counts().places), places)
    assert.equal(await page.evaluate(() => window.__MURAL__.counts().checks), 1)
    await page.evaluate(() => window.__MURAL__.refresh('PAUSED')); await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('paused'); await expect(confirm).toBeDisabled()
    await page.evaluate(() => window.__MURAL__.refresh('FROZEN')); await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('frozen'); await expect(confirm).toBeDisabled()
    await page.screenshot({ path: resolve(out, `${width}-frozen.png`), fullPage: true })
    await page.evaluate(() => { window.__MURAL__.refresh(); window.__MURAL__.readOnly() }); await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('read-only')
    await page.evaluate(() => window.__MURAL__.logout()); await expect(surface).toHaveCount(0); await expect(page.getByRole('spinbutton')).toHaveCount(0)
    assert.equal(await page.evaluate(() => window.__MURAL__.read().command), null)
    assert.deepEqual(errors, []); report.widths.push(width); await cdp.detach(); await context.close()
  }
  report.checks = ['drag out-and-back does not preview or place', 'Chromium multitouch cancels selection', 'pointercancel does not preview or place',
    'tap survives automatic capture release', 'Escape cancels preview', 'arrow keys preview, Enter on canvas never places', 'explicit keyboard confirm places once',
    'all button/input targets >=44px; no horizontal overflow', 'bounded DOM under130 nodes', '127,127 DOM coordinate alternative',
    'acknowledgement advances confirmed revision; cooldown disables placement', 'server cooldown and terminal failure rollback',
    'unknown outcome blocks new placement across refresh', 'read-only status recovery never replays placement', 'paused/frozen/read-only states', 'logout scrubs DOM/command', 'no browser runtime errors']
  report.result = 'PASS'
} finally {
  await browser?.close(); await server.close()
  await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(JSON.stringify(report, null, 2))
