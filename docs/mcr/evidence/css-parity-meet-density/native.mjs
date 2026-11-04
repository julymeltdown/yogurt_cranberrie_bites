// CSS-MEET-DENSITY: adapted from213 native.mjs at7a87414a; actual shell, retained DEV fixture.
// Only this density candidate is executed; qualified213 captures are reused read-only.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, rm, stat, statfs, writeFile } from 'node:fs/promises'
import { extname, relative, resolve } from 'node:path'
import { createRequire } from 'node:module'
import { pathToFileURL } from 'node:url'

const repo = resolve(import.meta.dirname, '../../../..'), frontend = resolve(repo, 'frontend')
const require = createRequire(resolve(frontend, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')))
const { chromium, expect } = require('@playwright/test')
const { CASES, CLOCK, REFERENCE_SHA256, viewportFor } = await import(pathToFileURL(resolve(frontend, 'e2e/reference-integrated/cases.mjs')))
const output = process.env.MCR_FIRST_VIEW_OUTPUT
assert(process.env.MCR_FIRST_VIEW_JOB, 'Named resource offer and actual acceptance required')
assert(output?.startsWith(resolve(repo, 'docs/mcr/evidence/css-parity-meet-density/') + '/'))
assert(/^\d+$/.test(process.env.MCR_FIRST_VIEW_PORT ?? ''))
const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 64 * 1024 * 1024 })
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const target = 'docs/mcr/design/reference-target-2026-11-04.png'
assert.equal(hash(await readFile(resolve(repo, target))), REFERENCE_SHA256)
const meet = CASES.find(item => item.id === 'meet')
const baselinePath = process.env.MCR_DENSITY_BASELINE ?? 'docs/mcr/evidence/reference-first-view/round-01/cohort/capture/report.json'
const baselineBytes = await readFile(resolve(repo, baselinePath))
assert.equal(hash(baselineBytes), '4062dc45d58b437ec15872355773887a002767244142a57d0d8b2197df28763a')
const baseline = JSON.parse(baselineBytes)
const publicFiles = new Set(git('ls-files', 'frontend/public').toString().trim().split('\n'))
const sourcePaths = ['frontend/src/app/reference-chrome-v2/chrome.css', 'frontend/src/features/party-discovery/ui/party-discovery.css', 'frontend/e2e/visual/qa06/preview.tsx']
const report = { task: 'CSS-MEET-DENSITY', status: 'IN_PROGRESS', job: process.env.MCR_FIRST_VIEW_JOB,
  candidate: git('rev-parse', 'HEAD').toString().trim(), diffSha256: hash(git('diff', 'HEAD')),
  qualification: 'Real default App/router/Phaser/Meet components with existing DEV synthetic HTTP/C05 fixture. No capability injection or feature/source overlays.',
  reference: { path: target, sha256: REFERENCE_SHA256 },
  baseline: { path: baselinePath, sha256: hash(baselineBytes), cases: baseline.cases.map(c => ({ width: c.width, height: c.height, kind: c.kind, screenshot: c.screenshot, initial: c.initial })) },
  sources: await Promise.all(sourcePaths.map(async path => ({ path, sha256: hash(await readFile(resolve(repo, path))) }))),
  limitations: ['No live service, physical device, software keyboard or full release acceptance', 'Historical baseline is the matched reference aspect, not an844px viewport', 'Only owned Meet density CSS changes; qualified213 shared composition is a read-only dependency', 'No member portraits or enabled Create capability are fabricated; fonts and selected artwork are unchanged'],
  cases: [], assets: [], startedAt: new Date().toISOString() }
let server, browser, prepared = false, screenshotBytes = 0
const previousCapture = process.env.MCR_FIRST_VIEW_PREVIOUS_CAPTURE
if (previousCapture) {
  assert(previousCapture.startsWith(resolve(repo, 'docs/mcr/evidence/css-parity-meet-density/') + '/'))
  const previous = JSON.parse(await readFile(resolve(previousCapture, 'report.json')))
  assert.equal(previous.sources[0].sha256, report.sources[0].sha256, 'Only unchanged production CSS images may be reused')
  report.reusedScreenshots = []
}
const cache = new Map(), cacheDir = resolve(process.env.TMPDIR, 'first-view-vite')
const mime = { '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.jpg': 'image/jpeg', '.webp': 'image/webp', '.ttf': 'font/ttf', '.woff2': 'font/woff2' }
const floor = async () => { const s = await statfs(repo); assert(s.bavail * s.bsize >= 8 * 1024 ** 3) }
async function screenshot(page, name) {
  if (previousCapture) {
    try {
      const bytes = await readFile(resolve(previousCapture, name))
      report.reusedScreenshots.push({ name, path: relative(output, resolve(previousCapture, name)), sha256: hash(bytes) })
      return relative(output, resolve(previousCapture, name))
    } catch (error) { if (error.code !== 'ENOENT') throw error }
  }
  const path = resolve(output, name); await page.screenshot({ path })
  screenshotBytes += (await stat(path)).size; assert(screenshotBytes <= 4 * 1024 ** 2, '4MiB new PNG bound')
  return name
}
function geometry() {
  const body = document.getElementById('shell-panel-body'), flow = body.querySelector('.mcr-reference-panel-flow')
  const content = flow.querySelector('.mcr-reference-panel-content'), party = flow.querySelector('.party-browse')
  const discovery = party.querySelector('.meet-discovery'), action = party.querySelector('.meet-create__action')
  const create = party.querySelector('.meet-create'), print = flow.querySelector('.mcr-chrome-parade-footer')
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom, right: r.right } }
  const scroll = e => e && ({ ...rect(e), clientHeight: e.clientHeight, scrollHeight: e.scrollHeight, scrollTop: e.scrollTop, overflowY: getComputedStyle(e).overflowY })
  return { viewport: { width: innerWidth, height: innerHeight }, horizontalOverflow: document.documentElement.scrollWidth > innerWidth || body.scrollWidth > body.clientWidth + 1,
    body: scroll(body), flow: rect(flow), content: rect(content), party: rect(party), discovery: scroll(discovery), create: rect(create), action: { ...rect(action), disabled: action.matches(':disabled'), tag: action.tagName },
    unavailableCopy: create.querySelector('.meet-create__unavailable')?.textContent ?? null,
    print: { ...rect(print), inert: print.getAttribute('aria-hidden'), pointerEvents: getComputedStyle(print).pointerEvents },
    rowTitles: [...party.querySelectorAll('.meet-party h3')].map(e => e.textContent),
    rowMetrics: [...party.querySelectorAll('.meet-party')].map(e => ({ ...rect(e), font: getComputedStyle(e.querySelector('h3')).fontSize })),
    canvasCount: document.querySelectorAll('.shell-world-host canvas').length,
    brokenImages: [...document.images].filter(e => !e.complete || !e.naturalWidth).map(e => new URL(e.src).pathname) }
}
async function focusedTarget(page) {
  return page.evaluate(() => {
    const e = document.activeElement, r = e.getBoundingClientRect(), body = document.getElementById('shell-panel-body')
    const scroller = e.closest('.meet-discovery') ?? body, s = scroller.getBoundingClientRect(), b = body.getBoundingClientRect()
    return { tag: e.tagName, type: e.getAttribute('type'), focusWithin: e.matches(':focus-within'), focusVisible: e.matches(':focus-visible'), name: e.getAttribute('name'), text: e.textContent?.trim().slice(0, 100),
      row: e.closest('.meet-party')?.querySelector('h3')?.textContent ?? null,
      width: r.width, height: r.height, visibleHeight: Math.max(0, Math.min(r.bottom, s.bottom, b.bottom, innerHeight) - Math.max(r.top, s.top, b.top, 0)),
      outline: getComputedStyle(e).outlineStyle, bodyScroll: body.scrollTop, discoveryScroll: document.querySelector('.meet-discovery')?.scrollTop }
  })
}
async function assertFocus(page, minimum = 44) {
  const value = await focusedTarget(page)
  assert(value.width >= minimum && value.height >= minimum, JSON.stringify(value))
  assert(value.visibleHeight >= value.height - 1, JSON.stringify(value))
  // Chromium's datetime-local keyboard focus is on a native shadow segment;
  // its host need not match :focus-visible. Preserve and report that distinction.
  assert(value.outline !== 'none' || (value.type === 'datetime-local' && value.focusWithin), JSON.stringify(value)); return value
}
function assertLayout(g, firstView = false) {
  assert.equal(g.horizontalOverflow, false); assert.deepEqual(g.brokenImages, []); assert.equal(g.canvasCount, 1)
  assert(g.discovery.height >= 44, 'Discovery retains a usable keyboard viewport')
  assert(g.discovery.y >= g.party.y && g.discovery.bottom <= g.create.y + 1, 'Discovery cannot overlap CTA')
  assert(g.create.bottom <= g.print.y + 1, 'Truthful CTA/copy cannot overlap the print')
  assert(g.action.width >= 44 && g.action.height >= 44)
  if (g.action.disabled) assert.equal(g.unavailableCopy, 'Party creation is currently unavailable.')
  assert.equal(g.print.inert, 'true'); assert.equal(g.print.pointerEvents, 'none')
  if (firstView) {
    assert(g.action.y >= g.body.y && g.create.bottom <= g.body.bottom + 1, 'Complete CTA and truthful copy above fold')
    assert(g.print.y >= g.body.y && g.print.bottom <= g.body.bottom + 1, 'Complete first-view print')
  }
}
async function runCase(url, width, height, kind) {
  await floor()
  const context = await browser.newContext({ viewport: { width, height }, isMobile: true, hasTouch: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
  const page = await context.newPage(), entry = { width, height, kind, errors: [] }; page.setDefaultTimeout(10000)
  page.on('pageerror', error => entry.errors.push(error.message))
  const name = `meet-${width}-${kind}`
  try {
    await page.clock.setFixedTime(new Date(CLOCK))
    await page.goto(`${url}e2e/visual/qa06/preview.html?entry=%2Fmeet&era=PARADE`)
    await page.waitForFunction(() => window.__QA06__?.state().published, undefined, { timeout: 25000 })
    await page.waitForLoadState('networkidle'); await page.evaluate(() => document.fonts.ready)
    await expect(page.locator('.meet-party')).toHaveCount(3)
    entry.initial = await page.evaluate(geometry); assertLayout(entry.initial, true)
    if (kind === 'short') assert(entry.initial.discovery.scrollHeight > entry.initial.discovery.clientHeight, 'Real discovery scroll required at short height')
    entry.completeRowsInFirstView = entry.initial.rowMetrics.filter(r => r.y >= entry.initial.discovery.y - 1 && r.bottom <= entry.initial.discovery.bottom + 1).length
    entry.contentMetrics = await page.locator('.meet-party').evaluateAll(rows => rows.map(row => {
      const rect = e => { const r=e.getBoundingClientRect(); return { x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right } };
      const select = s => row.querySelector(s);
      return { row:rect(row), title: { ...rect(select('h3 a')),font:getComputedStyle(select('h3')).fontSize },
        metadata: { ...rect(select('.meet-party__where')), font:getComputedStyle(select('.meet-party__where')).fontSize, text:select('.meet-party__where').textContent },
        unavailable: { ...rect(select('.meet-party__members-unavailable')), font:getComputedStyle(select('.meet-party__members-unavailable')).fontSize, text:select('.meet-party__members-unavailable').textContent },
        portrait:rect(select('.meet-party__portrait')),join:rect(select('.meet-party__join')),capacity:rect(select('.meet-party__capacity')) };
    }));
    for (const row of entry.contentMetrics) {
      assert.equal(row.title.font,'18px'); assert.equal(row.metadata.font,'14px'); assert.equal(row.unavailable.font,'10px');
      assert.equal(row.unavailable.text,'Member portraits unavailable');
      for (const item of [row.title,row.metadata,row.unavailable,row.portrait,row.join,row.capacity]) {
        assert(item.y >= row.row.y && item.bottom <= row.row.bottom + 1, 'Retained content cannot overflow its own card');
        assert(item.x >= row.row.x && item.right <= row.row.right + 1, 'Retained content cannot overflow its own card horizontally');
      }
      assert(row.join.height >=44 && row.title.height >=44);
    }
    await page.evaluate(() => { window.__firstViewCanvas = document.querySelector('.shell-world-host canvas') })
    entry.screenshot = await screenshot(page, `${name}.png`)
    // Native Tab focus must reach all retained rows through the inner viewport.
    await page.getByRole('button', { name: 'MY PLAN', exact: true }).focus()
    const reached = new Set(), targets = []
    for (let step = 0; step < 16; step++) {
      await page.keyboard.press('Tab'); const focused = await focusedTarget(page)
      if (focused.row) { targets.push(await assertFocus(page)); reached.add(focused.row) }
      if (focused.text === 'Filter parties') break
    }
    assert.deepEqual([...reached].sort(), [...entry.initial.rowTitles].sort()); entry.rowKeyboard = targets
    entry.innerEnd = await page.evaluate(geometry); assertLayout(entry.innerEnd, true)
    if (kind === 'short') assert(entry.innerEnd.discovery.scrollTop > 0)
    // Filters expand inside the already bounded discovery region.
    const filter = page.locator('.party-filters'), summary = filter.locator('summary')
    await summary.focus(); await page.keyboard.press('Enter'); await expect(filter).toHaveAttribute('open', '')
    const fieldTargets = [], fieldNames = new Set(); entry.filterKeyboard = fieldTargets
    for (let step = 0; step < 36; step++) {
      await page.keyboard.press('Tab'); const focused = await focusedTarget(page)
      if (focused.name || focused.text === 'Show parties') {
        // The native checkbox has24px ink inside its44px label target.
        if (focused.name === 'has_space') {
          const labelBox = await page.locator('.party-checkbox').boundingBox(); assert(labelBox.height >= 44)
          assert(focused.visibleHeight >= focused.height - 1)
        } else fieldTargets.push(await assertFocus(page))
        if (focused.name) fieldNames.add(focused.name)
      }
      if (focused.text === 'Show parties') break
    }
    assert.deepEqual([...fieldNames].sort(), ['has_space', 'starts_after', 'starts_before', 'state', 'ticket_section'].sort())
    entry.filterKeyboard = fieldTargets; entry.filtersOpen = await page.evaluate(geometry); assertLayout(entry.filtersOpen, true)
    if (kind === 'short') entry.filterScreenshot = await screenshot(page, `${name}-filters.png`)
    await summary.focus(); await page.keyboard.press('Space'); await expect(filter).not.toHaveAttribute('open')
    // Expanding outer actions deliberately restores natural outer scrolling.
    const more = page.locator('.shell-more-sections'), moreSummary = more.locator('summary')
    await moreSummary.focus(); await page.keyboard.press('Enter'); await expect(more).toHaveAttribute('open', '')
    const moreTargets = []
    for (const [path, label] of [['/meet/parties/new', 'Create party'], ['/meet/people', 'People'], ['/meet/intro/edit', 'Your intro']]) {
      await page.keyboard.press('Tab'); const link = more.getByRole('link', { name: label, exact: true })
      await expect(link).toBeFocused(); await expect(link).toHaveAttribute('href', path)
      moreTargets.push(await assertFocus(page))
    }
    entry.moreKeyboard = moreTargets; entry.moreOpen = await page.evaluate(geometry); assertLayout(entry.moreOpen)
    if (kind === 'short') entry.moreScreenshot = await screenshot(page, `${name}-more.png`)
    await page.locator('#shell-panel-body').evaluate(e => { e.scrollTop = e.scrollHeight })
    await expect(page.locator('.mcr-chrome-parade-footer img')).toBeInViewport({ ratio: 1 })
    entry.moreOpenEnd = await page.evaluate(geometry)
    await moreSummary.focus(); await page.keyboard.press('Space'); await expect(more).not.toHaveAttribute('open')
    // My Plan is outside the constrained discovery mode and keeps its original flow.
    await page.getByRole('button', { name: 'MY PLAN', exact: true }).click()
    await expect(page.locator('.meet-discovery')).toHaveCount(0)
    await page.locator('#shell-panel-body').evaluate(e => { e.scrollTop = e.scrollHeight })
    await expect(page.locator('.mcr-chrome-parade-footer img')).toBeInViewport({ ratio: 1 })
    entry.myPlan = { reachablePrint: true }
    await page.getByRole('button', { name: 'Parties', exact: true }).click()
    await expect(page.locator('.meet-party')).toHaveCount(3)
    entry.restored = await page.evaluate(geometry); assertLayout(entry.restored, true)
    assert.deepEqual(entry.restored.rowTitles, entry.initial.rowTitles)
    assert.equal(entry.restored.action.disabled, entry.initial.action.disabled)
    entry.fullPanelWorld = await page.evaluate(() => window.__QA06__.state())
    // The real mobile full panel intentionally suspends the hidden world.
    // Reveal it through the existing control before asserting one connection.
    assert.equal(entry.fullPanelWorld.activeSockets, 0)
    await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
    await page.waitForFunction(() => window.__QA06__.state().activeSockets === 1)
    entry.persistence = await page.evaluate(() => ({ sameCanvas: window.__firstViewCanvas === document.querySelector('.shell-world-host canvas'), state: window.__QA06__.state() }))
    assert(entry.persistence.sameCanvas); assert.equal(entry.persistence.state.activeSockets, 1); assert.equal(entry.persistence.state.socketPeak, 1)
    assert.deepEqual(entry.errors, []); entry.status = 'PASS'
  } catch (error) { entry.status = 'FAIL'; entry.error = error.message }
  finally {
    await page.evaluate(() => window.__QA06__?.dispose()).catch(() => {}); await context.close()
    report.cases.push(entry); await writeFile(resolve(output, 'report.partial.json'), JSON.stringify(report, null, 2) + '\n')
    console.log(JSON.stringify({ width, height, kind, status: entry.status, error: entry.error }))
  }
}
try {
  await floor(); await mkdir(output, { recursive: false }); prepared = true
  server = await createServer({ root: frontend, configLoader: 'runner', publicDir: false, cacheDir, logLevel: 'error', resolve: { dedupe: ['react', 'react-dom'] },
    plugins: [{ name: 'frozen-public-assets', configureServer(s) { s.middlewares.use((req, res, next) => {
      const path = 'frontend/public' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname)
      if (!publicFiles.has(path) || !mime[extname(path)]) return next()
      try {
        if (!cache.has(path)) { const bytes = git('show', `HEAD:${path}`); assert([...cache.values()].reduce((n, b) => n + b.length, bytes.length) <= 200 * 1024 ** 2); cache.set(path, bytes); report.assets.push({ path, sha256: hash(bytes), bytes: bytes.length }) }
        res.setHeader('Content-Type', mime[extname(path)]); res.end(cache.get(path))
      } catch (error) { next(error) }
    }) } }], server: { host: '127.0.0.1', port: Number(process.env.MCR_FIRST_VIEW_PORT), strictPort: true, hmr: false } })
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) for (const [kind, height] of [['reference', viewportFor(meet, width).height], ['normal', 844], ['short', 480]]) {
    await runCase(server.resolvedUrls.local[0], width, height, kind)
    if (report.cases.at(-1).status === 'FAIL') throw Error('Resolve first retained affected-cohort failure')
  }
  report.status = 'PASS'
} finally {
  await browser?.close(); await server?.close(); await rm(cacheDir, { recursive: true, force: true })
  if (report.status !== 'PASS') report.status = 'FAIL'
  report.finishedAt = new Date().toISOString(); report.screenshotBytes = screenshotBytes
  if (prepared) await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
assert.equal(report.status, 'PASS')
