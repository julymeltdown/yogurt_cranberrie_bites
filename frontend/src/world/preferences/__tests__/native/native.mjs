// Controlled same-shell production integration. Actual Phaser/Settings/ThemeController; synthetic HTTP/C05/assets.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { readFile, writeFile, readdir, lstat, statfs, mkdir, mkdtemp, rm } from 'node:fs/promises'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer, build } from 'vite'
const root = resolve(import.meta.dirname, '../../../../../..'), frontend = resolve(root, 'frontend')
const kst = date => new Date(date.getTime() + 9 * 3600000).toISOString().replace('Z', '+09:00')
const slot = process.env.MCR_FRONTEND_SLOT, output = process.env.MCR_PREFERENCES_OUTPUT
assert(slot?.startsWith('WORLD-PREFERENCES-'), 'New explicit coordinator acceptance required')
assert(output?.startsWith(resolve(root, 'docs/mcr/evidence/world-runtime-preferences') + '/'))
async function bytes(path) {
 let rows; try { rows = await readdir(path, { withFileTypes: true }) } catch (e) { if (e.code === 'ENOENT') return 0; throw e }
 let total = 0
 for (const row of rows) { try { const p = resolve(path, row.name); if (row.isDirectory()) total += await bytes(p); else { const s = await lstat(p); total += Math.max(s.size, s.blocks * 512) } } catch (e) { if (e.code !== 'ENOENT') throw e } }
 return total
}
async function guard() {
 const disk = await statfs(root), free = disk.bavail * disk.bsize
 assert(free >= 16 * 1024 ** 3, '16GiB launch guard')
 const roots = execFileSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' }).split('\n').filter(x => x.includes('--remote-debugging-pipe') && x.includes('/Google Chrome.app/Contents/MacOS/Google Chrome '))
 assert.equal(roots.filter(x => !/--user-data-dir=\S*mcp-chrome-/.test(x)).length, 0, 'Other validation browser active')
 return { availableBytes: free, validationBrowsers: 0, interactiveMCPPreserved: roots.length }
}
const source = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()
const dirty = execFileSync('git', ['status', '--porcelain', '--untracked-files=all'], { cwd: root, encoding: 'utf8' }).trim().split('\n').filter(x => x && x !== '?? frontend/node_modules')
assert.deepEqual(dirty, [], 'Committed clean capture source required')
const report = { status: 'IN_PROGRESS', source, slot, branch: execFileSync('git', ['branch', '--show-current'], { cwd: root, encoding: 'utf8' }).trim(),
 startedAtUTC: new Date().toISOString(), startedAtKST: kst(new Date()), preflight: await guard(), pins: {}, captures: [], errors: [], checks: {},
 limits: ['Controlled frozen C03/C05/C06/selected-source fixtures, not live backend or production art admission', 'Actual production Root/Settings/adopter/composition/default Phaser and ThemeController from one checkout', 'Reduced motion covers nonessential tweens; character artwork/manual walk renderer unchanged', 'Low power caps actual canvas paints, not updates/network/authoritative physics; no battery or physical-device claim', 'No real eligible audio source; enable-sound is explicitly unavailable', 'Pending ignored-abort theme candidate disposal qualified by source tests; fixture lacks alternate admitted theme assets'] }
await mkdir(output)
const temporary = await mkdtemp('/private/tmp/mcr-world-preferences-native-'), profile = resolve(temporary, 'profile')
// Keep browser/framework default temporary artifacts inside the monitored owned tree.
process.env.TMPDIR = temporary; process.env.TMP = temporary; process.env.TEMP = temporary
let minimumFreeBytes = report.preflight.availableBytes
let server, context, peak = 0, monitorTimer, monitorBusy = false, monitorFlight = Promise.resolve(), resourceFailure = null
async function budget() {
 const durable = await bytes(resolve(root, 'docs/mcr/evidence/world-runtime-preferences')), working = await bytes(temporary), link = await lstat(resolve(frontend, 'node_modules'))
 assert(link.isSymbolicLink(), 'Borrowed dependency link only'); const linkBytes = Math.max(link.size, link.blocks * 512)
 peak = Math.max(peak, durable + working + linkBytes)
 assert(durable <= 40 * 1024 ** 2, 'Accumulated durable cap40MiB')
 assert(durable + working + linkBytes <= .18 * 1024 ** 3, 'Early whole-workflow stop0.18GiB before hard0.2GiB')
 const disk = await statfs(root), available = disk.bavail * disk.bsize; minimumFreeBytes = Math.min(minimumFreeBytes, available)
 assert(available >= 8 * 1024 ** 3, '8GiB stop floor')
}
const state = page => page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.state())
const engine = page => page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.engine())
async function single(page) {
 const pong = (await state(page)).framesByType.pong ?? 0
 await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.heartbeat())
 await expect.poll(async () => (await state(page)).framesByType.pong ?? 0).toBe(pong + 1)
 const s = await state(page), actual = await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.inspectWorld())
 assert.equal(s.activeSockets, 1); assert.equal(s.socketPeak, 1); assert.equal(s.canvases, 1); assert(s.sameCamera)
 assert.equal(actual.games, 1); assert.equal(actual.peakGames, 1); assert.equal(actual.totalGames, 1); assert(actual.sameGame); assert.equal(actual.sprites, 6)
 return { path: s.path, entities: s.entities, theme: s.theme, map: s.map, activeSockets: s.activeSockets, socketPeak: s.socketPeak, ...actual }
}
async function capture(page, name, screen) {
 await page.screenshot({ path: resolve(output, name) })
 report.captures.push({ path: resolve(output, name).slice(root.length + 1), screen, viewport: { width: 390, height: 844 }, source, branch: report.branch,
  capturedAtUTC: new Date().toISOString(), capturedAtKST: kst(new Date()), privacy: 'Synthetic source-controlled identity/preferences/characters only; no real person, account, message, credential or location' })
 await budget()
}
async function measure(page, ms) {
 await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.heartbeat())
 const before = await engine(page), start = Date.now()
 await page.waitForTimeout(ms)
 const after = await engine(page), elapsed = Date.now() - start
 return { elapsed, updates: after.counts.updates - before.counts.updates, pre: after.counts.pre - before.counts.pre,
  render: after.counts.render - before.counts.render, post: after.counts.post - before.counts.post,
  tweenDelta: after.tweenValue - before.tweenValue, before, after }
}
try {
 await budget() // Record even failures that settle before the first periodic sample.
 monitorTimer = setInterval(() => {
  if (monitorBusy || resourceFailure) return
  monitorBusy = true
  monitorFlight = budget().catch(async error => {
   resourceFailure = String(error); await Promise.allSettled([context?.close(), server?.close()])
  }).finally(() => { monitorBusy = false })
 }, 150)
 const paths = execFileSync('git', ['ls-files', 'frontend/src/world', 'frontend/src/features/theme', 'frontend/src/features/settings', 'frontend/src/features/me', 'frontend/src/app/account-entry', 'frontend/src/app/world-panels', 'frontend/src/app/ports.ts', 'frontend/src/entities/avatar/character-source-pack.json', 'contracts/fixtures/core', 'contracts/fixtures/realtime', 'contracts/realtime/v1', 'frontend/src/app/world-entry/__tests__/reference-scene-v5/data.ts'], { cwd: root, encoding: 'utf8' }).trim().split('\n').filter(path => !path.includes('/evidence/') && /\.(?:[cm]?[jt]sx?|json|css|html|proto)$/.test(path))
 // Supplementary tracked source/contract pins; the committed HEAD pins the complete tree.
 // Archived screenshot evidence can be tracked but intentionally unmaterialized.
 for (const path of paths) report.pins[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')
 context = await chromium.launchPersistentContext(profile, { channel: 'chrome', headless: true, reducedMotion: 'no-preference', viewport: { width: 390, height: 844 } })
 context.on('page', page => page.on('pageerror', e => report.errors.push(e.message)))
 server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'vite-cache'), logLevel: 'error', server: { host: '127.0.0.1', port: 0, hmr: false } })
 await server.listen(); const origin = server.resolvedUrls.local[0].replace(/\/$/, '')
 const page = await context.newPage()
 await page.goto(origin + '/src/world/preferences/__tests__/native/preview.html?era=PARADE')
 await expect.poll(async () => page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__?.state().phase), { timeout: 30000 }).toBe('live')
 await expect.poll(async () => (await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.inspectWorld())).sprites, { timeout: 30000 }).toBe(6)
 await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.navigate('/me/settings'))
 await expect(page.getByRole('heading', { name: 'SETTINGS', exact: true })).toBeVisible()
 await expect(page.getByLabel('Low power', { exact: true })).toBeEnabled()
 const reduce = page.getByRole('button', { name: 'Reduce panel', exact: true })
 if (await reduce.count()) await reduce.click()
 const initial = await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.beginProbe())
 assert(initial.normalMethods); assert.equal(initial.tweenScale, 1); assert(initial.noAudio); assert.equal(initial.cssProbeTransition, '0.12s')
 report.checks.initial = initial; report.checks.initialWorld = await single(page)
 const save = () => page.getByRole('button', { name: 'Save preferences', exact: true }).click()
 const apply = () => page.getByRole('button', { name: 'Apply saved preferences in this session', exact: true }).click()
 const saved = () => expect(page.getByText('Preferences saved. Sound only starts after you explicitly apply it in this session.', { exact: true })).toBeVisible()
 const applied = () => expect(page.getByText('Saved preferences applied in this session.', { exact: true })).toBeVisible()
 await expect(page.getByLabel('Enable sound', { exact: true })).not.toBeChecked()
 // Make saved preference match the genuinely available incumbent era; saving alone cannot affect the engine.
 const era = page.getByRole('combobox', { name: /^Preferred era/ })
 await era.selectOption('PARADE'); await expect(era).toHaveValue('PARADE')
 await page.getByLabel('Low power', { exact: true }).check(); await save(); await saved()
 const stored = await state(page); assert(stored.preferenceWrites >= 1); assert.equal(stored.savedPreferences.low_power, true); assert.equal(stored.savedPreferences.theme, 'PARADE')
 report.checks.savedOnly = { engine: await engine(page), state: await single(page), preferences: stored.savedPreferences, writes: stored.preferenceWrites }
 assert(report.checks.savedOnly.engine.normalMethods); assert.equal(report.checks.savedOnly.engine.tweenScale, 1)
 report.checks.normal = await measure(page, 800)
 assert(report.checks.normal.pre >= 4); assert.equal(report.checks.normal.pre, report.checks.normal.post)
 await capture(page, 'settings-saved-only.png', 'Actual Settings saved values remain separate from incumbent engine application')
 await apply(); await applied(); await expect.poll(async () => (await engine(page)).normalMethods).toBe(false)
 report.checks.lowPower = await measure(page, 1200)
 const low = report.checks.lowPower
 assert(low.pre >= 2); assert(low.pre <= Math.ceil(low.elapsed * 15 / 1000) + 1)
 assert.equal(low.pre, low.post); assert.equal(low.pre, low.render); assert(low.updates > low.pre)
 assert(low.after.sameLoopCallback && low.after.sameCamera); assert.equal(low.after.tweenScale, 1)
 assert(report.checks.normal.pre / report.checks.normal.elapsed > low.pre / low.elapsed)
 report.checks.applied = await single(page)
 await capture(page, 'settings-applied-low-power.png', 'Actual production adapter applies a paint-only limit to the same Phaser game')
 await page.emulateMedia({ reducedMotion: 'reduce' }); await expect.poll(async () => (await engine(page)).tweenScale).toBe(0)
 await expect.poll(async () => (await engine(page)).shellMotionAttribute).toBe('true')
 report.checks.osReduced = await measure(page, 300); assert.equal(report.checks.osReduced.after.cssProbeTransition, '0s')
 assert.equal(report.checks.osReduced.tweenDelta, 0); assert(report.checks.osReduced.updates > 0)
 await page.emulateMedia({ reducedMotion: 'no-preference' }); await expect.poll(async () => (await engine(page)).tweenScale).toBe(1)
 report.checks.osRestored = await measure(page, 300); assert(report.checks.osRestored.tweenDelta > 0)
 // No source means a saved sound request is an explicit application failure, never fake audio readiness.
 await page.getByLabel('Enable sound', { exact: true }).check(); await save(); await saved(); await apply()
 await expect(page.getByText('Could not apply preferences. Saved values are unchanged. Try again.', { exact: true })).toBeVisible()
 report.checks.soundUnavailable = await engine(page); assert.equal(report.checks.soundUnavailable.audioElements, 0); assert(report.checks.soundUnavailable.noAudio)
 await page.getByLabel('Enable sound', { exact: true }).uncheck(); await page.getByLabel('Reduced motion', { exact: true }).check(); await save(); await saved(); await apply(); await applied()
 await expect.poll(async () => (await engine(page)).tweenScale).toBe(0)
 await expect.poll(async () => (await engine(page)).shellMotionAttribute).toBe('true'); assert.equal((await engine(page)).cssProbeTransition, '0s')
 report.checks.userReduced = await single(page)
 // Ordinary panel navigation withdraws only the Settings caller's applied effect lifetime.
 await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.navigate('/world'))
 await expect.poll(async () => (await engine(page)).normalMethods).toBe(true); assert.equal((await engine(page)).tweenScale, 1)
 assert.equal((await engine(page)).shellMotionAttribute, initial.shellMotionAttribute); assert.equal((await engine(page)).cssProbeTransition, '0.12s')
 report.checks.navigation = await single(page)
 // Public producer caller abort remains linked after its acquisition promise settles.
 await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.borrowEffects()); assert.equal((await engine(page)).tweenScale, 0)
 const aborted = await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.abortBorrowed()); assert(aborted.normalMethods); assert.equal(aborted.tweenScale, 1)
 report.checks.callerAbort = aborted
 const unavailable = await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.selectUnavailableTheme())
 assert.equal(unavailable.selected, 'PARADE'); assert.equal(unavailable.phase, 'error'); assert.equal(unavailable.pending, null)
 report.checks.themeUnavailable = unavailable; await single(page)
 await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.navigate('/me/settings'))
 await expect(page.getByLabel('Low power', { exact: true })).toBeEnabled(); await apply(); await applied()
 await expect.poll(async () => (await engine(page)).tweenScale).toBe(0)
 await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.suspend())
 await expect.poll(async () => (await state(page)).activeSockets).toBe(0); await expect.poll(async () => (await state(page)).canvases).toBe(0)
 report.checks.privacy = { state: await state(page), engine: await engine(page) }; assert(report.checks.privacy.engine.normalMethods); assert.equal(report.checks.privacy.engine.shellMotionAttribute, initial.shellMotionAttribute)
 await capture(page, 'settings-privacy-withdrawn.png', 'Privacy withdrawal clears private presentation and restores the actual incumbent effects')
 await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.dispose())
 await expect.poll(async () => (await page.evaluate(() => window.__WORLD_PREFERENCES_ROOT__.inspectWorld())).games).toBe(0)
 const final = await state(page)
 assert.equal(final.activeSockets, 0); assert.equal(final.canvases, 0); assert.deepEqual([...final.urls.created].sort(), [...final.urls.revoked].sort())
 report.checks.final = final; assert.deepEqual(report.errors, [])
 await context.close(); context = null; await server.close(); server = null
 await rm(profile, { recursive: true, force: true }); await rm(resolve(temporary, 'vite-cache'), { recursive: true, force: true })
 await build({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'build-cache'), logLevel: 'error', build: { target: 'es2022', outDir: resolve(temporary, 'application'), copyPublicDir: false } })
 report.applicationBuild = { result: 'PASS', entry: 'frontend/index.html/src/main.tsx', afterBrowserServerProfileCleanup: true, copyPublicDir: false }
 await budget(); assert.equal(resourceFailure, null); report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.error = String(error.stack ?? error); process.exitCode = 1 }
finally {
 clearInterval(monitorTimer); await monitorFlight
 if (resourceFailure) { report.status = 'FAIL'; report.resourceFailure = resourceFailure; process.exitCode = 1 }
 const closed = await Promise.allSettled([context?.close(), server?.close()])
 await rm(temporary, { recursive: true, force: true })
 report.cleanup = { browserClosed: closed[0].status === 'fulfilled', serverClosed: closed[1].status === 'fulfilled', temporaryRemoved: true }
 if (closed.some(x => x.status === 'rejected')) { report.status = 'FAIL'; process.exitCode = 1 }
 report.peakMeasuredWorkflowBytes = peak
 report.minimumFreeBytes = minimumFreeBytes
 report.completedAtUTC = new Date().toISOString(); report.completedAtKST = kst(new Date())
 report.resourceLimits = { expectedMiB: 160, earlyStopGiB: .18, hardGiB: .2, accumulatedDurableMiB: 40, launchFreeGiB: 16, stopFreeGiB: 8, borrowedLinkIncluded: true }
 await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
 console.log(JSON.stringify({ status: report.status, source, captures: report.captures.length, cleanup: report.cleanup }))
}
