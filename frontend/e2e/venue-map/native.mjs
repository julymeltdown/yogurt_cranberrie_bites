// Actual main/Auth/World with explicit TEST_ONLY route/provider injection. No production mount claim.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { readFile, writeFile, mkdir, statfs } from 'node:fs/promises'
import { resolve, dirname, extname } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../..'), frontend = resolve(root, 'frontend')
const output = process.env.MCR_VENU_OUTPUT, temporary = process.env.MCR_VENU_TMP, lease = process.env.MCR_VENU_LEASE, port = Number(process.env.MCR_VENU_PORT)
const deadline = Number(process.env.MCR_VENU_ACCEPT_EPOCH) + Number(process.env.MCR_VENU_WORK_SECONDS)
assert(output?.startsWith(root + '/docs/mcr/evidence/paradise-city-map/') && temporary?.startsWith('/private/tmp/mcr-paradise-city-map-') && lease && port > 1023 && Number.isFinite(deadline))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const candidate = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()
const readset = JSON.parse(await readFile(resolve(root, 'docs/mcr/evidence/paradise-city-map/readset.json'), 'utf8'))
const artCommit = process.env.MCR_VENU_ART_COMMIT
assert(artCommit, 'Exact actual ART input commit required; no guessed asset')
const artPath = 'frontend/public/venue/paradise-city/v1/siteplan-photo-5.jpg'
const art = execFileSync('git', ['show', `${artCommit}:${artPath}`], { cwd: root, maxBuffer: 1024 * 1024 })
assert.equal(hash(art), 'd0713633289f87fc98603927f7cbc02df2054e989f6360332a93476d6c9da4e0')
const manifestPath = 'frontend/public/venue/paradise-city/v1/asset-interface.json'
const manifestBytes = execFileSync('git', ['show', `${artCommit}:${manifestPath}`], { cwd: root })
assert.equal(hash(manifestBytes), 'a29a47add30fe9fe9cd20dcab5f5dd5004cf1290106794fb407991f1c5142dcd')
const spatialCommit = '4f94914d06d54c3b465930cc23d40be91a1255cb', spatialPrefix = 'frontend/src/features/paradise-city-map/spatial/'
const spatialHashes = { 'contract.ts': 'b75f7cf748949a1a858626fe098f3d1b0dd22e65eeea31d551b37fa309745a42', 'image-resource.ts': '6d5e0f7c607d8ae17f03dd3030aa52e7c57d2f0ff1377d9384c6bed2554aba18', 'model.ts': '370023d8a25d135fce56969c5b5e2e8a20bfdd17e92eb0ab24da04b8aae219bd', 'registration.ts': '75c57f43a81ecaeab1cd9b0dad86329744516285501137fa5d4abb8ff25fe314', 'viewport.ts': '19489edc14c739050c3f4c6a8b32d76156f059f7620eeaa155148ff4a5a193f9' }
const spatialModules = new Map()
for (const [name, sha256] of Object.entries(spatialHashes)) { const bytes = execFileSync('git', ['show', `${spatialCommit}:${spatialPrefix}${name}`], { cwd: root }); assert.equal(hash(bytes), sha256); spatialModules.set(name, bytes.toString('utf8')) }
const schema = JSON.parse(await readFile(resolve(root, 'contracts/assets/v2/reviewed/avatar-manifest.schema.json'), 'utf8'))
const participant = JSON.parse(await readFile(resolve(root, 'contracts/fixtures/core/bootstrap-open.json'), 'utf8')).participant
const durable = { avatar: { revision: 7, config: schema.$defs.ReviewedMotionAvatarManifest.const.presets[0].config }, participantRevision: participant.revision, reads: 0, writes: [] }
const input = { revision: 'TEST_ONLY_PARADISE_CITY_not_C05_authority', reviewedBy: 'Synthetic actual-App fixture', movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
const report = { candidate, lease, qualification: 'ACTUAL_MAIN_AUTH_WORLD_WITH_TEST_ONLY_VENUE_COMPOSITION_NOT_PRODUCTION_OR_LIVE', status: 'IN_PROGRESS', artInput: { commit: artCommit, path: artPath, sha256: hash(art), bytes: art.length, qualification: 'EXACT_PRODUCER_INPUT_NOT_PRODUCER_QUALIFICATION' }, spatialInput: { commit: spatialCommit, hashes: spatialHashes, classification: 'ACTUAL_PRODUCER_INPUT_NO_PEER_PREFIX_COPY' }, injections: [], cases: [], errors: [], limits: ['Synthetic venue capabilities, people, notes and emulated geolocation; no physical-device or service acceptance.', 'Borrowed meeting entry only; actual developer204 invitation/conversation leaf integration remains separate.', 'TEST_ONLY clock profile feeds the original canonical bootstrap timeline; no second loader or timeline.', 'Reference ROI/capture overlays and personal visual iteration remain separate acceptance evidence.'] }
const publicPaths = new Set(execFileSync('git', ['ls-tree', '-r', '--name-only', candidate, 'frontend/public'], { cwd: root, encoding: 'utf8' }).trim().split('\n'))
const replacements = {
 'frontend/src/main.tsx': [["import { createAppRouter } from './app/router'", "import { createVenueFixtureRouter as createAppRouter } from '/e2e/venue-map/composition'"]],
 'frontend/src/app/WorldShell.tsx': [
  ['<ProductionWorldFeatures privacyScope={privacyScope}>', '<VenueFixtureBoundary auth={auth} scope={privacyScope} binding={community}><ProductionWorldFeatures privacyScope={privacyScope}>'],
  ['</ProductionWorldFeatures></CommunityFeatures>', '</ProductionWorldFeatures></VenueFixtureBoundary></CommunityFeatures>'],
 ],
}
function transform(code, path) {
 const expected = readset.pins.find(row => row.path === path); assert(expected, 'Preimage must be frozen: ' + path); assert.equal(hash(code), expected.sha256, 'Actual preimage drift: ' + path)
 const preimage = code
 for (const [before, after] of replacements[path]) { assert.equal(code.split(before).length - 1, 1, 'Exact single hunk: ' + path); code = code.replace(before, after) }
 if (path.endsWith('WorldShell.tsx')) code = "import { VenueFixtureBoundary } from '/e2e/venue-map/composition'\n" + code
 const patch = { path, preimageSHA256: hash(preimage), postimageSHA256: hash(code), classification: 'VIRTUAL_TEST_ONLY_NO_SOURCE_FILE_WRITE', replacements: replacements[path], importAdded: path.endsWith('WorldShell.tsx') }
 if (!report.injections.some(row => row.path === path)) report.injections.push(patch)
 return code
}
async function guard() { const disk = await statfs(root); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3); assert(Date.now() / 1000 < deadline) }
let server, browser, context, diagnosticPage
await mkdir(output, { recursive: true })
try {
 await guard()
 server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'vite'), logLevel: 'error',
 define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'), 'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)) },
 plugins: [{ name: 'paradise-city-actual-app-test-only', enforce: 'pre',
 resolveId(source, importer) {
   const virtual = '\0paradise-spatial:'
   if (importer?.startsWith(virtual) && source.startsWith('./')) { const name = source.slice(2).replace(/\.ts$/, '') + '.ts'; if (spatialModules.has(name)) return virtual + name }
   const absolute = source.startsWith('/src/') ? resolve(frontend, '.' + source) : importer && source.startsWith('.') ? resolve(dirname(importer.split('?')[0]), source) : source
   if (absolute.startsWith(resolve(root, spatialPrefix) + '/')) { const name = absolute.slice(resolve(root, spatialPrefix).length + 1).replace(/\.ts$/, '') + '.ts'; if (spatialModules.has(name)) return virtual + name }
 },
 load(id) { if (id.startsWith('\0paradise-spatial:')) return spatialModules.get(id.slice('\0paradise-spatial:'.length)) },
 transform(code, id) { const path = id.split('?')[0].slice(root.length + 1); if (replacements[path]) return transform(code, path) },
 configureServer(instance) { instance.middlewares.use(async (req, res, next) => {
   const pathname = new URL(req.url, 'http://localhost').pathname
   if (pathname === '/__reviewed_persistence_motion/state') { durable.reads++; res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store'); res.end(JSON.stringify(durable)); return }
   const path = 'frontend/public' + decodeURIComponent(pathname)
   if (path === artPath || path === manifestPath || publicPaths.has(path)) { const bytes = path === artPath ? art : path === manifestPath ? manifestBytes : execFileSync('git', ['show', `${candidate}:${path}`], { cwd: root, maxBuffer: 64 * 1024 ** 2 }); const mime = { '.jpg': 'image/jpeg', '.png': 'image/png', '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.json': 'application/json' }[extname(path)]; if (mime) res.setHeader('Content-Type', mime); res.end(bytes); return }
   if (!req.headers.accept?.includes('text/html')) return next()
   res.setHeader('Content-Type', 'text/html'); res.end(await instance.transformIndexHtml(req.url, await readFile(resolve(import.meta.dirname, 'preview.html'), 'utf8')))
 }) } }], server: { host: '127.0.0.1', port, strictPort: true, watch: null } })
 await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
 for (const [width, height] of [[360, 800], [390, 844], [430, 932], [390, 420]]) {
  await guard(); context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul', serviceWorkers: 'block', permissions: ['geolocation'], geolocation: { latitude: 37.437, longitude: 126.459, accuracy: 15 } })
  const page = await context.newPage(); diagnosticPage = page; page.setDefaultTimeout(10000); page.on('pageerror', e => report.errors.push(e.message)); await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
  const item = { width, height, dpr: 1, status: 'IN_PROGRESS', captures: [], componentCaptures: [], checks: [] }; report.cases.push(item)
  const fixture = () => page.evaluate(() => window.__PARADISE_CITY_FIXTURE__.state())
  const world = () => page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.state())
  async function shot(state) {
   const file = `${width}-${height}-${state}.png`; await page.screenshot({ path: resolve(output, file), fullPage: false }); item.captures.push({ state, file, sha256: hash(await readFile(resolve(output, file))) })
   const selectors = { friends: '.pc-friend-row', notes: '.pc-note-card', 'note-composer': '.pc-composer' }
   if (width === 390 && height === 844 && selectors[state]) {
    const component = page.locator(selectors[state]).first(), componentFile = `${width}-${height}-${state}-component.png`
    await component.screenshot({ path: resolve(output, componentFile) })
    item.componentCaptures.push({ state, selector: selectors[state], file: componentFile, sha256: hash(await readFile(resolve(output, componentFile))), box: await component.boundingBox(), viewport: { width, height }, dpr: await page.evaluate(() => devicePixelRatio), clock: (await fixture()).clockFromOriginalBinding, camera: await page.locator('.pc-map-viewport').evaluate(e => ({ left: e.scrollLeft, top: e.scrollTop, width: e.clientWidth, height: e.clientHeight })), classification: 'CONTROLLED_FIXTURE_EMULATED_GPS_NOT_LIVE' })
   }
  }
  await page.goto(`http://127.0.0.1:${port}/login`); await page.getByLabel('Email', { exact: true }).fill('venue-fixture@example.test'); await page.getByLabel('Password', { exact: true }).fill('fixture-password'); await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  await expect.poll(async () => (await world()).active).toBe(1); await expect(page.locator('.shell-world-host canvas')).toHaveCount(1)
  const host = page.locator('[data-world-host]'); await host.evaluate(e => Reflect.set(e, '__venueIncumbent', 'same'))
  await page.getByRole('button', { name: 'World tools', exact: true }).click(); await page.locator('a[href="/world/map"]').click()
  await expect(page.getByRole('tab', { name: 'MAP', exact: true })).toBeVisible(); await expect(page.locator('.pc-map-plane > img')).toBeVisible(); await expect.poll(async () => page.locator('.pc-map-plane > img').evaluate(e => e.complete && e.naturalWidth === 1024)).toBe(true)
  await expect(page.getByText('SYNTHETIC EXAMPLE · positions, people and times are not live.', { exact: true })).toBeVisible(); assert.equal((await fixture()).calls.length, 0); assert.equal((await fixture()).siteplan.landmarks.length, 22); assert.equal(await page.locator('.pc-map-plane > img').evaluate(e => e.currentSrc.startsWith('blob:')), true); item.siteplan = (await fixture()).siteplan; await shot('map-overview')
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
  const plane = page.locator('.pc-map-plane'), before = await plane.boundingBox(); await page.getByRole('button', { name: 'Zoom in', exact: true }).click(); const zoomed = await plane.boundingBox(); assert(zoomed.width > before.width)
  const viewport = page.getByRole('region', { name: 'Paradise City siteplan; arrow keys or drag to pan', exact: true })
  await viewport.focus(); await viewport.press('Home'); await viewport.press('ArrowDown'); await expect.poll(() => viewport.evaluate(e => e.scrollTop)).toBeGreaterThan(0)
  await viewport.press('ArrowRight'); await expect.poll(() => viewport.evaluate(e => e.scrollLeft)).toBeGreaterThan(0)
  await viewport.press('End'); const landmark = page.locator('[data-landmark-id="main-gate"]'); await landmark.focus(); await expect(landmark).toBeFocused()
  const v = await viewport.boundingBox(), marker = await landmark.boundingBox(); assert(marker.x >= v.x && marker.y >= v.y && marker.x + marker.width <= v.x + v.width && marker.y + marker.height <= v.y + v.height)
  await viewport.focus(); await viewport.press('Home'); await viewport.press('ArrowDown')
  const dragBefore = await viewport.evaluate(e => e.scrollTop), dragBox = await viewport.boundingBox()
  await page.mouse.move(dragBox.x + 12, dragBox.y + dragBox.height / 2); await page.mouse.down(); await page.mouse.move(dragBox.x + 12, dragBox.y + dragBox.height / 2 - 24, { steps: 6 }); await page.mouse.up()
  await expect.poll(() => viewport.evaluate(e => e.scrollTop)).toBeGreaterThan(dragBefore); item.checks.push('keyboard-pan-home-end-landmark-focus-reveal-and-pointer-pan')
  if (width === 390 && height === 844) {
   await viewport.focus(); await viewport.press('Home'); const touchBox = await viewport.boundingBox(), cdp = await context.newCDPSession(page)
   const x = touchBox.x + 16, y = touchBox.y + touchBox.height * .75
   await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y, id: 1 }] })
   for (const dy of [20, 40, 70, 100]) await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x, y: y - dy, id: 1 }] })
   await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] }); await expect.poll(() => viewport.evaluate(e => e.scrollTop)).toBeGreaterThan(0); await cdp.detach(); item.checks.push('native-browser-touch-pan')
  }
  await page.getByRole('button', { name: 'Fit complete siteplan', exact: true }).click(); await page.getByRole('button', { name: 'Reset map view', exact: true }).click(); item.checks.push('zoom-fit-reset')
  await page.getByRole('tab', { name: 'MAP', exact: true }).focus(); await page.getByRole('tab', { name: 'MAP', exact: true }).press('ArrowRight'); await expect(page.getByRole('tab', { name: 'FRIENDS', exact: true })).toBeFocused(); await expect(page.getByRole('tab', { name: 'FRIENDS', exact: true })).toHaveAttribute('aria-selected', 'true'); item.checks.push('roving-tab-keyboard-selection')
  await page.getByRole('tab', { name: 'FRIENDS', exact: true }).click(); await shot('friends')
  await page.getByRole('button', { name: 'inkparade', exact: true }).click(); await expect(page.getByRole('dialog', { name: 'inkparade' })).toBeVisible(); await shot('friend-selected'); await page.getByRole('button', { name: 'MESSAGE', exact: true }).click(); await expect.poll(async () => (await fixture()).opens.length).toBe(1); assert.equal((await fixture()).calls.length, 0)
  await page.getByRole('button', { name: 'Close friend details', exact: true }).click()
  await page.evaluate(() => window.__PARADISE_CITY_FIXTURE__.noLocation()); await page.getByRole('button', { name: 'inkparade', exact: true }).click(); await expect(page.getByRole('button', { name: 'MESSAGE', exact: true })).toBeEnabled(); await expect(page.getByRole('button', { name: 'MEET HERE', exact: true })).toBeDisabled(); await page.getByRole('button', { name: 'MESSAGE', exact: true }).click(); assert.equal((await fixture()).opens.at(-1).landmark, undefined); item.checks.push('authorized-message-without-location-landmark-invitation')
  await page.getByRole('button', { name: 'Close friend details', exact: true }).click(); await page.getByRole('tab', { name: 'NOTES', exact: true }).click(); await shot('notes'); await page.getByRole('button', { name: '+ NEW NOTE', exact: true }).click(); await page.getByRole('textbox', { name: 'Note text', exact: true }).fill('Explicit synthetic note'); await shot('note-composer'); await page.getByRole('button', { name: '+ POST NOTE', exact: true }).click(); await expect(page.getByText('Explicit synthetic note', { exact: true })).toBeVisible(); assert.equal((await fixture()).calls.filter(c => c.name === 'postNote').length, 1); item.checks.push('one-explicit-note-write')
  const note = page.locator('.pc-note-card').filter({ has: page.getByText('Explicit synthetic note', { exact: true }) })
  await note.getByRole('button', { name: 'REPLY', exact: true }).click(); await expect(page.getByRole('combobox', { name: 'Note place', exact: true })).toBeDisabled(); await expect(page.getByRole('button', { name: 'EVERYONE', exact: true })).toHaveCount(0)
  await page.getByRole('textbox', { name: 'Reply text', exact: true }).fill('Explicit synthetic reply'); await page.getByRole('button', { name: 'POST REPLY', exact: true }).click(); await expect(page.getByText('Explicit synthetic reply', { exact: true })).toBeVisible(); assert.equal((await fixture()).calls.filter(c => c.name === 'reply').length, 1); item.checks.push('one-explicit-reply-inherits-parent-place-and-audience')
  await page.getByRole('button', { name: 'Share location', exact: true }).click(); const consent = page.getByRole('button', { name: 'Allow phone location', exact: true }); await expect(consent).toBeDisabled(); await page.getByRole('checkbox', { name: 'I want to share my phone location.', exact: true }).check(); await consent.click(); await expect.poll(async () => (await fixture()).calls.filter(c => c.name === 'publishPhoneFix').length).toBeGreaterThan(0); await page.getByRole('button', { name: 'Pause', exact: true }).click(); await expect(page.getByText('Location sharing paused', { exact: true })).toBeVisible(); item.checks.push('explicit-emulated-geolocation-opt-in-and-pause')
  await page.evaluate(() => window.__PARADISE_CITY_FIXTURE__.withdraw()); await expect(page.getByText('Explicit synthetic note', { exact: true })).toHaveCount(0); await expect(page.getByText(/Live location, check-in, meetings and notes are unavailable/)).toBeVisible(); await shot('unavailable'); item.checks.push('private-withdrawal-scrub')
  assert.equal((await world()).canvases, 1); assert.equal((await world()).sockets, 1); assert.equal(await host.evaluate(e => Reflect.get(e, '__venueIncumbent')), 'same'); item.checks.push('one-incumbent-world-and-socket')
  await page.getByRole('button', { name: 'Back to previous panel', exact: true }).click(); await expect(page).toHaveURL(/\/world(?:\?|$)/); item.status = 'PASS'; await context.close(); context = null
 }
 assert.deepEqual(report.errors, []); assert.equal(report.injections.length, 2); report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.failure = String(error.stack ?? error); if (diagnosticPage && !diagnosticPage.isClosed()) await diagnosticPage.screenshot({ path: resolve(output, 'failure.png'), fullPage: false }).catch(() => {}) }
finally { await context?.close().catch(() => {}); await browser?.close().catch(() => {}); await server?.close().catch(() => {}); await writeFile(resolve(output, 'native-report.json'), JSON.stringify(report, null, 2) + '\n') }
if (report.status !== 'PASS') process.exitCode = 1
