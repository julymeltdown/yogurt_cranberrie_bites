import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const frontend = resolve(import.meta.dirname, '../../../../../..')
const output = process.env.MCR_MURAL_OUTPUT, scratch = process.env.MCR_MURAL_SCRATCH
const port = Number(process.env.MCR_MURAL_PORT)
if (!output || !scratch || !Number.isInteger(port) || port < 1024) throw Error('A fresh accepted allocation must supply output, scratch and port')
await mkdir(output, { recursive: true })
const prefix = '/src/app/world-entry/mural-production/__tests__/default-entry/'
const placement = JSON.parse(await readFile(resolve(frontend, '.' + prefix + '../../placement.json'), 'utf8'))
const input = { revision: 'TEST_ONLY_MURAL_DEFAULT_APP', reviewedBy: 'Synthetic map admission',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
const server = await createServer({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner',
  cacheDir: resolve(scratch, 'vite'), logLevel: 'warn',
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
    'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)) },
  plugins: [{ name: 'mural-actual-main-fixture', configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (!request.headers.accept?.includes('text/html')) return next()
      response.setHeader('Content-Type', 'text/html')
      response.end(await server.transformIndexHtml(request.url, await readFile(resolve(frontend, '.' + prefix + 'preview.html'), 'utf8')))
    })
  } }], server: { host: '127.0.0.1', port, strictPort: true, watch: null } })
const report = { result: 'FAIL', qualification: 'ACTUAL_MAIN_CANONICAL_AUTH_PRODUCTION_WALL_INCUMBENT_PHASER_SYNTHETIC_HTTP_C05_NOT_LIVE_IN04',
  builtInPlacement: placement, fixturePlacement: null, cases: [], errors: [] }
const targetedBackoff = process.env.MCR_MURAL_CASE === 'BACKOFF'
let browser, stage = 'prepare fixture map admission'
try {
  // Test-only map provenance changes its content hash. Admit that exact test map
  // through the existing explicit environment config; never weaken production pins.
  const { wholeData } = await server.ssrLoadModule('/e2e/reference-integrated/whole/data.ts')
  const data = await wholeData(path => readFile(resolve(frontend, 'public', path), 'utf8').then(JSON.parse))
  const map = data.bootstrap.map_manifest
  const fixturePlacement = { ...placement, map: { ...placement.map, id: map.map_id, revision: map.map_revision,
    contentHash: map.content_hash, geometryHash: map.geometry_hash,
    widthPx: map.geometry.width_cells * map.geometry.tile_width_px, heightPx: map.geometry.height_cells * map.geometry.tile_height_px } }
  report.fixturePlacement = fixturePlacement
  server.config.define['import.meta.env.VITE_MCR_MURAL_CONFIG'] = JSON.stringify(JSON.stringify(fixturePlacement))
  await server.listen()
  const base = `http://127.0.0.1:${port}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const [width, height] of (targetedBackoff ? [[390, 844]] : [[360, 800], [390, 844], [430, 932], [390, 480]])) {
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    const page = await context.newPage()
    page.on('pageerror', error => report.errors.push(error.message))
    const state = () => page.evaluate(() => window.__MURAL_ROOT_ENTRY__.state())
    const proof = () => page.evaluate(() => window.__MURAL_ROOT_ENTRY__.worldProof())
    const shot = async name => { const path = `${width}x${height}-${name}.png`; await page.screenshot({ path: resolve(output, path), fullPage: true }); return path }
    const result = { width, height, result: 'FAIL', screenshots: [] }
    report.cases.push(result)
    stage = `${width}x${height}/canonical UI login`
    await page.goto(`${base}/login`)
    await page.getByLabel('Email', { exact: true }).fill('wall-fixture@example.test')
    await page.getByLabel('Password', { exact: true }).fill('fixture-password')
    await page.getByRole('button', { name: 'Sign in with email' }).click()
    await expect.poll(async () => (await state()).active).toBe(1)
    try { await expect.poll(async () => (await proof()).mural?.objects ?? 0).toBe(1) } catch (error) {
      result.debug = { state: await state(), proof: await proof(), text: await page.locator('body').innerText() }; result.screenshots.push(await shot('admission-failure')); throw error
    }
    const host = page.locator('[data-world-host]')
    const hostIdentity = await host.evaluate(node => { Reflect.set(node, '__muralIdentity', 'incumbent'); return 'incumbent' })
    const initial = await proof()
    assert.deepEqual({ x: initial.mural.x, y: initial.mural.y, width: initial.mural.width, height: initial.mural.height, depth: initial.mural.depth }, placement.anchor)
    assert.equal(initial.mural.island[3], 255)
    stage = `${width}x${height}/full Wall`
    await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Wall', exact: true }).click()
    const editor = page.getByRole('region', { name: 'Wall editor' })
    await expect(editor.getByRole('button', { name: 'Fill preview', exact: true })).toBeEnabled()
    await expect(page.locator('.shell-stage')).toHaveAttribute('data-panel-size', 'full')
    assert.equal(await host.evaluate(node => Reflect.get(node, '__muralIdentity')), hostIdentity)
    assert.equal((await proof()).mural.objects, 1)
    if (targetedBackoff) {
      stage = '390x844/real World delta ticks after denied tools read'
      const icons = page.locator('img.mcr-reference-window-ink')
      await expect(icons).toHaveCount(3)
      await expect.poll(() => icons.evaluateAll(nodes => nodes.every(node => node.complete && node.naturalWidth > 0))).toBe(true)
      await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.denyTools(true))
      await expect.poll(async () => (await proof()).mural, { timeout: 8000 }).toBeNull()
      const startReads = (await state()).requests.filter(r => r.path.endsWith('/mural/tools')).length
      let ticks = 0
      for (let i = 0; i < 5; i++) { ticks += await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.ticks(10)); await page.waitForTimeout(20) }
      await page.waitForTimeout(300)
      const burstReads = (await state()).requests.filter(r => r.path.endsWith('/mural/tools')).length - startReads
      assert(burstReads <= 1, 'World delta updates cannot bypass denied-read retry delay')
      await page.waitForTimeout(2100)
      for (let i = 0; i < 5; i++) { ticks += await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.ticks(10)); await page.waitForTimeout(20) }
      const delayedReads = (await state()).requests.filter(r => r.path.endsWith('/mural/tools')).length - startReads
      assert(ticks === 100 && delayedReads >= 1 && delayedReads <= 2)
      await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.denyTools(false))
      await expect.poll(async () => (await proof()).mural?.objects ?? 0, { timeout: 8000 }).toBe(1)
      result.screenshots.push(await shot('backoff-recovered'))
      await page.getByRole('button', { name: 'Back to previous panel', exact: true }).click()
      await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Me', exact: true }).click()
      await page.getByText('Profile options', { exact: true }).click()
      await page.getByRole('link', { name: 'Account', exact: true }).click()
      await page.getByRole('button', { name: 'Sign out', exact: true }).click()
      await page.getByRole('button', { name: 'Confirm sign out', exact: true }).click()
      await expect.poll(async () => (await state()).active).toBe(0)
      await expect.poll(async () => (await state()).blobURLs).toBe(0)
      assert.equal(await page.locator('.shell-world-host canvas').count(), 0)
      Object.assign(result, { result: 'PASS', deniedWorldTicks: ticks, burstReads, delayedReads, iconsLoaded: 3, canonicalSignoutResources: 0 })
      await context.close(); continue
    }
    const beforePoll = (await state()).requests.filter(r => r.path.endsWith('/mural/tools')).length
    await page.waitForTimeout(2200)
    const afterPoll = (await state()).requests.filter(r => r.path.endsWith('/mural/tools')).length
    assert(afterPoll > beforePoll && afterPoll - beforePoll <= 8, 'bounded independent polls, no tools/admission recursion')
    const coordinates = async () => {
      const details = editor.locator('details')
      if (!await details.getAttribute('open')) await editor.getByText('Coordinates & help', { exact: true }).click()
      await editor.getByLabel('X', { exact: true }).fill('10'); await editor.getByLabel('Y', { exact: true }).fill('10')
      await editor.getByRole('button', { name: 'Preview pixel', exact: true }).click()
    }
    const chooseColor = () => editor.getByRole('button', { name: /^Color 2 / }).click()
    stage = `${width}x${height}/fill reset and actual World texture`
    await editor.getByRole('button', { name: 'Fill preview', exact: true }).click(); await chooseColor(); await coordinates()
    await expect(editor.getByRole('button', { name: 'Confirm fill', exact: true })).toBeEnabled()
    const revision = (await state()).revision
    await editor.getByRole('button', { name: 'Confirm fill', exact: true }).click()
    await expect.poll(async () => (await state()).revision).toBe(revision + 1)
    await expect(editor.getByText(`REV ${revision + 1} · EDITABLE`, { exact: true })).toBeVisible()
    await expect.poll(async () => (await proof()).mural?.island).toEqual([162, 38, 53, 255])
    result.screenshots.push(await shot('filled'))
    stage = `${width}x${height}/undo and opaque erase`
    await expect(editor.getByRole('button', { name: 'Preview undo of my last action' })).toBeEnabled()
    await editor.getByRole('button', { name: 'Preview undo of my last action' }).click()
    await editor.getByRole('button', { name: 'Confirm undo', exact: true }).click()
    await expect.poll(async () => (await proof()).mural?.island).toEqual(initial.mural.island)
    await editor.getByRole('button', { name: 'Erase preview', exact: true }).click(); await coordinates()
    await editor.getByRole('button', { name: 'Confirm erase', exact: true }).click()
    await expect.poll(async () => (await proof()).mural?.island).toEqual(initial.mural.background)
    stage = `${width}x${height}/lost response and exact original-key recovery`
    await expect(editor.getByRole('button', { name: 'Pencil preview', exact: true })).toBeEnabled()
    await editor.getByRole('button', { name: 'Pencil preview', exact: true }).click(); await chooseColor(); await coordinates()
    await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.loseNext())
    await editor.getByRole('button', { name: 'Confirm pixel', exact: true }).click()
    await expect(editor.getByRole('button', { name: 'Check original command', exact: true })).toBeEnabled()
    const posts = (await state()).requests.filter(r => r.method === 'POST' && r.path.includes('/mural/'))
    const original = posts.at(-1)
    await editor.getByRole('button', { name: 'Check original command', exact: true }).click()
    await expect.poll(async () => (await proof()).mural?.island).toEqual([162, 38, 53, 255])
    assert.equal((await state()).requests.filter(r => r.method === 'POST' && r.path.includes('/mural/')).length, posts.length)
    assert((await state()).requests.some(r => r.method === 'GET' && (r.path.includes(original.key) || r.path.endsWith('/commands'))))
    stage = `${width}x${height}/visibility withdrawal and requalification`
    await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.hidden(true))
    await expect.poll(async () => (await proof()).mural).toBeNull()
    await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.hidden(false))
    await expect.poll(async () => (await proof()).mural?.objects ?? 0).toBe(1)
    assert.equal((await state()).peak, 1)
    stage = `${width}x${height}/denied tools and honest unavailable view`
    await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.denyTools(true))
    await expect.poll(async () => (await proof()).mural, { timeout: 8000 }).toBeNull()
    await page.evaluate(() => window.__MURAL_ROOT_ENTRY__.denyTools(false))
    await expect.poll(async () => (await proof()).mural?.objects ?? 0, { timeout: 8000 }).toBe(1)
    assert.equal(await host.evaluate(node => Reflect.get(node, '__muralIdentity')), hostIdentity)
    assert.equal((await state()).active, 1)
    assert.equal((await state()).peak, 1) // Hidden-document suspension legitimately reconnects the one transport.
    result.screenshots.push(await shot('restored'))
    stage = `${width}x${height}/canonical UI signout`
    await page.getByRole('button', { name: 'Back to previous panel', exact: true }).click()
    await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Me', exact: true }).click()
    await page.getByText('Profile options', { exact: true }).click()
    await page.getByRole('link', { name: 'Account', exact: true }).click()
    await page.getByRole('button', { name: 'Sign out', exact: true }).click()
    await page.getByRole('button', { name: 'Confirm sign out', exact: true }).click()
    await expect.poll(async () => (await state()).active).toBe(0)
    await expect.poll(async () => (await state()).blobURLs).toBe(0)
    assert.equal(await page.locator('.shell-world-host canvas').count(), 0)
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
    Object.assign(result, { result: 'PASS', actualMain: true, actualProductionFactory: true, uiLogin: true, fullWallKeepsIncumbent: true,
      boundedToolsPolling: afterPoll - beforePoll, fillResetTexture: true, undo: true, opaqueErase: true, unknownOriginalKey: original.key,
      noDuplicatePost: true, visibilityWithdrawal: true, deniedReadWithdrawal: true, peakSocket: 1, uiSignoutResources: 0 })
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.errors.push(`${stage}: ${error.stack ?? error}`); throw error }
finally { await browser?.close(); await server.close(); await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report))
