import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const phase = process.env.UIV04_PHASE ?? 'after'
assert(['before', 'after'].includes(phase))
const root = new URL('../../../../../../', import.meta.url)
const evidence = new URL(`./evidence/${phase}/`, import.meta.url)
await mkdir(evidence, { recursive: true })
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const sources = {}
for (const path of ['frontend/src/features/me/ui/MePages.tsx', 'frontend/src/features/me/ui/me.css', 'frontend/src/features/me/ui/AvatarThumbnail.tsx', 'frontend/src/features/settings/SettingsPage.tsx', 'frontend/src/features/me/model/controller.ts', 'frontend/src/shared/ui/concert-window/index.tsx', 'frontend/src/shared/ui/concert-window/concert-window.css', 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg']) sources[path] = hash(await readFile(new URL(path, root)))
if (phase === 'after') for (const name of ['preview.tsx', 'preview.html', 'browser.mjs', 'presentation.test.tsx']) sources[`frontend/src/features/me/__tests__/visual-adoption/${name}`] = hash(await readFile(new URL(name, import.meta.url)))
const dev = await createServer({ configLoader: 'runner', cacheDir: `/tmp/mcr-uiv04-vite-${phase}`, server: { host: '127.0.0.1', port: 0 } })
let browser
const results = [], errors = []
try {
  await dev.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const base = `http://127.0.0.1:${dev.httpServer.address().port}/src/features/me/__tests__/visual-adoption/preview.html`
  const cases = phase === 'before' ? [['hub', 'loaded'], ['profile', 'draft'], ['settings', 'loaded']] : [
    ...['loaded', 'empty', 'unavailable', 'error', 'pending', 'expired', 'long'].map(s => ['hub', s]),
    ...['loaded', 'empty', 'unavailable', 'error', 'pending', 'draft', 'conflict', 'saving', 'expired'].map(s => ['profile', s]), ['settings', 'loaded']]
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' })
    const page = await context.newPage()
    page.on('pageerror', e => errors.push(e.message))
    for (const [route, state] of cases) {
      await page.goto(`${base}?route=${route}&state=${state}`)
      await expect(page.locator('main > .me-view')).toBeVisible()
      await page.waitForFunction(s => window.__UIV04__?.controller.read().phase === ({ pending: 'loading', unavailable: 'idle', error: 'error', expired: 'expired', conflict: 'conflict', saving: 'saving' }[s] ?? 'ready'), state)
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${route}/${state}/${width} overflow`)
      const controls = await page.locator('main a,main button,main input,main textarea,main select').evaluateAll(els => els.map(el => ({ label: el.textContent || el.getAttribute('aria-label') || el.id, height: el.getBoundingClientRect().height, width: el.getBoundingClientRect().width })))
      if (phase === 'after' && route !== 'settings') for (const c of controls) assert(c.height >= 44 && c.width >= 44, `${route}/${state}/${width} small control ${JSON.stringify(c)}`)
      if (route === 'hub') {
        assert.equal(await page.getByRole('link', { name: 'Dress up', exact: true }).getAttribute('href'), '/me/avatar')
        assert.equal(await page.getByRole('link', { name: 'My gifts', exact: true }).getAttribute('href'), '/gifts/mine')
        assert.equal(await page.locator('canvas,img').count(), 0)
      }
      if (route === 'profile') {
        const save = page.getByRole('button', { name: 'Save profile', exact: true })
        if (state !== 'draft') await expect(save).toBeDisabled()
        else { await expect(save).toBeEnabled(); await page.getByRole('button', { name: 'Cancel profile changes' }).click(); await expect(page.getByLabel('Bio (optional)')).toHaveValue('Here for the music.\nSee you in the world.'); await page.getByLabel('Bio (optional)').fill('Unsaved public bio') }
        if (state === 'conflict') await expect(page.getByLabel('Bio (optional)')).toHaveValue('Unsaved public bio')
        if (state === 'saving') await expect(page.getByLabel('Display name')).toBeDisabled()
      }
      const png = `${route}-${state}-${width}.png`
      await page.screenshot({ path: new URL(png, evidence).pathname, fullPage: true })
      const signature = route === 'settings' ? await page.locator('main').evaluate(el => [...el.querySelectorAll('*')].map(n => { const b = n.getBoundingClientRect(), s = getComputedStyle(n); return { tag: n.tagName, text: n.childElementCount ? '' : n.textContent, x: b.x, y: b.y, width: b.width, height: b.height, color: s.color, background: s.backgroundColor, font: s.font, border: s.border } })) : undefined
      if (signature && phase === 'after') { const before = JSON.parse(await readFile(new URL('../before/result.json', evidence))); assert.deepEqual(signature, before.results.find(r => r.route === route && r.width === width).signature, 'Settings presentation drift') }
      if (phase === 'after') {
        await page.keyboard.press('Tab')
        const focused = await page.evaluate(() => ({ tag: document.activeElement.tagName, outline: getComputedStyle(document.activeElement).outlineStyle }))
        assert.notEqual(focused.tag, 'BODY'); assert.notEqual(focused.outline, 'none')
        await page.setViewportSize({ width, height: 420 }); await page.getByRole('link', { name: 'Back to world' }).scrollIntoViewIfNeeded(); await expect(page.getByRole('link', { name: 'Back to world' })).toBeInViewport()
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
        if ((route === 'hub' && state === 'loaded') || (route === 'profile' && state === 'conflict')) await page.screenshot({ path: new URL(`${route}-${state}-${width}-height420.png`, evidence).pathname })
        await page.setViewportSize({ width, height: 844 })
      }
      results.push({ route, state, width, screenshot: png, sha256: hash(await readFile(new URL(png, evidence))), controls: controls.length, signature })
    }
    await context.close()
  }
  assert.deepEqual(errors, [])
  await writeFile(new URL('result.json', evidence), JSON.stringify({ status: 'PASS', phase, fixtureOnly: true, sources, results, errors }, null, 2) + '\n')
  console.log(JSON.stringify({ status: 'PASS', phase, cases: results.length }))
} finally { await browser?.close(); await dev.close() }
