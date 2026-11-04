// FE18 actual Root + strict C05 + default Phaser qualification. Synthetic server inputs only.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { lstat, mkdir, mkdtemp, readFile, readdir, rm, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { build, createServer } from 'vite'
const root = resolve(import.meta.dirname, '../../../../../..'), frontend = resolve(root, 'frontend')
const output = process.env.MCR_FE18_OUTPUT, slot = process.env.MCR_FRONTEND_SLOT
assert(slot?.startsWith('FE18-'), 'Explicit NEW coordinator slot acceptance required')
assert(output?.startsWith(resolve(root, 'docs/mcr/evidence/fe18-rooms-nearby') + '/'))
async function bytes(path) {
  let entries; try { entries = await readdir(path, { withFileTypes: true }) } catch (e) { if (e.code === 'ENOENT') return 0; throw e }
  let total = 0
  for (const entry of entries) { try { const p = resolve(path, entry.name); if (entry.isDirectory()) total += await bytes(p); else { const s = await lstat(p); total += Math.max(s.size, s.blocks * 512) } } catch (e) { if (e.code !== 'ENOENT') throw e } }
  return total
}
async function guard() {
  const disk = await statfs(root), free = disk.bavail * disk.bsize
  assert(free >= 16 * 1024 ** 3, 'Fresh expanded 16GiB launch guard')
  const roots = execFileSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' }).split('\n')
    .filter(line => line.includes('--remote-debugging-pipe') && line.includes('/Google Chrome.app/Contents/MacOS/Google Chrome '))
  assert.equal(roots.filter(line => !/--user-data-dir=\S*mcp-chrome-/.test(line)).length, 0, 'Other validation browser active')
  return { availableBytes: free, otherValidationBrowserRoots: 0, interactiveBrowsersPreserved: roots.length }
}
const report = { status: 'IN_PROGRESS', slot, source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  capturedAtUTC: new Date().toISOString(), preflight: await guard(), pins: {}, cases: [], captures: [], errors: [],
  limits: ['Synthetic canonical Auth/C03/C05 source7005 fixture, not live backend or real BE37 block enforcement', 'Actual default Phaser + authorized DOM client behavior; not production art admission or whole reference fidelity', 'Automated desktop Chrome keyboard/touch events and viewports, not physical devices/software keyboards', 'W04 invitation authority unresolved; capacity is not measured occupancy'] }
await mkdir(output)
const temporary = await mkdtemp('/private/tmp/mcr-fe18-completion-'), profile = resolve(temporary, 'chrome-profile')
let context, server, peak = 0
const state = page => page.evaluate(() => window.__FE18_COMPLETION__.state())
const rootState = page => page.evaluate(() => window.__SCENE_V5_ROOT__.state())
const engine = page => page.evaluate(() => window.__SCENE_V5_ROOT__.inspectWorld())
async function budget() {
  const durable = await bytes(resolve(root, 'docs/mcr/evidence/fe18-rooms-nearby')), working = await bytes(temporary)
  peak = Math.max(peak, durable + working)
  assert(durable <= 40 * 1024 ** 2, 'Accumulated task durable cap')
  assert(durable + working <= .2 * 1024 ** 3, 'Whole task temporary/output disk cap')
  const disk = await statfs(root); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3, '8GiB stop floor')
}
async function paint(page) {
  await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))))
  await page.waitForTimeout(100)
}
async function capture(page, name, screen) {
  await paint(page)
  const capturedAtUTC = new Date().toISOString()
  await page.screenshot({ path: resolve(output, name) })
  report.captures.push({ path: resolve(output, name), screen, viewport: page.viewportSize(), capturedAtUTC, source: report.source,
    privacy: 'Only synthetic source-controlled fan/character fixtures; no real account, message, location or credential data' })
  await budget()
}
async function single(page) {
  const s = await rootState(page), e = await engine(page)
  assert.equal(s.activeSockets, 1); assert.equal(s.socketPeak, 1); assert.equal(s.canvases, 1); assert.equal(s.canvasPeak, 1)
  assert.equal(e.games, 1); assert.equal(e.sameGame, true); assert.equal(s.productionAdmitted, false)
  return { phase: s.phase, entities: s.entities, activeSockets: s.activeSockets, games: e.games, sameGame: e.sameGame, sprites: e.sprites }
}
async function dispose(page) {
  await page.evaluate(() => window.__FE18_COMPLETION__.dispose())
  await expect.poll(async () => (await rootState(page)).activeSockets).toBe(0)
  await expect.poll(async () => (await rootState(page)).canvases).toBe(0)
  await expect.poll(async () => { const u = (await rootState(page)).urls; return u.created.filter(v => !u.revoked.includes(v)).length }).toBe(0)
  const s = await rootState(page); await page.close(); return { disposed: s.disposed, activeSockets: s.activeSockets, canvases: s.canvases, urls: s.urls }
}
try {
  const paths = execFileSync('git', ['ls-files', 'frontend/src/features/world-rooms', 'frontend/src/features/world-nearby'], { cwd: root, encoding: 'utf8' }).trim().split('\n')
  paths.push('frontend/src/app/world-entry/__tests__/reference-scene-v5/preview.tsx', 'frontend/src/app/world-entry/__tests__/reference-scene-v5/data.ts',
    'frontend/src/app/world-entry/root-features.tsx', 'frontend/src/world/composition/index.ts', 'frontend/src/world/scenes/MapScene.ts', 'frontend/src/world/network/replication.ts',
    'frontend/src/entities/avatar/character-source-pack.json', ...['operations.json', 'world.schema.json', 'policy.json'].map(p => 'contracts/realtime/v1/' + p),
    ...['rooms.json', 'movement-heartbeat.json', 'ticket-response.json', 'filtered-viewers.json'].map(p => 'contracts/fixtures/realtime/' + p))
  for (const path of paths) report.pins[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')
  context = await chromium.launchPersistentContext(profile, { channel: 'chrome', headless: true, viewport: { width: 390, height: 844 } })
  report.browser = context.browser()?.version()
  context.on('page', page => page.on('pageerror', error => report.errors.push(error.message)))
  server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'vite-cache'), logLevel: 'error', server: { host: '127.0.0.1', port: 0, hmr: false } })
  await server.listen(); const origin = server.resolvedUrls.local[0].replace(/\/$/, '')
  for (const width of [360, 390, 430]) {
    const page = await context.newPage(); await page.setViewportSize({ width, height: 844 })
    await page.goto(origin + '/src/features/world-rooms/__tests__/completion/preview.html?era=PARADE')
    await expect.poll(async () => page.evaluate(() => window.__FE18_COMPLETION__?.state().root.phase), { timeout: 30000 }).toBe('live')
    await expect.poll(async () => (await engine(page)).sprites, { timeout: 30000 }).toBe(6)
    await page.getByRole('button', { name: 'World tools', exact: true }).click()
    await page.getByRole('link', { name: 'Rooms', exact: true }).click()
    const row = label => page.getByRole('heading', { name: label, exact: true, level: 3 }).locator('..').locator('..')
    const current = label => expect(row(label)).toHaveAttribute('aria-current', 'true')
    const change = () => page.getByRole('button', { name: 'Switch to SECOND ROOM', exact: true })
    const refresh = () => page.getByRole('button', { name: 'Refresh rooms', exact: true }).click()
    await expect(change()).toBeVisible(); await current('MAIN STAGE')
    const rowResult = { width, before: await single(page), states: [] }
    for (const availability of ['FULL', 'UNAVAILABLE']) {
      await page.evaluate(value => window.__FE18_COMPLETION__.catalog(value), availability); await refresh()
      await expect(change()).toBeDisabled(); await expect(page.getByText(`Capacity 50 · ${availability === 'FULL' ? 'Full' : 'Unavailable'}`, { exact: true })).toBeVisible()
      await current('MAIN STAGE'); assert.equal((await single(page)).entities, 6)
      rowResult.states.push({ availability, ...await single(page) })
      if (availability === 'FULL') await capture(page, `rooms-full-${width}.png`, 'Full target retains current virtual membership')
    }
    await page.evaluate(() => { window.__FE18_COMPLETION__.catalog('AVAILABLE'); window.__FE18_COMPLETION__.failCatalog(true) }); await refresh()
    await expect(page.getByRole('alert')).toContainText('Rooms are temporarily unavailable')
    await page.evaluate(() => window.__FE18_COMPLETION__.failCatalog(false)); await page.getByRole('button', { name: 'Read rooms again', exact: true }).click()
    await expect(change()).toBeEnabled()
    await page.evaluate(() => window.__FE18_COMPLETION__.denyTicket(true)); await change().focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('region', { name: 'Room switch status' })).toContainText('not accepted')
    await current('MAIN STAGE'); assert.equal((await single(page)).entities, 6)
    assert.equal((await state(page)).frames.filter(f => f.type === 'switchRoom').length, 0)
    await page.evaluate(() => window.__FE18_COMPLETION__.denyTicket(false))
    for (const outcome of ['reject', 'admit']) {
      await change().focus(); await page.keyboard.press('Enter')
      await expect.poll(async () => (await state(page)).pending?.roomId).toBeTruthy()
      await expect(page.getByRole('region', { name: 'Room switch status' })).toContainText('current room remains in place')
      await current('MAIN STAGE'); assert.equal((await single(page)).entities, 6)
      await page.evaluate(name => window.__FE18_COMPLETION__[name](), outcome)
      const feedback = page.getByRole('region', { name: 'Room switch status' })
      await expect(feedback).toContainText(outcome === 'admit' ? 'You are now in SECOND ROOM' : 'not accepted')
      await expect(feedback).toBeFocused(); await current(outcome === 'admit' ? 'SECOND ROOM' : 'MAIN STAGE')
      rowResult.states.push({ outcome, ...await single(page) })
    }
    await capture(page, `rooms-admitted-${width}.png`, 'Correlated switch admitted; keyboard focus on result')
    await page.getByRole('link', { name: 'Nearby in world', exact: true }).click()
    const roster = page.getByRole('list', { name: 'People authorized in your current virtual room' })
    await expect(roster.getByRole('link')).toHaveCount(5)
    await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
    await expect.poll(async () => page.evaluate(() => {
      const c = document.querySelector('.shell-world-host canvas'), r = document.querySelector('.world-controls-stage')?.getBoundingClientRect(), camera = window.__SCENE_V5_ROOT__.state().camera?.camera
      return !!c && r?.height > 0 && Math.abs(c.height - r.height) < 1 && camera?.height === c.height
    })).toBe(true)
    await paint(page)
    const actor = (await state(page)).serverSnapshot.players[1]
    await expect(roster.getByRole('link', { name: actor.name, exact: true })).toHaveCount(1)
    rowResult.paintBefore = await page.evaluate(id => window.__FE18_COMPLETION__.actorProbe(id), actor.id)
    await capture(page, `nearby-before-${width}.png`, 'Authorized roster and actual Phaser before filtered removal')
    await page.evaluate(id => window.__FE18_COMPLETION__.remove(id), actor.id)
    await expect(roster.getByRole('link', { name: actor.name, exact: true })).toHaveCount(0)
    await expect.poll(async () => (await engine(page)).sprites).toBe(5); await paint(page)
    rowResult.paintAfter = await page.evaluate(() => window.__FE18_COMPLETION__.removedPaint())
    assert(rowResult.paintAfter.changedChannels > 20); assert.notEqual(rowResult.paintAfter.beforeHash, rowResult.paintAfter.afterHash)
    assert.equal(rowResult.paintAfter.removedActorSpritePresent, false)
    assert.equal((await single(page)).entities, 5)
    await capture(page, `nearby-removed-${width}.png`, 'One filtered projection removes the name and actual painted avatar')
    await page.evaluate(() => window.__FE18_COMPLETION__.epochSelfOnly())
    await expect(roster).toHaveCount(0); await expect.poll(async () => (await engine(page)).sprites).toBe(1)
    rowResult.epoch = await single(page)
    rowResult.geometry = await page.evaluate(() => ({ overflow: document.documentElement.scrollWidth - innerWidth,
      controls: Array.from(document.querySelectorAll('.world-access a,.world-access button')).filter(e => e.getBoundingClientRect().height > 0).map(e => ({ name: e.textContent, width: e.getBoundingClientRect().width, height: e.getBoundingClientRect().height })) }))
    assert.equal(rowResult.geometry.overflow, 0)
    for (const c of rowResult.geometry.controls) assert(c.width >= 44 && c.height >= 44)
    if (width === 390) {
      await capture(page, 'nearby-epoch-self-only.png', 'Epoch replacement removes every previous remote person')
      await page.getByRole('link', { name: 'Choose a room', exact: true }).click()
      const back = page.getByRole('button', { name: 'Switch to MAIN STAGE', exact: true }); await back.click()
      await expect.poll(async () => (await state(page)).pending?.roomId).toBeTruthy()
      await page.evaluate(() => window.__SCENE_V5_ROOT__.suspend())
      await expect.poll(async () => (await rootState(page)).activeSockets).toBe(0)
      await expect.poll(async () => (await rootState(page)).canvases).toBe(0)
      await page.evaluate(() => window.__FE18_COMPLETION__.admit())
      await paint(page)
      assert.equal((await rootState(page)).canvases, 0)
      assert.equal(await page.getByText(actor.name, { exact: true }).count(), 0)
      await capture(page, 'privacy-cancelled-late-admission.png', 'Privacy cancellation rejects late pending admission and clears private presentation')
    }
    rowResult.cleanup = await dispose(page); report.cases.push(rowResult); await budget()
  }
  await context.close(); context = null
  await server.close(); server = null
  await rm(profile, { recursive: true, force: true }); await rm(resolve(temporary, 'vite-cache'), { recursive: true, force: true })
  await build({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'build-cache'), logLevel: 'error', build: { target: 'es2022', outDir: resolve(temporary, 'application'), copyPublicDir: false } })
  report.applicationBuild = { result: 'PASS', entry: 'frontend/index.html/src/main.tsx', sequentialAfterBrowser: true, copyPublicDir: false }
  await budget(); assert.deepEqual(report.errors, [])
  report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.error = String(error.stack ?? error); process.exitCode = 1 }
finally {
  const closed = await Promise.allSettled([context?.close(), server?.close()])
  await rm(temporary, { recursive: true, force: true })
  report.cleanup = { browserClosed: closed[0].status === 'fulfilled', serverClosed: closed[1].status === 'fulfilled', temporaryRemoved: true }
  if (closed.some(x => x.status === 'rejected')) { report.status = 'FAIL'; process.exitCode = 1 }
  report.peakMeasuredWorkflowBytes = peak
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify({ status: report.status, source: report.source, cases: report.cases.length, captures: report.captures.length, cleanup: report.cleanup }))
}
