import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import { chromium, expect } from '@playwright/test'

const root = resolve(import.meta.dirname, '../../../../../../..'), front = resolve(root, 'frontend')
assert(process.env.MCR_WALL_B_ACCEPTED, 'Fresh explicitly accepted allocation required')
const scratch = process.env.TMPDIR
assert(scratch?.startsWith(`${root}/.wall-tools-b-`), 'Own scratch custody required')
const out = resolve(root, 'docs/mcr/evidence/wall-tools-b', process.env.MCR_WALL_B_NATIVE_ROUND ?? 'native-round01')
await mkdir(out, { recursive: true })
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const report = { result: 'FAIL', candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  scope: 'Changed production editor/sync/tools-http, shared protected transport and incumbent world sink against a deterministic HTTP fixture using the frozen C04 wire. No actual BE, full production composition, physical-device or live IN04 claim.', widths: [], sourceHashes: {}, checks: [] }
for (const file of ['editor/MuralEditor.tsx', 'editor/controller.ts', 'editor/tools.ts', 'editor/tools-b.css', 'editor/editor.css', 'editor/input.ts',
  'sync/controller.ts', 'sync/MuralSyncPanel.tsx', 'sync/tools-http.ts', 'sync/http.ts', 'editor/__tests__/wall-tools-b/preview.tsx', 'editor/__tests__/wall-tools-b/browser.mjs'])
  report.sourceHashes[`frontend/src/features/mural/${file}`] = hash(await readFile(resolve(front, 'src/features/mural', file)))
for (const file of ['http.ts', 'domain.ts', 'domain-operations.generated.ts'])
  report.sourceHashes[`frontend/src/shared/api/${file}`] = hash(await readFile(resolve(front, 'src/shared/api', file)))
report.referenceSha256 = hash(await readFile(resolve(root, 'docs/mcr/design/reference-target-2026-11-04.png')))
const focusedRecovery = process.env.MCR_WALL_B_FOCUSED_RECOVERY === '1'
report.focusedRecovery = focusedRecovery
const port = Number(process.env.MCR_WALL_B_PORT ?? 5299)
const server = await createServer({ configFile: false, root: front, plugins: [react()], cacheDir: resolve(scratch, 'vite-native'),
  logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
let browser
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    args: ['--disk-cache-size=1048576', '--media-cache-size=1', '--disable-component-update', '--disable-background-networking'] })
  report.browser = browser.version()
  for (const width of focusedRecovery ? [390] : [360, 390, 430]) {
    const disk = await statfs(root); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3, 'Live disk floor')
    const page = await browser.newPage({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' }), errors = []
    page.setDefaultTimeout(8000); page.on('pageerror', error => errors.push(error.message))
    const surface = page.getByRole('group', { name: 'Mural pixel preview' })
    const status = page.getByRole('status', { name: 'Wall command status' })
    const model = () => page.evaluate(() => window.__WALL_B__.read())
    async function open() {
      await page.goto(`http://127.0.0.1:${port}/src/features/mural/editor/__tests__/wall-tools-b/preview.html`)
      await expect.poll(async () => (await model()).editor.toolStatus).toBe('ready')
    }
    async function geometry() {
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Horizontal page overflow')
      assert(await page.locator('button,input,summary').evaluateAll(elements => elements.filter(e => e.getClientRects().length).every(e => {
        const rect = e.getBoundingClientRect(); return rect.width >= 44 && rect.height >= 44
      })), 'Control target below 44px')
    }
    async function capture(name) {
      await geometry(); await page.screenshot({ path: resolve(out, `${width}-${name}.png`), fullPage: true })
      report.checks.push({ width, captured: name })
    }
    async function coordinates(x = 10, y = 10) {
      const detail = page.locator('details'); if (!await detail.evaluate(e => e.open)) await page.getByText('Coordinates & help', { exact: true }).click()
      await page.getByRole('spinbutton', { name: 'X', exact: true }).fill(String(x)); await page.getByRole('spinbutton', { name: 'Y', exact: true }).fill(String(y))
      await page.getByRole('button', { name: 'Preview pixel', exact: true }).click()
      await page.getByText('Coordinates & help', { exact: true }).click()
    }
    async function fill() { await page.getByRole('button', { name: 'Fill preview', exact: true }).click(); await page.getByRole('button', { name: 'Color 2 #A22635', exact: true }).click(); await coordinates() }
    if (!focusedRecovery) {
    await open(); await geometry()
    await expect(page.getByRole('heading', { name: 'THE BLACK PARADE' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Erase preview' })).toBeEnabled()
    await expect(page.getByRole('button', { name: 'Preview undo of my last action' })).toBeDisabled()
    assert.equal(await page.getByRole('button', { name: /^Color \d+ #/ }).count(), 16)
    await capture('ready')
    await fill(); await expect(page.getByLabel('Selection preview')).toContainText('16 changed cells')
    assert.equal((await model()).posts, 0); assert.equal((await model()).editor.snapshot.pixels[1290], 1)
    await capture('fill-preview')
    await page.getByRole('button', { name: 'Confirm fill' }).focus(); await page.keyboard.press('Enter')
    await expect.poll(async () => (await model()).editor.snapshot?.revision).toBe(11)
    assert.equal((await model()).posts, 1); assert.equal((await model()).editor.snapshot.pixels[1290], 2)
    await expect(page.getByRole('button', { name: 'Preview undo of my last action' })).toBeEnabled()
    await page.getByRole('button', { name: 'Preview undo of my last action' }).click(); await capture('undo-preview')
    await page.getByRole('button', { name: 'Confirm undo' }).click()
    await expect.poll(async () => (await model()).editor.snapshot?.revision).toBe(12)
    assert.equal((await model()).editor.snapshot.pixels[1290], 1); assert.equal((await model()).remaining, 480)

    await open(); await page.getByRole('button', { name: 'Erase preview' }).click(); await coordinates()
    await surface.focus(); await page.keyboard.press('ArrowRight'); await page.keyboard.press('Enter')
    assert.equal((await model()).posts, 0); assert.equal((await model()).editor.draft.operation, 'erase')
    await page.keyboard.press('Escape'); assert.equal((await model()).editor.draft, null)
    const rect = await surface.boundingBox(), offset = (rect.height - rect.width) / 2
    await surface.tap({ position: { x: rect.width * 10.5 / 128, y: offset + rect.width * 10.5 / 128 } })
    await expect(page.getByRole('button', { name: 'Confirm erase' })).toBeEnabled()
    await page.getByRole('button', { name: 'Confirm erase' }).click()
    await expect.poll(async () => (await model()).editor.snapshot?.revision).toBe(11)
    assert.equal((await model()).editor.snapshot.pixels[1290], 0)

    await open(); await page.evaluate(() => window.__WALL_B__.mode('lost')); await fill()
    await page.getByRole('button', { name: 'Confirm fill' }).click(); await expect(status).toContainText('unknown')
    const original = (await model()).editor.action.key
    await page.evaluate(() => window.__WALL_B__.reloadEditor())
    await expect.poll(async () => (await model()).editor.action?.key).toBe(original)
    assert.equal((await model()).posts, 1); await capture('recovered-original')
    await page.getByRole('button', { name: 'Check original command' }).click()
    await expect.poll(async () => (await model()).editor.phase).toBe('confirmed')
    await expect.poll(async () => (await model()).editor.action).toBeNull()
    assert.equal((await model()).editor.snapshot.revision, 11); assert.equal((await model()).posts, 1)

    }
    await open(); await page.evaluate(() => window.__WALL_B__.mode('competing')); await fill()
    await page.getByRole('button', { name: 'Confirm fill' }).click(); await expect(status).toContainText('unknown')
    const attempted = (await model()).editor.action.key, competing = (await model()).competingKey
    await page.getByRole('button', { name: 'Check original command' }).click(); await expect(status).toContainText('pending')
    assert.equal((await model()).editor.action.key, attempted); assert.equal((await model()).posts, 1)
    await page.getByRole('button', { name: 'Check original command' }).click()
    await expect.poll(async () => (await model()).editor.action?.key).toBe(competing)
    assert.deepEqual((await model()).editor.action.body, { expected_revision: 10, x: 10, y: 10 })
    assert.equal((await model()).editor.action.operation, 'erase'); assert.equal((await model()).posts, 1)
    await capture('competing-original')
    await page.evaluate(() => window.__WALL_B__.finishCompeting())
    await page.getByRole('button', { name: 'Check original command' }).click()
    await expect.poll(async () => (await model()).editor.phase).toBe('confirmed')
    assert.equal((await model()).editor.action, null); assert.equal((await model()).posts, 1)
    assert.equal((await model()).editor.snapshot.pixels[1290], 0)

    if (!focusedRecovery) {
    await open(); await fill(); await page.evaluate(() => window.__WALL_B__.remote())
    await expect.poll(async () => (await model()).editor.snapshot?.revision).toBe(11)
    await expect(page.getByRole('button', { name: 'Confirm fill' })).toHaveCount(0); assert.equal((await model()).posts, 0)
    await page.evaluate(() => window.__WALL_B__.budget(0)); await fill()
    await expect(page.getByRole('button', { name: 'Confirm fill' })).toBeDisabled(); await capture('budget-exhausted')
    await page.evaluate(() => window.__WALL_B__.oversized()); await fill()
    await expect(status).toContainText('exceeds 256'); assert.equal((await model()).posts, 0)
    await page.evaluate(() => window.__WALL_B__.freeze())
    await expect(page.getByRole('button', { name: 'Erase preview' })).toBeDisabled(); await expect(page.getByRole('button', { name: 'Pan wall' })).toBeEnabled()
    await capture('frozen')
    await page.setViewportSize({ width, height: 420 }); await geometry()
    await page.evaluate(() => window.__WALL_B__.logout()); await expect(page.locator('canvas')).toHaveCount(0)
    assert.equal((await model()).editor.toolContext, null); assert.equal((await model()).editor.action, null)
    }
    assert.deepEqual(errors, []); report.widths.push({ width, errors, checks: focusedRecovery ? 'ambiguous POST/pending retention, terminal FAILED-receipt retirement, different original key/operation/body GET recovery, one local POST, authoritative reset, no overflow/44px' : 'explicit fill/erase/undo, snapshot reset, keyboard/touch, server budget, stale draft, exact original reload recovery, ambiguous POST/pending retention then FAILED-receipt competing-original GET recovery, frozen/privacy, no overflow/44px' })
    await page.close()
  }
  report.result = 'PASS'
} finally {
  await browser?.close(); await server.close()
  await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(JSON.stringify({ result: report.result, browser: report.browser, widths: report.widths }, null, 2))
