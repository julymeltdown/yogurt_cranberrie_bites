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
const dev = await createServer({ root, configLoader: 'runner', cacheDir: '/private/tmp/mcr-fe13-options-native', logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'test-only-avatar-options', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
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
        if (mode === 'conflict') return reply(res, 409, conflict)
        saved = { revision: saved.revision + 1, config: body.config }
        if (mode === 'uncertain') return reply(res, 503, { code: 'GATEWAY', message: 'Fixture upstream unavailable after commit', details: { reason: 'DEPENDENCY_UNAVAILABLE' } })
        return reply(res, 200, saved)
      }
      reply(res, 404, {})
    } catch (error) { errors.push(String(error)); if (!res.destroyed) reply(res, 500, {}) }
  })
} }] })
const state = page => page.evaluate(() => window.__FE13_OPTIONS__.state())
const layout = async page => {
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
  for (const control of await page.locator('.mcr-avatar-editor button,.mcr-avatar-editor select').all()) {
    const box = await control.boundingBox(); if (box) assert(box.width >= 44 && box.height >= 44, 'control below 44px')
  }
}
try {
  await dev.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const base = dev.resolvedUrls.local[0]+'src/features/avatar/__tests__/option-previews/preview.html'
  for (const width of [360, 390, 430]) {
    saved = structuredClone(participant.profile.avatar); mode = 'ok'; const beforeWrites = writes.length, beforeAssets = assets.length
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1 }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message)); await page.goto(base)
    await expect(page.getByRole('button', { name: 'Try look 3 · full outfit', exact: true })).toBeEnabled()
    assert(await page.evaluate(() => window.__FE13_OPTIONS__.mainPixels()))
    const pixels = await page.evaluate(() => window.__FE13_OPTIONS__.optionPixels()); assert.equal(pixels.length, 3); assert(pixels.every(p => p.equal && !p.mainCanvas))
    assert.notDeepEqual(pixels[0].marker, pixels[1].marker)
    const initial = await state(page); assert.equal(initial.owners, 1); assert.equal(initial.reads, 1); assert.equal(initial.aliveCanvases, 4)
    await layout(page); await page.screenshot({ path: out+`loaded-${width}.png`, fullPage: true })
    await page.getByRole('button', { name: 'Try look 2 · full outfit', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('button', { name: 'Try look 2 · full outfit', exact: true })).toHaveAttribute('aria-pressed', 'true')
    await expect(page.getByRole('button', { name: 'Try look 3 · full outfit', exact: true })).toBeEnabled()
    assert.equal(writes.length, beforeWrites)
    await page.getByRole('button', { name: 'Next outfit choices', exact: true }).tap()
    await expect(page.getByRole('button', { name: 'Try look 4 · full outfit', exact: true })).toBeEnabled()
    const draft = (await state(page)).editor.draft
    await page.getByRole('tab', { name: 'Tones', exact: true }).tap()
    await expect(page.getByRole('button', { name: 'Try tone unsupported · full outfit', exact: true })).toBeDisabled()
    await page.getByRole('tab', { name: 'Hair', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('combobox', { name: 'hair', exact: true })).toHaveValue('hair_fixture')
    assert.deepEqual((await state(page)).editor.draft, draft); assert.equal(writes.length, beforeWrites)
    await page.screenshot({ path: out+`hair-${width}.png`, fullPage: true })
    await page.getByRole('button', { name: 'Save outfit', exact: true }).tap(); await expect(page.getByRole('status')).toContainText('Outfit saved.')
    assert.equal(writes.length, beforeWrites + 1)
    await page.evaluate(() => window.__FE13_OPTIONS__.hold(true))
    await page.getByRole('tab', { name: 'Presets', exact: true }).tap()
    await expect.poll(async () => (await state(page)).pending).toBe(1)
    for (let n = 0; n < 12; n++) await page.getByRole('tab', { name: n % 2 ? 'Hair' : 'Eyes', exact: true }).tap()
    assert.equal((await state(page)).active, 1); assert.equal((await state(page)).pending, 1)
    await page.evaluate(() => window.__FE13_OPTIONS__.release())
    await expect.poll(async () => (await state(page)).active).toBe(0)
    await expect.poll(async () => (await state(page)).aliveCanvases).toBeLessThanOrEqual(4)
    assert((await page.evaluate(() => window.__FE13_OPTIONS__.optionPixels())).every(p => p.equal))
    assert.equal((await state(page)).owners, 1); assert.equal(assets.length, beforeAssets + 1)
    await page.evaluate(() => window.__FE13_OPTIONS__.suspend())
    await expect(page.locator('.mcr-avatar-editor canvas')).toHaveCount(0)
    assert.equal((await state(page)).aliveCanvases, 0); assert.equal((await state(page)).atlasDisposed, 1)
    await page.evaluate(() => window.__FE13_OPTIONS__.dispose())
    const beforeUnavailable = assets.length
    await page.goto(base+'?state=unavailable'); await expect(page.getByRole('status')).toContainText('Reviewed avatar artwork is unavailable')
    await expect(page.locator('.mcr-avatar-editor canvas')).toHaveCount(0); assert.equal((await state(page)).reads, 0); assert.equal(assets.length, beforeUnavailable)
    await layout(page); await page.screenshot({ path: out+`unavailable-${width}.png`, fullPage: true })
    await page.evaluate(() => window.__FE13_OPTIONS__.dispose()); await context.close()
    checks.push({ width, pixelsMatchSharedCompositor: true, noMainLeaseTheft: true, oneOwnerOneAtlasRead: true, keyboardTouchNoImplicitPut: true, explicitSave: true, boundedIgnoredAbort: true, scopeCleanup: true, unavailableNoReads: true })
  }
  assert.deepEqual(errors, []); assert(assets.every(a => a.authorization === null && a.cookie === null && a.referer === null))
  await writeFile(out+'report.json', JSON.stringify({ result: 'PASS', base: 'e869480395b70b4aae6cfcf50e904b079a59a0e7', browser: browser.version(), syntheticOnly: true, pngSha256: pngHash, hashes, checks, errors, assetRequests: assets }, null, 2)+'\n')
} catch (error) {
  await writeFile(out+'report.json', JSON.stringify({ result: 'FAIL', hashes, checks, errors, failure: String(error) }, null, 2)+'\n')
  throw error
} finally { await browser?.close(); await dev.close() }
