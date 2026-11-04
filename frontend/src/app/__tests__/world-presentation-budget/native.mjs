// New, bounded driver of the existing actual Root/Phaser fixture. No production admission.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, readdir, rm, stat, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { chromium, expect } from '@playwright/test'
import { build, createServer } from 'vite'

const root = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(root, 'frontend')
const base = 'd1a5deaae0f6b79b365cb4c451d921711573ab8c'
const slot = process.env.MCR_FRONTEND_SLOT, output = process.env.MCR_WORLD_BUDGET_OUTPUT
assert(slot?.startsWith('FE26E-WORLD-PRESENTATION-'), 'An explicitly accepted NEW allocation is required')
assert(!slot.endsWith('T0429Z'), 'The unaccepted 0429Z offer was returned')
assert(output?.startsWith(resolve(root, 'docs/mcr/evidence/world-presentation-budget') + '/'))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const git = (...args) => execFileSync('git', args, { cwd: root, maxBuffer: 16 * 1024 ** 2 })
async function bytes(path) {
  let total = 0
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const file = resolve(path, entry.name)
    total += entry.isDirectory() ? await bytes(file) : (await stat(file)).size
  }
  return total
}
async function preflight(ownedProfile = '') {
  const disk = await statfs(root), available = disk.bavail * disk.bsize
  assert(available >= 8 * 1024 ** 3, 'Fresh 8GiB floor')
  const peers = execFileSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' }).split('\n')
    .filter(line => line.includes('--remote-debugging-pipe') && (!ownedProfile || !line.includes(ownedProfile)))
  assert.equal(peers.length, 0, 'Another controlled browser is active; do not preempt it')
  return { availableBytes: available, otherControlledBrowsers: peers.length }
}
const report = { status: 'IN_PROGRESS', slot, base, candidate: git('rev-parse', 'HEAD').toString().trim(),
  qualification: 'ACTUAL_ROOT_DEFAULT_PHASER_WITH_UNREVIEWED_SOURCE_FIXTURE', sourcePins: {}, inputs: {}, cases: [], interactions: [],
  limits: ['Desktop Chrome viewports; no physical iOS/Android/software-keyboard/notch acceptance',
    'Existing source7005 synthetic C03/C05 fixture, not public/live/backend6 admission',
    'Six-player fixture positions and approved art are unchanged; no full ten-screen fidelity acceptance'] }
report.preflight = await preflight()
await mkdir(output, { mode: 0o700 })
const temporary = await mkdtemp('/private/tmp/mcr-world-presentation-'), profile = resolve(temporary, 'chrome-profile')
let context, server, peakBytes = 0
const before = new Map(['frontend/src/app/WorldShell.tsx', 'frontend/src/app/shell.css'].map(path => [resolve(root, path), git('show', `${base}:${path}`).toString()]))
const fixture = '/src/app/world-entry/__tests__/reference-scene-v5/preview.html'
const readState = page => page.evaluate(() => window.__SCENE_V5_ROOT__.state())
const inspect = page => page.evaluate(() => window.__SCENE_V5_ROOT__.inspectWorld())
async function budget() {
  const durable = await bytes(output), working = await bytes(temporary)
  peakBytes = Math.max(peakBytes, durable + working)
  assert(durable <= 40 * 1024 ** 2, '40MiB durable cap')
  assert(durable + working <= .2 * 1024 ** 3, '0.2GiB workflow cap')
}
async function measure(page) {
  return page.evaluate(() => {
    const rect = selector => {
      const node = document.querySelector(selector), r = node?.getBoundingClientRect()
      return r && { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom }
    }
    return { stage: rect('.shell-stage'), canvas: rect('.shell-world-host canvas'), controlsStage: rect('.world-controls-stage'),
      summary: rect('.world-composition-summary'), tools: rect('.shell-world-tools'), toggle: rect('.world-tools-toggle'),
      header: rect('.mcr-chrome-header'), eras: rect('.shell-era-region'), navigation: rect('.mcr-chrome-bottom'),
      status: document.querySelector('.world-composition-status')?.textContent,
      notice: document.querySelector('.world-composition-notice')?.textContent,
      overflow: document.documentElement.scrollWidth - innerWidth }
  })
}
async function dispose(page) {
  await page.evaluate(() => window.__SCENE_V5_ROOT__.dispose())
  await expect.poll(async () => (await readState(page)).activeSockets).toBe(0)
  await expect.poll(async () => (await readState(page)).canvases).toBe(0)
  await expect.poll(async () => {
    const { urls } = await readState(page)
    return urls.created.filter(url => !urls.revoked.includes(url)).length
  }).toBe(0)
  const state = await readState(page)
  assert.equal(state.disposed, true)
  assert(state.urls.created.every(url => state.urls.revoked.includes(url)), 'Every owned blob URL revoked')
  await page.close()
  return state
}
async function ready(page, url) {
  await page.goto(url)
  await expect.poll(async () => page.evaluate(() => window.__SCENE_V5_ROOT__?.state().phase), { timeout: 30000 }).toBe('live')
  await expect.poll(async () => (await inspect(page)).sprites, { timeout: 30000 }).toBe(6)
  await page.evaluate(() => document.fonts.ready)
  await page.waitForTimeout(100)
}
async function assertSingle(page) {
  const state = await readState(page), engine = await inspect(page)
  assert.equal(state.activeSockets, 1); assert.equal(state.socketPeak, 1)
  assert.equal(state.canvases, 1); assert.equal(state.canvasPeak, 1)
  assert.equal(engine.games, 1); assert.equal(engine.sameGame, true)
  assert.equal(state.productionAdmitted, false)
  return { state, engine }
}
try {
  for (const path of ['frontend/src/app/WorldShell.tsx', 'frontend/src/app/shell.css', 'frontend/src/app/world-presentation/WorldTools.tsx',
    'frontend/src/world/composition/index.ts', 'frontend/src/world/scenes/MapScene.ts',
    'frontend/src/world/reference-scene/spatial-v5/camera.ts', 'frontend/src/entities/avatar/character-source-pack.json',
    ...['preview.tsx', 'preview.html', 'data.ts'].map(file => 'frontend/src/app/world-entry/__tests__/reference-scene-v5/' + file)]) {
    report.sourcePins[path] = hash(await readFile(resolve(root, path)))
  }
  const packetPath = 'docs/mcr/evidence/visual/qa06/world-presentation-budget-packet.json'
  const packetBytes = git('show', `e8dac180:${packetPath}`), packet = JSON.parse(packetBytes)
  report.inputs.packet = { commit: 'e8dac180cd20b9135a32b13243f5f05dccd34de2', path: packetPath, sha256: hash(packetBytes) }
  for (const input of [packet.reference.artifact, ...packet.visualArtifacts]) {
    const original = git('show', `e3173efb:${input.path}`)
    assert.equal(hash(original), input.sha256)
    report.inputs[input.path] = { sha256: hash(original), bytes: original.length }
    if (input === packet.reference.artifact) await writeFile(resolve(output, 'reference-target.png'), original)
  }
  report.referenceCrop = packet.reference.crop
  context = await chromium.launchPersistentContext(profile, { channel: 'chrome', headless: true, viewport: { width: 390, height: 554 } })
  report.browser = context.browser()?.version()
  // Observe only the existing fixture's Socket constructor. Normal DTO/auth/game behavior is unchanged.
  await context.addInitScript(() => {
    if (location.protocol !== 'http:' && location.protocol !== 'https:') return
    localStorage.clear()
    const define = Object.defineProperty
    window.__WORLD_BUDGET_SOCKETS__ = []
    Object.defineProperty = function (target, key, descriptor) {
      if (target === window && key === 'WebSocket' && typeof descriptor.value === 'function') {
        Object.defineProperty = define
        const value = new Proxy(descriptor.value, { construct(Type, args, NewTarget) {
          const socket = Reflect.construct(Type, args, NewTarget)
          window.__WORLD_BUDGET_SOCKETS__.push(socket)
          return socket
        } })
        return define(target, key, { ...descriptor, value })
      }
      return define(target, key, descriptor)
    }
  })
  const errors = []
  context.on('page', page => page.on('pageerror', error => errors.push(error.message)))
  for (const mode of ['baseline', 'candidate']) {
    report[`${mode}Preflight`] = await preflight(profile)
    server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'vite-cache'), logLevel: 'error',
      plugins: mode === 'baseline' ? [{ name: 'immutable-published-world-baseline', enforce: 'pre', load(id) { return before.get(id.split('?')[0]) ?? null } }] : [],
      server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false } })
    await server.listen()
    const origin = server.resolvedUrls.local[0].replace(/\/$/, '')
    for (const era of mode === 'baseline' ? ['PARADE'] : ['REVENGE', 'PARADE', 'DANGER']) {
      for (const width of [360, 390, 430]) for (const height of [554, 844]) {
        const page = await context.newPage(); await page.setViewportSize({ width, height })
        await ready(page, `${origin}${fixture}?era=${era}`)
        const metrics = await measure(page), single = await assertSingle(page)
        const projection = await page.evaluate(() => window.__SCENE_V5_ROOT__.cameraProjection())
        assert.equal(metrics.overflow, 0)
        assert.equal(metrics.notice, 'Virtual position is not physical arrival.')
        if (mode === 'candidate') {
          assert(metrics.toggle.width >= 44 && metrics.toggle.height >= 44)
          assert(Math.abs(metrics.canvas.height - metrics.stage.height) < 2, 'Canvas fills the recovered stage')
          const baseline = report.cases.find(row => row.mode === 'baseline' && row.width === width && row.height === height)
          assert(metrics.canvas.height > baseline.metrics.canvas.height, 'Actual measured canvas budget improves')
          assert.deepEqual(projection.self, baseline.projection.self, 'Server self unchanged')
        }
        const image = `${mode}-${era.toLowerCase()}-${width}-h${height}.png`
        await page.screenshot({ path: resolve(output, image) })
        const row = { mode, era, width, height, image, metrics, ...single, projection }
        report.cases.push(row)
        if (mode === 'candidate' && era === 'PARADE' && width === 390 && height === 554) {
          const toggle = page.getByRole('button', { name: 'World tools', exact: true })
          await toggle.focus(); await page.keyboard.press('Enter')
          const nav = page.getByRole('navigation', { name: 'World tools', exact: true })
          for (const label of ['Map', 'Rooms', 'Nearby', 'Help']) {
            const link = nav.getByRole('link', { name: label, exact: true }), box = await link.boundingBox()
            assert(box.width >= 44 && box.height >= 44)
            const id = await link.getAttribute('id')
            await link.click(); await expect(page.getByRole('button', { name: 'Back to previous panel', exact: true })).toBeVisible()
            await page.getByRole('button', { name: 'Back to previous panel', exact: true }).click()
            await expect(page.locator(`#${id}`)).toBeFocused()
            await assertSingle(page)
          }
          await page.screenshot({ path: resolve(output, 'tools-expanded-390-h554.png') })
          await page.keyboard.press('Escape'); await expect(toggle).toBeFocused()
          await page.getByRole('link', { name: 'Skip to content', exact: true }).focus()
          await page.keyboard.press('Enter'); await expect(page.locator('main#shell-content')).toBeFocused()
          for (const path of ['/feed', '/meet', '/gifts', '/wall', '/me', '/world']) {
            await page.evaluate(path => window.__SCENE_V5_ROOT__.navigate(path), path)
            await expect.poll(async () => (await readState(page)).path).toBe(path)
            await assertSingle(page)
            await page.screenshot({ path: resolve(output, `route-${path.slice(1)}-390-h554.png`) })
          }
          await page.setViewportSize({ width: 390, height: 420 })
          await expect(toggle).toBeVisible(); await toggle.focus(); await page.keyboard.press('Enter')
          await expect(nav).toBeVisible(); await page.keyboard.press('Escape'); await expect(toggle).toBeFocused()
          report.interactions.push({ name: 'reduced viewport and physical-keyboard disclosure, not a software keyboard', metrics: await measure(page), ...await assertSingle(page) })
          await page.screenshot({ path: resolve(output, 'keyboard-reduced-390-h420.png') })
          await page.setViewportSize({ width: 390, height: 554 })
          await page.evaluate(() => window.__WORLD_BUDGET_SOCKETS__.at(-1).serverClose(4400))
          const reconnect = page.getByRole('button', { name: 'Reconnect world', exact: true })
          await expect(reconnect).toBeVisible()
          await page.screenshot({ path: resolve(output, 'disconnected-390-h554.png') })
          await reconnect.click(); await expect.poll(async () => (await readState(page)).phase).toBe('live')
          await assertSingle(page)
          assert.deepEqual((await page.evaluate(() => window.__SCENE_V5_ROOT__.cameraProjection())).self, projection.self)
          report.interactions.push({ name: 'tools/routes/keyboard/reconnect', metrics: await measure(page), ...await assertSingle(page) })
          await page.evaluate(() => window.__SCENE_V5_ROOT__.suspend())
          await expect.poll(async () => (await readState(page)).activeSockets).toBe(0)
          await expect.poll(async () => (await readState(page)).canvases).toBe(0)
          await page.screenshot({ path: resolve(output, 'privacy-withdrawn-390-h554.png') })
          report.interactions.push({ name: 'actual privacy withdrawal', state: await readState(page) })
        }
        row.cleanup = await dispose(page)
        await budget()
      }
    }
    if (mode === 'candidate') {
      const page = await context.newPage(); await page.setViewportSize({ width: 390, height: 554 })
      await page.route('**/world/reference-scene-v2/runtime/spatial-v5/canonical-map.json', route => route.abort())
      await page.goto(`${origin}${fixture}?era=PARADE`)
      const retry = page.getByRole('button', { name: 'Try opening world again', exact: true })
      await expect(retry).toBeVisible({ timeout: 30000 })
      await page.screenshot({ path: resolve(output, 'critical-retry-390-h554.png') })
      await page.unroute('**/world/reference-scene-v2/runtime/spatial-v5/canonical-map.json')
      await retry.click(); await expect.poll(async () => (await readState(page)).phase, { timeout: 30000 }).toBe('live')
      report.interactions.push({ name: 'actual resource failure and retry', ...await assertSingle(page), metrics: await measure(page) })
      await dispose(page)
      const denied = await context.newPage(); await denied.setViewportSize({ width: 390, height: 554 })
      await denied.goto(`${origin}${fixture}?era=PARADE&mode=production-denied`)
      await expect.poll(async () => denied.evaluate(() => Boolean(window.__SCENE_V5_ROOT__)), { timeout: 30000 }).toBe(true)
      await denied.waitForTimeout(300)
      const denial = await readState(denied)
      assert.equal(denial.canvases, 0); assert.equal(denial.activeSockets, 0)
      assert.equal(denial.productionAdmitted, false)
      report.interactions.push({ name: 'unchanged production release rejection', state: denial })
      await denied.screenshot({ path: resolve(output, 'production-denied-390-h554.png') })
      await dispose(denied)
    }
    await server.close(); server = null
    await rm(resolve(temporary, 'vite-cache'), { recursive: true, force: true })
  }
  const crop = report.referenceCrop, scale = 390 / crop.width
  const html = `<!doctype html><html lang="en"><meta charset="utf-8"><title>World presentation comparison</title>
    <style>body{margin:12px;background:#eee9df;color:#191817;font:14px Arial,sans-serif}h1{font-size:20px;margin:0 0 8px}p{margin:6px 0}.comparisons{display:grid;grid-template-columns:repeat(3,390px);gap:16px}figure{margin:0}figcaption{height:36px;font-weight:700}.viewport{position:relative;width:390px;height:${crop.height * scale}px;overflow:hidden}.crop{position:absolute;width:${crop.width}px;height:${crop.height}px;overflow:hidden;transform:scale(${scale});transform-origin:top left}.crop img{position:absolute;left:-${crop.x}px;top:-${crop.y}px}figure>img{display:block;width:390px;height:554px}</style>
    <h1>World scene at the original reference aspect</h1><p>Actual Root/Phaser source fixture. Art and six-player snapshot coordinates unchanged; no live admission.</p>
    <div class="comparisons"><figure><figcaption>Unchanged supplied reference · proportional crop</figcaption><div class="viewport"><div class="crop"><img src="reference-target.png" alt="Supplied World reference"></div></div></figure>
    <figure><figcaption>Published ${base.slice(0, 8)} source baseline</figcaption><img src="baseline-parade-390-h554.png" alt="Measured source baseline"></figure>
    <figure><figcaption>World presentation candidate</figcaption><img src="candidate-parade-390-h554.png" alt="Measured presentation candidate"></figure></div></html>`
  await writeFile(resolve(output, 'comparison.html'), html)
  const comparison = await context.newPage(); await comparison.setViewportSize({ width: 1226, height: 670 })
  await comparison.goto(pathToFileURL(resolve(output, 'comparison.html')).href)
  await comparison.evaluate(() => Promise.all(Array.from(document.images, image => image.decode())))
  await comparison.screenshot({ path: resolve(output, 'comparison.png'), fullPage: true }); await comparison.close()
  await budget()
  await context.close(); context = null
  await rm(profile, { recursive: true, force: true })
  const bundle = await build({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'build-cache'), logLevel: 'error',
    build: { target: 'es2022', outDir: resolve(temporary, 'application'), copyPublicDir: false } })
  report.applicationBuild = { result: 'PASS', actualEntry: 'frontend/index.html and src/main.tsx', copyPublicDir: false,
    outputs: Array.isArray(bundle) ? bundle.length : 1, plugins: 'retained frontend/vite.config.ts React/Tailwind', sequentialAfterBrowser: true }
  await budget()
  assert.deepEqual(errors, [], 'No browser exceptions')
  report.peakMeasuredWorkflowBytes = peakBytes
  report.status = 'PASS'
} catch (error) {
  report.status = 'FAIL'; report.error = String(error?.stack ?? error); process.exitCode = 1
} finally {
  await context?.close()
  await server?.close()
  await rm(temporary, { recursive: true, force: true })
  report.cleanup = { browserClosed: true, serverClosed: true, temporaryRemoved: true }
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify({ status: report.status, candidate: report.candidate, cases: report.cases.length, interactions: report.interactions.length, cleanup: report.cleanup }))
}
