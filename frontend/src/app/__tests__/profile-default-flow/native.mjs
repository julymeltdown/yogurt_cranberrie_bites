// New actual-main default journey only. Reuse earlier25/45/7 qualification.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFile, writeFile, mkdir, stat, statfs } from 'node:fs/promises'
import { resolve, extname } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { frozenSource } from './frozen-source.mjs'

const repo = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(repo, 'frontend')
const output = process.env.MCR_PROFILE_DEFAULT_OUTPUT, temporary = process.env.MCR_PROFILE_DEFAULT_TMP
const job = process.env.MCR_PROFILE_DEFAULT_JOB, port = Number(process.env.MCR_PROFILE_DEFAULT_PORT)
assert(output?.startsWith(repo + '/docs/mcr/evidence/profile-activity-media/default-flow/') && temporary?.startsWith('/private/tmp/mcr-profile-default-') && job && port > 1023)
const source = frozenSource(repo, process.env.MCR_PROFILE_DEFAULT_SOURCE)
const input = JSON.parse(source.bytes('frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'))
const localInput = JSON.parse(source.bytes('frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/local-input.json'))
const report = { job, status: 'IN_PROGRESS', candidate: source.revision,
  harnessHead: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim(),
  qualification: 'Actual frozen main, canonical UI login, default ports/router/providers and actual Phaser; controlled strict HTTP/C05 only, not live IN03',
  reused: 'Earlier25 leaf,45 cached-media,7 MeHub native and Root combined caller checks. No replay.',
  cases: [], pngBytes: 0, startedAt: new Date().toISOString() }
let server, browser, context
const state = page => page.evaluate(() => window.__PROFILE_DEFAULT_FLOW__.state())
async function navigate(page, label, beforeClick) {
  const navigation = page.getByRole('navigation', { name: 'Main navigation' })
  // Actual mobile full-panel UI hides shell chrome. Use its real restore control.
  if (!await navigation.isVisible()) {
    await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
    await expect(navigation).toBeVisible()
    await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))))
  }
  await beforeClick?.()
  await navigation.getByRole('link', { name: label, exact: true }).click()
}
async function shot(page, filename) {
  await page.screenshot({ path: resolve(output, filename) })
  report.pngBytes += (await stat(resolve(output, filename))).size
  assert(report.pngBytes <= 4 * 1024 ** 2); return filename
}
async function signout(page) {
  await page.getByText('Profile options', { exact: true }).click()
  await page.getByRole('link', { name: 'Account', exact: true }).click()
  await page.getByRole('button', { name: 'Sign out', exact: true }).click()
  await page.getByRole('button', { name: 'Confirm sign out', exact: true }).click()
  await expect.poll(async () => { const s = await state(page); return s.activeSockets === 0 && s.canvases === 0 && s.urls.created.every(url => s.urls.revoked.includes(url)) }).toBe(true)
}
try {
  const disk = await statfs(repo); assert(disk.bavail * disk.bsize >= 8 * 1024 ** 3)
  await mkdir(output, { recursive: false })
  server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(temporary, 'vite'), publicDir: false, logLevel: 'error',
    define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(input.event.event_id), 'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(localInput)) },
    plugins: [source.plugin, { name: 'profile-default-frozen-public-and-spa', configureServer(s) {
      s.middlewares.use(async (req, res, next) => {
        const path = 'frontend/public' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname)
        if (source.files.has(path)) {
          const mime = { '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.ttf': 'font/ttf', '.woff2': 'font/woff2' }[extname(path)]
          if (mime) res.setHeader('Content-Type', mime)
          res.end(source.bytes(path)); return
        }
        if (!req.headers.accept?.includes('text/html')) return next()
        res.setHeader('Content-Type', 'text/html')
        res.end(await s.transformIndexHtml(req.url, await readFile(resolve(import.meta.dirname, 'preview.html'), 'utf8')))
      })
    } }], server: { host: '127.0.0.1', port, strictPort: true, hmr: false, watch: null } })
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const mode of ['loaded', 'denied-media', 'cold']) {
    context = await browser.newContext({ viewport: { width: 390, height: 844 }, reducedMotion: 'reduce', serviceWorkers: 'block' })
    const page = await context.newPage(), entry = { mode, status: 'IN_PROGRESS', errors: [] }; report.cases.push(entry)
    page.setDefaultTimeout(10000); page.on('pageerror', error => entry.errors.push(error.message))
    await page.goto(`http://127.0.0.1:${port}/login?mode=${mode}`)
    await page.getByLabel('Email', { exact: true }).fill('profile-controlled@example.test')
    await page.getByLabel('Password', { exact: true }).fill('controlled-password')
    await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
    await expect(page.locator('.shell-world-host canvas')).toHaveCount(1)
    await expect.poll(async () => (await state(page)).activeSockets).toBe(1)
    await expect.poll(async () => (await state(page)).admittedStreams).toBe(1)
    const host = page.locator('[data-world-host]')
    await host.evaluate(element => Reflect.set(element, '__profileIncumbent', 'same'))
    if (mode !== 'cold') {
      await navigate(page, 'Feed'); await expect(page).toHaveURL(/\/feed(?:\?|$)/)
      // Actual server C03 Feed=true cannot establish SOCIAL availability by itself.
      await expect(page.getByText('This section is unavailable for the current account or event.', { exact: true })).toBeVisible()
      entry.beforePolicy = await state(page); assert.equal(entry.beforePolicy.feedReads, 0)
      await page.evaluate(() => window.__PROFILE_DEFAULT_FLOW__.baseline())
      await expect(page.locator('.feed-posts > li')).toHaveCount(2)
      entry.loadedFeedUrls = []
      for (let index = 0; index < 2; index++) {
        const first = page.locator('.feed-post').nth(index).locator('.feed-media figure').first()
        await first.scrollIntoViewIfNeeded()
        if (mode === 'loaded') {
          const image = first.locator('.feed-image'); await expect(image).toBeVisible()
          await expect.poll(() => image.evaluate(element => element.complete && element.naturalWidth === 197)).toBe(true)
          entry.loadedFeedUrls.push(await image.getAttribute('src'))
        } else await expect(first.getByText('Image unavailable. Post text remains available.', { exact: true })).toBeVisible()
      }
      entry.feed = await state(page); assert.equal(entry.feed.feedReads, 1, 'No readiness /feed probe or duplicate Feed load')
    } else await page.evaluate(() => window.__PROFILE_DEFAULT_FLOW__.baseline())
    await navigate(page, 'Me', async () => { entry.beforeProfile = await state(page) }); await expect(page).toHaveURL(/\/me(?:\?|$)/)
    await expect(page.locator('.me-hub')).toBeVisible()
    if (mode === 'cold') {
      await expect(page.getByText('Recent activity is unavailable in this connection.', { exact: true })).toBeVisible()
      entry.profile = await state(page); assert.equal(entry.profile.feedReads, 0); assert.equal(entry.profile.mediaReads, 0)
    } else {
      await expect(page.getByRole('link', { name: /Posted to the feed/ })).toHaveCount(2)
      if (mode === 'loaded') {
        await expect(page.locator('.reference-activity__visual img')).toHaveCount(2)
        await page.waitForFunction(() => [...document.querySelectorAll('.reference-activity__visual img')].every(image => image.complete && image.naturalWidth > 0))
      } else await expect(page.locator('.reference-activity__visual .concert-icon')).toHaveCount(2)
      entry.profile = await state(page)
      assert.equal(entry.beforeProfile.feedReads, 1, 'Reducing Feed must not reload its page')
      assert.equal(entry.profile.feedReads, entry.beforeProfile.feedReads); assert.equal(entry.profile.mediaReads, entry.beforeProfile.mediaReads, 'Entering Profile cannot start media reads')
    }
    assert.equal(await host.evaluate(element => Reflect.get(element, '__profileIncumbent')), 'same')
    assert.equal(entry.profile.socketsCreated, 1); assert.equal(entry.profile.canvases, 1)
    assert.equal(entry.profile.peakSockets, 1); assert.equal(entry.profile.peakCanvases, 1)
    entry.media = await page.locator('.reference-activity__visual img').evaluateAll(images => images.map(image => ({ url: image.src, width: image.getBoundingClientRect().width, height: image.getBoundingClientRect().height })))
    for (const image of entry.media) assert(image.url.startsWith('blob:') && image.width <= 52 && image.height <= 52)
    if (mode === 'loaded') assert.deepEqual(entry.media.map(image => image.url), entry.loadedFeedUrls, 'Profile borrows the exact existing Feed Blob URLs')
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    if (mode !== 'cold') await page.locator('.reference-activity').scrollIntoViewIfNeeded()
    entry.screenshot = await shot(page, `default-${mode}-390.png`)
    if (mode !== 'cold') {
      const links = page.getByRole('link', { name: /Posted to the feed/ }); await links.nth(0).focus(); await page.keyboard.press('Tab'); await expect(links.nth(1)).toBeFocused()
      assert.notEqual(await links.nth(1).evaluate(element => getComputedStyle(element).outlineStyle), 'none')
      await page.evaluate(() => window.__PROFILE_DEFAULT_FLOW__.invalidate())
      await expect(page.getByRole('link', { name: /Posted to the feed/ })).toHaveCount(0)
      await expect(page.locator('.reference-activity__visual img')).toHaveCount(0)
      await expect.poll(async () => { const s = await state(page); return entry.media.every(image => s.urls.revoked.includes(image.url)) }).toBe(true)
      // Do not rebaseline after invalidation; old cached rows must stay unavailable.
      await expect(page.getByText('Profile options', { exact: true })).toBeVisible()
      await expect(page.getByRole('link', { name: /Posted to the feed/ })).toHaveCount(0)
      await expect(page.locator('.reference-activity__visual img')).toHaveCount(0)
      entry.invalidated = await state(page)
      assert.equal(entry.invalidated.feedReads, entry.profile.feedReads); assert.equal(entry.invalidated.mediaReads, entry.profile.mediaReads)
      assert(entry.invalidated.peakSockets <= 1 && entry.invalidated.peakCanvases <= 1)
    }
    await signout(page); entry.signedOut = await state(page)
    assert.deepEqual(entry.errors, []); entry.status = 'PASS'
    await context.close(); context = null
    await writeFile(resolve(output, 'report.partial.json'), JSON.stringify(report, null, 2) + '\n')
  }
  report.status = 'PASS'
} catch (error) {
  report.status = 'FAIL'; report.error = String(error.stack ?? error); process.exitCode = 1
  report.failure = await context?.pages()[0]?.evaluate(() => ({ state: window.__PROFILE_DEFAULT_FLOW__?.state(), text: document.body.innerText })).catch(() => null)
} finally {
  await context?.close(); await browser?.close(); await server?.close()
  report.sourceReceipts = source.receipts(); report.finishedAt = new Date().toISOString()
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
