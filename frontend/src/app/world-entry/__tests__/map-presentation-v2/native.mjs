// Root153 canonical runner ONLY. Fresh allocation/ACCEPT is an external execution prerequisite.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, writeFile, statfs, readdir, stat, rm } from 'node:fs/promises'
import { resolve, extname } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const root = resolve(import.meta.dirname, '../../../../../..'), frontend = resolve(root, 'frontend')
const output = process.env.MCR_MAP4_OUTPUT, lease = process.env.MCR_MAP4_LEASE, temporary = process.env.MCR_MAP4_TEMP
const port = Number(process.env.MCR_MAP4_PORT ?? '5298')
assert(Number.isSafeInteger(port) && port > 1023 && port < 65536, 'Named local Vite port')
const deadline = Date.parse(process.env.MCR_MAP4_DEADLINE ?? ''), cap = 64 * 1024 ** 2
assert(output?.startsWith('/') && temporary?.startsWith('/private/tmp/mcr-root-map4-') && lease && Number.isFinite(deadline), 'Set fresh Root output/private TMP/lease/deadline')
assert(deadline > Date.now() + 120_000, 'Reserve useful execution plus cleanup')
const hash = bytes => 'sha256:' + createHash('sha256').update(bytes).digest('hex')
const base = '4fa9b84b1ffe20c0f4793e73ab72247e77830649'
const relativeFactory = 'frontend/src/app/world-entry/reference-scene-overview.ts'
const baseline = execFileSync('git', ['show', `${base}:${relativeFactory}`], { cwd: root, encoding: 'utf8' })
const candidate = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()
const publicPaths = new Set(execFileSync('git', ['ls-tree', '-r', '--name-only', candidate, 'frontend/public'], { cwd: root, encoding: 'utf8' }).trim().split('\n'))
const scenario = 'eligible-attendee-contract-fixture'
const report = { status: 'IN_PROGRESS', qualification: 'EXPLICIT_CONTRACT_FIXTURE_MAP_REFERENCE_COMPOSITION_DEFAULT_CLIENT_ONLY', scenario,
  lease, candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  baseline: { commit: base, module: relativeFactory, sha256: hash(baseline),
    method: 'BEFORE only: Vite serves exact retained v1 Map factory; AFTER current complete-composition factory. Actual production loader/resolver and all World sources remain current.' },
  sourcePins: {}, assetPins: {}, checks: [], limits: ['Explicit eligible-attendee contract fixture via controlled HTTP/C05; original Map4 map/theme/avatar manifest DTOs and hashes preserved. Participant era selected per context.',
    'Fixture eligibility is not a public operator admission receipt. The actual default client, decoder, loader, resolver, renderer and production guards remain in use.',
    'Not current default-production availability, real-cohort acceptance, live-server admission, human fidelity, physical devices, or ten-screen acceptance. Complete-scene fit still requires direct owner/Artist reference assessment and fixes.'] }
for (const path of [relativeFactory, 'frontend/src/world/reference-scene/overview.ts',
  'frontend/src/world/scenes/MapScene.ts', 'frontend/src/world/runtime.ts',
  ...['operator-bootstrap.json', 'local-input.json', 'eligible-attendee-scenario.json'].map(p => `frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/${p}`),
  ...['eligible-attendee.mjs', 'input-readiness.mjs', 'preview.tsx'].map(p => `frontend/src/app/world-entry/__tests__/map-presentation-v1/${p}`),
  'frontend/public/world/map-firstview-cozy-v1/parade-ground.png',
  'frontend/src/world/reference-scene/map4-service-v1/style-source.json',
  'frontend/src/world/reference-scene/map4-service-v1/annotations.ts',
  'frontend/src/app/world-entry/map-presentation-v1/descriptor.ts',
  'frontend/src/app/world-entry/map-presentation-v2/loader.ts',
  ...['contract.ts', 'current.ts', 'registration.ts', 'resources.json', 'layout-source.json', 'model.ts', 'labels.ts', 'semantics.ts', 'style.ts'].map(p => `frontend/src/world/reference-scene/map4-service-v2/${p}`),
  'frontend/public/world/map-reference-composition-v1/layout.json',
  'frontend/public/world/map-reference-composition-v1/source-provenance.json',
  ...['revenge', 'parade', 'danger'].map(era => `frontend/public/world/map-reference-composition-v1/${era}-scene.png`),
]) report.sourcePins[path] = hash(await readFile(resolve(root, path)))

async function directoryBytes(path) {
  let total = 0
  for (const item of await readdir(path, { withFileTypes: true })) {
    const child = resolve(path, item.name)
    total += item.isDirectory() ? await directoryBytes(child) : (await stat(child)).size
  }
  return total
}
async function guard() {
  const disk = await statfs(root)
  assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3, '8GiB continuous disk floor')
  assert(Date.now() < deadline - 60_000, 'Preserve cleanup minute')
  assert(await directoryBytes(output) < cap, '64MiB output cap')
}
const startDisk = await statfs(root); assert(startDisk.bavail * startDisk.bsize >= 8 * 1024 ** 3, '8GiB accepted-allocation continuous disk floor')
await mkdir(output, { recursive: true, mode: 0o700 })
const registrationSource = await readFile(resolve(root, 'frontend/src/world/reference-scene/map4-service-v2/registration.ts'), 'utf8')
const expectedPresentationHash = registrationSource.match(/presentationHash:\s*'(sha256:[a-f0-9]{64})'/)?.[1]
assert(expectedPresentationHash, 'Frozen registration declares the exact composition hash')
report.expectedPresentationHash = expectedPresentationHash
const state = page => page.evaluate(() => window.__MAP4_DEFAULT__?.state())
const navigate = (page, path) => page.evaluate(path => window.__MAP4_DEFAULT__.navigate(path), path)
// WorldHost is resized by the shell on panel navigation. Sample camera only after
// its existing ResizeObserver/rAF has reached the actual render-mount dimensions.
// The outer shell host also contains controls and is not the camera viewport.
async function settledCamera(page) {
  await expect.poll(() => page.evaluate(() => {
    const camera = window.__MAP4_DEFAULT__.state().camera?.camera
    const box = document.querySelector('.shell-world-host canvas')?.parentElement?.getBoundingClientRect()
    return !!camera && !!box && camera.width === Math.max(1, Math.floor(box.width))
      && camera.height === Math.max(1, Math.floor(box.height))
  }), { timeout: 5000 }).toBe(true)
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  return (await state(page)).camera
}
let browser, server, context
try {
  browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const phase of ['before', 'after']) {
    await guard()
    server = await createServer({ configLoader: 'runner', root: frontend, cacheDir: resolve(temporary, `vite-${phase}`), logLevel: 'error',
      plugins: [{ name: 'immutable-public-assets', configureServer(server) { server.middlewares.use((req, res, next) => {
        const path = 'frontend/public' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname)
        if (!publicPaths.has(path)) return next()
        const bytes = execFileSync('git', ['show', `${candidate}:${path}`], { cwd: root, maxBuffer: 64 * 1024 ** 2 })
        report.assetPins[path] = { sha256: hash(bytes), bytes: bytes.length, commit: candidate }
        const mime = { '.png': 'image/png', '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.json': 'application/json', '.jpg': 'image/jpeg' }[extname(path)]
        if (mime) res.setHeader('Content-Type', mime)
        res.end(bytes)
      }) } }, ...(phase === 'before' ? [{ name: 'immutable-before-factory', enforce: 'pre',
        load(id) { return id.replaceAll('\\', '/').split('?')[0] === resolve(root, relativeFactory) ? baseline : null } }] : [])],
      server: { host: '127.0.0.1', port, strictPort: true } })
    await server.listen()
    ;(report.servers ??= []).push({ phase, address: server.resolvedUrls.local[0], port })
    for (const era of ['REVENGE', 'PARADE', 'DANGER']) for (const { width, height } of [
      ...[360, 390, 430].map(width => ({ width, height: 844 })),
      ...(phase === 'after' && era === 'PARADE' ? [{ width: 390, height: 420 }] : []),
    ]) {
      await guard()
      context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce', serviceWorkers: 'block' })
      const page = await context.newPage(), errors = [], requests = []
      report.current = { phase, era, width, height, errors, requests }
      page.on('pageerror', e => errors.push(e.message)); page.on('request', r => requests.push(new URL(r.url()).pathname))
      await page.clock.setFixedTime(new Date('2026-11-04T10:00:00Z'))
      await page.goto(server.resolvedUrls.local[0] + 'src/app/world-entry/__tests__/map-presentation-v1/preview.html?' + new URLSearchParams({ era, scenario }))
      await expect.poll(async () => (await state(page))?.overview?.imageUrl, { timeout: 30_000 }).toBeTruthy()
      await expect.poll(async () => (await state(page))?.overview?.presentationVersion, { timeout: 30_000 }).toBe(phase === 'after' ? 'map4-reference-composition-v1' : 'map4-service-v1')
      await navigate(page, '/world')
      await expect(page.locator('.shell-world-host canvas')).toHaveCount(1)
      const worldCamera = await settledCamera(page)
      const world = await page.evaluate(() => window.__MAP4_DEFAULT__.worldPixels())
      assert.equal(world.width, worldCamera.camera.width); assert.equal(world.height, worldCamera.camera.height)
      await navigate(page, '/world/map')
      const image = page.getByRole('img', { name: `${era} illustrated virtual map`, exact: true })
      await expect(image).toBeVisible()
      await expect(page.getByRole('button', { name: 'Fit map', exact: true })).toBeEnabled()
      await settledCamera(page)
      const initial = await state(page)
      assert.equal(initial.qualification, 'WORLD_POSITIVE_CONTRACT_FIXTURE_DEFAULT_PRODUCTION_GUARDS')
      assert.equal(initial.productionAdmitted, false)
      assert.equal(initial.inputReadiness.status, 'INPUT_CANDIDATE')
      assert.equal(initial.scenario, scenario)
      assert.equal(initial.inputScenario.qualification, 'WORLD_POSITIVE_CONTRACT_FIXTURE_NOT_OPERATOR_OR_LIVE_ADMISSION')
      assert.deepEqual(initial.capabilities.world, { available: true, reason: null })
      assert.equal(initial.overview.mapRevision, 4); assert.equal(initial.overview.themeId, era)
      assert.equal(initial.canvases, 1); assert.equal(initial.sockets, 1); assert.equal(initial.sameCamera, true)
      assert.equal(initial.overview.presentationVersion, phase === 'after' ? 'map4-reference-composition-v1' : 'map4-service-v1')
      if (phase === 'after') assert.equal(initial.overview.presentationHash, expectedPresentationHash, 'Actual adopted overview matches frozen complete composition registration')
      const screenshot = `${phase}-${era.toLowerCase()}-${width}${height === 420 ? '-short' : ''}.png`
      const captureStartedAt = new Date().toISOString()
      await page.screenshot({ path: resolve(output, screenshot), fullPage: false })
      const result = { phase, era, width, height, world, worldCamera, initial, screenshot, captureStartedAt, captureFinishedAt: new Date().toISOString(), screenshotHash: hash(await readFile(resolve(output, screenshot))),
        virtualActions: [], controls: {}, requests, errors }
      if (phase === 'after') {
        for (const intent of ['wall', 'feed', 'meet', 'gifts']) {
          const link = page.locator(`#map-feature-${intent}`)
          await link.scrollIntoViewIfNeeded(); const box = await link.boundingBox()
          assert(box && box.width >= 44 && box.height >= 44)
          assert(box.x >= 0 && box.y >= 0 && box.x + box.width <= width && box.y + box.height <= height, 'Full target visible after local scroll')
          const unobstructed = await link.evaluate(element => {
            const r = element.getBoundingClientRect()
            return document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.closest('a') === element
          })
          assert(unobstructed, 'Feature target must not be blocked by controls/canonical markers')
          await expect(link).toHaveAttribute('href', '/' + intent)
          await link.focus(); await page.keyboard.press('Enter')
          await expect.poll(async () => (await state(page)).path).toBe('/' + intent)
          const routeWorld = await state(page)
          assert.equal(routeWorld.canvases, 1, 'Feature navigation retains the incumbent World canvas')
          assert.equal(routeWorld.sockets, 1, 'Feature navigation creates no second socket')
          assert(routeWorld.activeSockets <= 1)
          assert.equal(routeWorld.sameCamera, true)
          await page.evaluate(() => window.__MAP4_DEFAULT__.back())
          await expect(link).toBeFocused()
          result.virtualActions.push({ intent, path: '/' + intent, width: box.width, height: box.height, keyboardAndBackFocus: true, routeWorld: { canvases: routeWorld.canvases, socketsCreated: routeWorld.sockets, activeSockets: routeWorld.activeSockets, sameCamera: routeWorld.sameCamera } })
        }
      }
      const cameraBefore = (await state(page)).camera
      await page.getByRole('button', { name: 'Zoom in map view', exact: true }).click()
      await page.getByRole('button', { name: 'Fit map', exact: true }).click()
      await page.getByRole('button', { name: 'Reset view', exact: true }).click()
      result.controls = { localZoomFitReset: true, cameraUnchanged: JSON.stringify((await state(page)).camera) === JSON.stringify(cameraBefore) }
      assert(result.controls.cameraUnchanged)
      const stable = await state(page)
      assert.equal(stable.canvases, 1); assert.equal(stable.sockets, 1); assert.equal(stable.sameCamera, true)
      assert.deepEqual(stable.entities, initial.entities)
      await expect(page.getByText('No physical arrival is implied.', { exact: true })).toBeVisible()
      assert.equal(errors.length, 0)
      assert(!requests.some(p => p.startsWith('/world/') && (p.includes('spatial-v5') || p.includes('quality-v10') || p.includes('cozy-v1/parade-atlas'))))
      await page.evaluate(() => window.__MAP4_DEFAULT__.dispose())
      // FE14 releases decoded resources after engine.closed, not synchronously at React unmount.
      await expect.poll(async () => {
        const value = await state(page)
        return value.activeSockets === 0 && value.canvases === 0 && value.urls.created.every(url => value.urls.revoked.includes(url))
      }, { timeout: 5000 }).toBe(true)
      result.cleanup = await state(page)
      assert.equal(result.cleanup.activeSockets, 0); assert.equal(result.cleanup.canvases, 0)
      assert.deepEqual([...new Set(result.cleanup.urls.created)].sort(), [...new Set(result.cleanup.urls.revoked)].sort())
      report.checks.push(result); await context.close(); context = null
      await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2))
    }
    await server.close(); server = null
    await rm(resolve(temporary, `vite-${phase}`), { recursive: true, force: true })
  }
  for (const after of report.checks.filter(c => c.phase === 'after' && c.height === 844)) {
    const before = report.checks.find(c => c.phase === 'before' && c.width === after.width && c.era === after.era && c.height === 844)
    assert.deepEqual(after.world, before.world, `WORLD RGBA/objects changed ${after.era}/${after.width}`)
    assert.deepEqual(after.worldCamera, before.worldCamera, `Settled WORLD camera changed ${after.era}/${after.width}`)
    assert.deepEqual(after.initial.map, before.initial.map); assert.deepEqual(after.initial.camera, before.initial.camera, `Settled MAP-host camera changed ${after.era}/${after.width}`)
  }
  report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.error = String(error); report.failureState = await context?.pages()[0]?.evaluate(() => window.__MAP4_DEFAULT__?.state()).catch(() => null); process.exitCode = 1 }
finally {
  await context?.close(); await browser?.close(); await server?.close()
  await rm(resolve(temporary, 'vite-before'), { recursive: true, force: true }); await rm(resolve(temporary, 'vite-after'), { recursive: true, force: true })
  report.finishedAt = new Date().toISOString(); report.cleanup = { contextClosed: true, browserClosed: true, serverClosed: true, viteCachesRemoved: true }
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2))
}
