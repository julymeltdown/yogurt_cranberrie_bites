// Root combined adoption harness derived byte-traceably from qualified Feed owner684cab41; source-only until fresh allocation.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { pathToFileURL } from 'node:url'
import { readFile, writeFile, mkdir, mkdtemp, rm, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'

const repo = process.env.MCR_FEED_REPO, phase = process.env.MCR_FEED_PHASE
assert(repo && phase && process.env.MCR_FEED_SLOT, 'explicit named allocation required')
const frontend = resolve(repo, 'frontend'), evidence = resolve(process.env.MCR_FEED_ROOT_OUTPUT)
const req = createRequire(resolve(frontend, 'package.json'))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 16 * 1024 ** 2 })
const fixtureCommit = '008248295c120c75c08c8a4bb8fa2ca2c1c6026a'
const assetCommit = '8f6fe07f389f73f09d9fc61a6012b49846b24ce6'
const cssPath = 'frontend/src/features/event-feed/feed.css'
const baselineCss = git('show', 'eb6e81bd7e5fbeb15613a45e00f1fe896e49f0da:' + cssPath).toString()
const inputs = []
for (const [commit, paths] of [
  [fixtureCommit, ['preview.tsx', 'preview.css', 'preview.html'].map(name => 'frontend/src/features/event-feed/__tests__/css-parity-feed/' + name)],
  [assetCommit, ['extract.py', 'source-provenance.json', 'reference-card-1.png', 'reference-card-2.png'].map(name => 'frontend/public/reference-feed-media-v1/' + name).concat('docs/mcr/art/reference-feed-media-v1.md')],
]) for (const path of paths) {
  const bytes = await readFile(resolve(repo, path))
  assert.deepEqual(bytes, git('show', commit + ':' + path), 'immutable external input: ' + path)
  inputs.push({ path, sourceCommit: commit, sha256: hash(bytes) })
}
const disk = await statfs(repo); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3)
await mkdir(evidence, { recursive: false })
const tmp = await mkdtemp(resolve(process.env.TMPDIR ?? '/tmp', 'mcr-css-feed-reference-'))
const { createServer } = await import(pathToFileURL(req.resolve('vite')))
const { default: react } = await import(pathToFileURL(req.resolve('@vitejs/plugin-react')))
const { chromium, expect } = req('@playwright/test')
const server = await createServer({ root: frontend, configFile: false, plugins: [react()], cacheDir: resolve(tmp, 'cache'), logLevel: 'error', server: { host: '127.0.0.1', port: 5292, strictPort: true, hmr: false } })
const report = { status: 'FAIL', phase, slot: process.env.MCR_FEED_SLOT, head: git('rev-parse', 'HEAD').toString().trim(), cssSha256: hash(await readFile(resolve(repo, cssPath))), baselineCssSha256: hash(baselineCss), inputs, cases: [], errors: [], qualification: 'Retro iteration supersedes historical exact-photo criterion. Actual EventFeed/FeedImage/FeedThread and actual default Auth/router/Phaser shell over frozen synthetic fixtures. No live admission or latest owner209 integration acceptance.' }
report.rootSourcePins = Object.fromEntries(await Promise.all(['frontend/src/shared/styles/cozy-community-backgrounds-v1.css','frontend/src/features/event-feed/feed.css','frontend/src/main.tsx','frontend/src/app/ports.ts'].map(async path => [path, hash(await readFile(resolve(repo, path)))])))
report.driverSha256 = hash(await readFile(resolve(repo, 'docs/mcr/evidence/feed-root-adoption/native.mjs')))
report.defaultFixture = { path: 'frontend/src/app/__tests__/cozy-backgrounds-v1/preview.tsx', sha256: hash(await readFile(resolve(frontend, 'src/app/__tests__/cozy-backgrounds-v1/preview.tsx'))) }
const base = 'http://127.0.0.1:5292'
const feedUrl = base + '/src/features/event-feed/__tests__/css-parity-feed/preview.html'
const rootUrl = base + '/src/app/__tests__/cozy-backgrounds-v1/preview.html?entry=%2Fworld&era=PARADE'
const shot = async (page, name) => { const path = resolve(evidence, name + '.png'); const bytes = await page.screenshot({ path, fullPage: true }); return { path: path.slice(repo.length + 1), sha256: hash(bytes) } }
const media = async page => page.locator('.feed-media img').evaluateAll(es => es.map(e => { const r = e.getBoundingClientRect(); return { width: r.width, height: r.height, naturalWidth: e.naturalWidth, naturalHeight: e.naturalHeight, objectFit: getComputedStyle(e).objectFit, blob: e.src.startsWith('blob:') } }))
const readyMedia = async page => { await expect(page.locator('.feed-media img')).toHaveCount(2); await page.locator('.feed-media img').evaluateAll(es => Promise.all(es.map(e => e.decode()))); await page.evaluate(() => document.fonts.ready) }
const metrics = async page => page.locator('.feed-browse').evaluate(el => {
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom, right: r.right } }
  const style = e => { const s = getComputedStyle(e); return { color: s.color, background: s.backgroundColor, image: s.backgroundImage, radius: s.borderRadius, fontSize: s.fontSize, fontFamily: s.fontFamily, position: s.position, outline: s.outline, lineHeight: s.lineHeight } }
  const body = el.closest('.shell-panel-body')
  return { viewport: { width: innerWidth, height: innerHeight }, documentWidth: document.documentElement.scrollWidth, feed: rect(el), cards: [...el.querySelectorAll('.feed-post')].map(e => ({ ...rect(e), style: style(e) })), copy: [...el.querySelectorAll('.feed-post-copy')].map(e => ({ ...rect(e), text: e.textContent, scrollHeight: e.scrollHeight, clientHeight: e.clientHeight, style: style(e) })), portraits: [...el.querySelectorAll('.feed-portrait')].map(rect), controls: [...el.querySelectorAll('button,summary,a')].filter(e => e.checkVisibility()).map(e => ({ name: e.getAttribute('aria-label') || e.textContent, ...rect(e), style: style(e) })), footer: el.querySelector('.feed-footer') ? { ...rect(el.querySelector('.feed-footer')), style: style(el.querySelector('.feed-footer')) } : null, shell: body ? { ...rect(body), scrollHeight: body.scrollHeight, clientHeight: body.clientHeight, scrollTop: body.scrollTop, overflow: getComputedStyle(body).overflowY, style: style(body) } : null }
})
const validate = data => {
  assert.equal(data.documentWidth, data.viewport.width, 'no document horizontal overflow')
  assert(data.controls.every(c => c.width >= 43.9 && c.height >= 43.9), 'Feed controls >=44 effective pixels: ' + JSON.stringify(data.controls.map(({ name, width, height }) => ({ name, width, height }))))
  assert(data.portraits.every(r => r.width === 48 && r.height === 48), 'portrait dimensions unchanged')
  assert(data.copy.every(c => parseFloat(c.style.fontSize) >= 14 && parseFloat(c.style.fontSize) <= 16 && c.scrollHeight <= c.clientHeight + 1), 'complete readable body text')
  assert(data.cards.every(c => c.style.image === 'none' && c.style.radius === '0px'), 'flat square cards')
  const last = data.cards.at(-1); if (last && data.footer) assert(data.footer.y >= last.bottom - .1, 'composer cannot overlap last post')
}
const contrast = async page => page.locator('.feed-browse').evaluate(el => {
  const rgb = s => s.match(/[\d.]+/g).slice(0,3).map(Number)
  const lum = c => c.map(v => { v /= 255; return v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4 }).reduce((a,v,i) => a + v * [.2126,.7152,.0722][i], 0)
  return [...el.querySelectorAll('.feed-post-copy,.feed-author-line strong,.feed-author-line time,.feed-compose,.mcr-tab')].map(e => {
    const cs = getComputedStyle(e); let p = e, bg
    do { bg = getComputedStyle(p).backgroundColor; p = p.parentElement } while (bg === 'rgba(0, 0, 0, 0)' && p)
    const a = lum(rgb(cs.color)), b = lum(rgb(bg)); return { name: e.getAttribute('aria-label') || e.textContent, ratio: (Math.max(a,b)+.05)/(Math.min(a,b)+.05) }
  })
})
const keyboardFocus = async page => {
  const button = page.getByRole('button', { name: 'Write a post', exact: true })
  await button.focus(); await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab'); await expect(button).toBeFocused()
  const value = await button.evaluate(e => ({ style: getComputedStyle(e).outlineStyle, width: getComputedStyle(e).outlineWidth, visible: e.matches(':focus-visible') }))
  assert.equal(value.style, 'solid'); assert.equal(value.width, '3px'); assert(value.visible); return value
}
const reachComposer = async page => {
  const button = page.getByRole('button', { name: 'Write a post', exact: true }); await button.scrollIntoViewIfNeeded()
  const r = await button.boundingBox(); assert(r && r.y >= 0 && r.y + r.height <= (await page.viewportSize()).height + 1, 'composer reachable in viewport')
  const unobscured = await button.evaluate(e => { const r = e.getBoundingClientRect(); return e.contains(document.elementFromPoint(r.x + r.width/2, r.y + r.height/2)) })
  assert(unobscured, 'composer hit target unobscured'); return { rect: r, unobscured, geometry: await metrics(page) }
}
let browser, page, stage = 'boot'
try {
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360,390,430]) for (const variant of ['before','after']) for (const surface of ['fixture','default-shell']) {
    stage = `${variant}/${surface}/${width}`
    const context = await browser.newContext({ viewport: { width, height: 843 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    if (variant === 'before') await page.route('**/src/features/event-feed/feed.css*', route => route.fulfill({ contentType: 'text/javascript', body: 'import { updateStyle } from "/@vite/client"; updateStyle("feed-retro-before", ' + JSON.stringify(baselineCss) + ');' }))
    await page.goto(surface === 'fixture' ? feedUrl : rootUrl)
    if (surface === 'fixture') { await expect(page.locator('.feed-post')).toHaveCount(2); await readyMedia(page) }
    else {
      await expect.poll(() => page.evaluate(() => window.__COZY_PANEL__?.state().phase), { timeout: 20000 }).toBe('live')
      await page.evaluate(() => window.__COZY_PANEL__.navigate('/feed')); await expect(page.locator('.feed-post')).toHaveCount(1); await expect(page.locator('.feed-portrait canvas')).toBeVisible()
    }
    const row = { width, variant, surface, geometry: await metrics(page), media: await media(page), capture: await shot(page, `${variant}-${surface}-${width}`) }; report.cases.push(row)
    if (variant === 'after' && surface === 'default-shell') { assert.equal(row.geometry.shell.style.background, 'rgb(248, 245, 236)', 'Root shared body matches Feed paper'); assert.equal(row.geometry.shell.style.image, 'none', 'Root shared Feed body has no obsolete pastel backdrop') }
    assert.equal(row.geometry.documentWidth, width)
    if (surface === 'fixture') {
      assert.deepEqual(row.media.map(i => [i.naturalWidth,i.naturalHeight]), [[197,67],[197,46]])
      assert(row.media.every(i => i.blob && i.objectFit === 'contain' && Math.abs(i.width/i.height-i.naturalWidth/i.naturalHeight) < .01), 'complete approved thumbnails')
      row.qualification = await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.qualification)
    } else { row.state = await page.evaluate(() => window.__COZY_PANEL__.state()); assert.equal(row.state.canvases,1); assert.equal(row.state.socketPeak,1) }
    if (variant === 'after') {
      validate(row.geometry); row.contrast = await contrast(page); assert(row.contrast.every(c => c.ratio >= 4.5), 'semantic text contrast >=4.5: ' + JSON.stringify(row.contrast))
      row.focus = await keyboardFocus(page)
      if (width === 390) row.focusCapture = await shot(page, `focus-${surface}-390`)
      // Actual shell DOM is recorded above before any sizing correction.
      row.reach = await reachComposer(page)
      await page.setViewportSize({ width, height: 480 }); row.shortBeforeScroll = await metrics(page); row.shortReach = await reachComposer(page); row.shortCapture = await shot(page, `short-${surface}-${width}`); validate(row.shortReach.geometry)
      await page.setViewportSize({ width, height: 843 })
      if (surface === 'fixture') {
        const before = report.cases.find(r => r.width === width && r.surface === surface && r.variant === 'before')
        assert.deepEqual(row.geometry.copy.map(c => c.text), before.geometry.copy.map(c => c.text), 'unchanged real text')
        await page.getByRole('button', { name: 'Open thread · 7 comments', exact: true }).click(); await expect(page.getByRole('region', { name: 'Post thread' })).toBeVisible(); await expect(page.locator('.feed-media img')).toHaveCount(1)
        row.thread = { media: await media(page), capture: await shot(page, `thread-${width}`) }; assert.equal(row.thread.media[0].naturalWidth,197); assert(row.thread.media[0].width <=197.1)
        await page.locator('.concert-window__close').click(); await readyMedia(page)
        await page.getByRole('tab', { name: 'ALL', exact: true }).focus(); await page.keyboard.press('ArrowRight'); await expect(page.getByRole('tab', { name: 'MEETUP', exact: true })).toBeFocused(); await expect(page.getByText('No posts yet.', { exact: true })).toBeVisible()
        await page.keyboard.press('End'); await expect(page.getByRole('tab', { name: 'NOTICE', exact: true })).toBeFocused()
        await page.getByRole('button', { name: 'Fixture notice', exact: true }).click(); const source = page.getByRole('link', { name: 'Source', exact: true }); await expect(source).toBeVisible(); const sourceBox = await source.boundingBox(); assert(sourceBox.width >=44 && sourceBox.height >=44, 'notice source effective target')
        await page.getByRole('tab', { name: 'ALL', exact: true }).click(); await readyMedia(page)
        await page.locator('.feed-tools summary').click(); const label = await page.getByRole('checkbox', { name: 'Images only' }).locator('..').boundingBox(); assert(label.height >=44); await page.getByRole('checkbox', { name: 'Images only' }).check(); await page.locator('.feed-tools summary').click()
        await page.getByRole('button', { name: 'Refresh feed', exact: true }).click(); await readyMedia(page)
        if (width === 390) { await page.emulateMedia({ forcedColors: 'active' }); row.forcedFocus = await keyboardFocus(page); row.forcedCapture = await shot(page, 'forced-focus-390'); await page.emulateMedia({ forcedColors: 'none' }) }
        await page.getByRole('button', { name: 'Write a post', exact: true }).click(); await expect(page.locator('#destination')).toHaveText('Fixture compose destination')
        await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.invalidate()); await expect(page.locator('.feed-media img')).toHaveCount(0); assert.equal(await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.resources()),0)
      } else {
        await page.getByRole('button', { name: 'Write a post', exact: true }).click(); await expect(page.getByRole('textbox', { name: 'Post text', exact: true })).toBeVisible()
      }
    }
    if (surface === 'fixture') { await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.dispose()); assert.equal(await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.resources()),0) }
    else { await page.evaluate(() => window.__COZY_PANEL__.dispose()); const state = await page.evaluate(() => window.__COZY_PANEL__.state()); assert.equal(state.activeSockets,0); assert.equal(state.canvases,0) }
    await context.close()
  }
  const context = await browser.newContext({ viewport: { width: 390, height: 480 }, reducedMotion: 'reduce' }); page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
  report.states = []
  for (const mode of ['empty','readonly','unavailable','pending','error']) {
    stage = 'state/' + mode; await page.goto(feedUrl + '?mode=' + mode)
    if (mode === 'readonly') { await expect(page.locator('.feed-post')).toHaveCount(2); await readyMedia(page); await expect(page.getByRole('button', { name: 'Write a post' })).toHaveCount(0) }
    else if (mode === 'error') await expect(page.getByRole('alert')).toBeVisible()
    else await expect(page.locator('.feed-browse [role=status]').first()).toBeVisible()
    const data = await metrics(page); assert.equal(data.documentWidth,390); report.states.push({ mode, geometry: data, capture: await shot(page, 'state-' + mode) }); await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.dispose())
  }
  await context.close(); assert.deepEqual(report.errors,[]); report.status = 'PASS'
} catch (error) {
  report.failure = { stage, error: String(error.stack) }; await page?.screenshot({ path: resolve(evidence, 'failure.png'), fullPage: true }).catch(() => {}); throw error
} finally {
  await browser?.close(); await server.close(); await rm(tmp, { recursive: true, force: true }); report.cleanup = { browserClosed: true, viteClosed: true, privateCacheRemoved: true }; await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report,null,2)+'\n')
}
console.log(JSON.stringify({ status: report.status, phase, cases: report.cases.length }))
