import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const root = new URL('../../../../../', import.meta.url), out = new URL('./evidence/', import.meta.url), sources = {}, results = [], errors = []
const hash = b => createHash('sha256').update(b).digest('hex')
const paths = ['frontend/src/app/mural-entry/owner.ts', 'frontend/src/app/mural-entry/ports.ts', 'frontend/src/app/mural-entry/provider.tsx', 'frontend/src/app/mural-entry/index.ts', 'frontend/src/app/mural-entry/__tests__/fixtures.ts', 'frontend/src/app/mural-entry/__tests__/preview.tsx', 'frontend/src/app/mural-entry/__tests__/preview.html', 'frontend/src/app/mural-entry/__tests__/browser.mjs', 'frontend/src/app/mural-entry/__tests__/owner.test.ts', 'frontend/src/app/mural-entry/__tests__/routes.test.tsx', 'frontend/src/features/mural/sync/controller.ts', 'frontend/src/features/mural/sync/http.ts', 'frontend/src/features/mural/sync/contract.ts', 'frontend/src/features/mural/sync/MuralSyncPanel.tsx', 'frontend/src/features/mural/editor/controller.ts', 'frontend/src/features/mural/editor/MuralEditor.tsx', 'frontend/src/features/mural/editor/editor.css', 'frontend/src/features/mural/sync/__tests__/fixture.ts', 'frontend/src/features/mural/editor/__tests__/fixture.ts', 'frontend/src/world/mural/authority.ts', 'frontend/src/world/mural/sink.ts', 'frontend/src/world/mural/phaser.ts', 'frontend/src/app/router.tsx', 'frontend/src/shared/api/http.ts', 'frontend/src/shared/query/privacy.ts', 'frontend/src/features/auth/model/session.ts', 'frontend/src/shared/styles/base.css', 'frontend/src/app/shell.css']
for (const path of paths) sources[path] = hash(await readFile(new URL(path, root)))
await mkdir(out, { recursive: true })
const dev = await createServer({ configLoader: 'runner', cacheDir: '/tmp/mcr-fe33-entry-cache', server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser
try {
  await dev.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const base = `http://127.0.0.1:${dev.httpServer.address().port}/src/app/mural-entry/__tests__/preview.html`
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, hasTouch: true, isMobile: true, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => errors.push(e.message))
    for (const state of ['loaded', 'read-only', 'unknown', 'unavailable']) {
      await page.goto(`${base}?state=${state}`)
      await expect.poll(() => page.evaluate(() => window.__MURAL_ENTRY__?.state().gamesActive)).toBe(1)
      if (state === 'unavailable') await expect(page.getByText(/Wall unavailable. Reconnect to a qualified/)).toBeVisible()
      else {
        await expect(page.getByText('Confirmed revision 10', { exact: true })).toBeVisible()
        await expect.poll(() => page.evaluate(() => window.__MURAL_ENTRY__.state().texturePresent)).toBe(true)
        if (state === 'unknown') {
          await page.evaluate(() => window.__MURAL_ENTRY__.lossMode())
          await page.getByRole('group', { name: 'Mural pixel preview', exact: true }).focus(); await page.keyboard.press('ArrowRight')
          assert.equal((await page.evaluate(() => window.__MURAL_ENTRY__.state())).posts, 0)
          await page.getByRole('button', { name: 'Confirm pixel', exact: true }).click()
          await expect(page.getByRole('status', { name: 'Original placement status' })).toContainText('unresolved')
          await page.evaluate(() => window.__MURAL_ENTRY__.navigate('/world'))
          await page.evaluate(() => window.__MURAL_ENTRY__.navigate('/feed'))
          await page.evaluate(() => window.__MURAL_ENTRY__.navigate('/wall'))
          await expect(page.getByRole('status', { name: 'Original placement status' })).toContainText('unresolved')
          await page.evaluate(() => window.__MURAL_ENTRY__.disconnect())
          await expect(page.getByText(/Wall unavailable. Reconnect to a qualified/)).toBeVisible()
          assert.equal((await page.evaluate(() => window.__MURAL_ENTRY__.state())).texturePresent, false)
          await page.evaluate(() => window.__MURAL_ENTRY__.reconnect())
          await expect(page.getByText('Confirmed revision 11', { exact: true })).toBeVisible()
          const live = await page.evaluate(() => window.__MURAL_ENTRY__.state())
          assert.equal(live.posts, 1); assert.equal(live.commandHeld, true)
        }
        if (state === 'read-only' || state === 'unknown') await expect(page.getByRole('button', { name: 'Confirm pixel', exact: true })).toBeDisabled()
      }
      const expand = page.getByRole('button', { name: 'Expand panel', exact: true }); if (await expand.count()) await expand.click()
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
      const data = await page.evaluate(() => window.__MURAL_ENTRY__.state())
      assert.equal(data.gamesCreated, 1); assert.equal(data.gamesActive, 1); assert.equal(data.peakGames, 1)
      if (state === 'unavailable' || state === 'read-only') assert.equal(data.posts, 0)
      if (state === 'unavailable') { assert.equal(data.authorityListeners, 0); assert.equal(data.texturePresent, false) }
      const file = `${state}-${width}.png`; await page.screenshot({ path: new URL(file, out).pathname, fullPage: true })
      results.push({ state, width, ...data, png: file, sha256: hash(await readFile(new URL(file, out))) })
      if (state !== 'unavailable') {
        const action = page.getByRole('button', { name: state === 'unknown' ? 'Check original command' : 'Confirm pixel', exact: true })
        await action.scrollIntoViewIfNeeded(); await expect(action).toBeInViewport()
        if (state === 'unknown') { await page.keyboard.press('Tab'); await action.focus(); assert(await action.evaluate(el => getComputedStyle(el).outlineStyle !== 'none')) }
        const controls = `${state}-controls-${width}.png`
        await page.screenshot({ path: new URL(controls, out).pathname })
        results.push({ state: `${state}-controls`, width, png: controls, sha256: hash(await readFile(new URL(controls, out))) })
      }
      const erased = await page.evaluate(() => { const canvas = document.querySelector('.mcr-mural__surface canvas'); window.__MURAL_ENTRY__.policy(); return canvas ? canvas.width === 0 && canvas.height === 0 : true })
      assert(erased, 'retained DOM canvas must be synchronously erased')
      await expect(page.locator('.mcr-mural__surface')).toHaveCount(0)
      assert.equal((await page.evaluate(() => window.__MURAL_ENTRY__.state())).texturePresent, false)
      await page.evaluate(() => window.__MURAL_ENTRY__.dispose())
    }
    await context.close()
  }
  assert.deepEqual(errors, [])
  await writeFile(new URL('report.json', out), JSON.stringify({ status: 'PASS', base: '01a8e6bceb583a5aba080e401478c7fae4ec0d58', synthetic: true, browser: browser.version(), sources, results, errors }, null, 2) + '\n')
  console.log(JSON.stringify({ status: 'PASS', captures: results.length, errors }))
} finally { await browser?.close(); await dev.close() }
