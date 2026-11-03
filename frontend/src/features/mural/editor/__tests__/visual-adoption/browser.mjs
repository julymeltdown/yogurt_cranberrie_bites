import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const phase = process.env.UIV05_PHASE ?? 'after', root = new URL('../../../../../../../', import.meta.url)
assert(['before', 'after'].includes(phase))
const out = new URL(`./evidence/${phase}/`, import.meta.url), sources = {}, results = [], errors = [], flows = []
await mkdir(out, { recursive: true })
const hash = b => createHash('sha256').update(b).digest('hex')
const mural = 'frontend/src/features/mural/'
for (const path of ['editor/MuralEditor.tsx', 'editor/editor.css', 'editor/controller.ts', 'editor/input.ts', 'editor/contract.ts', 'editor/__tests__/fixture.ts', 'editor/__tests__/visual-adoption/preview.tsx', 'editor/__tests__/visual-adoption/browser.mjs', 'sync/MuralSyncPanel.tsx', 'sync/controller.ts', 'sync/projection.ts', 'sync/contract.ts', 'sync/__tests__/fixture.ts', 'sync/__tests__/visual-adoption/preview.tsx'].map(p => mural + p).concat(['frontend/src/shared/ui/concert-window/index.tsx', 'frontend/src/shared/ui/concert-window/concert-window.css', 'frontend/src/shared/styles/base.css', 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg', 'contracts/domain/v1/mural.schema.json'])) sources[path] = hash(await readFile(new URL(path, root)))
if (phase === 'after') for (const lane of ['editor', 'sync']) for (const file of ['preview.html', 'presentation.test.tsx']) { const path = `${mural}${lane}/__tests__/visual-adoption/${file}`; sources[path] = hash(await readFile(new URL(path, root))) }
const server = await createServer({ configLoader: 'runner', cacheDir: `/tmp/mcr-uiv05-${phase}-vite`, server: { host: '127.0.0.1', port: 0 } })
let browser
try {
  await server.listen()
  const base = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const cases = phase === 'before' ? [['editor', 'loaded'], ['editor', 'preview'], ['sync', 'loaded']] : ['editor', 'sync'].flatMap(route => ['loaded', 'empty', 'error', 'locked', 'frozen', 'cooldown', 'pending', 'unknown', 'preview', 'loading', 'read-only', 'sending', 'paused', ...(route === 'sync' ? ['gap', 'unavailable'] : [])].map(state => [route, state]))
  const url = (route, state) => `${base}/src/features/mural/${route}/__tests__/visual-adoption/preview.html?state=${state}`
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => errors.push(e.message))
    for (const [route, state] of cases) {
      await page.goto(url(route, state)); await expect(page.locator('.mcr-mural')).toBeVisible()
      await page.waitForFunction(() => Boolean(window.__WALL_VISUAL__ || window.__WALL_SYNC_VISUAL__))
      const c = await page.evaluate(() => { const c = window.__WALL_VISUAL__?.controller ?? window.__WALL_SYNC_VISUAL__.sync.editor; return { phase: c.read().phase, availability: c.read().availability, selection: c.read().selection, snapshot: c.read().snapshot !== null, canConfirm: c.canConfirm() } })
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${route}/${state}/${width} overflow`)
      const controls = await page.locator('.mcr-mural button,.mcr-mural input').evaluateAll(els => els.map(el => { const r = el.getBoundingClientRect(); return { width: r.width, height: r.height, name: el.getAttribute('aria-label') || el.textContent } }))
      for (const r of controls) assert(r.width >= 44 && r.height >= 44, JSON.stringify({ route, state, width, r }))
      if (c.snapshot) {
        assert.equal(await page.getByRole('button', { name: /^Color \d+ #/ }).count(), 16)
        await expect(page.getByRole('spinbutton', { name: 'X', exact: true })).toBeVisible()
        await expect(page.getByRole('button', { name: 'Confirm pixel', exact: true })).toBeEnabled({ enabled: c.canConfirm })
      }
      if (['unknown', 'pending', 'sending'].includes(state)) await expect(page.getByRole('button', { name: 'Check original command' })).toBeVisible()
      if (state === 'locked') await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('region is locked')
      if (state === 'gap') await expect(page.getByRole('status', { name: 'Wall synchronization status' })).toContainText('out of order')
      const name = `${route}-${state}-${width}.png`
      await page.screenshot({ path: new URL(name, out).pathname, fullPage: true })
      results.push({ route, state, width, ...c, png: name, sha256: hash(await readFile(new URL(name, out))), controls: controls.length })
      if (phase === 'after') {
        await page.keyboard.press('Tab'); assert(await page.evaluate(() => document.activeElement.tagName !== 'BODY' && getComputedStyle(document.activeElement).outlineStyle !== 'none'))
        await page.setViewportSize({ width, height: 420 }); await page.getByRole('button', { name: 'Refresh confirmed wall' }).scrollIntoViewIfNeeded(); await expect(page.getByRole('button', { name: 'Refresh confirmed wall' })).toBeInViewport()
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
        if (['preview', 'unknown'].includes(state)) await page.screenshot({ path: new URL(`${route}-${state}-${width}-height420.png`, out).pathname })
        await page.setViewportSize({ width, height: 844 })
      }
      await page.evaluate(() => (window.__WALL_VISUAL__ ?? window.__WALL_SYNC_VISUAL__).dispose())
    }
    if (phase === 'after') {
      await page.goto(url('editor', 'loaded')); const surface = page.getByRole('group', { name: 'Mural pixel preview' }), confirm = page.getByRole('button', { name: 'Confirm pixel', exact: true })
      await expect(surface).toBeVisible(); await page.getByRole('button', { name: 'Color 2 #A22635', exact: true }).click()
      await surface.focus(); await surface.press('ArrowRight'); await surface.press('ArrowDown'); await surface.press('Enter')
      assert.equal((await page.evaluate(() => window.__WALL_VISUAL__.counts())).places, 0)
      await surface.press('Escape'); await expect(confirm).toBeDisabled()
      await page.getByRole('button', { name: 'Zoom in', exact: true }).click(); await expect(page.getByLabel('Zoom level')).toHaveText('2×'); await page.getByRole('button', { name: 'Fit wall' }).click()
      await page.getByRole('spinbutton', { name: 'X', exact: true }).fill('127'); await page.getByRole('spinbutton', { name: 'Y', exact: true }).fill('127'); await page.getByRole('button', { name: 'Preview pixel', exact: true }).click()
      await expect(page.getByLabel('Selection preview')).toContainText('(127, 127)')
      assert.equal(await page.evaluate(() => window.__WALL_VISUAL__.controller.read().snapshot.pixels[16383]), 0)
      await confirm.focus(); await page.keyboard.press('Enter'); await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('Server confirmed')
      assert.deepEqual(await page.evaluate(() => ({ ...window.__WALL_VISUAL__.counts(), revision: window.__WALL_VISUAL__.controller.read().snapshot.revision, pixel: window.__WALL_VISUAL__.controller.read().snapshot.pixels[16383] })), { places: 1, checks: 0, revision: 11, pixel: 2 })
      await expect(confirm).toBeDisabled(); await page.evaluate(() => window.__WALL_VISUAL__.dispose())
      await page.goto(url('sync', 'unknown')); await expect(page.getByRole('button', { name: 'Check original command' })).toBeVisible()
      const key = await page.evaluate(() => window.__WALL_SYNC_VISUAL__.sync.editor.read().command.key)
      await page.getByRole('button', { name: 'Refresh confirmed wall' }).click(); await page.getByRole('button', { name: 'Check original command' }).focus(); await page.keyboard.press('Enter')
      await expect(page.getByRole('status', { name: 'Wall command status' })).toContainText('Outcome unknown')
      assert.deepEqual(await page.evaluate(() => ({ ...window.__WALL_SYNC_VISUAL__.counts(), key: window.__WALL_SYNC_VISUAL__.sync.editor.read().command.key })), { places: 1, checks: 1, key })
      await page.evaluate(() => window.__WALL_SYNC_VISUAL__.dispose()); flows.push({ width, keyboardAndCoordinates: true, previewNoWrite: true, explicitConfirm: true, cooldown: true, originalCommandNoReplay: true })
    }
    await context.close()
  }
  assert.deepEqual(errors, [])
  await writeFile(new URL('report.json', out), JSON.stringify({ status: 'PASS', phase, base: '3e3b54a3aa6052536cc48b5afadc0992e7482084', fixtureOnly: true, browser: browser.version(), sources, results, flows, errors }, null, 2) + '\n')
  console.log(JSON.stringify({ status: 'PASS', phase, cases: results.length, flows: flows.length }))
} finally { await browser?.close(); await server.close() }
