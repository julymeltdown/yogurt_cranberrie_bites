// FE34 focused changed-candidate proof. Run only with a fresh accepted browser allocation.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, rm, stat, statfs, writeFile } from 'node:fs/promises'
import { extname, resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const repo = resolve(import.meta.dirname, '../../../../../..'), frontend = resolve(repo, 'frontend')
const output = process.env.MCR_PROFILE_MEDIA_OUTPUT, temporary = process.env.MCR_PROFILE_MEDIA_TMP
assert(process.env.MCR_PROFILE_MEDIA_JOB && output?.startsWith(resolve(repo, 'docs/mcr/evidence/profile-activity-media/') + '/'))
assert(temporary?.startsWith('/private/tmp/mcr-profile-activity-media-'))
assert(/^\d+$/.test(process.env.MCR_PROFILE_MEDIA_PORT ?? ''))
const hash = value => createHash('sha256').update(value).digest('hex')
const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 64 * 1024 ** 2 })
const reference = 'docs/mcr/design/reference-target-2026-11-04.png'
assert.equal(hash(await readFile(resolve(repo, reference))), 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617')
const paths = ['frontend/src/features/me/ui/reference-profile/ProfileActivity.tsx', 'frontend/src/features/me/ui/MePages.tsx']
const report = { task: 'FE34-PROFILE-ACTIVITY-MEDIA-01', job: process.env.MCR_PROFILE_MEDIA_JOB, status: 'IN_PROGRESS',
  head: git('rev-parse', 'HEAD').toString().trim(), diffSha256: hash(git('diff', 'HEAD')), reference,
  sources: await Promise.all(paths.map(async path => ({ path, sha256: hash(await readFile(resolve(repo, path))) }))),
  qualification: 'Actual MeHub and protected Root cached-only thumbnail with canonical Auth and labeled synthetic authorized Feed/media HTTP. Explicit Feed load precedes loaded Profile; cold Profile performs no Feed/media read.',
  limitations: ['No live service, physical device, full ten-screen or whole FE34 acceptance', 'Existing avatar and CSS remain unchanged; synthetic POST photo is not a new product/art binding'], cases: [], startedAt: new Date().toISOString() }
const publicFiles = new Set(git('ls-files', 'frontend/public').toString().trim().split('\n'))
const mime = { '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.jpg': 'image/jpeg', '.webp': 'image/webp', '.woff2': 'font/woff2', '.ttf': 'font/ttf' }
const cache = new Map(), cacheDir = resolve(temporary, 'vite')
let server, browser, bytes = 0
async function capture(page, name) {
  await page.screenshot({ path: resolve(output, name) })
  bytes += (await stat(resolve(output, name))).size; assert(bytes <= 4 * 1024 ** 2)
  return name
}
async function metrics(page) {
  return page.evaluate(() => {
    const box = e => { const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height, bottom: b.bottom, right: b.right } }
    return { horizontalOverflow: document.documentElement.scrollWidth > innerWidth,
      rows: [...document.querySelectorAll('.reference-activity__list li')].map(row => {
        const visual = row.querySelector('.reference-activity__visual'), image = visual.querySelector('img'), link = row.querySelector('.reference-activity__entry')
        return { visual: box(visual), image: image ? { ...box(image), src: image.getAttribute('src'), complete: image.complete, naturalWidth: image.naturalWidth } : null,
          fallback: !!visual.querySelector('.concert-icon'), interactiveVisual: !!visual.querySelector('a,button,input,[tabindex]'), link: box(link), href: link.getAttribute('href'), text: link.textContent }
      }) }
  })
}
try {
  const fs = await statfs(repo); assert(fs.bavail * fs.bsize >= 8 * 1024 ** 3)
  await mkdir(output, { recursive: false })
  server = await createServer({ root: frontend, configLoader: 'runner', publicDir: false, cacheDir, logLevel: 'error',
    resolve: { dedupe: ['react', 'react-dom'] }, plugins: [{ name: 'frozen-profile-public-assets', configureServer(s) {
      s.middlewares.use((req, res, next) => {
        const path = 'frontend/public' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname)
        if (!publicFiles.has(path) || !mime[extname(path)]) return next()
        try {
          if (!cache.has(path)) { const value = git('show', `HEAD:${path}`); assert([...cache.values()].reduce((n, b) => n + b.length, value.length) <= 32 * 1024 ** 2); cache.set(path, value) }
          res.setHeader('Content-Type', mime[extname(path)]); res.end(cache.get(path))
        } catch (error) { next(error) }
      })
    } }], server: { host: '127.0.0.1', port: Number(process.env.MCR_PROFILE_MEDIA_PORT), strictPort: true, hmr: false } })
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) for (const mode of ['loaded', 'unavailable']) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, isMobile: true, hasTouch: true, reducedMotion: 'reduce' })
    const page = await context.newPage(), entry = { width, height: 844, mode, errors: [] }
    page.on('pageerror', error => entry.errors.push(error.message))
    try {
      await page.goto(`${server.resolvedUrls.local[0]}src/features/me/__tests__/profile-activity-media/preview.html?mode=${mode}`)
      await page.waitForFunction(() => window.__PROFILE_ACTIVITY_MEDIA__?.state().ready)
      await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__.prepareFeed())
      const before = await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__.state())
      await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__.showProfile())
      await expect(page.getByRole('link', { name: /Posted to the feed/ })).toHaveCount(2)
      if (mode === 'loaded') {
        await expect(page.locator('.reference-activity__visual img')).toHaveCount(before.expectedImages)
        await page.waitForFunction(expected => { const images = [...document.querySelectorAll('.reference-activity__visual img')]; return images.length === expected && images.every(image => image.complete && image.naturalWidth > 0) }, before.expectedImages)
      }
      else await expect(page.locator('.reference-activity__visual .concert-icon')).toHaveCount(2)
      await page.evaluate(() => document.fonts.ready)
      entry.initial = await metrics(page); entry.before = before; entry.after = await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__.state())
      assert.equal(entry.initial.horizontalOverflow, false)
      assert.deepEqual(entry.after.readCounts, before.readCounts, 'Opening Profile cannot start Feed/media HTTP')
      assert.equal(entry.initial.rows.length, 2)
      for (const row of entry.initial.rows) {
        assert(row.visual.width >= 44 && row.visual.width <= 54 && row.visual.height >= 44 && row.visual.height <= 54)
        assert(row.link.width >= 44 && row.link.height >= 44); assert.equal(row.interactiveVisual, false)
        if (row.image) {
          assert(row.image.src.startsWith('blob:') && row.image.complete && row.image.naturalWidth > 0)
          assert(row.image.width <= row.visual.width + 1 && row.image.height <= row.visual.height + 1)
        }
      }
      entry.screenshot = await capture(page, `profile-${mode}-${width}.png`)
      const links = page.getByRole('link', { name: /Posted to the feed/ })
      await links.nth(0).focus(); await page.keyboard.press('Tab'); await expect(links.nth(1)).toBeFocused()
      assert.notEqual(await links.nth(1).evaluate(e => getComputedStyle(e).outlineStyle), 'none')
      await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__.revoke())
      await expect(page.locator('.reference-activity__visual img')).toHaveCount(0)
      await expect(links).toHaveCount(0)
      entry.revoked = await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__.state())
      assert.deepEqual(entry.revoked.readCounts, before.readCounts)
      if (width === 390) entry.revokeScreenshot = await capture(page, `profile-${mode}-revoked-${width}.png`)
      assert.deepEqual(entry.errors, []); entry.status = 'PASS'
    } catch (error) { entry.error = error.message; entry.status = 'FAIL' }
    finally { await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__?.dispose()).catch(() => {}); await context.close(); report.cases.push(entry); await writeFile(resolve(output, 'report.partial.json'), JSON.stringify(report, null, 2) + '\n') }
    console.log(JSON.stringify({ width, mode, status: entry.status, error: entry.error }))
    assert.equal(entry.status, 'PASS', 'Resolve retained changed-candidate failure before continuing')
  }
  const context = await browser.newContext({ viewport: { width: 390, height: 844 } }), page = await context.newPage()
  try {
    await page.goto(`${server.resolvedUrls.local[0]}src/features/me/__tests__/profile-activity-media/preview.html?mode=cold`)
    await page.waitForFunction(() => window.__PROFILE_ACTIVITY_MEDIA__?.state().ready)
    await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__.showProfile())
    await expect(page.getByText('Recent activity is unavailable in this connection.')).toBeVisible()
    const state = await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__.state())
    assert.deepEqual(state.readCounts, { feed: 0, media: 0 }); assert.equal(await page.locator('.reference-activity__visual img').count(), 0)
    report.cases.push({ width: 390, height: 844, mode: 'cold', status: 'PASS', state, screenshot: await capture(page, 'profile-cold-390.png') })
  } finally { await page.evaluate(() => window.__PROFILE_ACTIVITY_MEDIA__?.dispose()).catch(() => {}); await context.close() }
  report.status = 'PASS'
} finally {
  await browser?.close(); await server?.close(); await rm(cacheDir, { recursive: true, force: true })
  if (report.status !== 'PASS') report.status = 'FAIL'
  report.finishedAt = new Date().toISOString(); report.pngBytes = bytes
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
