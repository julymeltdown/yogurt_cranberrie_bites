// Developer-owned CSS comparison. Requires resource21d's named, accepted output lease.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile, rm, statfs, readdir, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createRequire } from 'node:module'
const repo = resolve(import.meta.dirname, '../../../..'), frontend = resolve(repo, 'frontend')
const require = createRequire(resolve(frontend, 'package.json'))
const { createServer } = await import(require.resolve('vite'))
const { chromium, expect } = await import(require.resolve('@playwright/test'))
assert(process.env.MCR_GIVEAWAY_JOB, 'Named resource allocation required')
assert.equal(process.version, 'v24.22.2')
const run = process.env.MCR_GIVEAWAY_ROUND ?? 'native-01'
assert(/^[a-z0-9-]+$/.test(run))
const evidence = import.meta.dirname, output = resolve(evidence, run)
const temp = '/private/tmp/mcr-css-giveaway-' + run
const base = '54320d71c4ae0bff3524d1b500cea772e0ba42e0'
const css = ['frontend/src/features/gift-discovery/ui/gifts.css', 'frontend/src/features/gift-discovery/ui/reference-gifts.css']
const git = (...args) => execFileSync('git', args, { cwd: repo, encoding: 'utf8', maxBuffer: 64 * 1024 ** 2 }).trim()
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const report = { job: process.env.MCR_GIVEAWAY_JOB, run, base, candidate: git('rev-parse', 'HEAD'), started: new Date().toISOString(),
  qualification: 'Actual selected-source cozy root, canonical Auth, synthetic HTTP/C05 only. Standalone production GiftDiscovery for long/unavailable/empty/error states. No live admission, real gift photos, backend mutation or physical device acceptance.',
  baselinePolicy: 'Same checkout/fixture; baseline substitutes only two assigned CSS files with exact base Git bytes through Vite load hook. Candidate uses actual working-tree CSS. No disk source replacement.',
  sources: {}, cases: [], failures: [], limits: { whole: 167772160, temp: 125829120, durable: 20971520 }, peaks: { whole: 0, temp: 0, durable: 0 }, minFree: Infinity }
for (const path of [...css, 'frontend/src/features/gift-discovery/ui/Gifts.tsx', 'frontend/src/features/gift-discovery/ui/reference-presentation.tsx', 'frontend/src/app/__tests__/cozy-backgrounds-v1/preview.tsx', 'frontend/e2e/reference-integrated/data.ts', 'frontend/e2e/reference-integrated/whole/data.ts', 'frontend/pnpm-lock.yaml']) {
  report.sources[path] = { candidate: hash(await readFile(resolve(repo, path))), baseline: hash(execFileSync('git', ['show', base + ':' + path], { cwd: repo })) }
}
await mkdir(output, { recursive: false }); await mkdir(temp, { recursive: false })
async function size(path) { let bytes = 0; for (const item of await readdir(path, { withFileTypes: true })) { const p = resolve(path, item.name); if (item.isDirectory()) bytes += await size(p); else if (item.isFile()) bytes += (await stat(p)).size } return bytes }
async function budget(start = false) { const d = await statfs(repo), free = Number(d.bavail) * Number(d.bsize); report.minFree = Math.min(report.minFree, free); assert(free >= (start ? 16 : 8) * 1024 ** 3, 'Disk floor'); const t = await size(temp), durable = await size(evidence); for (const [key, value] of Object.entries({ temp: t, durable, whole: t + durable })) { report.peaks[key] = Math.max(report.peaks[key], value); assert(value <= report.limits[key], key + ' cap') } }
let browser, server, cache
const started = Date.now()
async function start(mode) {
  cache = resolve(temp, mode)
  server = await createServer({ root: frontend, configLoader: 'runner', logLevel: 'error', cacheDir: cache,
    plugins: [{ name: 'giveaway-exact-baseline-css', enforce: 'pre', load(id) { const path = id.split('?')[0]; if (mode === 'baseline' && css.some(p => resolve(repo, p) === path)) return execFileSync('git', ['show', base + ':' + path.slice(repo.length + 1)], { cwd: repo, encoding: 'utf8' }) } }],
    server: { host: '127.0.0.1', port: 5296, strictPort: true, hmr: false } })
  await server.listen(); return 'http://127.0.0.1:5296/'
}
async function stop() { await server?.close(); server = null; if (cache) await rm(cache, { recursive: true, force: true }) }
function geometry() {
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, right: r.right, bottom: r.bottom } }
  const overlap = (a, b) => Math.min(a.right, b.right) - Math.max(a.x, b.x) > .5 && Math.min(a.bottom, b.bottom) - Math.max(a.y, b.y) > .5
  return { overflow: document.documentElement.scrollWidth > innerWidth + 1,
    text: document.querySelector('.gifts-reference')?.innerText,
    targets: [...document.querySelectorAll('.gifts-reference :is(button,a,input,select,summary)')].filter(e => e.checkVisibility()).map(e => ({ name: e.getAttribute('aria-label') ?? e.textContent, ...rect(e) })),
    rows: [...document.querySelectorAll('.gift-reference-row')].map(e => { const title = rect(e.querySelector('h3')), byline = rect(e.querySelector('.gift-reference-byline')), location = rect(e.querySelector('.gift-reference-location')), action = rect(e.querySelector('.gift-reference-action')); return { ...rect(e), title, byline, location, action, image: rect(e.querySelector('.gift-reference-image')), overlap: overlap(title, byline) || overlap(location, action) || overlap(byline, action) } }),
    images: [...document.images].filter(e => e.checkVisibility()).map(e => ({ src: new URL(e.currentSrc).pathname, ...rect(e) })) }
}
async function capture(origin, mode, width, scenario = 'root', forcedColors = false) {
  assert(Date.now() - started < 220000, 'Reserve cleanup time')
  const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce', forcedColors: forcedColors ? 'active' : 'none', serviceWorkers: 'block' })
  const page = await context.newPage(), entry = { mode, width, scenario, forcedColors, errors: [] }
  page.on('pageerror', e => entry.errors.push(e.message)); page.setDefaultTimeout(12000)
  const state = () => page.evaluate(() => window.__COZY_PANEL__?.state())
  try {
    await page.clock.setFixedTime(new Date('2026-11-07T07:00:00.000Z'))
    await page.goto(origin + (scenario === 'root' ? 'src/app/__tests__/cozy-backgrounds-v1/preview.html?entry=/gifts&era=PARADE' : 'src/features/gift-discovery/__tests__/reference-native/preview.html?scenario=' + scenario))
    if (scenario === 'root') await expect.poll(async () => (await state())?.published, { timeout: 25000 }).toBe(true)
    if (scenario === 'empty') await expect(page.getByText('No matching gifts', { exact: true })).toBeVisible()
    else if (scenario === 'error') await expect(page.getByRole('alert')).toBeVisible()
    else await expect(page.locator('.gift-reference-row')).toHaveCount(3)
    await page.evaluate(() => document.fonts.ready)
    entry.geometry = await page.evaluate(geometry)
    assert(!entry.geometry.overflow, 'Horizontal overflow')
    assert(entry.geometry.rows.every(r => !r.overlap), 'Overlapping row copy/action')
    assert(entry.geometry.targets.every(t => t.width >= 43.9 && t.height >= 43.9), '44px targets')
    if (scenario === 'root') { entry.state = await state(); assert(entry.state.activeSockets <= 1); assert.equal(entry.state.canvases, 1) }
    entry.file = `${mode}-${scenario}-${width}${forcedColors ? '-forced' : ''}.png`
    await page.screenshot({ path: resolve(output, entry.file) })
    entry.sha256 = hash(await readFile(resolve(output, entry.file)))
    if (mode === 'candidate' && scenario === 'root') {
      const before = report.cases.find(c => c.mode === 'baseline' && c.width === width && !c.forcedColors)
      if (before) { assert.equal(entry.geometry.text, before.geometry.text, 'Truthful copy changed'); assert.deepEqual(entry.geometry.images, before.geometry.images, 'Selected artwork changed') }
      const browse = page.getByRole('tab', { name: 'Browse', exact: true })
      await browse.focus(); await page.keyboard.press('ArrowRight'); await expect(page.getByRole('tab', { name: 'My giving' })).toBeFocused()
      await page.keyboard.press('End'); await expect(page.getByRole('tab', { name: 'My pickups' })).toBeFocused()
      await page.keyboard.press('Home'); await expect(browse).toBeFocused()
      await expect(page.locator('.gift-reference-row')).toHaveCount(3)
      entry.focus = await browse.evaluate(e => ({ outline: getComputedStyle(e).outlineStyle, width: getComputedStyle(e).outlineWidth, color: getComputedStyle(e).outlineColor }))
      assert.equal(entry.focus.outline, 'solid'); assert(parseFloat(entry.focus.width) >= 2)
      await page.screenshot({ path: resolve(output, `focus-${width}${forcedColors ? '-forced' : ''}.png`) })
      for (const action of await page.locator('.gift-reference-action a').all()) { await action.scrollIntoViewIfNeeded(); await expect(action).toBeInViewport({ ratio: 1 }); assert(await action.evaluate(e => { const r = e.getBoundingClientRect(); return e.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)) }), 'Action not hittable') }
      const summary = page.getByText('Filter giveaways', { exact: true }); await summary.focus(); await page.keyboard.press('Enter'); await expect(page.getByRole('combobox', { name: 'Status', exact: true })).toBeVisible(); await page.keyboard.press('Enter')
      const action = page.locator('.gift-reference-action a').first(), href = await action.getAttribute('href'); assert(href?.endsWith('/reservation')); await action.focus(); await page.keyboard.press('Enter'); await expect.poll(async () => (await state()).path).toBe(href)
      entry.after = await state(); assert(entry.after.activeSockets <= 1); assert.equal(entry.after.canvases, 1); assert(entry.after.requests.filter(r => r.path.includes('/giveaways')).every(r => r.method === 'GET')); entry.interactions = 'PASS: roving tabs/Home/End, normal/forced focus, filter keyboard, reachable Reserve real route, GET-only gifts, persistent world'
    }
    if (scenario === 'unavailable' || scenario === 'read-only') { assert.equal(await page.locator('.gift-reference-action button:disabled').count(), 3); assert.equal(await page.locator('.gift-reference-action a').count(), 0) }
    assert.deepEqual(entry.errors, []); entry.result = 'PASS'
  } catch (error) { entry.result = 'FAIL'; entry.error = error.message; report.failures.push({ mode, width, scenario, error: error.message }); await page.screenshot({ path: resolve(output, `failure-${mode}-${scenario}-${width}.png`) }).catch(() => {}) }
  finally { await page.evaluate(() => { window.__COZY_PANEL__?.dispose(); window.referenceGifts?.dispose() }).catch(() => {}); await context.close(); report.cases.push(entry); await budget(); await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify({ mode, width, scenario, result: entry.result, error: entry.error })) }
}
try {
  await budget(true); browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--disk-cache-dir=' + resolve(temp, 'chrome-cache')] }); report.browser = browser.version()
  for (const mode of ['baseline', 'candidate']) { const origin = await start(mode); for (const width of [390, 360, 430]) await capture(origin, mode, width)
    if (mode === 'candidate') { for (const width of [360, 390, 430]) for (const scenario of ['long', 'unavailable']) await capture(origin, mode, width, scenario); for (const scenario of ['read-only', 'empty', 'error']) await capture(origin, mode, 390, scenario); await capture(origin, mode, 390, 'root', true) } await stop() }
  report.result = report.failures.length ? 'FAIL' : 'PASS_CHECKS_VISUAL_ASSESSMENT_REQUIRED'
} finally { await browser?.close(); await stop(); await rm(temp, { recursive: true, force: true }); report.finished = new Date().toISOString(); report.cleanup = { browserClosed: true, viteClosed: true, tempRemoved: true }; await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
if (report.failures.length) process.exitCode = 1
