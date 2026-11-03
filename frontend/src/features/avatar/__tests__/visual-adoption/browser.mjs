import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const phase = process.env.UIV04_PHASE ?? 'after', root = new URL('../../../../../../', import.meta.url)
assert(['before', 'after'].includes(phase))
const out = new URL(`./evidence/${phase}/`, import.meta.url), sources = {}, results = [], flows = [], errors = []
await mkdir(out, { recursive: true })
const hash = b => createHash('sha256').update(b).digest('hex')
for (const path of ['frontend/src/features/avatar/AvatarEditor.tsx', 'frontend/src/features/avatar/avatar-editor.css', 'frontend/src/features/avatar/controller.ts', 'frontend/src/features/avatar/contract.ts', 'frontend/src/features/avatar/preview.ts', 'frontend/src/features/avatar/__tests__/fixtures/harness.ts', 'frontend/src/features/avatar/__tests__/visual-adoption/preview.tsx', 'frontend/src/features/avatar/__tests__/visual-adoption/preview.html', 'frontend/src/features/avatar/__tests__/visual-adoption/browser.mjs', 'frontend/src/shared/ui/concert-window/index.tsx', 'frontend/src/shared/ui/concert-window/concert-window.css', 'frontend/src/shared/ui/Tabs.tsx', 'frontend/src/shared/styles/base.css', 'frontend/src/app/account-entry/panels.tsx', 'frontend/src/entities/avatar/character-source-pack.json', 'contracts/fixtures/assets/avatar-manifest.json', 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg']) sources[path] = hash(await readFile(new URL(path, root)))
const dev = await createServer({ configLoader: 'runner', cacheDir: `/tmp/mcr-uiv04-avatar-${phase}`, server: { host: '127.0.0.1', port: 0 } })
let browser
try {
  await dev.listen(); const base = `http://127.0.0.1:${dev.httpServer.address().port}/src/features/avatar/__tests__/visual-adoption/preview.html`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => errors.push(e.message))
    const states = phase === 'before' ? ['loaded', 'unavailable', 'candidate'] : ['loaded', 'unavailable', 'loading', 'load-error', 'draft', 'denied', 'unknown', 'conflict', 'candidate', 'saving', 'saved', 'preview-loading', 'expired']
    for (const state of states) {
      await page.goto(`${base}?state=${state}`); await expect(page.locator('.mcr-avatar-editor')).toBeVisible()
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${state}/${width} overflow`)
      const controls = await page.locator('.mcr-avatar-editor button,.mcr-avatar-editor select').evaluateAll(els => els.filter(el => el.getClientRects().length).map(el => { const b = el.getBoundingClientRect(); return { width: b.width, height: b.height, label: el.textContent } }))
      for (const c of controls) assert(c.width >= 44 && c.height >= 44, JSON.stringify({ state, width, c }))
      const save = page.getByRole('button', { name: 'Save outfit', exact: true })
      if (['loaded', 'unavailable', 'loading', 'load-error', 'unknown', 'conflict', 'candidate', 'saving', 'saved', 'expired'].includes(state)) await expect(save).toBeDisabled()
      if (state === 'draft' || state === 'denied') await expect(save).toBeEnabled()
      if (state === 'unavailable') { await expect(page.getByRole('status')).toContainText('Reviewed avatar artwork is unavailable'); assert.equal(await page.locator('canvas').count(), 0); assert.equal((await page.evaluate(() => window.__AVATAR_VISUAL__.counts())).assetReads, 0) }
      if (state === 'candidate') await expect(page.getByRole('region', { name: 'Saved outfit reconciliation' })).toBeVisible()
      const file = `${state}-${width}.png`; await page.screenshot({ path: new URL(file, out).pathname, fullPage: true })
      results.push({ state, width, controls: controls.length, png: file, sha256: hash(await readFile(new URL(file, out))) })
      if (phase === 'after') {
        if (await page.locator('.mcr-avatar-editor button:not(:disabled),.mcr-avatar-editor select:not(:disabled)').count()) { await page.keyboard.press('Tab'); assert(await page.evaluate(() => document.activeElement.tagName !== 'BODY' && getComputedStyle(document.activeElement).outlineStyle !== 'none')) }
        await page.setViewportSize({ width, height: 420 }); await page.getByRole('button', { name: 'Cancel changes', exact: true }).scrollIntoViewIfNeeded(); await expect(page.getByRole('button', { name: 'Cancel changes', exact: true })).toBeInViewport()
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
        if (['draft', 'candidate', 'unavailable'].includes(state)) await page.screenshot({ path: new URL(`${state}-${width}-height420.png`, out).pathname })
        await page.setViewportSize({ width, height: 844 })
      }
      await page.evaluate(() => window.__AVATAR_VISUAL__.dispose())
    }
    if (phase === 'after') {
      await page.goto(`${base}?state=loaded`); await expect(page.getByRole('tab', { name: 'Presets', exact: true })).toBeVisible()
      for (const category of ['Presets', 'Body', 'Eyes', 'Hair', 'Outfit', 'Accessory', 'Tones']) {
        await page.getByRole('tab', { name: category, exact: true }).click()
        const file = `category-${category.toLowerCase()}-${width}.png`
        await page.screenshot({ path: new URL(file, out).pathname, fullPage: true })
        results.push({ state: `category-${category.toLowerCase()}`, width, png: file, sha256: hash(await readFile(new URL(file, out))) })
      }
      const initial = await page.evaluate(() => window.__AVATAR_VISUAL__.controller.read().draft)
      await page.getByRole('tab', { name: 'Presets', exact: true }).focus(); await page.keyboard.press('ArrowDown'); await expect(page.getByRole('tab', { name: 'Body', exact: true })).toBeFocused()
      await page.getByRole('tab', { name: 'Hair', exact: true }).click(); await page.getByRole('combobox', { name: 'hair', exact: true }).selectOption('hair_second')
      await page.getByRole('tab', { name: 'Tones', exact: true }).click(); await expect(page.getByRole('combobox', { name: 'Skin tone', exact: true }).locator('option[value="tone_unsupported"]')).toBeDisabled()
      await page.getByRole('tab', { name: 'Presets', exact: true }).click(); await expect(page.getByRole('button', { name: 'fixture unsupported', exact: true })).toBeDisabled()
      assert.equal((await page.evaluate(() => window.__AVATAR_VISUAL__.counts())).writes, 0)
      await page.getByRole('button', { name: 'Cancel changes', exact: true }).click(); assert.deepEqual(await page.evaluate(() => window.__AVATAR_VISUAL__.controller.read().draft), initial)
      await page.getByRole('button', { name: 'fixture alternate', exact: true }).focus(); await page.keyboard.press('Enter'); await page.getByRole('button', { name: 'Save outfit', exact: true }).focus(); await page.keyboard.press('Enter')
      await expect(page.getByRole('status')).toContainText('Outfit saved.'); assert.equal((await page.evaluate(() => window.__AVATAR_VISUAL__.counts())).writes, 1)
      await page.evaluate(() => window.__AVATAR_VISUAL__.dispose())
      await page.goto(`${base}?state=candidate`); const writes = (await page.evaluate(() => window.__AVATAR_VISUAL__.counts())).writes
      await page.getByRole('button', { name: 'Keep my draft', exact: true }).focus(); await page.keyboard.press('Enter')
      assert.equal((await page.evaluate(() => window.__AVATAR_VISUAL__.counts())).writes, writes)
      assert.equal(await page.evaluate(() => window.__AVATAR_VISUAL__.controller.read().draft.item_ids.hair), 'hair_second')
      await expect(page.getByRole('button', { name: 'Save outfit', exact: true })).toBeEnabled(); await page.evaluate(() => window.__AVATAR_VISUAL__.dispose())
      flows.push({ width, categoryKeyboardNoWrite: true, catalogEligibility: true, cancelNoWrite: true, explicitSave: true, reconciliationNoReplay: true })
    }
    await context.close()
  }
  assert.deepEqual(errors, [])
  await writeFile(new URL('report.json', out), JSON.stringify({ status: 'PASS', phase, base: 'b3889feeed0fa7485a2326cf5bd00ffc4bb4f894', fixtureOnly: true, browser: browser.version(), sources, results, flows, errors }, null, 2) + '\n')
  console.log(JSON.stringify({ status: 'PASS', phase, cases: results.length, flows: flows.length }))
} finally { await browser?.close(); await dev.close() }
