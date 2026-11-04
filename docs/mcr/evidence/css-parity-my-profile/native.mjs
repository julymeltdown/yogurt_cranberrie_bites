// Developer-owned CSS comparison driver. Run only under an accepted resource21d offer.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { resolve, join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile, rm, readdir, stat, statfs } from 'node:fs/promises'
const repo = resolve(import.meta.dirname, '../../../..'), frontend = join(repo, 'frontend')
const require = createRequire(join(frontend, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')).href)
const { chromium, expect } = require('@playwright/test')
const job = process.env.MCR_PROFILE_JOB, output = process.env.MCR_PROFILE_OUTPUT, temp = process.env.MCR_PROFILE_TEMP
assert(job && output?.startsWith(join(repo, 'docs/mcr/evidence/css-parity-my-profile/')) && temp?.startsWith('/tmp/mcr-profile-'))
const base = '54320d71c4ae0bff3524d1b500cea772e0ba42e0', port = 5294
const paths = ['frontend/src/features/me/ui/me.css', 'frontend/src/features/profile/reference-presentation/activity.css']
const baseline = Object.fromEntries(paths.map(p => [p, execFileSync('git', ['show', `${base}:${p}`], { cwd: repo, encoding: 'utf8' })]))
const hash = data => createHash('sha256').update(data).digest('hex')
const report = { job, base, candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim(),
  startedAt: new Date().toISOString(), status: 'IN_PROGRESS', qualification: 'MOCK_READY only: production components/default root with synthetic HTTP/C05; not live services or physical devices',
  referenceSha256: '2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23', sources: {}, cases: [], errors: [],
  limits: { whole: 200 * 1024 ** 2, temp: 160 * 1024 ** 2, durable: 30 * 1024 ** 2 }, peaks: { whole: 0, temp: 0, durable: 0 }, minFree: Infinity }
async function size(path) { let total = 0; try { for (const item of await readdir(path, { withFileTypes: true })) { const p = join(path, item.name); total += item.isDirectory() ? await size(p) : (await stat(p)).size } } catch (error) { if (error.code !== 'ENOENT') throw error } return total }
async function budget(start = false) { const fs = await statfs(repo), free = fs.bavail * fs.bsize; assert(free >= (start ? 16 : 8) * 1024 ** 3, 'Disk floor'); report.minFree = Math.min(report.minFree, free); const t = await size(temp), d = await size(output); for (const [k, n] of Object.entries({ whole: t + d, temp: t, durable: d })) { report.peaks[k] = Math.max(report.peaks[k], n); assert(n <= report.limits[k], `${k} byte cap`) } }
await mkdir(output, { recursive: false }); await mkdir(temp, { recursive: false }); process.env.TMPDIR = temp; await budget(true)
for (const p of paths) { const current = await readFile(join(repo, p), 'utf8'); report.sources[p] = { baseline: hash(baseline[p]), candidate: hash(current) }; assert.deepEqual(current.match(/background[^;{}]*[;}]/g), baseline[p].match(/background[^;{}]*[;}]/g), 'No background declaration changes') }
let browser, server, page
// End browser work early enough to retain the resource round's cleanup reserve.
const deadline = setTimeout(() => { report.errors.push('220s native-work deadline'); void browser?.close(); void server?.close() }, 220000)
let checkingBudget = false
const monitor = setInterval(() => {
  if (checkingBudget) return
  checkingBudget = true
  void budget().catch(error => { report.errors.push(String(error)); void browser?.close(); void server?.close() }).finally(() => { checkingBudget = false })
}, 1500)
async function capture(name, locator) { const file = `${name}.png`; if (locator) await locator.screenshot({ path: join(output, file) }); else await page.screenshot({ path: join(output, file), fullPage: true }); const bytes = await readFile(join(output, file)); return { file, bytes: bytes.length, sha256: hash(bytes) } }
async function measure() { return page.evaluate(async () => {
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } }
  const selectors = ['.me-hub--reference', '.me-profile-header', '.me-identity', '.me-avatar', '.me-identity h3', '.me-bio', '.me-profile-icons', '.me-activity-navigation', '.reference-activity', '.reference-activity__visual', '.me-profile-options']
  const geometry = Object.fromEntries(selectors.map(s => { const e = document.querySelector(s); return [s, e ? { ...rect(e), font: getComputedStyle(e).font } : null] }))
  const canvases = await Promise.all([...document.querySelectorAll('.me-avatar canvas')].map(async c => ({ rect: rect(c), width: c.width, height: c.height, rgba: [...new Uint8Array(await crypto.subtle.digest('SHA-256', c.getContext('2d').getImageData(0, 0, c.width, c.height).data))].map(n => n.toString(16).padStart(2, '0')).join('') })))
  const controls = [...document.querySelectorAll('.me-hub--reference a,.me-hub--reference button,.me-hub--reference summary')].filter(e => e.checkVisibility()).map(e => ({ label: e.getAttribute('aria-label') || e.textContent.trim(), href: e.getAttribute('href'), rect: rect(e) }))
  return { geometry, canvases, controls, overflow: document.documentElement.scrollWidth > innerWidth, text: document.querySelector('.me-hub--reference')?.textContent }
}) }
async function targets() { const m = await measure(); assert(!m.overflow, 'No horizontal document overflow'); for (const c of m.controls) assert(c.rect.width >= 44 && c.rect.height >= 44, `44px control ${c.label}`); return m }
async function open(url, root = false) { await page.goto(url); await expect(page.locator('.me-hub--reference')).toBeVisible(); if (root) { await expect(page.locator('.me-avatar [data-whole-character-state="ready"]')).toBeVisible({ timeout: 25000 }) } await page.evaluate(() => document.fonts.ready) }
async function dispose(root) { await page.evaluate(root => root ? window.__COZY_PANEL__?.dispose() : window.__REF_PROFILE__?.dispose(), root) }
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  // A before snapshot restores only the two CSS inputs in Vite's in-memory transform.
  // No product file, selected artwork, background, data or markup is replaced.
  for (const stage of ['baseline', 'candidate']) {
    server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: join(temp, `vite-${stage}`), logLevel: 'error',
      plugins: stage === 'baseline' ? [{ name: 'profile-css-preimage', enforce: 'pre', transform(_code, id) { const own = paths.find(p => id.split('?')[0] === join(repo, p)); return own ? { code: baseline[own], map: null } : null } }] : [],
      server: { host: '127.0.0.1', port, strictPort: true } })
    await server.listen()
    const baseUrl = `http://127.0.0.1:${port}`, fixture = `${baseUrl}/src/features/me/__tests__/reference-target/preview.html`, rootUrl = `${baseUrl}/src/app/__tests__/cozy-backgrounds-v1/preview.html?entry=/me&era=PARADE`
    for (const width of [390, 360, 430]) {
      await budget(); const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce', serviceWorkers: 'block' }); page = await context.newPage(); page.setDefaultTimeout(10000)
      const errors = []; page.on('pageerror', error => errors.push(error.message))
      await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
      const item = { stage, width, errors, status: 'IN_PROGRESS' }; report.cases.push(item)
      await open(rootUrl, true); item.root = await targets(); item.root.state = await page.evaluate(() => window.__COZY_PANEL__.state())
      assert.equal(item.root.state.canvases, 1); assert(item.root.state.socketPeak <= 1); assert.equal(item.root.canvases.length, 1); assert.equal(item.root.canvases[0].width, 128); assert.equal(item.root.canvases[0].height, 128)
      item.root.screenshot = await capture(`${stage}-root-${width}`)
      item.root.foreground = await capture(`${stage}-root-foreground-${width}`, page.locator('.me-hub--reference'))
      if (stage === 'candidate') { const before = report.cases.find(r => r.stage === 'baseline' && r.width === width).root; assert.deepEqual(item.root.canvases, before.canvases, 'Selected portrait pixels, source size and DOM framing preserved'); assert.equal(item.root.text, before.text, 'Truthful root content preserved') }
      await dispose(true)
      await open(fixture); item.fixture = await targets(); item.fixture.screenshot = await capture(`${stage}-activity-${width}`)
      await expect(page.getByRole('link', { name: /Posted to the feed/ })).toHaveCount(2)
      await expect(page.getByText(/This is a partial view of activity/)).toBeVisible()
      if (stage === 'candidate') {
        assert.equal(item.fixture.text, report.cases.find(r => r.stage === 'baseline' && r.width === width).fixture.text)
        await page.getByRole('link', { name: 'Edit profile', exact: true }).focus(); await page.keyboard.press('Tab'); await expect(page.getByRole('link', { name: 'Dress up', exact: true })).toBeFocused()
        item.focus = await page.getByRole('link', { name: 'Dress up', exact: true }).evaluate(e => ({ outline: getComputedStyle(e).outlineStyle, width: getComputedStyle(e).outlineWidth, color: getComputedStyle(e).outlineColor, shadow: getComputedStyle(e).boxShadow }))
        assert.notEqual(item.focus.outline, 'none'); if (width === 390) item.focus.screenshot = await capture('candidate-keyboard-390')
        await page.emulateMedia({ forcedColors: 'active' }); item.forcedFocus = await page.getByRole('link', { name: 'Dress up', exact: true }).evaluate(e => ({ outline: getComputedStyle(e).outlineStyle, width: getComputedStyle(e).outlineWidth, color: getComputedStyle(e).outlineColor })); assert.notEqual(item.forcedFocus.outline, 'none'); if (width === 390) await capture('candidate-forced-colors-390'); await page.emulateMedia({ forcedColors: 'none' })
        await page.getByText('Profile options', { exact: true }).click(); await expect(page.getByRole('link', { name: 'Settings', exact: true })).toBeVisible(); await targets()
        await page.getByRole('link', { name: 'Edit profile', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Edit profile', exact: true })).toBeVisible()
        await page.getByLabel('Bio (optional)').fill('Local draft'); await page.getByRole('button', { name: 'Cancel profile changes' }).click(); await expect(page.getByLabel('Bio (optional)')).toHaveValue('mcr / live music / the black parade\nSee you at the show.')
        await dispose(false)
        for (const mode of ['long', 'empty', 'no-activity', 'loading', 'error', 'expired']) {
          await open(`${fixture}?mode=${mode}`); await targets()
          if (mode === 'no-activity') await expect(page.getByText('Recent activity is unavailable in this connection.')).toBeVisible()
          if (mode === 'expired') { await expect(page.getByRole('heading', { name: 'Synthetic listener' })).toHaveCount(0); await expect(page.getByRole('link', { name: /Posted to the feed/ })).toHaveCount(0) }
          if (width === 390 || mode === 'long') await capture(`candidate-${mode}-${width}`)
          await dispose(false)
        }
        if (width === 390) for (const [name, path] of [['Dress up', '/me/avatar'], ['Manage private social links', '/me/social'], ['Sharing requests', '/me/requests'], ['Parties and MY PLAN', '/meet'], ['My gifts', '/gifts/mine']]) {
          await open(fixture); await page.getByRole('link', { name, exact: true }).click(); await expect.poll(() => page.evaluate(() => window.__REF_PROFILE__.path())).toBe(path); await dispose(false)
        }
        await open(fixture); await page.evaluate(() => window.__REF_PROFILE__.revoke()); await expect(page.getByRole('heading', { name: 'Synthetic listener' })).toHaveCount(0); await expect(page.getByRole('link', { name: /Posted to the feed/ })).toHaveCount(0)
        item.interactions = 'PASS: route actions, options, keyboard/forced focus, edit cancel, privacy scrub, loading/error/empty/long/unavailable/expired'
      }
      assert.deepEqual(errors, []); await dispose(false); item.status = 'PASS'; await context.close(); await budget(); await writeFile(join(output, 'report.partial.json'), JSON.stringify(report, null, 2) + '\n'); console.log(`${stage} ${width}: PASS`)
    }
    await server.close(); server = null
  }
  assert.deepEqual(report.errors, []); report.status = 'CAPTURED_PENDING_OWNER_VISUAL_COMPARISON'
} catch (error) { report.errors.push(error.stack); report.status = 'FAIL'; if (page && !page.isClosed()) await capture('failure').catch(() => {}); throw error }
finally { clearTimeout(deadline); clearInterval(monitor); await browser?.close(); await server?.close(); await rm(temp, { recursive: true, force: true }); report.finishedAt = new Date().toISOString(); report.cleanup = { browserClosed: true, viteClosed: true, tempRemoved: true }; await writeFile(join(output, 'report.json'), JSON.stringify(report, null, 2) + '\n'); await rm(join(output, 'report.partial.json'), { force: true }) }
