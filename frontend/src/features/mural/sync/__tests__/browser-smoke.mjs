import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../../..'), out = resolve(import.meta.dirname, 'evidence/browser')
await mkdir(out, { recursive: true })
const server = await createServer({ root: resolve(root, 'frontend'), logLevel: 'error', server: { host: '127.0.0.1', port: 5220, strictPort: true } })
const report = { result: 'FAIL', scope: 'FE33 two independent canonical-session consumers against in-process deterministic C04 HTTP fixtures, one actual Phaser game/two texture sinks, no world socket. C04 0.1.2 original-result HTTP route is simulated as unavailable then enabled; deployment is NOT_RUN. Real multiuser, device and IN04 acceptance NOT_RUN.',
  base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), widths: [], sourceHashes: {}, checks: [] }
for (const name of ['frontend/src/features/mural/sync/controller.ts', 'frontend/src/features/mural/sync/contract.ts', 'frontend/src/features/mural/sync/projection.ts',
  'frontend/src/features/mural/sync/http.ts', 'frontend/src/features/mural/sync/MuralSyncPanel.tsx', 'frontend/src/world/mural/sink.ts',
  'frontend/src/world/mural/phaser.ts', 'frontend/src/world/mural/authority.ts', 'frontend/src/features/mural/sync/__tests__/fixture.ts',
  'frontend/src/features/mural/sync/__tests__/preview.tsx', 'frontend/src/features/mural/sync/__tests__/browser-smoke.mjs']) {
  report.sourceHashes[name] = createHash('sha256').update(await readFile(resolve(root, name))).digest('hex')
}
let browser
try {
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' }), errors = []
    page.on('pageerror', e => errors.push(e.message))
    await page.goto('http://127.0.0.1:5220/src/features/mural/sync/__tests__/preview.html')
    const syncStatus = page.getByRole('status', { name: 'Wall synchronization status' }), confirm = page.getByRole('button', { name: 'Confirm pixel', exact: true })
    await expect(syncStatus).toContainText('revision 10'); await expect(page.getByLabel('Peer revisions')).toContainText('A revision 10 · B revision 10')
    await page.waitForFunction(() => window.__MURAL_SYNC__.read().every(c => c.resources.textures === 1))
    assert.deepEqual(await page.evaluate(() => window.__MURAL_SYNC__.counts()), { sockets: 0, games: 1, placements: 0, requests: 2 })
    await page.getByRole('spinbutton', { name: 'X', exact: true }).fill('3'); await page.getByRole('spinbutton', { name: 'Y', exact: true }).fill('4')
    await page.getByRole('button', { name: 'Preview pixel', exact: true }).click(); await page.getByRole('button', { name: 'Color 2 #A22635', exact: true }).click()
    assert.equal(await page.evaluate(() => window.__MURAL_SYNC__.counts().placements), 0)
    await confirm.focus(); await page.keyboard.press('Enter'); await expect(syncStatus).toContainText('revision 11')
    await page.evaluate(() => window.__MURAL_SYNC__.refreshAll()); await expect(page.getByLabel('Peer revisions')).toContainText('A revision 11 · B revision 11')
    await page.evaluate(() => window.__MURAL_SYNC__.reordered()); await expect(page.getByLabel('Peer revisions')).toContainText('A revision 13 · B revision 13')
    assert(await page.evaluate(() => { const [a, b] = window.__MURAL_SYNC__.read(); return JSON.stringify(a.editor.snapshot.pixels) === JSON.stringify(b.editor.snapshot.pixels) }))
    await page.screenshot({ path: resolve(out, `${width}-converged.png`), fullPage: true })
    await page.getByRole('button', { name: 'Client B', exact: true }).click(); await expect(syncStatus).toContainText('revision 13')
    await page.getByRole('button', { name: 'Client A', exact: true }).click()
    await page.evaluate(() => window.__MURAL_SYNC__.mode('lost'))
    await page.getByRole('spinbutton', { name: 'X', exact: true }).fill('127'); await page.getByRole('spinbutton', { name: 'Y', exact: true }).fill('127')
    await page.getByRole('button', { name: 'Preview pixel', exact: true }).click(); await confirm.click()
    const unresolved = page.getByRole('status', { name: 'Original placement status' })
    await expect(unresolved).toContainText('unresolved'); await expect(confirm).toBeDisabled()
    const placements = await page.evaluate(() => window.__MURAL_SYNC__.counts().placements)
    await page.evaluate(() => { window.__MURAL_SYNC__.mode('ack'); return window.__MURAL_SYNC__.refreshAll() })
    await expect(page.getByLabel('Peer revisions')).toContainText('A revision 14 · B revision 14'); await expect(confirm).toBeDisabled()
    await page.getByRole('button', { name: 'Check original command' }).click(); await expect(unresolved).toContainText('unresolved')
    assert.equal(await page.evaluate(() => window.__MURAL_SYNC__.counts().placements), placements)
    await page.screenshot({ path: resolve(out, `${width}-unknown.png`), fullPage: true })
    await page.evaluate(() => window.__MURAL_SYNC__.allowOriginal()); await page.getByRole('button', { name: 'Check original command' }).click()
    await expect(unresolved).toHaveCount(0); await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('Server confirmed')
    assert.equal(await page.evaluate(() => window.__MURAL_SYNC__.counts().placements), placements)
    await page.evaluate(() => window.__MURAL_SYNC__.restore('PAUSED')); await expect(syncStatus).toContainText('revision 34'); await expect(confirm).toBeDisabled()
    await page.evaluate(() => window.__MURAL_SYNC__.restore('FROZEN')); await expect(syncStatus).toContainText('revision 54'); await expect(confirm).toBeDisabled()
    await page.screenshot({ path: resolve(out, `${width}-frozen.png`), fullPage: true })
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
    assert(await page.locator('button,input').evaluateAll(elements => elements.every(e => { const r = e.getBoundingClientRect(); return r.width >= 44 && r.height >= 44 })), 'undersized control')
    assert.deepEqual(await page.evaluate(() => { const c = window.__MURAL_SYNC__.counts(); return { games: c.games, sockets: c.sockets } }), { games: 1, sockets: 0 })
    await page.evaluate(() => window.__MURAL_SYNC__.logout()); await expect(page.getByRole('group', { name: 'Mural pixel preview' })).toHaveCount(0)
    assert(await page.evaluate(() => window.__MURAL_SYNC__.read().every(c => c.editor.snapshot === null && c.editor.command === null && c.resources.textures === 0 && c.resources.objects === 0 && c.resources.listeners === 0)))
    assert.deepEqual(errors, []); report.widths.push(width); await page.evaluate(() => window.__MURAL_SYNC__.dispose()); await page.close()
  }
  report.result = 'PASS'; report.checks = ['two canonical actors converge with ordered/reordered/duplicate delivery', 'new/reconnect snapshot and editor interplay',
    'preview sends no mutation; keyboard confirm sends once', 'actual one Phaser game, zero world sockets, two independently owned textures',
    'unknown survives authoritative snapshot and SUCCEEDED metadata without original own result', 'C04 0.1.2 HTTP own-result recovers without POST replay',
    'pause/freeze create higher revisions and disable editing', '360/390/430 no overflow and targets>=44px', 'privacy clears editor and all texture/object/frame resources', 'no runtime errors']
} finally { await browser?.close(); await server.close(); await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report, null, 2))
