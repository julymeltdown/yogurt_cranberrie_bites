// CSS-worker evidence only. Root owns the immutable fixture; artist owns its PNGs.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { pathToFileURL } from 'node:url'
import { readFile, writeFile, mkdir, mkdtemp, rm, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'

const repo = process.env.MCR_FEED_REPO, phase = process.env.MCR_FEED_PHASE
assert(repo && phase && process.env.MCR_FEED_SLOT, 'explicit named allocation required')
const frontend = resolve(repo, 'frontend'), evidence = resolve(repo, 'docs/mcr/evidence/css-parity-feed', phase)
const req = createRequire(resolve(frontend, 'package.json'))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 16 * 1024 ** 2 })
const fixtureCommit = '008248295c120c75c08c8a4bb8fa2ca2c1c6026a'
const assetCommit = '8f6fe07f389f73f09d9fc61a6012b49846b24ce6'
const cssPath = 'frontend/src/features/event-feed/feed.css'
const baselineCss = git('show', '654bbb16a045c3ce6306066eb66a36f6c34f471e:' + cssPath).toString()
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
const report = { status: 'FAIL', phase, slot: process.env.MCR_FEED_SLOT, head: git('rev-parse', 'HEAD').toString().trim(), cssSha256: hash(await readFile(resolve(repo, cssPath))), baselineCssSha256: hash(baselineCss), inputs, cases: [], errors: [], qualification: 'Actual Feed/FeedImage/FeedThread over Root synthetic fixture. Complete approved screenshot thumbnails with pinned native metadata and explicitly permitted CSS display scaling in the Feed fixture; no original photographs, live posts, real HTTP acceptance or full parity certification.' }
report.driverSha256 = hash(await readFile(resolve(repo, 'docs/mcr/evidence/css-parity-feed/native-reference-media.mjs')))
report.trackedChanges = git('diff', '--name-only').toString().trim().split('\n').filter(Boolean)
report.displayPolicy = 'Latest human/Main clarification permits CSS display enlargement of complete approved fixture crops. Source bytes and true intrinsic dimensions stay pinned. Feed fixture only; Thread native display and production authorized-media policy remain unchanged.'
const geometry = async page => page.locator('.reference-frame').evaluate(frame => {
  const rect = element => { const r = element.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } }
  return { viewport: innerWidth, documentWidth: document.documentElement.scrollWidth, frame: rect(frame), cards: [...frame.querySelectorAll('.feed-post')].map(rect), authors: [...frame.querySelectorAll('.feed-author-line strong')].map(rect), copy: [...frame.querySelectorAll('.feed-post-copy')].map(e => ({ ...rect(e), textStartX: e.getBoundingClientRect().x + parseFloat(getComputedStyle(e).paddingLeft) })), actions: [...frame.querySelectorAll('.feed-post-actions button')].map(rect), portraits: [...frame.querySelectorAll('.feed-portrait')].map(rect), images: [...frame.querySelectorAll('.feed-media img')].map(e => ({ ...rect(e), naturalWidth: e.naturalWidth, naturalHeight: e.naturalHeight, declaredWidth: e.getAttribute('width'), declaredHeight: e.getAttribute('height'), blob: e.src.startsWith('blob:') })), figures: [...frame.querySelectorAll('.feed-media figure')].map(rect) }
})
const capture = async (page, name) => {
  const path = resolve(evidence, name + '.png'), bytes = await page.screenshot({ path, fullPage: true })
  return { path: path.slice(repo.length + 1), sha256: hash(bytes) }
}
const readyImages = async (page, count) => {
  await expect(page.locator('.feed-media img')).toHaveCount(count)
  await page.locator('.feed-media img').evaluateAll(es => Promise.all(es.map(e => e.decode())))
  await page.evaluate(() => document.fonts.ready)
}
const assertMedia = (images, heights, displayWidth = null) => {
  assert.equal(images.length, heights.length)
  images.forEach((image, i) => {
    assert.equal(image.naturalWidth, 197); assert.equal(image.naturalHeight, heights[i])
    assert.equal(image.declaredWidth, '197'); assert.equal(image.declaredHeight, String(heights[i])); assert(image.blob)
    if (displayWidth === null) assert(image.width <= 197.1 && image.height <= heights[i] + .1, 'Thread/baseline native display remains unchanged')
    else assert(Math.abs(image.width - displayWidth) < .1, 'approved reference Feed display width')
    assert(Math.abs(image.width / image.height - 197 / heights[i]) < .01, 'complete native crop retains ratio')
  })
}
let browser, page, stage = 'boot'
try {
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) for (const variant of ['published', 'candidate']) {
    stage = variant + '/' + width
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    if (variant === 'published') await page.route('**/src/features/event-feed/feed.css*', route => route.fulfill({ contentType: 'text/javascript', body: 'import { updateStyle } from "/@vite/client"; updateStyle("feed-published-comparison", ' + JSON.stringify(baselineCss) + ');' }))
    await page.goto('http://127.0.0.1:5292/src/features/event-feed/__tests__/css-parity-feed/preview.html')
    await expect(page.locator('.feed-post')).toHaveCount(2); await readyImages(page, 2)
    const row = { width, variant, feed: await geometry(page), capture: await capture(page, variant + '-feed-' + width), detail: [] }
    report.cases.push(row); assert.equal(row.feed.documentWidth, width); assertMedia(row.feed.images, [67, 46], variant === 'candidate' ? width - 137 : null)
    row.qualification = await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.qualification)
    assert.equal(row.qualification.inputCommit, assetCommit); assert.equal(row.qualification.productionMedia, false)
    for (const [index, height] of [67, 46].entries()) {
      await page.locator('.feed-post-actions button[aria-label^="Open thread"]').nth(index).click()
      await expect(page.getByRole('region', { name: 'Post thread' })).toBeVisible(); await readyImages(page, 1)
      const detail = { index, geometry: await geometry(page), capture: await capture(page, variant + '-thread-' + index + '-' + width) }
      row.detail.push(detail); assert.equal(detail.geometry.documentWidth, width); assertMedia(detail.geometry.images, [height])
      await page.locator('.concert-window__close').click(); await expect(page.locator('.feed-post')).toHaveCount(2); await readyImages(page, 2)
    }
    if (variant === 'candidate') {
      row.selectedTabDecoration = await page.getByRole('tab', { name: 'ALL', exact: true }).evaluate(e => getComputedStyle(e).textDecorationLine)
      assert.equal(row.selectedTabDecoration, 'none', 'Feed selected-tab override beats shared underline specificity')
      const previous = report.cases.find(item => item.width === width && item.variant === 'published')
      assert(row.feed.authors.every(item => Math.abs(item.x - 105) < .1), 'reference author column at x105')
      assert(row.feed.copy.every(item => Math.abs(item.textStartX - 105) < .1), 'reference body column at x105')
      assert(row.feed.figures.every(item => Math.abs(item.x - 102) < .1), 'reference media column at x102')
      assert.deepEqual(row.feed.portraits, previous.feed.portraits.map((item, i) => ({ ...item, y: row.feed.portraits[i].y })), 'portrait x/width/height preserved')
      assert.deepEqual(row.feed.actions.map(({ x, width }) => ({ x, width })), previous.feed.actions.map(({ x, width }) => ({ x, width })), 'action horizontal positions/target widths preserved')
      assert(row.feed.actions.every(item => item.width >= 44 && item.height >= 44), '44px action targets')
      row.threadGeometryUnchanged = row.detail.every((item, i) => JSON.stringify(item.geometry) === JSON.stringify(previous.detail[i].geometry))
      assert(row.threadGeometryUnchanged, 'Feed-only CSS must preserve actual Thread geometry')
      await page.getByRole('button', { name: 'Write a post' }).focus()
      await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab')
      await expect(page.getByRole('button', { name: 'Write a post' })).toBeFocused()
      row.focus = await page.getByRole('button', { name: 'Write a post' }).evaluate(e => getComputedStyle(e).outline)
      assert.match(row.focus, /3px/)
      assert.notEqual(await page.getByRole('button', { name: 'Write a post' }).evaluate(e => getComputedStyle(e).outlineStyle), 'none', 'keyboard focus must be visibly painted')
      if (width === 390) row.focusCapture = await capture(page, 'candidate-keyboard-focus-390')
      await page.keyboard.press('Enter'); await expect(page.locator('#destination')).toHaveText('Fixture compose destination')
      await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.invalidate()); await expect(page.locator('.feed-media img')).toHaveCount(0)
      assert.equal(await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.resources()), 0)
    }
    await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.dispose()); assert.equal(await page.evaluate(() => window.__REFERENCE_MEDIA_FEED__.resources()), 0)
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.status = 'PASS'
} catch (error) {
  report.failure = { stage, error: String(error.stack) }; await page?.screenshot({ path: resolve(evidence, 'failure.png') }).catch(() => {}); throw error
} finally {
  await browser?.close(); await server.close(); await rm(tmp, { recursive: true, force: true })
  report.cleanup = { browserClosed: true, viteClosed: true, privateCacheRemoved: true }
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(JSON.stringify({ status: report.status, phase, cases: report.cases.length }))
