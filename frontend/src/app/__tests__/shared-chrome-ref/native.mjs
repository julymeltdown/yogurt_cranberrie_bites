// CSS-SHARED-CHROME-REF-01. Existing actual router/Auth/Phaser, synthetic HTTP/C05.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const repo = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(repo, 'frontend')
const output = process.env.MCR_SHARED_OUTPUT, temporary = process.env.MCR_SHARED_TEMP
const job = process.env.MCR_SHARED_JOB, port = Number(process.env.MCR_SHARED_PORT)
const paths = (process.env.MCR_SHARED_PATHS ?? 'gifts,meet,me').split(',').map(name => '/' + name)
assert(paths.length && paths.every(path => ['/gifts', '/meet', '/me'].includes(path)))
assert(job && output?.startsWith(repo + '/docs/mcr/evidence/css-shared-chrome-ref-01/'))
assert(temporary?.startsWith('/private/tmp/mcr-shared-chrome-') && port > 1024)
assert.equal(process.version, 'v24.22.2')
const base = 'f339f6240db94353e990fed722da0feb598ae9d2'
const seam = '61f3accd901fa93d1d558b14e4f45abc99948148'
const css = ['frontend/src/app/reference-chrome-v2/chrome.css', 'frontend/src/app/shell.css']
const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 32 * 1024**2 })
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const reference = await readFile(resolve(repo, 'docs/mcr/design/reference-target-2026-11-04.png'))
const rootPostimage = git('show', seam + ':frontend/src/app/WorldShell.tsx').toString()
const readset = [...css, 'frontend/src/app/WorldShell.tsx', 'frontend/src/app/shell-chrome/index.tsx',
  'frontend/src/app/shell-chrome/shell-chrome.css', 'frontend/src/app/shell-chrome/parade-footer.tsx',
  'frontend/src/app/community-adoption/__tests__/native/preview.tsx',
  'frontend/src/shared/styles/cozy-community-backgrounds-v1.css',
  'frontend/src/shared/styles/cozy-identity-backgrounds-v1.css', 'frontend/pnpm-lock.yaml']
const report = { task: 'CSS-SHARED-CHROME-REF-01', job, base, candidate: git('rev-parse', 'HEAD').toString().trim(),
  rootOwnedDom: { commit: seam, path: 'frontend/src/app/WorldShell.tsx', sha256: hash(rootPostimage),
    consumption: 'Read-only Vite pre-load postimage; never edited or staged by this owner.' },
  reference: { path: 'docs/mcr/design/reference-target-2026-11-04.png', sha256: hash(reference) },
  scope: 'Shared chrome on actual default router. Synthetic existing Auth/HTTP/C05/clock fixture; not live/device/full visual acceptance.',
  startedAt: new Date().toISOString(), readset: {}, cases: [], errors: [] }
for (const path of readset) report.readset[path] = { current: hash(await readFile(resolve(repo, path))), baseline: hash(git('show', base + ':' + path)) }
assert.equal(report.readset['frontend/src/app/WorldShell.tsx'].current, report.readset['frontend/src/app/WorldShell.tsx'].baseline)
await mkdir(output, { recursive: true })
let server, browser
const origin = `http://127.0.0.1:${port}`
const fixture = '/src/app/community-adoption/__tests__/native/preview.html'
async function start(mode) {
  server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, mode), logLevel: 'error',
    resolve: { dedupe: ['react', 'react-dom'] }, plugins: [{ name: 'shared-chrome-qualified-readset', enforce: 'pre', load(id) {
      const path = id.split('?')[0]
      if (mode === 'candidate' && path === resolve(frontend, 'src/app/WorldShell.tsx')) return rootPostimage
      const owned = css.find(item => resolve(repo, item) === path)
      if (mode === 'baseline' && owned) return git('show', base + ':' + owned).toString()
    } }], server: { host: '127.0.0.1', port, strictPort: true, hmr: false } })
  await server.listen()
}
async function stop() { await server?.close(); server = undefined }
async function capture(page, name) {
  const bytes = await page.screenshot({ path: resolve(output, name), fullPage: true })
  return { path: name, sha256: hash(bytes), bytes: bytes.length }
}
async function hit(locator) {
  await locator.scrollIntoViewIfNeeded()
  assert(await locator.evaluate(element => { const r = element.getBoundingClientRect(); const top = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return element === top || element.contains(top) }), 'Control center must be reachable')
}
async function state(page) { return page.evaluate(() => window.__COMMUNITY_AUTHORITY_NATIVE__.state()) }
async function open(page, path, scenario = 'enabled') {
  await page.goto(origin + fixture + '?' + new URLSearchParams({ entry: '/world', era: 'PARADE', scenario }))
  await expect(page.locator('[data-mcr-shell]')).toBeVisible()
  if (scenario !== 'world-only') await expect.poll(async () => (await state(page)).published, { timeout: 25000 }).toBe(true)
  // Use genuine pointer navigation: normal heading focus stays with the router.
  await page.locator(`.mcr-chrome-bottom a[href='${path}']`).click()
  await expect(page.locator('.shell-panel')).toBeVisible()
  await expect.poll(async () => (await state(page)).path).toBe(path)
  if (path === '/gifts' && scenario === 'enabled') await expect(page.locator('.gift-reference-row')).toHaveCount(3)
  if (path === '/meet') await expect(page.locator('.meet-parties > li').first()).toBeVisible()
  if (path === '/me') await expect(page.locator('.me-identity')).toBeVisible()
  if (path === '/gifts' && scenario === 'no-policy') await expect(page.locator('.shell-panel-body')).toContainText(/unavailable|could not|unable|not available/i)
  await page.evaluate(() => document.fonts.ready)
}
async function geometry(page) {
  return page.evaluate(() => {
    const box = element => element?.getBoundingClientRect().toJSON() ?? null
    const panel = document.querySelector('.shell-panel'), header = panel.querySelector('.mcr-chrome-panel-header'), body = panel.querySelector('.shell-panel-body')
    const details = body.querySelector('.shell-more-sections'), content = body.querySelector('.gifts-reference,.party-browse,.me-hub')
    return { viewport: { width: innerWidth, height: innerHeight }, overflow: document.documentElement.scrollWidth > innerWidth,
      shell: box(document.querySelector('.world-shell')), header: box(header), body: box(body), title: box(header.querySelector('h1')),
      headerPaint: getComputedStyle(header).backgroundColor, background: getComputedStyle(body).background,
      titleText: header.querySelector('h1').textContent, copy: body.innerText,
      controls: [...header.querySelectorAll('button')].map(element => ({ label: element.getAttribute('aria-label'), ...box(element) })),
      moreActions: box(details), content: box(content),
      actionsAfterContent: details && content ? !!(content.compareDocumentPosition(details) & Node.DOCUMENT_POSITION_FOLLOWING) : null,
      footer: box(body.querySelector('.mcr-chrome-parade-footer')),
      artwork: [...body.querySelectorAll('canvas')].map(canvas => ({ width: canvas.width, height: canvas.height, pixels: canvas.toDataURL() })) }
  })
}
function check(g) {
  assert(!g.overflow, 'No horizontal page overflow')
  assert(g.header.height >= 44 && g.header.height <= 51, 'Compact header retains touch geometry')
  assert(g.title.width > 80 && g.title.height > 20, 'Heading has readable space')
  for (const control of g.controls) assert(control.width >= 44 && control.height >= 44, '44px header target: ' + control.label)
}
function stripArtwork(g) { g.artwork = g.artwork.map(item => ({ ...item, pixels: hash(item.pixels) })); return g }
async function focus(page) {
  const heading = page.locator('#panel-title'), reduce = page.getByRole('button', { name: 'Reduce panel', exact: true })
  // Router headings intentionally have tabindex=-1. Establish keyboard modality,
  // then qualify their programmatic destination focus rather than adding them
  // to the sequential tab order or expecting reverse Tab to enter them.
  await page.keyboard.press('Tab'); await heading.focus(); await expect(heading).toBeFocused()
  const result = await heading.evaluate(element => { const s = getComputedStyle(element); return { style: s.outlineStyle, width: s.outlineWidth, color: s.outlineColor, offset: s.outlineOffset, background: getComputedStyle(element.parentElement).backgroundColor } })
  assert(result.style !== 'none' && parseFloat(result.width) >= 2, 'Qualified title focus retained')
  await page.keyboard.press('Tab'); await expect(reduce).toBeFocused()
  await page.keyboard.press('Tab'); await expect(page.getByRole('button', { name: 'Minimize panel', exact: true })).toBeFocused()
  await page.keyboard.press('Shift+Tab'); await expect(reduce).toBeFocused()
  await heading.focus()
  return result
}
async function run(mode, width, path) {
  const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
  const page = await context.newPage(), name = path.slice(1), entry = { mode, width, path, errors: [] }
  page.setDefaultTimeout(12000); page.on('pageerror', error => entry.errors.push(error.message))
  try {
    await open(page, path); entry.geometry = stripArtwork(await geometry(page)); check(entry.geometry)
    assert(entry.geometry.content, 'Feature content must be ready before visual qualification')
    const world = await state(page); assert.equal(world.canvases, 1); assert(world.activeSockets <= 1); entry.world = { canvases: world.canvases, activeSockets: world.activeSockets, avatarPin: world.avatarPin }
    entry.capture = await capture(page, `${mode}-${name}-${width}.png`)
    if (mode === 'candidate') {
      const before = report.cases.find(item => item.mode === 'baseline' && item.path === path && item.width === width)
      assert.equal(entry.geometry.background, before.geometry.background, 'Cute background unchanged')
      assert.deepEqual(entry.geometry.artwork, before.geometry.artwork, 'Visible character pixels unchanged')
      if (entry.geometry.actionsAfterContent !== null) assert(entry.geometry.actionsAfterContent, 'More actions follows feature in DOM order')
      for (const control of await page.locator('.mcr-chrome-panel-header > button').all()) await hit(control)
      if (path === '/gifts') {
        assert(entry.geometry.content.y < before.geometry.content.y - 35, 'Tabs/cards recover the pre-content disclosure space')
        await page.setViewportSize({ width, height: 568 }); check(stripArtwork(await geometry(page)))
        const more = page.locator('.shell-more-sections > summary'); await more.click()
        for (const link of await page.locator('.shell-more-sections .shell-section-links a').all()) {
          await hit(link); const r = await link.boundingBox(); assert(r.width >= 44 && r.height >= 44)
        }
        entry.short = await capture(page, `short-actions-${width}.png`); await more.click()
        if (width === 360) {
          await page.setViewportSize({ width, height: 480 })
          await page.getByText('Filter giveaways', { exact: true }).click()
          const input = page.locator('input[type="datetime-local"]').first(); await input.fill('2026-11-07T15:30'); await hit(input); await expect(input).toBeFocused()
          entry.input = { value: await input.inputValue(), capture: await capture(page, 'short-input-360.png'), viewportResize: true, physicalKeyboard: 'NOT_RUN' }
        }
        if (width === 390) {
          await open(page, path); entry.focus = await focus(page); entry.focusCapture = await capture(page, 'title-focus-390.png')
          await page.emulateMedia({ forcedColors: 'active' }); entry.forcedFocus = await focus(page); entry.forcedCapture = await capture(page, 'title-forced-focus-390.png'); await page.emulateMedia({ forcedColors: 'none' })
          await page.setViewportSize({ width: 900, height: 700 }); await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
          await expect(page.locator('.mcr-chrome-bottom')).toBeVisible()
          for (const link of await page.locator('.mcr-chrome-bottom > a').all()) { await hit(link); const r = await link.boundingBox(); assert(r.width >= 44 && r.height >= 44) }
          entry.responsive = await capture(page, 'responsive-half-900.png')
          await page.locator('.mcr-chrome-bottom a[href="/feed"]').click(); await expect.poll(async () => (await state(page)).path).toBe('/feed')
          assert.equal((await state(page)).canvases, 1); assert((await state(page)).activeSockets <= 1)
        }
      }
    }
    assert.deepEqual(entry.errors, []); entry.status = 'PASS'
  } catch (error) { entry.status = 'FAIL'; entry.error = error.stack; report.errors.push({ mode, width, path, error: error.message }); entry.failure = await capture(page, `failure-${mode}-${name}-${width}.png`).catch(() => null) }
  finally { await page.evaluate(() => window.__COMMUNITY_AUTHORITY_NATIVE__?.dispose()).catch(() => {}); await context.close(); report.cases.push(entry); await writeFile(resolve(output, 'report.partial.json'), JSON.stringify(report, null, 2) + '\n') }
}
try {
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--disk-cache-size=10485760'] }); report.browser = browser.version()
  for (const mode of ['baseline', 'candidate']) { await start(mode); for (const width of [360, 390, 430]) for (const path of paths) await run(mode, width, path); await stop() }
  await start('candidate')
  const context = await browser.newContext({ viewport: { width: 390, height: 568 }, reducedMotion: 'reduce' }), page = await context.newPage()
  await open(page, '/gifts', 'no-policy'); report.unavailable = { text: await page.locator('.shell-panel-body').innerText(), capture: await capture(page, 'unavailable-390.png') }
  assert.match(report.unavailable.text, /unavailable|could not|unable|not available/i, 'Truthful unavailable state stays present')
  assert.equal(await page.locator('.gift-reference-row').count(), 0)
  for (const control of await page.locator('.mcr-chrome-panel-header > button').all()) await hit(control)
  await page.evaluate(() => window.__COMMUNITY_AUTHORITY_NATIVE__.dispose()); await context.close()
} catch (error) { report.errors.push({ workflow: error.stack }) }
finally {
  await browser?.close(); await stop(); report.finishedAt = new Date().toISOString()
  report.status = report.errors.length ? 'FAIL' : 'PASS_PENDING_OWNER_VISUAL_INSPECTION'
  report.cleanup = { browserClosed: true, viteClosed: true }
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify({ status: report.status, errors: report.errors, output }))
}
if (report.errors.length) process.exitCode = 1
