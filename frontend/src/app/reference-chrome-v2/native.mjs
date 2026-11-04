// Developer-owned default-router composition proof. Synthetic services are labeled below.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, rm, stat, statfs, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { extname, resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { CASES, CLOCK, REFERENCE_SHA256, viewportFor } from '../../../e2e/reference-integrated/cases.mjs'

assert(process.env.MCR_CHROME_JOB, 'A fresh named21d offer and explicit acceptance are required')
const repo = resolve(import.meta.dirname, '../../../..'), frontend = resolve(repo, 'frontend')
const output = process.env.MCR_CHROME_OUTPUT
assert(output?.startsWith(resolve(repo, 'docs/mcr/evidence/reference-chrome-v2/') + '/'))
assert(/^\d+$/.test(process.env.MCR_CHROME_PORT ?? ''), 'Use the admitted explicit port')
const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 64 * 1024 * 1024 })
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const target = 'docs/mcr/design/reference-target-2026-11-04.png'
assert.equal(hash(await readFile(resolve(repo, target))), REFERENCE_SHA256)
// Qualified artist commits can be read without taking write ownership of their paths.
const art = {
  'frontend/public/reference-parade-print-v2/': process.env.MCR_CHROME_PRINT_COMMIT,
  'frontend/public/reference-window-controls-v2/': process.env.MCR_CHROME_CONTROL_COMMIT,
}
const feedCommit = process.env.MCR_CHROME_FEED_COMMIT
if (feedCommit) assert(/^[a-f0-9]{40}$/.test(feedCommit), 'Exact qualified Feed CSS commit required')
const feedPath = 'frontend/src/features/event-feed/feed.css'
const flatPaperCommit = process.env.MCR_CHROME_FLAT_PAPER_COMMIT
if (flatPaperCommit) assert(/^[a-f0-9]{40}$/.test(flatPaperCommit), 'Exact Root flat-paper source commit required')
const flatPaperPath = 'frontend/src/shared/styles/cozy-community-backgrounds-v1.css'
const baseCssPath = 'frontend/src/shared/styles/base.css'
const selectedCases = new Set((process.env.MCR_CHROME_CASES ?? 'landing,login,feed,meet,gifts,me').split(','))
assert([...selectedCases].every(id => ['landing', 'login', 'feed', 'meet', 'gifts', 'me'].includes(id)))
for (const commit of Object.values(art)) assert(/^[a-f0-9]{40}$/.test(commit ?? ''), 'Qualified artist commit required')
const report = { task: 'FE-REFERENCE-CHROME-02', status: 'IN_PROGRESS', job: process.env.MCR_CHROME_JOB,
  candidate: git('rev-parse', 'HEAD').toString().trim(), diffSha256: hash(git('diff', 'HEAD')),
  reference: { path: target, sha256: REFERENCE_SHA256, dimensions: [1536, 1024] }, art,
  feedOverlay: feedCommit ? { commit: feedCommit, path: feedPath, sha256: hash(git('show', `${feedCommit}:${feedPath}`)) } : null,
  flatPaperOverlay: flatPaperCommit ? { commit: flatPaperCommit, path: flatPaperPath, sha256: hash(git('show', `${flatPaperCommit}:${flatPaperPath}`)), injection: 'Exact pinned CSS replaces its single base.css @import before PostCSS resolution', basePath: baseCssPath, baseSha256: hash(git('show', `HEAD:${baseCssPath}`)) } : null,
  selectedCases: [...selectedCases],
  qualification: 'Real production components/default router/Phaser with explicit DEV synthetic HTTP/C05 fixture; production anonymous entry separately captured',
  visualScope: 'Shared frame, public header and CTA-safe placement only. Retro styling belongs only to post cards and replies; per-screen bodies, tabs and composer are not visually accepted by this run.',
  limitations: ['No live backend or two-account acceptance', 'No physical iOS/Android or software keyboard proof', 'QA06 map admission fixture is not Map/C06 product acceptance', 'Per-screen rendering remains with its implementation owner'],
  cases: [], assets: [], startedAt: new Date().toISOString() }
let server, browser, prepared = false, screenshotBytes = 0
const cacheDir = resolve(process.env.TMPDIR ?? tmpdir(), `mcr-reference-chrome-v2-${process.pid}`), cache = new Map()
const floor = async () => { const s = await statfs(repo); assert(s.bavail * s.bsize >= 8 * 1024 ** 3, 'Runtime8GiB floor') }
const mime = { '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.jpg': 'image/jpeg', '.webp': 'image/webp', '.ttf': 'font/ttf', '.woff2': 'font/woff2' }
const tracked = new Set(git('ls-files', 'frontend/public').toString().trim().split('\n'))
async function screenshot(page, name) {
  const path = resolve(output, name); await page.screenshot({ path })
  screenshotBytes += (await stat(path)).size
  assert(screenshotBytes <= 12 * 1024 ** 2, '12MiB screenshot cap')
}
function metrics() {
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom } }
  const visible = e => !!e.getClientRects().length && getComputedStyle(e).visibility !== 'hidden'
  const controls = [...document.querySelectorAll('.mcr-chrome-panel-header button,.concert-window__titlebar button,.mcr-reference-public-titlebar a')].filter(visible)
  const footer = document.querySelector('.mcr-chrome-parade-footer'), body = document.getElementById('shell-panel-body')
  return { overflow: document.documentElement.scrollWidth > innerWidth,
    controls: controls.map(e => ({ name: e.getAttribute('aria-label'), ...rect(e) })),
    brokenImages: [...document.images].filter(e => e.src.includes('/reference-') && (!e.complete || !e.naturalWidth)).map(e => new URL(e.src).pathname),
    footer: footer ? { ...rect(footer), insideScroll: body.contains(footer), hidden: footer.getAttribute('aria-hidden'), pointerEvents: getComputedStyle(footer).pointerEvents } : null,
    heading: document.getElementById('panel-title') ? { ...rect(document.getElementById('panel-title')), outline: getComputedStyle(document.getElementById('panel-title')).outline,
      color: getComputedStyle(document.getElementById('panel-title')).color, background: getComputedStyle(document.querySelector('.mcr-chrome-panel-header')).backgroundColor } : null }
}
function contrast(a, b) {
  const luminance = color => color.match(/[\d.]+/g).slice(0, 3).map(Number).map(n => n / 255).map(n => n <= .04045 ? n / 12.92 : ((n + .055) / 1.055) ** 2.4).reduce((sum, n, i) => sum + n * [.2126, .7152, .0722][i], 0)
  const x = luminance(a), y = luminance(b); return (Math.max(x, y) + .05) / (Math.min(x, y) + .05)
}
async function publicHeaderInk(page) {
  const ink = await page.locator('.public-header.mcr-reference-public-titlebar').evaluate(e => {
    const style = getComputedStyle(e)
    return { background: style.backgroundColor, image: style.backgroundImage, color: style.color }
  })
  assert.deepEqual(ink, { background: 'rgb(37, 16, 23)', image: 'none', color: 'rgb(242, 234, 223)' })
  return ink
}
async function printPixels(page) {
  return page.locator('.mcr-chrome-parade-footer img').evaluate(image => {
    const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight
    const context = canvas.getContext('2d'); context.drawImage(image, 0, 0)
    const data = context.getImageData(0, 0, canvas.width, canvas.height).data
    let opaque = 0, unexpected = 0, transparent = 0
    for (let i = 0; i < data.length; i += 4) {
      if (data[i + 3] === 255) { opaque++; if (data[i] !== 25 || data[i + 1] !== 24 || data[i + 2] !== 23) unexpected++ }
      if (data[i + 3] === 0) transparent++
    }
    return { dimensions: [canvas.width, canvas.height], opaque, unexpected, transparent }
  })
}
async function checkMoreActions(page, item, short = false) {
  const expected = item.id === 'meet'
    ? [['/meet/parties/new', 'Create party'], ['/meet/people', 'People'], ['/meet/intro/edit', 'Your intro']]
    : [['/gifts/mine', 'My gifts'], ['/gifts/new', 'Create giveaway']]
  const disclosure = page.locator('.shell-more-sections'), summary = disclosure.locator('summary')
  await expect(disclosure).not.toHaveAttribute('open')
  await summary.focus(); await page.keyboard.press('Enter'); await expect(disclosure).toHaveAttribute('open', '')
  const box = await summary.boundingBox(); assert(box.width >= 44 && box.height >= 44)
  assert.notEqual(await summary.evaluate(e => getComputedStyle(e).outlineStyle), 'none')
  const destinations = []
  for (const [path, label] of expected) {
    const link = disclosure.getByRole('link', { name: label, exact: true })
    await expect(link).toHaveAttribute('href', path)
    await expect(link).toHaveAttribute('id', `section-${path.replaceAll('/', '-')}`)
    await summary.focus()
    for (let index = 0; index <= destinations.length; index++) await page.keyboard.press('Tab')
    await expect(link).toBeFocused(); await expect(link).toBeInViewport()
    const linkBox = await link.boundingBox(); assert(linkBox.height >= 44 && linkBox.width >= 44)
    if (!short) {
      await page.keyboard.press('Enter')
      await expect.poll(() => page.evaluate(() => window.__QA06__.state().path)).toBe(path)
      await expect(page.locator('#panel-title')).toHaveText(label)
      await page.getByRole('button', { name: 'Back to previous panel' }).click()
      await expect.poll(() => page.evaluate(() => window.__QA06__.state().path)).toBe(item.route)
      if (!await disclosure.evaluate(e => e.open)) { await summary.focus(); await page.keyboard.press('Enter') }
    }
    destinations.push({ path, label, keyboardReachable: true, routeVisited: !short })
  }
  await summary.focus(); await page.keyboard.press('Space'); await expect(disclosure).not.toHaveAttribute('open')
  return { summary: box, destinations, collapsed: true }
}
async function capture(url, item, width) {
  await floor()
  const context = await browser.newContext({ viewport: viewportFor(item, width), hasTouch: true, isMobile: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
  const page = await context.newPage(), errors = [], entry = { id: item.id, width, route: item.route, errors }
  page.on('pageerror', error => errors.push(error.message)); page.setDefaultTimeout(15000)
  try {
    await page.clock.setFixedTime(new Date(CLOCK))
    await page.goto(url + 'e2e/visual/qa06/preview.html?' + new URLSearchParams({ entry: item.route, era: 'PARADE' }))
    await page.waitForFunction(() => window.__QA06__)
    if (!item.public) await expect.poll(() => page.evaluate(() => window.__QA06__.state().published), { timeout: 25000 }).toBe(true)
    await page.waitForLoadState('networkidle'); await page.evaluate(() => document.fonts.ready)
    entry.metrics = await page.evaluate(metrics); entry.state = await page.evaluate(() => window.__QA06__.state())
    if (item.id === 'feed') {
      entry.feedFrameInk = { background: entry.metrics.heading.background, color: entry.metrics.heading.color }
      assert.deepEqual(entry.feedFrameInk, { background: 'rgb(37, 16, 23)', color: 'rgb(242, 234, 223)' })
    }
    if (item.id === 'landing') entry.publicHeaderInk = await publicHeaderInk(page)
    if (item.id === 'feed' && flatPaperCommit) {
      entry.feedPaper = await page.locator('#shell-panel-body').evaluate(e => ({ color: getComputedStyle(e).backgroundColor, image: getComputedStyle(e).backgroundImage }))
      assert.deepEqual(entry.feedPaper, { color: 'rgb(248, 245, 236)', image: 'none' })
    }
    assert.deepEqual(errors, []); assert.equal(entry.metrics.overflow, false); assert.deepEqual(entry.metrics.brokenImages, [])
    if (entry.metrics.heading) { entry.titleContrast = contrast(entry.metrics.heading.color, entry.metrics.heading.background); assert(entry.titleContrast >= 4.5) }
    assert(entry.metrics.controls.length > 0)
    for (const control of entry.metrics.controls) assert(control.width >= 44 && control.height >= 44, JSON.stringify(control))
    if (entry.metrics.footer) {
      assert(entry.metrics.footer.insideScroll); assert.equal(entry.metrics.footer.hidden, 'true'); assert.equal(entry.metrics.footer.pointerEvents, 'none')
      entry.printPixels = await printPixels(page); assert.deepEqual(entry.printPixels.dimensions, [360, 96]); assert.equal(entry.printPixels.unexpected, 0); assert(entry.printPixels.opaque > 1000 && entry.printPixels.transparent > 1000)
    }
    entry.screenshot = `${item.id}-${width}.png`; await screenshot(page, entry.screenshot)
    if (!item.public) {
      const canvas = page.locator('.shell-world-host canvas'); await expect(canvas).toHaveCount(1)
      await page.evaluate(() => { window.__chromeCanvas = document.querySelector('.shell-world-host canvas') })
      const body = page.locator('#shell-panel-body')
      await body.evaluate(e => { e.scrollTop = e.scrollHeight })
      if (entry.metrics.footer) {
        await expect(page.locator('.mcr-chrome-parade-footer img')).toBeInViewport()
        entry.scrolledFooter = await page.evaluate(metrics)
      }
      if (['meet', 'gifts'].includes(item.id)) entry.moreActions = await checkMoreActions(page, item)
      if (item.id === 'gifts') entry.actionPrintGeometry = await page.locator('#gift-own').evaluate(e => {
        const action = e.getBoundingClientRect(), print = document.querySelector('.mcr-chrome-parade-footer').getBoundingClientRect()
        return { action: e.textContent, actionBottom: action.bottom, printTop: print.top, gap: print.top - action.bottom }
      })
      if (item.id === 'gifts') assert(entry.actionPrintGeometry.gap >= 0 && entry.actionPrintGeometry.gap <= 8, 'No secondary navigation between Gifts action and print')
      let create = item.id === 'meet' ? page.getByRole('link', { name: 'Create party', exact: true }) : item.id === 'feed' ? page.getByRole('button', { name: 'Write a post', exact: true }) : null
      if (item.id === 'meet' && await create.count() === 0) {
        // Current Root presentation omits canCreate. Preserve its denied action;
        // a visual fixture cannot grant product authority to enable it.
        const unavailable = page.getByRole('button', { name: 'Create party', exact: true })
        await expect(unavailable).toBeDisabled(); await unavailable.scrollIntoViewIfNeeded(); await expect(unavailable).toBeInViewport()
        entry.createAction = 'TRUTHFUL_DISABLED_BY_PRODUCTION_PRESENTATION_NO_CREATE_FLOW_CLAIM'
        entry.actionPrintGeometry = await unavailable.evaluate(e => {
          const action = e.getBoundingClientRect(), print = document.querySelector('.mcr-chrome-parade-footer').getBoundingClientRect()
          return { actionBottom: action.bottom, printTop: print.top, gap: print.top - action.bottom }
        })
        assert(entry.actionPrintGeometry.gap >= 0 && entry.actionPrintGeometry.gap <= 32, 'Only truthful availability copy between Meet CTA and print')
        create = null
      }
      if (create) {
        await expect(create).toHaveCount(1)
        await create.scrollIntoViewIfNeeded(); const previous = await body.evaluate(e => e.scrollTop)
        await create.click(); await expect(page.locator('#panel-title')).toHaveText(item.id === 'meet' ? 'Create party' : 'New post')
        await page.getByRole('button', { name: 'Back to previous panel' }).click(); await expect(page.locator('#panel-title')).toHaveText(item.id === 'meet' ? 'Meet' : 'Feed')
        entry.scrollReturn = { before: previous, after: await body.evaluate(e => e.scrollTop) }
        assert(Math.abs(entry.scrollReturn.before - entry.scrollReturn.after) <= 1)
      }
      await page.getByRole('button', { name: 'Reduce panel' }).click()
      entry.collapseInk = await page.getByRole('button', { name: 'Minimize panel' }).locator('img').evaluate(e => ({ src: e.getAttribute('src'), transform: getComputedStyle(e).transform }))
      assert.equal(entry.collapseInk.src, '/reference-window-controls-v2/back.svg')
      await page.getByRole('button', { name: 'Minimize panel' }).click(); await expect(body).toBeHidden()
      entry.restoreInk = await page.getByRole('button', { name: 'Show panel content' }).locator('img').evaluate(e => ({ src: e.getAttribute('src'), transform: getComputedStyle(e).transform }))
      assert.equal(entry.restoreInk.src, entry.collapseInk.src); assert.notEqual(entry.restoreInk.transform, entry.collapseInk.transform)
      await page.getByRole('button', { name: 'Show panel content' }).click(); await expect(body).toBeVisible()
      await page.getByRole('button', { name: 'Expand panel' }).click()
      await page.getByRole('button', { name: 'Back to previous panel' }).focus(); await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab')
      entry.focus = await page.evaluate(() => { const s = getComputedStyle(document.activeElement); return { outline: s.outlineStyle, width: s.outlineWidth, color: s.outlineColor } })
      assert.notEqual(entry.focus.outline, 'none')
      await page.emulateMedia({ forcedColors: 'active' }); entry.forcedColors = await page.evaluate(() => getComputedStyle(document.activeElement).outlineStyle); assert.notEqual(entry.forcedColors, 'none')
      if (entry.metrics.footer) { entry.forcedPrintPixels = await printPixels(page); assert.deepEqual(entry.forcedPrintPixels, entry.printPixels) }
      await page.emulateMedia({ forcedColors: 'none' })
      entry.persistence = await page.evaluate(() => ({ sameCanvas: window.__chromeCanvas === document.querySelector('.shell-world-host canvas'), state: window.__QA06__.state() }))
      assert(entry.persistence.sameCanvas); assert.equal(entry.persistence.state.activeSockets, 1); assert.equal(entry.persistence.state.socketPeak, 1)
      if (entry.metrics.footer) {
        await page.setViewportSize({ width, height: 480 })
        if (['meet', 'gifts'].includes(item.id)) entry.shortMoreActions = await checkMoreActions(page, item, true)
        await body.evaluate(e => { e.scrollTop = e.scrollHeight })
        await expect(page.locator('.mcr-chrome-parade-footer img')).toBeInViewport()
        entry.shortViewport = await page.evaluate(metrics)
        assert.equal(entry.shortViewport.overflow, false)
        entry.shortScreenshot = `${item.id}-${width}-short-end.png`; await screenshot(page, entry.shortScreenshot)
      }
    } else {
      const control = item.id === 'landing' ? page.getByRole('link', { name: 'Sign in', exact: true }).first() : page.getByRole('button', { name: 'Close sign-in and return home' })
      await control.focus(); await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab')
      entry.focus = await control.evaluate(e => getComputedStyle(e).outlineStyle); assert.notEqual(entry.focus, 'none')
      await control.click(); await expect(page.locator(item.id === 'landing' ? '.auth-sign-in' : '.public-entry')).toBeVisible()
      entry.publicNavigation = 'PASS'
    }
    assert.deepEqual(errors, [])
    entry.status = 'PASS'
  } catch (error) { entry.status = 'FAIL'; entry.error = error.message }
  finally { await page.evaluate(() => window.__QA06__?.dispose()).catch(() => {}); await context.close(); report.cases.push(entry); await writeFile(resolve(output, 'report.partial.json'), JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify({ id: item.id, width, status: entry.status, error: entry.error })) }
}
try {
  await floor(); await mkdir(output, { recursive: false }); prepared = true
  server = await createServer({ root: frontend, configLoader: 'runner', publicDir: false, cacheDir, logLevel: 'error', resolve: { dedupe: ['react', 'react-dom'] },
    plugins: [{ name: 'qualified-read-only-public-assets', enforce: 'pre', load(id) {
      if (flatPaperCommit && id.split('?')[0] === resolve(repo, baseCssPath)) {
        const base = git('show', `HEAD:${baseCssPath}`).toString(), directive = '@import "./cozy-community-backgrounds-v1.css";'
        assert.equal(base.split(directive).length, 2, 'Exact single Root community background import required')
        return base.replace(directive, git('show', `${flatPaperCommit}:${flatPaperPath}`).toString())
      }
      if (feedCommit && id.split('?')[0] === resolve(repo, feedPath)) return git('show', `${feedCommit}:${feedPath}`).toString()
      if (flatPaperCommit && id.split('?')[0] === resolve(repo, flatPaperPath)) return git('show', `${flatPaperCommit}:${flatPaperPath}`).toString()
    }, configureServer(s) { s.middlewares.use((req, res, next) => {
      const path = 'frontend/public' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname), prefix = Object.keys(art).find(p => path.startsWith(p))
      if (!prefix && !tracked.has(path)) return next()
      if (!mime[extname(path)]) return next()
      try { if (!cache.has(path)) { const bytes = git('show', `${prefix ? art[prefix] : 'HEAD'}:${path}`); assert([...cache.values()].reduce((n, b) => n + b.length, bytes.length) <= 200 * 1024 ** 2, '200MiB asset cache cap'); cache.set(path, bytes); report.assets.push({ path, sha256: hash(bytes), bytes: bytes.length }) }
        res.setHeader('Content-Type', mime[extname(path)]); res.end(cache.get(path)) } catch (error) { next(error) }
    }) } }], server: { host: '127.0.0.1', port: Number(process.env.MCR_CHROME_PORT), strictPort: true, hmr: false } })
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    for (const item of CASES.filter(c => selectedCases.has(c.id))) {
      await capture(server.resolvedUrls.local[0], item, width)
      if (report.cases.at(-1).status === 'FAIL') { report.status = 'FAIL'; throw new Error('Resolve first retained native failure before continuing the cohort') }
    }
    if (process.env.MCR_CHROME_PRODUCTION_ANONYMOUS === '0') continue
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce', serviceWorkers: 'block' })
    const page = await context.newPage(), entry = { id: 'production-anonymous', width, qualification: 'Unmodified index.html and production ports; no service interception or login' }
    try {
      await page.goto(server.resolvedUrls.local[0]); await expect(page.locator('.public-entry')).toBeVisible()
      await page.evaluate(() => document.fonts.ready); await page.waitForLoadState('networkidle')
      entry.metrics = await page.evaluate(metrics); assert.equal(entry.metrics.overflow, false); assert.deepEqual(entry.metrics.brokenImages, [])
      entry.publicHeaderInk = await publicHeaderInk(page)
      entry.screenshot = `production-anonymous-${width}.png`; await screenshot(page, entry.screenshot); entry.status = 'PASS'
    } catch (error) { entry.status = 'FAIL'; entry.error = error.message }
    finally { await context.close(); report.cases.push(entry) }
    if (entry.status === 'FAIL') { report.status = 'FAIL'; throw new Error('Resolve retained production-entry failure') }
  }
  report.status = report.cases.every(c => c.status === 'PASS') ? 'PASS' : 'FAIL'
} finally {
  await browser?.close(); await server?.close(); await rm(cacheDir, { recursive: true, force: true })
  report.finishedAt = new Date().toISOString(); report.screenshotBytes = screenshotBytes; report.cleanup = { browserClosed: true, serverClosed: true, cacheRemoved: true }
  if (prepared) await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
assert.equal(report.status, 'PASS', 'Resolve developer-owned failures before handoff')
