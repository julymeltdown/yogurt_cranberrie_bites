// Developer-owned CSS validation. Uses the existing actual-router synthetic fixture.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile, rm, readdir, lstat, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const repo = resolve(import.meta.dirname, '../../../..'), frontend = resolve(repo, 'frontend')
const require = createRequire(resolve(frontend, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')))
const { chromium, expect } = require('@playwright/test')
assert(process.env.MCR_SIGNIN_JOB, 'A fresh named, accepted resource lease is required')
const round = process.env.MCR_SIGNIN_ROUND ?? 'first'
assert(/^[a-z0-9-]+$/.test(round))
const output = resolve(import.meta.dirname, round), temp = process.env.MCR_SIGNIN_TEMP
assert(temp?.startsWith('/private/tmp/mcr-css-signin-'))
const base = '29a42ca1f5d86262ee18141d2bab75996af7e6fd'
const cssPath = 'frontend/src/features/auth/ui/sign-in.css'
const target = '/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/AB983BDC-49A0-4F5E-A56F-9F45A5560B6D/1-Photo-1.jpg'
const git = (...args) => execFileSync('git', args, { cwd: repo }).toString().trim()
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const photo = await readFile(target)
assert.equal(hash(photo), '2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23')
const report = { task: 'CSS-PARITY-SIGNIN', job: process.env.MCR_SIGNIN_JOB, round, base,
  candidate: git('rev-parse', 'HEAD'), cssSha256: hash(await readFile(resolve(repo, cssPath))), targetSha256: hash(photo),
  fixture: 'Existing cozy-backgrounds-v1 actual root, canonical Auth, synthetic HTTP/C05; not live admission or device acceptance',
  startedAt: new Date().toISOString(), limits: { durable: 24 * 1024 ** 2, temp: 120 * 1024 ** 2, whole: 144 * 1024 ** 2 },
  cases: [], errors: [], minFree: Infinity, peaks: { durable: 0, temp: 0, whole: 0 } }
async function size(path) { let sum = 0; for (const e of await readdir(path, { withFileTypes: true }).catch(() => [])) { const p = resolve(path, e.name); sum += e.isDirectory() ? await size(p) : (await lstat(p).catch(() => ({ size: 0 }))).size } return sum }
async function budget(start = false) {
  const disk = await statfs(repo), free = disk.bavail * disk.bsize
  report.minFree = Math.min(report.minFree, free)
  assert(free >= (start ? 16 : 8) * 1024 ** 3, 'Free-space floor')
  const values = { durable: await size(import.meta.dirname), temp: await size(temp) }; values.whole = values.durable + values.temp
  for (const [key, value] of Object.entries(values)) { report.peaks[key] = Math.max(report.peaks[key], value); assert(value < report.limits[key], key + ' byte cap') }
}
await mkdir(output, { recursive: true }); await mkdir(temp, { recursive: true }); await budget(true)
let server, browser
async function start(mode) {
  server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(temp, 'vite-' + mode),
    logLevel: 'error', resolve: { dedupe: ['react', 'react-dom'] },
    plugins: [{ name: 'signin-baseline-css-only', enforce: 'pre', load(id) {
      if (mode === 'baseline' && id.split('?')[0] === resolve(repo, cssPath)) return execFileSync('git', ['show', base + ':' + cssPath], { cwd: repo }).toString()
    } }], server: { host: '127.0.0.1', port: 5295, strictPort: true, hmr: false } })
  await server.listen()
}
async function stop() { await server?.close(); server = undefined }
async function capture(page, name) { const bytes = await page.screenshot({ path: resolve(output, name), fullPage: true }); return { path: name, sha256: hash(bytes), bytes: bytes.length } }
async function open(page, authMode = '') {
  await page.goto('http://127.0.0.1:5295/src/app/__tests__/cozy-backgrounds-v1/preview.html?' + new URLSearchParams({ entry: '/login', era: 'PARADE', authMode }))
  await page.waitForFunction(() => window.__COZY_PANEL__)
  await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible()
  await page.evaluate(async () => { await document.fonts.ready; await document.fonts.load('800 40px "MCR Auth Print"') })
}
async function run(mode, width) {
  const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
  const page = await context.newPage(), entry = { mode, width, errors: [] }
  page.setDefaultTimeout(15000); page.on('pageerror', e => entry.errors.push(e.message))
  try {
    await open(page)
    entry.geometry = await page.evaluate(() => {
      const box = selector => document.querySelector(selector).getBoundingClientRect().toJSON()
      return { heading: box('.auth-display-heading'), email: box('#sign-in-email'), password: box('#sign-in-password'),
        submit: box('.auth-submit'), providers: box('.auth-provider-buttons'), create: box('.auth-create'),
        frame: box('.concert-window'), overflow: document.documentElement.scrollWidth > innerWidth,
        text: document.querySelector('.auth-paper-content').innerText,
        decorative: ['.concert-window', '.auth-cemetery--top', '.auth-cemetery--bottom'].map(selector => {
          const e = document.querySelector(selector), css = getComputedStyle(e), before = getComputedStyle(e, '::before')
          return { selector, background: css.background, before: before.background,
            images: [...e.querySelectorAll('image')].map(image => { const rect = image.getBoundingClientRect(); return { href: image.href.baseVal, width: rect.width, height: rect.height } }) }
        }),
        controls: [...document.querySelectorAll('.auth-sign-in input,.auth-sign-in button,.auth-sign-in a')].filter(e => e.checkVisibility()).map(e => ({ label: e.getAttribute('aria-label') || e.textContent || e.name, disabled: e.disabled ?? false, rect: e.getBoundingClientRect().toJSON() })) }
    })
    assert(!entry.geometry.overflow)
    for (const control of entry.geometry.controls) assert(control.rect.width >= 44 && control.rect.height >= 44, '44px target: ' + control.label)
    for (const provider of ['X', 'Discord', 'Google']) await expect(page.getByRole('button', { name: 'Continue with ' + provider, exact: true })).toBeDisabled()
    assert.equal(await page.getByRole('checkbox').count(), 0, 'No invented Remember me')
    entry.screenshot = await capture(page, mode + '-' + width + '.png')
    if (mode === 'candidate') {
      if (width === 390) {
        const baselineReport = round === 'first' ? report : JSON.parse(await readFile(resolve(import.meta.dirname, 'first/report.json'), 'utf8'))
        const baseline = baselineReport.cases.find(item => item.mode === 'baseline' && item.width === 390)
        assert.deepEqual(entry.geometry.decorative, baseline.geometry.decorative, 'Background declarations, decorative art dimensions and source preserved')
        assert.equal(entry.geometry.text, baseline.geometry.text, 'Actual foreground copy preserved')
      }
      const email = page.getByLabel('Email', { exact: true }), password = page.getByLabel('Password', { exact: true })
      await email.focus(); await page.keyboard.press('Tab'); await expect(password).toBeFocused()
      entry.focus = await password.evaluate(e => ({ outline: getComputedStyle(e).outlineStyle, width: getComputedStyle(e).outlineWidth }))
      assert.notEqual(entry.focus.outline, 'none'); assert.notEqual(entry.focus.width, '0px')
      if (width === 390) entry.focusScreenshot = await capture(page, 'focus-390.png')
      await page.emulateMedia({ forcedColors: 'active' }); await email.focus(); await page.keyboard.press('Tab')
      entry.forcedFocus = await password.evaluate(e => ({ outline: getComputedStyle(e).outlineStyle, width: getComputedStyle(e).outlineWidth }))
      assert.notEqual(entry.forcedFocus.outline, 'none'); assert.notEqual(entry.forcedFocus.width, '0px')
      if (width === 390) entry.forcedScreenshot = await capture(page, 'forced-focus-390.png')
      await page.emulateMedia({ forcedColors: 'none' })
      for (const control of await page.locator('.auth-sign-in input,.auth-sign-in button:not(:disabled),.auth-sign-in a').all()) {
        await control.scrollIntoViewIfNeeded()
        assert(await control.evaluate(e => { const r = e.getBoundingClientRect(), hit = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return hit === e || e.contains(hit) }), 'Foreground hit target')
      }
      await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
      assert.equal(await page.evaluate(() => window.__COZY_PANEL__.state().requests.filter(r => r.path === '/api/auth/login').length), 0, 'Native validity')
      await page.getByRole('link', { name: 'Forgot password?', exact: true }).click(); await expect.poll(() => page.evaluate(() => window.__COZY_PANEL__.state().path)).toBe('/reset-password')
      await open(page); await page.getByRole('link', { name: 'Create an account', exact: true }).click(); await expect.poll(() => page.evaluate(() => window.__COZY_PANEL__.state().path)).toBe('/signup')
      await open(page); await page.getByRole('button', { name: 'Close sign-in and return home', exact: true }).click(); await expect.poll(() => page.evaluate(() => window.__COZY_PANEL__.state().path)).toBe('/')
      await open(page, 'error'); await email.fill('fixture@example.test'); await password.fill('fixture-password'); await password.press('Enter')
      await expect(page.getByText('We could not sign you in. Check your details or try again later.', { exact: true })).toBeVisible(); await expect(password).toHaveValue('')
      if (width === 390) entry.errorScreenshot = await capture(page, 'error-390.png')
      await open(page, 'pending'); await email.fill('fixture@example.test'); await password.fill('fixture-password'); await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Signing in…', exact: true })).toBeDisabled(); await expect(password).toHaveValue('')
      await page.locator('form').evaluate(e => e.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })))
      assert.equal(await page.evaluate(() => window.__COZY_PANEL__.state().requests.filter(r => r.path === '/api/auth/login').length), 1)
      if (width === 390) entry.pendingScreenshot = await capture(page, 'pending-390.png')
      await open(page, 'options-unavailable'); await expect(page.getByText('Sign-in options are unavailable.', { exact: true })).toBeVisible()
      if (width === 390) entry.unavailableScreenshot = await capture(page, 'options-unavailable-390.png')
      await page.setViewportSize({ width, height: 568 }); await open(page)
      for (const control of await page.locator('.auth-sign-in input,.auth-sign-in button:not(:disabled),.auth-sign-in a').all()) {
        await control.scrollIntoViewIfNeeded(); assert(await control.evaluate(e => { const r = e.getBoundingClientRect(), hit = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return hit === e || e.contains(hit) }), 'Short-height reachability')
      }
      entry.shortHeight = { viewport: 568, reachable: true, overflow: await page.evaluate(() => document.documentElement.scrollWidth > innerWidth) }; assert(!entry.shortHeight.overflow)
      if (width === 390) entry.shortScreenshot = await capture(page, 'short-height-390.png')
    }
    assert.deepEqual(entry.errors, []); entry.status = 'PASS'
  } catch (error) { entry.status = 'FAIL'; entry.error = error.stack; report.errors.push({ mode, width, error: error.message }) }
  finally { await page.evaluate(() => window.__COZY_PANEL__?.dispose()).catch(() => {}); await context.close(); report.cases.push(entry); await budget(); await writeFile(resolve(output, 'report.partial.json'), JSON.stringify(report, null, 2) + '\n') }
}
try {
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--disk-cache-size=10485760'] }); report.browser = browser.version()
  if (round === 'first') { await start('baseline'); await run('baseline', 390); await stop() }
  await start('candidate'); for (const width of [390, 360, 430]) await run('candidate', width); await stop()
  const baseline = round === 'first' ? resolve(output, 'baseline-390.png') : resolve(import.meta.dirname, 'first/baseline-390.png')
  const inline = async path => 'data:image/png;base64,' + (await readFile(path)).toString('base64')
  const comparison = `<!doctype html><html lang="en"><meta charset="utf-8"><title>Sign-in foreground comparison</title><style>body{margin:0;background:#ddd;font:16px Arial}main{display:flex;gap:16px}section{width:390px;flex:none}h2{font:16px Arial;margin:8px 0}.photo{width:390px;height:726px;overflow:hidden;position:relative}.photo img{position:absolute;width:2122.105px;max-width:none;left:-425.053px;top:-.632px}section>img{width:390px}</style><main><section><h2>Reference · frame normalized to 378px</h2><div class="photo"><img src="data:image/jpeg;base64,${photo.toString('base64')}"></div></section><section><h2>Current-master CSS baseline</h2><img src="${await inline(baseline)}"></section><section><h2>Candidate · ${round} · actual fixture</h2><img src="${await inline(resolve(output, 'candidate-390.png'))}"></section></main></html>`
  await writeFile(resolve(output, 'comparison.html'), comparison)
  const context = await browser.newContext({ viewport: { width: 1202, height: 880 } }), page = await context.newPage()
  await page.setContent(comparison); await page.locator('img').evaluateAll(nodes => Promise.all(nodes.map(e => e.decode())))
  report.comparison = await capture(page, 'comparison-390.png'); await context.close()
} catch (error) { report.errors.push({ workflow: error.stack }) }
finally {
  await browser?.close(); await stop(); await budget(); await rm(temp, { recursive: true, force: true })
  report.finishedAt = new Date().toISOString(); report.cleanup = { browserClosed: true, viteClosed: true, ownTempRemoved: true }
  report.status = report.errors.length ? 'FAIL' : 'PASS_PENDING_OWNER_VISUAL_INSPECTION'
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n'); await rm(resolve(output, 'report.partial.json'), { force: true })
  console.log(JSON.stringify({ status: report.status, errors: report.errors, output }))
}
if (report.errors.length) process.exitCode = 1
