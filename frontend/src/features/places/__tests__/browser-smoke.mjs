// Test-only C03 fixtures + actual FE14/FE15/Phaser camera; no live service or venue claim.
import assert from 'node:assert/strict'
import { createServer as createNetServer } from 'node:net'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const root = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(root, 'frontend')
const evidence = process.env.MCR_FE19_EVIDENCE
assert(evidence && evidence.startsWith('/'), 'Set MCR_FE19_EVIDENCE to a NEW absolute evidence directory')
await mkdir(evidence, { mode: 0o700 })
const hash = bytes => createHash('sha256').update(bytes).digest('hex'), hashes = {}
async function collect(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = resolve(dir, entry.name)
    if (entry.isDirectory()) await collect(path)
    else if (/\.(ts|tsx|mjs|html|css|json)$/.test(entry.name)) hashes[relative(root, path)] = hash(await readFile(path))
  }
}
await collect(resolve(frontend, 'src'))
const report = { scope: 'FE19 deterministic C03/C06 fixtures, actual Phaser camera and existing shell; no backend/venue/production acceptance',
  result: 'FAIL', revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourceHashes: hashes,
  contracts: Object.fromEntries(await Promise.all(['contracts/generated/mcr-core.openapi.json', 'contracts/assets/v1/map-manifest.schema.json', 'contracts/assets/v1/common.schema.json'].map(async path => [path, hash(await readFile(resolve(root, path)))]))),
  widths: [], checks: [], cleanup: 'NOT_RUN' }
const reservation = createNetServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve))
const port = reservation.address().port; await new Promise(resolve => reservation.close(resolve))
const server = await createServer({ root: frontend, logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
let browser, stage = 'start'
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const [width, height] of [[360, 800], [390, 844], [430, 932]]) {
    stage = `phone-${width}`
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage(), errors = []
    page.on('pageerror', () => errors.push('pageerror'))
    const state = () => page.evaluate(() => window.__FE19__.state())
    const mode = async value => { await page.evaluate(value => window.__FE19__.mode(value), value) }
    await page.goto(`http://127.0.0.1:${port}/src/features/places/__tests__/fixtures/preview.html`)
    await expect(page.getByRole('heading', { name: 'Places in the world' })).toBeVisible()
    await expect.poll(async () => (await state()).runtime?.phase).toBe('ready')
    await expect.poll(async () => (await state()).places.list.phase).toBe('ready')
    await expect(page.getByRole('button', { name: 'Expand panel' })).toBeVisible(); await page.getByRole('button', { name: 'Expand panel' }).click()
    await expect(page.getByRole('img', { name: /Virtual map overview/ })).toBeVisible()
    assert.equal((await state()).canvases, 1); assert.equal((await state()).mounts, 1)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal page overflow')
    for (const target of await page.locator('.places-view button,.places-view a,.places-view input,.places-view summary').all()) {
      const box = await target.boundingBox(); assert(box && box.width >= 44 && box.height >= 44, 'small feature touch target')
    }
    const legend = page.locator('.places-legend-disclosure > summary')
    await legend.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('region', { name: 'Virtual and physical place legend' })).toBeVisible()
    await page.keyboard.press('Enter')
    const foot = (await state()).foot
    await page.getByRole('button', { name: 'Zoom in', exact: true }).last().click()
    await expect.poll(async () => (await state()).camera?.camera?.zoom).toBe(2)
    await page.getByRole('button', { name: 'Show Synthetic reviewed landmark on virtual map' }).click()
    await expect.poll(async () => (await state()).camera?.following).toBe(false)
    assert.deepEqual((await state()).foot, foot, 'camera action moved avatar')
    await page.getByRole('button', { name: 'Recenter avatar' }).click()
    await expect.poll(async () => (await state()).camera?.following).toBe(true)
    const concept = page.getByRole('link', { name: 'View Synthetic virtual stage' })
    await concept.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('heading', { name: 'Synthetic virtual stage', exact: true })).toBeVisible()
    await expect(page.locator('#panel-title')).toBeFocused()
    await expect(page.getByText('This landmark exists in the virtual world only. Do not use it as a physical meeting destination.')).toBeVisible()
    assert.equal((await state()).mounts, 1); assert.equal((await state()).canvases, 1)
    await page.goBack()
    await expect(page.getByRole('link', { name: 'View Synthetic virtual stage' })).toBeFocused()
    await page.getByRole('link', { name: 'View Synthetic reviewed landmark' }).click()
    await expect(page.getByRole('heading', { name: 'Synthetic reviewed landmark', exact: true })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Open source reference (new tab)' })).toHaveAttribute('href', 'https://venue.example.invalid/reference')
    await expect(page.getByRole('link', { name: 'Open source reference (new tab)' })).toHaveAttribute('rel', 'noopener noreferrer')
    assert.equal(await page.locator('.places-view img').count(), 0)
    await page.getByRole('link', { name: 'Read controls and meeting safety' }).click()
    await expect(page.getByRole('heading', { name: 'One world, at your pace' })).toBeVisible()
    await expect(page.getByText(/Walking an avatar does not confirm real arrival/)).toBeVisible()
    await page.getByRole('link', { name: 'Browse places' }).click()
    await expect(page.getByRole('heading', { name: 'Places in the world' })).toBeVisible()
    if (width === 390) {
      for (const [label, text] of [['Synthetic unverified gate', 'Physical place — verification pending'], ['Synthetic closed landmark', 'Closed — do not use as a meeting destination'], ['Synthetic unavailable source', 'Physical reference — source unavailable']]) {
        await page.getByRole('link', { name: `View ${label}` }).click()
        await expect(page.getByRole('heading', { name: label, exact: true })).toBeVisible(); await expect(page.getByText(text, { exact: true })).toBeVisible()
        if (label.includes('closed')) await expect(page.getByRole('button', { name: 'Show on virtual map' })).toBeDisabled()
        if (label.includes('source')) await expect(page.getByRole('link', { name: 'Open source reference (new tab)' })).toHaveCount(0)
        await page.getByRole('link', { name: 'All places', exact: true }).click()
      }
      await page.getByRole('searchbox', { name: 'Find a place' }).fill('not present'); await expect(page.getByText('No places match. Try a different name.')).toBeVisible()
      await page.getByRole('searchbox').fill('')
      for (const [value, heading] of [['error', 'Places could not be loaded'], ['map-error', 'Your map and places do not match'], ['mismatch', 'Your map and places do not match'], ['rate', 'Please wait before retrying'], ['denied', 'Places are not available to this account']]) {
        await mode(value); await expect(page.getByRole('heading', { name: heading })).toBeVisible()
        const target = page.getByRole('button', { name: 'Show Synthetic virtual stage on virtual map' })
        if (await target.count()) await expect(target).toBeDisabled()
        if (value === 'mismatch') { await page.locator('.shell-panel-body').evaluate(element => { element.scrollTop = 0 }); await page.screenshot({ path: resolve(evidence, 'mismatch-390.png'), fullPage: true }) }
        await mode('ok'); await expect(page.getByRole('link', { name: 'View Synthetic virtual stage' })).toBeVisible()
      }
      await mode('empty'); await expect(page.getByText('No places are published for this event yet.')).toBeVisible()
      await mode('ok'); await expect(page.getByRole('link', { name: 'View Synthetic virtual stage' })).toBeVisible()
      await page.locator('.shell-panel-body').evaluate(element => { element.scrollTop = 0 }); await page.screenshot({ path: resolve(evidence, 'overview-390.png'), fullPage: true })
      await page.getByRole('link', { name: 'View Synthetic reviewed landmark' }).click()
      await expect(page.getByRole('heading', { name: 'Synthetic reviewed landmark', exact: true })).toBeVisible()
      await page.locator('.shell-panel-body').evaluate(element => { element.scrollTop = 0 }); await page.screenshot({ path: resolve(evidence, 'detail-390.png'), fullPage: true })
      await mode('removed'); await expect(page.getByRole('heading', { name: 'This place is unavailable' })).toBeVisible()
      await mode('ok'); await expect(page.getByRole('heading', { name: 'Synthetic reviewed landmark', exact: true })).toBeVisible()
      await mode('conflict'); await expect(page.getByRole('heading', { name: 'Place information changed' })).toBeVisible()
      await mode('ok'); await expect(page.getByRole('heading', { name: 'Synthetic reviewed landmark', exact: true })).toBeVisible()
      await page.getByRole('link', { name: 'Read controls and meeting safety' }).click()
      await expect(page.getByRole('heading', { name: 'One world, at your pace' })).toBeVisible()
      await page.locator('.shell-panel-body').evaluate(element => { element.scrollTop = 0 }); await page.screenshot({ path: resolve(evidence, 'help-390.png'), fullPage: true })
      report.checks.push('All five place statuses, source link safety, empty/search/error/rate/denied/removed/conflict/map mismatch and recovery')
    }
    assert.deepEqual(errors, [])
    await page.evaluate(() => window.__FE19__.dispose())
    assert.equal(await page.locator('canvas').count(), 0); assert.equal(await page.locator('.world-controls').count(), 0)
    assert.equal((await state()).disposals, 1)
    report.widths.push({ width, height, result: 'PASS', onePersistentCanvas: true, unchangedAvatarFoot: true, focusRestored: true, cleanup: 'PASS' })
    await context.close()
  }
  report.checks.push('Actual Phaser zoom/look/recenter through existing FE15 camera, one persistent shell canvas, keyboard navigation and focus return, >=44px feature controls at 360/390/430')
  report.result = 'PASS'
} catch (error) { report.failureStage = stage; report.failure = { name: error.name, message: error.message }; process.exitCode = 1 }
finally {
  await browser?.close(); await server.close(); report.cleanup = 'PASS'
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n', { mode: 0o600 })
}
console.log(JSON.stringify({ result: report.result, widths: report.widths.length, cleanup: report.cleanup, failureStage: report.failureStage }))
