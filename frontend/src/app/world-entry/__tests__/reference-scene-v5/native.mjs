// Actual root/Places/Phaser with explicit source resolver and synthetic C03/C05. No admission bypass.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, rm, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'

const root = resolve(import.meta.dirname, '../../../../../..'), frontend = resolve(root, 'frontend')
const output = process.env.MCR_SCENE_V5_ROOT_OUTPUT
assert(output?.startsWith('/'), 'Set MCR_SCENE_V5_ROOT_OUTPUT to a new absolute output directory')
const disk = await statfs(root); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3, '8GiB disk floor')
await mkdir(output, { mode: 0o700 })
const labelSource = JSON.parse(await readFile(resolve(frontend, 'src/world/reference-scene/spatial-v5/map-labels-source.json'), 'utf8'))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const report = { status: 'IN_PROGRESS', qualification: 'ACTUAL_ROOT_AND_DEFAULT_PHASER_WITH_EXPLICIT_SOURCE_FIXTURE_RESOLVER',
  candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourcePins: {}, checks: [],
  limits: ['Synthetic map7005 is not an allocated backend release', 'No invented REVIEWED provenance; optional skins remain guarded',
    'Desktop Chrome viewports and reduced height, not iOS Safari/Android device or actual software keyboard/notch acceptance',
    'No live Auth/server/multiuser/physical/fidelity or release admission'] }
for (const path of ['frontend/src/app/world-entry/reference-scene-overview.ts', 'frontend/src/app/WorldShell.tsx',
  'frontend/src/app/world-entry/production.ts', 'frontend/src/app/world-entry/resolver.ts', 'frontend/src/app/world-panels/index.tsx',
  'frontend/src/world/scenes/MapScene.ts', 'frontend/src/world/composition/index.ts', 'frontend/src/world/composition/release.ts',
  'frontend/src/world/theme/compatibility.ts', 'frontend/src/features/places/ui/IllustratedMap.tsx', 'frontend/src/features/places/ui/reference-scene/spatial-v5-label-obstacles.ts',
  ...['presentation.json', 'handoff.json', 'map-labels-bold.ttf', 'OFL.txt'].map(p => 'frontend/public/world/reference-scene-v2/map-labels-v1/' + p),
  ...['model.ts', 'metadata.ts', 'index.ts', 'loader.ts', 'phaser.ts', 'overview.ts'].map(p => 'frontend/src/world/reference-scene/' + p),
  ...['model.ts', 'source-inputs.json', 'tile-pin.json', 'build-source-inputs.py', 'map-labels.ts', 'map-labels-source.json', 'map-labels-pins.json','map-landscape-source.json','map-landscape-pin.json','build-map-landscape-inputs.py','backend-allocation.json','camera.ts'].map(p => 'frontend/src/world/reference-scene/spatial-v5/' + p),
  ...['data.ts', 'preview.tsx', 'preview.html', 'native.mjs', 'contracts.test.ts'].map(p => 'frontend/src/app/world-entry/__tests__/reference-scene-v5/' + p),
  ...['canonical-map.json', 'source-binding.json'].map(p => 'frontend/public/world/reference-scene-v2/runtime/spatial-v5/' + p)]) {
  report.sourcePins[path] = hash(await readFile(resolve(root, path)))
}
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(output, 'vite-cache'), logLevel: 'error',
  server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false } })
let browser
const state = page => page.evaluate(() => window.__SCENE_V5_ROOT__?.state())
const world = page => page.evaluate(() => window.__SCENE_V5_ROOT__.inspectWorld())
const navigate = (page, path) => page.evaluate(path => window.__SCENE_V5_ROOT__.navigate(path), path)
const assets = requests => requests.filter(p => p.includes('/runtime/spatial-v5/') || p.includes('/quality-v10/') || p.includes('/quality-v8/'))
const projection = page => page.evaluate(() => window.__SCENE_V5_ROOT__.cameraProjection())
function assertProjection(value) {
  for (const { client, world, roundtrip } of value.points) {
    assert(client, 'Authored corner must project')
    if (roundtrip) assert(Math.abs(roundtrip.x - world.x) < .001 && Math.abs(roundtrip.y - world.y) < .001)
    else assert(Math.min(Math.abs(client.x - value.surface.left), Math.abs(client.x - value.surface.right), Math.abs(client.y - value.surface.top), Math.abs(client.y - value.surface.bottom)) < .01, 'Only exact viewport edges are excluded by the unchanged public hit policy')
    assert(client.x >= value.surface.left - .01 && client.x <= value.surface.right + .01)
    assert(client.y >= value.surface.top - .01 && client.y <= value.surface.bottom + .01)
  }
}
const mapBounds = page => page.evaluate(() => {
  const rect = node => { const r = node.getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height } }
  return { image: rect(document.querySelector('.places-map-plane > img')), viewport: rect(document.querySelector('.places-map-viewport')) }
})
function assertFit({ image, viewport }) {
  assert(image.width > 0 && image.height > 0)
  assert(image.left >= viewport.left - 1 && image.right <= viewport.right + 1)
  assert(image.top >= viewport.top - 1 && image.bottom <= viewport.bottom + 1)
}
async function open(width, era, suffix = '') {
  const space = await statfs(root); assert(space.bavail * space.bsize >= 8 * 1024 ** 3, 'Disk floor before next browser context')
  const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce', serviceWorkers: 'block' })
  const page = await context.newPage(), errors = [], requests = []
  page.on('pageerror', error => errors.push(error.message)); page.on('request', r => requests.push(new URL(r.url()).pathname))
  if (suffix.includes('font=corrupt')) await page.route('**/map-labels-v1/map-labels-bold.ttf', route => route.fulfill({ status: 200, body: Buffer.alloc(145012), contentType: 'font/ttf' }))
  await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
  await page.goto(server.resolvedUrls.local[0] + `src/app/world-entry/__tests__/reference-scene-v5/preview.html?era=${era}${suffix}`)
  return { context, page, errors, requests }
}
async function close({ context, page }) {
  await page.evaluate(() => window.__SCENE_V5_ROOT__?.dispose())
  await expect.poll(async () => { const s = await state(page); return s.urls.created.every(url => s.urls.revoked.includes(url)) }).toBe(true)
  const s = await state(page), w = await world(page)
  assert.equal(w.games, 0); assert.equal(s.activeSockets, 0); assert.equal(s.canvases, 0)
  assert.deepEqual(new Set(s.urls.created), new Set(s.urls.revoked))
  assert.equal(await page.evaluate(() => [...document.fonts].filter(f => f.family === 'MCR Map Labels').length), 0)
  assert(s.sourceCaches.every(cache => cache.disposed && cache.entries === 0 && cache.residentBytes === 0))
  await context.close(); return { state: s, world: w }
}
try {
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) for (const era of ['REVENGE', 'PARADE', 'DANGER']) {
    const fixture = await open(width, era), { page, errors, requests } = fixture
    try {
      await expect.poll(async () => (await state(page))?.overview?.themeId, { timeout: 20000 }).toBe(era)
      await expect.poll(async () => (await world(page)).sprites).toBe(6)
      const initial = { state: await state(page), world: await world(page), projection: await projection(page) }; assertProjection(initial.projection)
      assert.equal(initial.state.map.revision, 7005); assert.equal(initial.state.factoryCalls, 1)
      assert.equal(initial.world.games, 1); assert.equal(initial.world.referenceObjects, 35)
      assert(initial.state.originalPlaces.every(p => p.place_kind === 'VIRTUAL_CONCEPT' && p.verification_state === 'CONCEPT_ONLY' && p.provenance.review_state === 'UNREVIEWED'))
      assert(initial.state.suppliedThemeProvenance.every(p => p.review_state === 'UNREVIEWED'))
      const pixels = await page.evaluate(() => window.__SCENE_V5_ROOT__.overviewPixels())
      assert.equal(pixels.width, 768); assert.equal(pixels.height, 1296)
      await page.screenshot({ path: resolve(output, `world-${era.toLowerCase()}-${width}.png`) })
      if (width === 390) {
        await page.setViewportSize({ width, height: 554 })
        await expect.poll(async () => (await projection(page)).surface.bottom - (await projection(page)).surface.top).not.toBe(initial.projection.surface.bottom - initial.projection.surface.top)
        const reduced = await projection(page); assertProjection(reduced); assert.deepEqual(reduced.self, initial.projection.self)
        initial.equalAspectProjection = reduced
        await page.screenshot({ path: resolve(output, `world-${era.toLowerCase()}-${width}-h554.png`) })
        await page.setViewportSize({ width, height: 844 })
        await expect.poll(async () => (await projection(page)).surface.bottom - (await projection(page)).surface.top).toBe(initial.projection.surface.bottom - initial.projection.surface.top)
      }
      await navigate(page, '/world/map')
      const image = page.getByAltText(`${era} illustrated virtual map`, { exact: true })
      await expect(image).toBeVisible(); await expect(page.locator('.places-map-plaque')).toHaveCount(4)
      await expect(page.locator('[data-map-painted-label]')).toHaveCount(2)
      assert.deepEqual(await page.locator('[data-map-painted-label]').evaluateAll(nodes => nodes.map(n => n.getAttribute('data-map-painted-label')).sort()), ['gifts_info', 'stage_info'])
      const defaultFit = await mapBounds(page); assertFit(defaultFit)
      const mapped = await state(page); assert.equal(mapped.metadata.mapRevision, 7005)
      assert.equal(mapped.metadata.geometryHash, mapped.overview.geometryHash)
      assert.equal(await image.getAttribute('src'), initial.state.overview.imageUrl)
      const camera = mapped.camera
      const compactMarkers = await page.locator('[data-map-compact-marker]').evaluateAll(nodes => nodes.map(node => ({
        name: node.getAttribute('title'), aria: node.getAttribute('aria-label'), width: node.getBoundingClientRect().width, height: node.getBoundingClientRect().height,
        icons: node.querySelectorAll('svg').length, textWidth: node.querySelector('span').getBoundingClientRect().width,
      })))
      assert.equal(compactMarkers.length, 2)
      for (const marker of compactMarkers) { assert.equal(marker.width, 44); assert.equal(marker.height, 44); assert.equal(marker.icons, 1); assert.equal(marker.textWidth, 1); assert(marker.aria.includes(marker.name)) }
      for (const marker of compactMarkers) {
        const link = page.getByRole('link', { name: `View ${marker.name}`, exact: true })
        await link.focus(); await expect(link).toBeFocused()
        await expect(page.locator('.places-map-caption')).toContainText(marker.name)
        await link.press('Enter')
        await expect(page.getByText('This landmark exists in the virtual world only. Do not use it as a physical meeting destination.', { exact: true })).toBeVisible()
        await page.getByRole('link', { name: 'All places', exact: true }).click(); await expect(image).toBeVisible()
      }
      await page.getByRole('button', { name: 'Zoom in map view', exact: true }).click()
      await expect.poll(async () => (await mapBounds(page)).image.width).toBeGreaterThan(defaultFit.image.width)
      await page.getByRole('button', { name: 'Reset view', exact: true }).click()
      await expect.poll(async () => (await mapBounds(page)).image.width).toBe(defaultFit.image.width)
      assertFit(await mapBounds(page))
      await page.locator('.places-map-viewport').focus(); await page.keyboard.press('ArrowDown')
      assert.deepEqual((await state(page)).camera, camera)
      await page.locator('.places-map-plaque').filter({ hasText: /stage/i }).click()
      await expect(page.getByText('This landmark exists in the virtual world only. Do not use it as a physical meeting destination.', { exact: true })).toBeVisible()
      await page.getByRole('link', { name: 'All places', exact: true }).click()
      await expect(image).toBeVisible()
      await page.getByRole('button', { name: 'Fit map', exact: true }).click()
      const labelLayout = await page.evaluate(labels => {
        const image = document.querySelector('.places-map-plane > img').getBoundingClientRect()
        const rectangles = labels.map(({ rect: r }) => ({ left: image.left + r.x / 768 * image.width, top: image.top + r.y / 1296 * image.height, width: r.width / 768 * image.width, height: r.height / 1296 * image.height }))
        const overlap = (a, b) => a.left < b.left + b.width && a.left + a.width > b.left && a.top < b.top + b.height && a.top + a.height > b.top
        return { duplicatePlaques: document.querySelectorAll('[data-map-painted-label] svg').length,
          overlaps: [...document.querySelectorAll('.places-map-plaque:not([data-map-painted-label]), .places-map-zoom, .places-map-caption')].flatMap(node => rectangles.filter(r => overlap(node.getBoundingClientRect(), r)).map(() => node.getAttribute('aria-label'))) }
      }, labelSource.labels)
      assert.equal(labelLayout.duplicatePlaques, 0); assert.deepEqual(labelLayout.overlaps, [])
      await page.locator('[data-map-painted-label=stage_info]').focus(); await expect(page.locator('[data-map-painted-label=stage_info]')).toBeFocused()
      await page.locator('.places-map-viewport').focus()
      await page.screenshot({ path: resolve(output, `map-${era.toLowerCase()}-${width}.png`) })
      if (width === 390) {
        await page.setViewportSize({ width, height: 754 })
        await expect.poll(async () => (await mapBounds(page)).viewport.height).not.toBe(defaultFit.viewport.height)
        assertFit(await mapBounds(page))
        const inkCoverage = await page.evaluate(labels => {
          const img = document.querySelector('.places-map-plane > img').getBoundingClientRect()
          const controls = [...document.querySelectorAll('.places-map-caption, .places-map-zoom')].map(n => n.getBoundingClientRect())
          return labels.flatMap(({ label, rect: r }) => {
            const ink = { left: img.left + r.x / 768 * img.width, top: img.top + r.y / 1296 * img.height, right: img.left + (r.x + r.width) / 768 * img.width, bottom: img.top + (r.y + r.height) / 1296 * img.height }
            return controls.some(c => ink.left < c.right && ink.right > c.left && ink.top < c.bottom && ink.bottom > c.top) ? [label] : []
          })
        }, labelSource.labels)
        assert.deepEqual(inkCoverage, [], 'All six source labels remain clear of caption/zoom at reference aspect')
        await page.screenshot({ path: resolve(output, `map-${era.toLowerCase()}-${width}-h754.png`) })
      }
      await page.setViewportSize({ width, height: 420 })
      await expect(image).toBeVisible()
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
      const viewport = await page.locator('.places-map-viewport').boundingBox(); assert(viewport.width > 0 && viewport.height > 0)
      await page.getByRole('button', { name: 'Places', exact: true }).click()
      const search = page.getByRole('searchbox', { name: 'Find a place', exact: true })
      await search.fill('stage'); await expect(search).toBeFocused()
      await page.getByRole('button', { name: 'Close map information', exact: true }).click()
      const keyboard = await page.evaluate(() => ({ activeRole: document.activeElement?.getAttribute('role'),
        viewportWidth: innerWidth, viewportHeight: innerHeight, visualViewportHeight: visualViewport?.height,
        safeAreaEvidence: 'Desktop environment; no hardware notch or software-keyboard claim' }))
      await page.setViewportSize({ width, height: 844 })
      await navigate(page, '/world'); await expect.poll(async () => (await state(page)).phase).toBe('live')
      const foreignEra = era === 'REVENGE' ? 'DANGER' : 'REVENGE'
      await page.getByRole('button', { name: new RegExp(`^${foreignEra}$`, 'i') }).click()
      await expect.poll(async () => (await state(page)).theme.phase).toBe('error')
      assert.equal((await state(page)).theme.selected, era)
      assert.equal((await state(page)).overview.imageUrl, initial.state.overview.imageUrl)
      await page.getByRole('button', { name: new RegExp(`^${era}$`, 'i') }).click()
      await navigate(page, '/world/map'); await expect(image).toBeVisible()
      const warm = { state: await state(page), world: await world(page) }
      assert.equal(warm.state.factoryCalls, 1); assert.equal(warm.world.totalGames, 1); assert.equal(warm.state.socketPeak, 1)
      assert.equal(warm.state.sameCamera, true); assert.equal(assets(requests).length, 3); assert.equal(requests.filter(p => p.endsWith('/map-labels-bold.ttf')).length, 1)
      assert.equal(await page.evaluate(() => [...document.fonts].filter(f => f.family === 'MCR Map Labels').length), 1)
      assert.deepEqual(errors, [])
      const cleanup = await close(fixture)
      report.checks.push({ width, era, status: 'PASS', initial, pixels, mapped, defaultFit, compactMarkers, labelLayout, keyboard, optionalEraGuard: 'UNREVIEWED denial preserved', warm, cleanup })
    } catch (error) { await page.screenshot({ path: resolve(output, 'failure.png') }).catch(() => {}); await close(fixture).catch(() => {}); throw error }
  }
  for (const status of ['closed', 'unavailable']) {
    const fixture = await open(390, 'PARADE', '&markerStatus=' + status), { page } = fixture
    await expect.poll(async () => (await state(page))?.phase, { timeout: 20000 }).toBe('live')
    await navigate(page, '/world/map')
    const marker = page.locator('[data-map-compact-marker*="park"]')
    await expect(marker).toBeVisible()
    if (status === 'unavailable') {
      await marker.click(); await expect(page.getByText('This place is unavailable', { exact: true })).toBeVisible()
      await page.getByRole('link', { name: 'All places', exact: true }).click(); await expect(marker).toBeVisible()
    }
    await expect(marker).toHaveAttribute('data-status', status)
    assert((await marker.getAttribute('aria-label')).includes(status))
    await marker.focus(); await expect(marker).toBeFocused()
    const bounds = await marker.evaluate(node => {
      const a = node.getBoundingClientRect(), b = node.querySelector('small').getBoundingClientRect()
      return { width: a.width, height: a.height, badgeInside: b.left >= a.left && b.right <= a.right && b.top >= a.top && b.bottom <= a.bottom }
    })
    assert.deepEqual(bounds, { width: 44, height: 44, badgeInside: true })
    assert.equal(await page.locator('.places-map-plaque').count(), 4)
    assert.deepEqual(fixture.errors, [])
    await page.screenshot({ path: resolve(output, `marker-${status}-390.png`) })
    report.checks.push({ name: `Bounded ${status} marker retains full semantics and four targets`, status: 'PASS', bounds, cleanup: await close(fixture) })
  }
  // Actual Places mismatch/reload controls; only the fixture catalog response is deliberately inconsistent.
  {
    const fixture = await open(390, 'PARADE', '&catalog=mismatch'), { page } = fixture
    await expect.poll(async () => (await state(page))?.phase, { timeout: 20000 }).toBe('live')
    await navigate(page, '/world/map')
    await expect(page.getByText('Your map and places do not match', { exact: true })).toBeVisible()
    assert.equal(await page.locator('[data-map-presentation="reference-scene"]').count(), 0)
    await page.evaluate(() => window.__SCENE_V5_ROOT__.prepareCatalogRecovery())
    await page.getByRole('button', { name: 'Reload event', exact: true }).click()
    await page.waitForLoadState('load')
    await expect.poll(async () => (await state(page))?.phase, { timeout: 20000 }).toBe('live')
    await navigate(page, '/world/map'); await expect(page.getByAltText('PARADE illustrated virtual map', { exact: true })).toBeVisible()
    assert.deepEqual(fixture.errors, [])
    report.checks.push({ name: 'Actual catalog mismatch and event reload recovery', status: 'PASS', cleanup: await close(fixture) })
  }
  {
    const fixture = await open(390, 'PARADE', '&font=corrupt'), { page } = fixture
    await expect.poll(async () => (await state(page))?.phase, { timeout: 20000 }).toBe('live')
    await expect.poll(() => fixture.requests.filter(p => p.endsWith('/map-labels-bold.ttf')).length).toBe(1)
    await navigate(page, '/world/map')
    assert.equal((await state(page)).overview, null)
    assert.equal(await page.getByAltText('PARADE illustrated virtual map', { exact: true }).count(), 0)
    assert.equal(await page.evaluate(() => [...document.fonts].filter(f => f.family === 'MCR Map Labels').length), 0)
    await page.unroute('**/map-labels-v1/map-labels-bold.ttf')
    await page.reload()
    await expect.poll(async () => (await state(page))?.overview?.themeId, { timeout: 20000 }).toBe('PARADE')
    await navigate(page, '/world/map'); await expect(page.getByAltText('PARADE illustrated virtual map', { exact: true })).toBeVisible()
    assert.deepEqual(fixture.errors, [])
    report.checks.push({ name: 'Corrupt exact-size font fails closed to existing fallback; genuine font reload recovers', status: 'PASS', cleanup: await close(fixture) })
  }
  for (const era of ['REVENGE', 'PARADE', 'DANGER']) {
    const fixture = await open(390, era, '&mode=production-denied'), { page } = fixture
    await expect(page.getByText('The world is unavailable. You can try opening it again.', { exact: true })).toBeVisible({ timeout: 20000 })
    assert.equal((await world(page)).games, 0); assert.equal((await state(page)).activeSockets, 0)
    assert.equal(assets(fixture.requests).length, 0); assert.deepEqual(fixture.errors, [])
    report.checks.push({ name: `Default production resolver denies unadmitted ${era}`, status: 'PASS', sceneAssetRequests: 0, cleanup: await close(fixture) })
  }
  report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.failure = String(error); throw error }
finally {
  await browser?.close(); await server.close(); await rm(resolve(output, 'vite-cache'), { recursive: true, force: true })
  report.cleanup = { browserClosed: !browser?.isConnected(), serverClosed: !server.httpServer?.listening }
  await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n')
}
