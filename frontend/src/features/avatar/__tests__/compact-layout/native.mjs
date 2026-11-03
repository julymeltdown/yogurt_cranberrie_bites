// Real Chrome/canvas. Only explicitly synthetic geometric fixtures are used; no product art admission.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { syntheticAvatarPng } from '../../../../world/avatar/__tests__/fixtures/png.mjs'
const root = fileURLToPath(new URL('../../../../../', import.meta.url)), out = fileURLToPath(new URL('./evidence/native/', import.meta.url))
await mkdir(out, { recursive: true })
const hash = bytes => createHash('sha256').update(bytes).digest('hex'), png = syntheticAvatarPng(), pngHash = hash(png)
const participant = JSON.parse(await readFile(root+'../contracts/fixtures/core/private-participant.json'))
const conflict = JSON.parse(await readFile(root+'../contracts/fixtures/core/error-version-conflict.json'))
const writes = [], assets = [], errors = [], checks = [], hashes = {}
for (const path of ['src/features/avatar/AvatarEditor.tsx', 'src/features/avatar/OptionStrip.tsx', 'src/features/avatar/controller.ts', 'src/features/avatar/avatar-editor.css', 'src/features/avatar/preview.ts', 'src/features/avatar/contract.ts', 'src/entities/avatar/runtime/compositor.ts']) hashes[path] = hash(await readFile(root+path))
let saved = structuredClone(participant.profile.avatar), mode = 'ok', browser
const reply = (res, status, body) => { res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(JSON.stringify(body)) }
const dev = await createServer({ root, configLoader: 'runner', cacheDir: '/private/tmp/mcr-fe13-compact-native', logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'test-only-avatar-options', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (req.url?.startsWith('/__compact/control?')) {
      const selected = new URL(req.url, 'http://fixture.test').searchParams.get('mode')
      if (selected === 'rebase') { saved = { revision: 4, config: structuredClone(participant.profile.avatar.config) }; mode = 'ok' } else mode = selected
      return reply(res, 200, {})
    }
    if (req.url === '/__fe13_options/metadata') return reply(res, 200, { sha256: `sha256:${pngHash}` })
    if (req.url === '/world/fe13-options-test/avatar.png') { assets.push({ authorization: req.headers.authorization ?? null, cookie: req.headers.cookie ?? null, referer: req.headers.referer ?? null }); res.writeHead(200, { 'Content-Type': 'image/png', 'Cache-Control': 'no-store' }); res.end(png); return }
    if (!req.url?.startsWith('/api/')) return next()
    try {
      assert.equal(req.headers.authorization, 'Bearer option-fixture-token')
      if (req.method === 'GET' && req.url.endsWith('/me')) return reply(res, 200, { ...participant, profile: { ...participant.profile, avatar: saved } })
      if (req.method === 'PUT' && req.url.endsWith('/me/avatar')) {
        const chunks = []; for await (const chunk of req) chunks.push(chunk)
        const body = JSON.parse(Buffer.concat(chunks).toString()); writes.push(body)
        assert.equal(body.expected_revision, saved.revision)
        if (mode === 'denied') return reply(res, 403, { code: 'GATEWAY_403', message: 'Test access denied', details: { reason: 'POLICY_DENIED' } })
        if (mode === 'conflict') return reply(res, 409, conflict)
        saved = { revision: saved.revision + 1, config: body.config }
        if (mode === 'uncertain') return reply(res, 503, { code: 'GATEWAY', message: 'Fixture upstream unavailable after commit', details: { reason: 'DEPENDENCY_UNAVAILABLE' } })
        return reply(res, 200, saved)
      }
      reply(res, 404, {})
    } catch (error) { errors.push(String(error)); if (!res.destroyed) reply(res, 500, {}) }
  })
} }] })
const state = page => page.evaluate(() => window.__FE13_COMPACT__.state())
async function layout(page) {
  const geometry = await page.evaluate(() => {
    const rect = selector => { const r = document.querySelector(selector).getBoundingClientRect(); return { top: r.top, bottom: r.bottom, height: r.height } }
    return { body: rect('.concert-window__body'), footer: rect('.concert-window__footer'), save: rect('.mcr-avatar-actions button'), nav: rect('nav'),
      overflow: document.documentElement.scrollWidth > innerWidth || document.documentElement.scrollHeight > innerHeight }
  })
  assert(!geometry.overflow, 'host overflow'); assert(geometry.body.bottom <= geometry.footer.top + 1, 'footer covers content')
  assert(geometry.save.bottom <= geometry.nav.top, 'actions cover navigation'); assert(geometry.save.top >= 0, 'actions above viewport')
  for (const control of await page.locator('.mcr-avatar-editor button,.mcr-avatar-editor select,.mcr-avatar-editor summary').all()) {
    if (!await control.isVisible()) continue
    const box = await control.boundingBox(); if (box) assert(box.width >= 44 && box.height >= 44, `target below 44px: ${await control.textContent()} ${JSON.stringify(box)}`)
  }
  return geometry
}
async function ready(page) { await expect(page.getByRole('button', { name: 'Try look 3 · full outfit', exact: true })).toBeEnabled() }
async function disclose(page) { const summary = page.getByText('Choose by ID', { exact: true }).filter({ visible: true }); await summary.focus(); await page.keyboard.press('Enter'); return summary }
async function focusedUncovered(page) {
  const result = await page.evaluate(() => {
    const element = document.activeElement, body = document.querySelector('.concert-window__body')
    const r = element.getBoundingClientRect(), bounds = body.contains(element) ? body.getBoundingClientRect() : document.querySelector('.concert-window__footer').getBoundingClientRect()
    const hit = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)
    for (let parent = element.parentElement; parent; parent = parent.parentElement) {
      const style = getComputedStyle(parent)
      if (style.display !== 'contents' && /(auto|scroll|hidden|clip)/.test(style.overflowY)) {
        const clip = parent.getBoundingClientRect()
        if (r.top < clip.top || r.bottom > clip.bottom) return { valid: false, control: element.outerHTML, top: r.top, bottom: r.bottom, clip: parent.className, clipTop: clip.top, clipBottom: clip.bottom }
      }
    }
    return { valid: r.top >= bounds.top && r.bottom <= bounds.bottom && (hit === element || element.contains(hit)), control: element.outerHTML, top: r.top, bottom: r.bottom, boundsTop: bounds.top, boundsBottom: bounds.bottom, hit: hit?.outerHTML }
  })
  assert(result.valid, `focused control clipped or covered: ${JSON.stringify(result)}`)
}
try {
  await dev.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const base = dev.resolvedUrls.local[0]+'src/features/avatar/__tests__/compact-layout/preview.html'
  for (const width of [360, 390, 430]) for (const height of [844, 560]) {
    saved = structuredClone(participant.profile.avatar); mode = 'ok'
    const beforeWrites = writes.length, beforeAssets = assets.length, key = `${width}x${height}`
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, isMobile: true, deviceScaleFactor: 1 }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message)); await page.goto(base); await ready(page)
    assert(await page.evaluate(() => window.__FE13_COMPACT__.mainPixels()))
    assert((await page.evaluate(() => window.__FE13_COMPACT__.optionPixels())).every(p => p.equal && !p.mainCanvas))
    assert.equal((await state(page)).aliveCanvases, 4); assert.equal((await state(page)).owners, 1); assert.equal((await state(page)).reads, 1)
    await expect(page.getByRole('button', { name: 'look 1', exact: true })).toHaveCount(0)
    const geometry = await layout(page); await page.screenshot({ path: out+`loaded-${key}.png` })
    await page.getByRole('tab', { name: 'Presets', exact: true }).focus(); await page.keyboard.press('Tab')
    await expect(page.getByRole('tabpanel')).toBeFocused(); await page.keyboard.press('Tab')
    await expect(page.getByRole('button', { name: 'Try look 1 · full outfit', exact: true })).toBeFocused(); await focusedUncovered(page)
    const main = await page.locator('.mcr-avatar-preview canvas').elementHandle()
    const next = page.getByRole('button', { name: 'Next outfit choices', exact: true }); await next.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('button', { name: 'Try look 6 · full outfit', exact: true })).toBeEnabled(); await expect(next).toBeFocused()
    await focusedUncovered(page)
    await page.keyboard.press('Shift+Tab'); await expect(page.getByRole('button', { name: 'Try look 6 · full outfit', exact: true })).toBeFocused()
    assert(await main.evaluate(canvas => canvas.isConnected && canvas.width > 0)); assert.equal(writes.length, beforeWrites)
    await page.getByRole('button', { name: 'Previous outfit choices', exact: true }).tap(); await ready(page)
    await page.getByRole('button', { name: 'Try look 2 · full outfit', exact: true }).focus(); await page.keyboard.press('Enter'); await ready(page)
    await expect(page.getByRole('button', { name: 'Save outfit', exact: true })).toBeEnabled(); assert.equal(writes.length, beforeWrites)
    await page.screenshot({ path: out+`draft-${key}.png` })
    await page.getByRole('tab', { name: 'Hair', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('combobox', { name: 'hair', exact: true })).toHaveCount(0)
    const summary = await disclose(page); await expect(page.getByRole('combobox', { name: 'hair', exact: true })).toHaveValue('hair_fixture')
    await page.getByRole('combobox', { name: 'hair', exact: true }).focus(); await focusedUncovered(page); await page.keyboard.press('Tab')
    assert.equal(writes.length, beforeWrites); await summary.focus(); await page.keyboard.press('Enter')
    if (width === 390 && height === 560) {
      await page.evaluate(() => window.__FE13_COMPACT__.mode('denied'))
      await page.getByRole('button', { name: 'Save outfit', exact: true }).tap(); await expect(page.getByRole('status')).toContainText('Your outfit was not saved.')
      await layout(page); await page.screenshot({ path: out+'error-390x560.png' })
      assert.equal(writes.length, beforeWrites + 1); assert.equal((await state(page)).editor.draft.item_ids.accessory, null)
      await page.evaluate(() => window.__FE13_COMPACT__.mode('uncertain'))
      await page.getByRole('button', { name: 'Save outfit', exact: true }).tap(); await expect(page.getByRole('status')).toContainText('save result is unknown')
      await expect(page.getByRole('button', { name: 'Save outfit', exact: true })).toBeDisabled()
      await page.getByRole('button', { name: 'Check latest saved outfit' }).focus(); await focusedUncovered(page); await page.keyboard.press('Tab')
      await layout(page); await page.screenshot({ path: out+'uncertain-390x560.png' })
      assert.equal(writes.length, beforeWrites + 2)
      await page.evaluate(() => window.__FE13_COMPACT__.mode('rebase'))
      await page.getByRole('button', { name: 'Check latest saved outfit' }).tap(); await expect(page.getByRole('button', { name: 'Keep my draft' })).toBeEnabled()
      await page.getByRole('button', { name: 'Keep my draft' }).tap(); assert.equal(writes.length, beforeWrites + 2)
      await page.getByRole('button', { name: 'Save outfit', exact: true }).tap(); await expect(page.getByRole('status')).toContainText('Outfit saved.')
      assert.equal(writes.at(-1).expected_revision, 4); assert.equal(writes.length, beforeWrites + 3)
    } else {
      await page.getByRole('button', { name: 'Cancel changes', exact: true }).tap()
      assert.deepEqual((await state(page)).editor.draft, participant.profile.avatar.config); assert.equal(writes.length, beforeWrites)
    }
    await page.evaluate(() => window.__FE13_COMPACT__.hold(true)); await page.getByRole('tab', { name: 'Presets', exact: true }).tap()
    await expect.poll(async () => (await state(page)).pending).toBe(1)
    for (let n = 0; n < 12; n++) await page.getByRole('tab', { name: n % 2 ? 'Hair' : 'Eyes', exact: true }).tap()
    assert.equal((await state(page)).active, 1); assert.equal((await state(page)).pending, 1)
    await page.evaluate(() => window.__FE13_COMPACT__.release()); await expect.poll(async () => (await state(page)).active).toBe(0)
    assert((await state(page)).aliveCanvases <= 4); assert.equal(assets.length, beforeAssets + 1)
    await page.evaluate(() => window.__FE13_COMPACT__.suspend()); await expect(page.locator('.mcr-avatar-editor canvas')).toHaveCount(0)
    assert.equal((await state(page)).aliveCanvases, 0); assert.equal((await state(page)).atlasDisposed, 1)
    await page.evaluate(() => window.__FE13_COMPACT__.dispose())
    await page.goto(base+'?state=unavailable'); await expect(page.getByRole('status')).toContainText('Reviewed avatar artwork is unavailable')
    await expect(page.locator('.mcr-avatar-editor canvas')).toHaveCount(0); assert.equal((await state(page)).reads, 0); assert.equal(assets.length, beforeAssets + 1)
    await layout(page); await page.screenshot({ path: out+`unavailable-${key}.png` })
    await page.evaluate(() => window.__FE13_COMPACT__.dispose())
    if (width === 360 && height === 560) {
      await page.goto(base+'?long=1'); await ready(page)
      const choice = page.getByRole('button', { name: /Try look an exceptionally long valid option identifier for testing · full outfit/ })
      await expect(choice).toBeEnabled(); await choice.focus(); await layout(page)
      await page.screenshot({ path: out+'long-label-360x560.png' }); await disclose(page)
      await expect(page.getByRole('button', { name: 'look an exceptionally long valid option identifier for testing', exact: true })).toBeVisible()
      await page.getByRole('button', { name: 'look an exceptionally long valid option identifier for testing', exact: true }).focus(); await focusedUncovered(page)
      await layout(page); await page.screenshot({ path: out+'long-label-IDs-360x560.png' }); await page.evaluate(() => window.__FE13_COMPACT__.dispose())
    }
    await context.close(); checks.push({ width, height, geometry, singleVisibleChooser: true, keyboardPaging: true, noImplicitPut: true, distinctDraft: true, disclosedNativeIDs: true, pixelsMatchSharedOwner: true, boundedChurnAndCleanup: true, unavailableNoReads: true })
  }
  for (const width of [360, 390, 430]) {
    saved = structuredClone(participant.profile.avatar); mode = 'ok'; const beforeWrites = writes.length
    const context = await browser.newContext({ viewport: { width, height: 560 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1 }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message)); await page.goto(base+'?nested=1'); await ready(page)
    const bodyHeight = await page.locator('.concert-window__body').evaluate(element => element.clientHeight)
    assert(bodyHeight >= 44, 'nested editor has no usable 44px scroll viewport')
    const next = page.getByRole('button', { name: 'Next outfit choices', exact: true }); await next.focus(); await focusedUncovered(page); await page.keyboard.press('Enter')
    await expect(page.getByRole('button', { name: 'Try look 6 · full outfit', exact: true })).toBeEnabled()
    await page.getByRole('button', { name: 'Previous outfit choices', exact: true }).tap(); await ready(page)
    await page.getByRole('button', { name: 'Try look 2 · full outfit', exact: true }).tap(); await ready(page)
    await page.getByRole('button', { name: 'Save outfit', exact: true }).focus(); await focusedUncovered(page)
    const saveBox = await page.getByRole('button', { name: 'Save outfit', exact: true }).boundingBox(), navBox = await page.locator('nav').boundingBox()
    assert(saveBox.y + saveBox.height <= navBox.y, 'nested Save covers global navigation')
    await page.screenshot({ path: out+`nested-actions-${width}x560.png` }); assert.equal(writes.length, beforeWrites)
    await page.getByRole('tab', { name: 'Hair', exact: true }).focus(); await disclose(page)
    await page.getByRole('combobox', { name: 'hair', exact: true }).focus(); await focusedUncovered(page)
    if (width === 390) {
      await page.evaluate(() => window.__FE13_COMPACT__.mode('uncertain'))
      await page.getByRole('button', { name: 'Save outfit', exact: true }).tap(); await expect(page.getByRole('status')).toContainText('save result is unknown')
      await page.getByRole('button', { name: 'Check latest saved outfit' }).focus(); await focusedUncovered(page)
      await page.screenshot({ path: out+'nested-uncertain-390x560.png' }); assert.equal(writes.length, beforeWrites + 1)
    }
    await page.evaluate(() => window.__FE13_COMPACT__.dispose()); assert.equal((await state(page)).aliveCanvases, 0)
    await context.close(); checks.push({ width, height: 560, nestedShellHalfPanel: true, bodyHeight, keyboardPaging: true, nativeIDsFullyRevealable: true, actionsFullyRevealableAboveNavigation: true, uncertainRecoveryFullyRevealable: width === 390, cleanup: true })
  }
  assert.deepEqual(errors, []); assert(assets.every(a => a.authorization === null && a.cookie === null && a.referer === null))
  await writeFile(out+'report.json', JSON.stringify({ result: 'PASS', base: '215da400385c400f059913c826105ca4ec5f134b', browser: browser.version(), syntheticOnly: true, hashes, checks, errors, assetRequests: assets, explicitWrites: writes }, null, 2)+'\n')
} catch (error) {
  await writeFile(out+'report.json', JSON.stringify({ result: 'FAIL', hashes, checks, errors, failure: String(error) }, null, 2)+'\n'); throw error
} finally { await browser?.close(); await dev.close() }
