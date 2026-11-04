// Native Chrome qualification. All authority in these pages is explicitly synthetic.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, readdir, writeFile, statfs, realpath } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { createServer as createNetServer } from 'node:net'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const root = resolve(import.meta.dirname, '../../../..'), frontend = resolve(root, 'frontend')
const output = resolve(root, 'docs/mcr/evidence/accessibility/qa07', process.env.QA07_RUN || 'current')
assert(/^[a-z0-9-]+$/.test(process.env.QA07_RUN || 'current'))
const disk = await statfs(root); assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, '8GiB disk floor')
await mkdir(output, { recursive: true })
const pins = {}
async function collect(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const file = resolve(dir, entry.name)
    if (entry.isDirectory()) { if (!['evidence', 'node_modules', '.cache'].includes(entry.name) && !entry.name.endsWith('-evidence')) await collect(file) }
    else if (/\.(tsx?|css|html|json|mjs)$/.test(file)) pins[relative(root, file)] = createHash('sha256').update(await readFile(file)).digest('hex')
  }
}
for (const dir of ['frontend/src', 'frontend/e2e/accessibility/qa07', 'contracts/domain/v1', 'contracts/realtime/v1', 'contracts/assets', 'contracts/fixtures/domain', 'contracts/fixtures/realtime']) await collect(resolve(root, dir))
for (const path of ['MCR_IMPLEMENTATION_PLAN_V2_EN.md', 'contracts/generated/mcr-domain.openapi.json', 'frontend/pnpm-lock.yaml']) pins[path] = createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')
await writeFile(resolve(output, 'input-pins.json'), JSON.stringify(pins, null, 2) + '\n')
const dependencyRoot = await realpath(resolve(frontend, 'node_modules'))
const sanitize = value => value.replaceAll(root + '/', '').replaceAll(dependencyRoot + '/', 'frontend/node_modules/').replaceAll(resolve(dependencyRoot, '..', '..') + '/node_modules/', 'frontend/node_modules/')
const report = { candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), qualification: 'MOCK_READY_NATIVE_CLIENT_FIXTURES', runtime: { node: process.version, platform: process.platform, arch: process.arch, headless: true }, selection: { widths: process.env.QA07_WIDTH ? [Number(process.env.QA07_WIDTH)] : [360, 390, 430], only: process.env.QA07_ONLY || null }, browser: null, cases: [], limits: ['Synthetic HTTP/WS, consent, pickup place/admission authority; no live server or two-principal journey', 'Native headless macOS Chrome at CSS widths, not physical iOS/Android or hardware keyboard', 'Browser accessibility tree and DOM labels, not real screen-reader announcements', 'All original 112 tasks/61 IA remain required; this is original QA07 evidence only'] }
const reservation = createNetServer()
await new Promise(done => reservation.listen(0, '127.0.0.1', done))
const port = reservation.address().port
await new Promise(done => reservation.close(done))
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(import.meta.dirname, '.cache'), logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
let browser, origin
const own = '/e2e/accessibility/qa07/fixture.html'
// Reach controls through actual Tab traversal, without focus(), click(), fill() or canvas coordinates.
async function tabTo(page, target) {
  await expect(target).toBeVisible(); await expect(target).toBeEnabled()
  const trail = []
  for (let i = 0; i < 70; i++) {
    const current = await target.evaluate(el => {
      const stops = [...document.querySelectorAll('button,input,select,textarea,a[href],summary,[tabindex="0"]')].filter(n => n.getClientRects().length && !n.closest('[inert]') && !n.disabled)
      return { focused: el === document.activeElement, before: stops.indexOf(el) < stops.indexOf(document.activeElement), active: document.activeElement?.outerHTML.slice(0, 180) }
    }, undefined, { timeout: 2000 })
    if (current.focused) return
    trail.push(current.active)
    await page.keyboard.press(current.before ? 'Shift+Tab' : 'Tab')
  }
  throw Error('Target unreachable through Tab: ' + await target.ariaSnapshot() + JSON.stringify(trail))
}
async function activate(page, target, key = 'Enter') { await tabTo(page, target); await page.keyboard.press(key) }
async function type(page, target, value) { await tabTo(page, target); await page.keyboard.press('ControlOrMeta+A'); await page.keyboard.type(value) }
const button = (p, name) => p.getByRole('button', { name, exact: true })
async function cancelTouch(page, target) {
  await target.scrollIntoViewIfNeeded(); const box = await target.boundingBox(); assert(box)
  const session = await page.context().newCDPSession(page)
  await page.evaluate(() => { window.__QA07_CANCELS__ = []; document.addEventListener('pointercancel', e => window.__QA07_CANCELS__.push({ trusted: e.isTrusted, type: e.pointerType }), { once: true }) })
  await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: box.x + box.width / 2, y: box.y + box.height / 2 }] })
  await session.send('Input.dispatchTouchEvent', { type: 'touchCancel', touchPoints: [] })
  assert.deepEqual(await page.evaluate(() => window.__QA07_CANCELS__), [{ trusted: true, type: 'touch' }]); await session.detach()
}
async function run(name, width, work) {
  if (process.env.QA07_ONLY && !name.includes(process.env.QA07_ONLY)) return
  const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce', hasTouch: true, serviceWorkers: 'block' })
  const page = await context.newPage(), errors = [], result = { name, width, status: 'FAIL', observations: [] }
  page.on('pageerror', e => errors.push(e.message))
  try {
    await work(page, result.observations)
    assert.deepEqual(errors, [], 'uncaught browser errors')
    result.status = 'PASS'
  } catch (e) { result.failure = sanitize(e.stack || String(e)); result.pageErrors = errors.map(sanitize) }
  finally {
    await writeFile(resolve(output, `${width}-${name}.aria.txt`), await page.locator('body').ariaSnapshot().catch(String))
    await page.screenshot({ path: resolve(output, `${width}-${name}.png`), fullPage: true }).catch(() => {})
    const ax = await context.newCDPSession(page)
    const tree = await ax.send('Accessibility.getFullAXTree')
    const nodes = tree.nodes.filter(n => !n.ignored).map(n => ({ role: n.role?.value, name: n.name?.value }))
    await writeFile(resolve(output, `${width}-${name}.ax.json`), JSON.stringify(nodes, null, 2) + '\n')
    result.unnamedControls = nodes.filter(n => ['button', 'link', 'textbox', 'spinbutton', 'combobox'].includes(n.role) && !n.name)
    if (result.unnamedControls.length) { result.status = 'FAIL'; result.failure = (result.failure || '') + '\nUnnamed interactive nodes in native accessibility tree' }
    await ax.detach()
    await page.evaluate(async () => { await window.__MURAL_ASSEMBLY__?.dispose(); await window.__SCENE_ROOT__?.dispose() }).catch(() => {})
    report.cases.push(result); console.log(`${result.status} ${width} ${name}${result.failure ? ': ' + result.failure.split('\n')[0] : ''}`)
    await context.close()
  }
}
try {
  await server.listen(); origin = server.resolvedUrls.local[0].replace(/\/$/, '')
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of process.env.QA07_WIDTH ? [Number(process.env.QA07_WIDTH)] : [360, 390, 430]) {
    await run('sheet', width, async (p, notes) => {
      await p.goto(origin + own)
      const opener = button(p, 'Open pickup sheet'), dialog = p.getByRole('dialog', { name: 'Pickup sheet fixture' })
      await activate(p, opener); await expect(p.getByLabel('Pickup note')).toBeFocused()
      for (let i = 0; i < 8; i++) { await p.keyboard.press(i % 2 ? 'Shift+Tab' : 'Tab'); assert(await dialog.evaluate(el => el.contains(document.activeElement))) }
      assert(await p.locator('#root').evaluate(el => el.inert))
      const motion = await dialog.evaluate(el => ({ animation: getComputedStyle(el).animationName, transition: getComputedStyle(el).transitionDuration }))
      assert.equal(motion.animation, 'none'); assert.equal(motion.transition, '0s')
      assert.equal(await opener.evaluate(el => getComputedStyle(el).transitionDuration), '0s')
      await p.emulateMedia({ reducedMotion: 'no-preference' }); assert.match(await opener.evaluate(el => getComputedStyle(el).transitionDuration), /0\.12s/)
      await p.emulateMedia({ reducedMotion: 'reduce' }); assert.equal(await opener.evaluate(el => getComputedStyle(el).transitionDuration), '0s')
      await p.keyboard.press('Escape'); await expect(dialog).toHaveCount(0); await expect(opener).toBeFocused()
      await p.keyboard.press('Enter'); await cancelTouch(p, p.locator('.mcr-overlay')); await expect(dialog).toBeVisible()
      await activate(p, button(p, 'Done')); await expect(opener).toBeFocused()
      notes.push('Actual shared sheet initial focus, forward/reverse containment, inert background, reduced-motion computed styles, trusted touchcancel, Escape/Done opener return')
    })
    await run('safety', width, async (p, notes) => {
      await p.goto(origin + '/src/features/safety/__tests__/preview.html')
      const opener = button(p, 'Open report')
      await activate(p, opener); await expect(button(p, 'Close report')).toBeFocused()
      await expect(p.getByRole('combobox', { name: 'Reason', exact: true })).toBeVisible()
      await type(p, p.getByRole('textbox', { name: /What happened/ }), 'QA07 keyboard report draft')
      await p.keyboard.press('Escape'); await expect(opener).toBeFocused(); await expect(p.getByRole('dialog')).toHaveCount(0)
      await activate(p, button(p, 'Block user')); await expect(button(p, 'Cancel block')).toBeFocused()
      await p.keyboard.press('Shift+Tab'); await expect(button(p, 'Confirm block')).toBeFocused()
      await p.keyboard.press('Escape'); await expect(button(p, 'Block user')).toBeFocused()
      notes.push('Actual FE36 report reason/text labels, keyboard draft, native dialog Escape, block confirmation reverse focus trap and opener return; no backend report/block claim')
    })
    await run('social', width, async (p, notes) => {
      await p.goto(origin + own + '?social=1')
      await expect(button(p, 'Reveal Instagram')).toBeEnabled()
      await expect(p.getByText('@qa07_synthetic_private', { exact: true })).toHaveCount(0)
      await activate(p, button(p, 'Reveal Instagram')); await expect(p.getByRole('region', { name: 'Revealed account name' })).toContainText('@qa07_synthetic_private')
      await activate(p, button(p, 'Hide account name')); await expect(p.getByText('@qa07_synthetic_private', { exact: true })).toHaveCount(0)
      await activate(p, button(p, 'Revoke sharing')); await activate(p, button(p, 'Confirm revocation'))
      await expect(button(p, 'Reveal Instagram')).toHaveCount(0)
      notes.push('Actual public SharingRequest/controller: keyboard explicit reveal, hide and confirmed revoke without canvas; approved consent is synthetic fixture authority')
    })
    await run('safety-block', width, async (p, notes) => {
      await p.goto(origin + '/src/features/safety/__tests__/preview.html')
      const opener = button(p, 'Block user')
      await activate(p, opener); await expect(button(p, 'Cancel block')).toBeFocused()
      await p.keyboard.press('Shift+Tab'); await expect(button(p, 'Confirm block')).toBeFocused()
      await p.keyboard.press('Escape'); await expect(opener).toBeFocused()
      notes.push('Fresh actual FE36 BlockUserButton keyboard confirmation, native modal reverse-Tab wrap and Escape focus return; no block mutation')
    })
    await run('account', width, async (p, notes) => {
      await p.goto(origin + '/src/features/safety/__tests__/preview.html?unavailable=1')
      await activate(p, button(p, 'Account')); await expect(button(p, 'Request account deletion')).toBeDisabled()
      await p.goto(origin + '/src/features/safety/__tests__/preview.html')
      await activate(p, button(p, 'Account')); const opener = button(p, 'Request account deletion')
      await activate(p, opener); await expect(button(p, 'Keep account')).toBeFocused()
      await expect(button(p, 'Confirm deletion request')).toBeDisabled(); await type(p, p.getByLabel('Deletion confirmation'), 'DELETE_MY_ACCOUNT')
      await expect(button(p, 'Confirm deletion request')).toBeEnabled()
      await p.keyboard.press('Escape'); await expect(opener).toBeFocused()
      assert.equal(await p.evaluate(() => window.__FE36__.deletionCalls()), 0)
      notes.push('Actual FE36 unavailable deletion port disabled, labeled confirmation keyboard input, Keep account initial focus, Escape opener return, zero deletion calls')
    })
    await run('pickup-recipient', width, async (p, notes) => {
      await p.goto(origin + '/src/features/gift-pickup/__tests__/preview.html?ready=1')
      await activate(p, button(p, 'Read private pickup details'))
      await expect(p.getByRole('region', { name: 'Authorized pickup details' })).toContainText('Private pickup fixture instructions')
      await activate(p, button(p, 'Request pickup code')); await expect(p.getByLabel('Pickup confirmation code')).toHaveText('654321')
      await expect(p.getByText('Virtual movement does not establish physical arrival.', { exact: true })).toBeVisible()
      notes.push('Actual GiftPickup/controller: Tab/Enter private instructions and labeled code output, no canvas interaction, fixture reviewed-place authority only')
    })
    await run('pickup-reservation', width, async (p, notes) => {
      await p.goto(origin + '/src/features/gift-pickup/__tests__/preview.html')
      await activate(p, button(p, 'Confirm reservation'))
      await expect(p.getByRole('status')).toContainText('Outcome unknown')
      const writes = await p.evaluate(() => window.__FE26__.writes())
      await p.reload(); assert.equal(await p.evaluate(() => window.__FE26__.writes()), writes)
      await activate(p, button(p, 'Check original command')); await expect(p.getByText('HELD', { exact: true })).toBeVisible()
      assert.equal(await p.evaluate(() => window.__FE26__.writes()), writes)
      notes.push('Keyboard reserve -> uncertain fixture response -> real reload -> keyboard original-command recovery; no repeated write and no canvas')
    })
    await run('pickup-owner', width, async (p, notes) => {
      await p.goto(origin + '/src/features/gift-pickup/__tests__/preview.html?owner=1')
      await type(p, p.getByLabel('Recipient’s six-digit code'), '654321')
      await activate(p, button(p, 'Confirm one-time collection')); await expect(p.getByRole('heading', { name: 'Collection receipt' })).toBeVisible()
      await expect(button(p, 'Confirm one-time collection')).toHaveCount(0)
      notes.push('Actual owner GiftPickup: keyboard-only labeled code entry/collection receipt, synthetic claim response, no live collection acceptance')
    })
    await run('mural', width, async (p, notes) => {
      await p.goto(origin + '/src/world/mural-entry-lifecycle/__tests__/assembly-native.html')
      await expect(p.getByText('REV 10 · EDITABLE', { exact: true })).toBeVisible({ timeout: 20000 })
      const surface = p.getByRole('group', { name: 'Mural pixel preview', exact: true })
      await cancelTouch(p, surface); await expect(button(p, 'Confirm pixel')).toHaveCount(0)
      assert.equal((await p.evaluate(() => window.__MURAL_ASSEMBLY__.read())).posts.length, 0)
      await tabTo(p, surface); await p.keyboard.press('ArrowRight'); await expect(button(p, 'Confirm pixel')).toBeEnabled()
      await p.keyboard.press('Escape'); await expect(button(p, 'Confirm pixel')).toHaveCount(0)
      await activate(p, p.getByText('Coordinates & help', { exact: true }))
      await type(p, p.getByRole('spinbutton', { name: 'X', exact: true }), '12'); await type(p, p.getByRole('spinbutton', { name: 'Y', exact: true }), '13')
      await activate(p, button(p, 'Preview pixel')); await expect(button(p, 'Confirm pixel')).toBeEnabled()
      assert.equal((await p.evaluate(() => window.__MURAL_ASSEMBLY__.read())).posts.length, 0)
      await expect(p.getByRole('status', { name: 'Wall command status' })).toHaveAttribute('aria-live', 'polite')
      await activate(p, button(p, 'Confirm pixel')); await expect.poll(async () => (await p.evaluate(() => window.__MURAL_ASSEMBLY__.read())).posts.length).toBe(1)
      notes.push('Actual FE32 editor through frozen FE33 provider/adapter/default MapScene: trusted touchcancel makes no preview/write, keyboard arrows/Escape, labeled DOM coordinates and explicit confirmation, live status')
    })
    await run('root', width, async (p, notes) => {
      await p.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
      await p.goto(origin + '/src/app/world-entry/__tests__/reference-scene-root/preview.html?version=spatial&era=PARADE&source=derived')
      const state = () => p.evaluate(() => window.__SCENE_ROOT__?.state())
      await expect.poll(async () => (await state())?.phase, { timeout: 30000 }).toBe('live')
      await activate(p, p.getByRole('link', { name: 'Map', exact: true }).first()); await expect(p.locator('#panel-title')).toBeFocused()
      await expect(p.getByRole('img', { name: 'PARADE illustrated virtual map', exact: true })).toBeVisible()
      const view = p.locator('.places-map-viewport'); await tabTo(p, view); await p.keyboard.press('ArrowDown')
      await activate(p, button(p, 'Zoom in map view')); await activate(p, button(p, 'Fit map'))
      const places = button(p, 'Places')
      await activate(p, places); await expect(p.getByRole('dialog', { name: 'Places', exact: true })).toBeVisible()
      await type(p, p.getByRole('searchbox', { name: 'Find a place' }), 'stage')
      await expect(p.getByRole('dialog').getByRole('button', { name: 'Virtual stage', exact: true })).toBeVisible()
      await p.keyboard.press('Escape'); await expect(places).toBeFocused()
      await activate(p, button(p, 'Back to previous panel'))
      await expect.poll(async () => (await state()).path).toBe('/world')
      await expect(p.getByRole('link', { name: 'Map', exact: true }).first()).toBeFocused()
      const sockets = (await state()).sockets
      await activate(p, p.getByRole('link', { name: 'Nearby', exact: true }))
      await expect(p.getByRole('region', { name: 'You in this world' })).toBeVisible()
      await expect(p.getByRole('region', { name: 'Other people in this world' })).toBeVisible()
      await activate(p, p.getByRole('link', { name: 'Choose a room', exact: true }))
      await expect(p.getByRole('list', { name: 'Available virtual rooms' })).toBeVisible()
      await activate(p, p.getByRole('link', { name: 'Back to world', exact: true }).first())
      await expect.poll(async () => (await state()).phase).toBe('live')
      assert.equal((await state()).sockets, 1, 'A full panel must preserve the original world socket')
      notes.push('Actual Root/FE18 Nearby authorized DOM alternatives and FE19 illustrated Map keyboard controls/labels; fixture route placement for Nearby, no live room admission')
      notes.push({ socketsBeforeNearby: sockets, socketsAfterNearby: (await state()).sockets, game: await p.evaluate(() => window.__SCENE_ROOT__.inspectWorld()) })
    })
    await run('profile-return', width, async (p, notes) => {
      await p.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
      await p.goto(origin + '/src/app/world-entry/__tests__/reference-scene-root/preview.html?source=derived')
      await expect.poll(() => p.evaluate(() => window.__SCENE_ROOT__?.state().phase), { timeout: 30000 }).toBe('live')
      await p.evaluate(() => window.__SCENE_ROOT__.navigate('/me/profile'))
      await type(p, p.getByRole('textbox', { name: 'Bio (optional)', exact: true }), 'QA07 temporary keyboard draft')
      // Route unmount while the native textarea owns focus is the actual stale-typing trigger.
      await p.evaluate(() => window.__SCENE_ROOT__.navigate('/world'))
      await expect.poll(() => p.evaluate(() => window.__SCENE_ROOT__.state().phase)).toBe('live')
      await expect(button(p, 'Recenter')).toBeEnabled(); await activate(p, button(p, 'Recenter'))
      assert.equal((await p.evaluate(() => window.__SCENE_ROOT__.state())).sockets, 1)
      notes.push('Actual Root ProfileForm native typing then focused textarea route unmount: World returns live with reachable enabled Recenter and original socket; fixture route trigger, not OS keyboard')
    })
  }
} finally {
  await browser?.close(); await server.close()
  report.status = report.cases.length && report.cases.every(c => c.status === 'PASS') ? 'PASS' : 'FAIL'
  await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n')
}
if (report.status !== 'PASS') process.exitCode = 1
