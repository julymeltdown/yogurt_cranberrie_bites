// TEST_ONLY default application qualification. Run only under a separately accepted finite browser lease.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile, rm, statfs, readdir, stat } from 'node:fs/promises'
import { readFileSync } from 'node:fs'
import { resolve, dirname, extname, relative } from 'node:path'

const repo = resolve(import.meta.dirname, '../../../../../..'), frontend = resolve(repo, 'frontend')
const out = process.env.PI05_NATIVE_OUT, deps = process.env.PI05_DEPS_ROOT
assert(out && deps && process.env.PI05_NATIVE_ACCEPTED === '1', 'An actual finite browser ACCEPT and private output are required')
const require = createRequire(resolve(deps, '../package.json'))
const { createServer } = require('vite'), react = require('@vitejs/plugin-react').default
const { chromium, expect } = require('@playwright/test')
const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 64 * 1024 * 1024 })
const sha = bytes => createHash('sha256').update(bytes).digest('hex')
const foundation = '056a8c48ff667134dee8e5e99710185294e4b898'
const shellCorrection = { source: 'b5b44e47531e3b75febc5ee5cdd6a11b1648779c', path: 'frontend/src/app/shell.css',
  sha256: 'b12f4bbddb55d144fb6da89ab2686e0225455ed3bedaff70ad8a34ef6a0161dc',
  readsetSha256: '9630cc0b8b487b6a6348612dbadc27be868729f5e5189f61c5a559b31558c2c9', qualification: 'NATIVE_UNQUALIFIED_CORRECTION' }
const readset = JSON.parse(git('show', `${foundation}:docs/mcr/evidence/parade-ink-foundation/readset.json`))
assert.equal(sha(git('show', `${shellCorrection.source}:docs/mcr/evidence/parade-ink-foundation/qualification-readset.json`)), shellCorrection.readsetSha256)
const overlay = new Map(), fonts = new Map(), assetReads = [], dependencyReads = [], cssReads = new Map()
for (const entry of readset.implementation) {
  const source = entry.path === shellCorrection.path ? shellCorrection.source : foundation
  const bytes = git('show', `${source}:${entry.path}`)
  assert.equal(sha(bytes), entry.path === shellCorrection.path ? shellCorrection.sha256 : entry.sha256, `PI01 source pin: ${entry.path}`)
  dependencyReads.push({ path: entry.path, source, sha256: sha(bytes), bytes: bytes.length })
  if (entry.path.endsWith('.ttf')) {
    fonts.set(`/__pi01font/${entry.path.split('/').at(-1)}`, bytes)
    fonts.set(`/${entry.path.slice('frontend/'.length)}`, bytes)
    fonts.set(`/@fs${resolve(repo, entry.path)}`, bytes)
  }
  overlay.set(resolve(repo, entry.path), bytes)
}
const tracked = new Set(git('ls-files', 'frontend/public').toString().trim().split('\n')), assets = new Map()
const pkg = JSON.parse(await readFile(resolve(frontend, 'package.json')))
const alias = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies }).map(name => ({
  find: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?=/|$)`), replacement: resolve(deps, name),
}))
const port = 5368
const cacheDir = resolve(out, 'vite-cache')
// Vite's CSS @import expansion reads the filesystem rather than plugin load hooks.
// Expand the same read-only import graph here so pinned virtual CSS requires no shared-file copies.
function cssSource(path, stack = new Set()) {
  assert(!stack.has(path), `CSS import cycle: ${path}`)
  const next = new Set(stack).add(path)
  const raw = overlay.get(path) ?? readFileSync(path)
  cssReads.set(path, { path: relative(repo, path), sha256: sha(raw), bytes: raw.length, pinned: overlay.has(path) })
  const rebased = raw.toString().replace(/url\(\s*(["']?)([^"')]+)\1\s*\)/g, (original, _quote, url) => {
    if (!url.startsWith('.')) return original
    const asset = resolve(dirname(path), url)
    if (asset.endsWith('.ttf') && overlay.has(asset)) return `url("/__pi01font/${asset.split('/').at(-1)}")`
    return `url("/${relative(frontend, asset)}")`
  })
  return rebased.replace(/@import\s+["']([^"']+)["']\s*;/g, (original, source) => {
    if (!source.startsWith('.')) return original
    return cssSource(resolve(dirname(path), source), next)
  })
}
const server = await createServer({ root: frontend, configFile: false, cacheDir, logLevel: 'error',
  plugins: [{ name: 'pi05-pinned-readonly-foundation', enforce: 'pre',
    resolveId(source, importer) {
      const clean = source.split('?')[0]
      const path = clean.startsWith('/src/') ? resolve(frontend, clean.slice(1))
        : clean.startsWith('.') && importer ? resolve(dirname(importer.split('?')[0]), clean) : clean
      if (overlay.has(path)) return path + (source.includes('?') ? `?${source.split('?')[1]}` : '')
    },
    load(id) {
      const bytes = overlay.get(id.split('?')[0]); if (!bytes) return
      if (id.split('?')[0].endsWith('.ttf')) return `export default ${JSON.stringify(`/__pi01font/${id.split('?')[0].split('/').at(-1)}`)}`
      if (id.split('?')[0].endsWith('.css')) return cssSource(id.split('?')[0])
      return bytes.toString()
    },
    configureServer(s) { s.middlewares.use((request, response, next) => {
      const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname)
      if (fonts.has(pathname)) { response.setHeader('Content-Type', 'font/ttf'); response.end(fonts.get(pathname)); return }
      const path = `frontend/public${pathname}`
      if (!tracked.has(path) || !['.png', '.jpg', '.webp', '.ttf', '.json'].includes(extname(path))) return next()
      if (!assets.has(path)) { const bytes = git('show', `HEAD:${path}`); assets.set(path, bytes); assetReads.push({ path, bytes: bytes.length, sha256: sha(bytes) }) }
      response.setHeader('Content-Type', extname(path) === '.json' ? 'application/json' : extname(path) === '.ttf' ? 'font/ttf' : `image/${extname(path).slice(1)}`)
      response.end(assets.get(path))
    }) },
  }, react()], resolve: { alias },
  optimizeDeps: { entries: ['src/features/party-discovery/__tests__/parade-ink/preview.html', 'src/features/meet-beacon/__tests__/parade-ink/preview.html'],
    include: ['react', 'react-dom/client', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-router-dom', '@tanstack/react-query', 'react-hook-form', '@hookform/resolvers/zod', 'zod', 'phaser'] },
  server: { host: '127.0.0.1', port, strictPort: true, hmr: false } })
const report = { classification: 'TEST_ONLY_SYNTHETIC_HTTP_DEFAULT_APP', physicalDevices: 'NOT_RUN',
  source: git('rev-parse', 'HEAD').toString().trim(), foundation, foundationQualification: readset.qualification, shellCorrection,
  viewportCases: [], dependencyReads, assetReads, status: 'RUNNING' }
let browser
async function pngBudget() {
  const paths = (await readdir(out)).filter(path => path.endsWith('.png'))
  const bytes = (await Promise.all(paths.map(path => stat(resolve(out, path))))).reduce((sum, item) => sum + item.size, 0)
  assert(bytes <= 8 * 1024 * 1024, 'All captures and normalized comparisons share the 8MiB PNG cap')
  report.pngBytes = bytes
}
async function compareReference(page, capture, bounds) {
  assert(bounds, 'Actual default-App panel bounds are required for normalized comparison')
  const referencePath = resolve(repo, 'docs/mcr/design/parade-ink-2026-11-05/bone-crimson.jpg')
  const reference = await readFile(referencePath)
  assert.equal(sha(reference), '1b782e6b28128bdd877bc714b86dfa9fe9c1a2fc74b5ce77807dcf7cafd38217')
  const native = await readFile(resolve(out, capture))
  const result = await page.evaluate(async ({ native, target, bounds }) => {
    const load = src => new Promise((done, fail) => { const img = new Image(); img.onload = () => done(img); img.onerror = fail; img.src = src })
    const [actual, reference] = await Promise.all([load(native), load(target)])
    const width = 390, referenceHeight = Math.round(773 * width / 386), actualHeight = Math.round(bounds.height * width / bounds.width)
    const height = Math.max(referenceHeight, actualHeight)
    const side = document.createElement('canvas'); side.width = width * 2 + 12; side.height = height + 32
    const context = side.getContext('2d'); context.fillStyle = '#fff'; context.fillRect(0, 0, side.width, side.height)
    context.font = '13px Arial'; context.fillStyle = '#171717'
    context.fillText('Photo4 anatomy; Photo2 supplies appearance', 4, 19)
    context.fillText('Actual PI05 default App; synthetic HTTP', width + 16, 19)
    context.drawImage(reference, 422, 70, 386, 773, 0, 32, width, referenceHeight)
    context.drawImage(actual, bounds.x, bounds.y, bounds.width, bounds.height, width + 12, 32, width, actualHeight)
    const overlay = document.createElement('canvas'); overlay.width = width; overlay.height = height
    const blend = overlay.getContext('2d'); blend.fillStyle = '#fff'; blend.fillRect(0, 0, width, height)
    blend.drawImage(reference, 422, 70, 386, 773, 0, 0, width, referenceHeight); blend.globalAlpha = .5
    blend.drawImage(actual, bounds.x, bounds.y, bounds.width, bounds.height, 0, 0, width, actualHeight)
    return { side: side.toDataURL('image/png'), overlay: overlay.toDataURL('image/png'), referenceHeight, actualHeight }
  }, { native: `data:image/png;base64,${native.toString('base64')}`, target: `data:image/jpeg;base64,${reference.toString('base64')}`, bounds })
  for (const key of ['side', 'overlay']) await writeFile(resolve(out, `meet-390-reference-${key}.png`), Buffer.from(result[key].split(',')[1], 'base64'))
  report.referenceComparison = { source: referencePath, sourceSha256: sha(reference), crop: [422, 70, 386, 773], normalizedWidth: 390,
    actualPanel: bounds, referenceHeight: result.referenceHeight, actualHeight: result.actualHeight,
    side: 'meet-390-reference-side.png', overlay: 'meet-390-reference-overlay.png',
    adaptation: 'Photo2 black selected tabs/light paper/crimson actions; Photo4 anatomy only; authorized content and unavailable member portraits remain truthful', developerAssessment: 'PENDING_IMAGE_INSPECTION' }
  await pngBudget()
}
async function measure(page) {
  return page.evaluate(() => {
    const box = el => { const b = el.getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height, right: b.right, bottom: b.bottom } }
    const feature = document.querySelector('[data-presentation="parade-ink-meet"]')
    return { overflow: document.documentElement.scrollWidth > innerWidth, feature: feature && box(feature),
      panel: feature?.closest('.shell-panel') ? box(feature.closest('.shell-panel')) : null,
      rows: [...document.querySelectorAll('.meet-party')].map(box),
      appearance: feature ? { font: getComputedStyle(feature).fontFamily, background: getComputedStyle(feature).backgroundColor,
        selectedBackground: feature.querySelector('[aria-pressed=true]') ? getComputedStyle(feature.querySelector('[aria-pressed=true]')).backgroundColor : null,
        loadedFonts: [...document.fonts].filter(face => face.status === 'loaded').map(face => face.family) } : null,
      controls: feature ? [...feature.querySelectorAll('a,button,input,select,textarea,summary')].filter(el => el.getClientRects().length)
        .map(el => ({ label: el.getAttribute('aria-label') || el.textContent.trim(), ...box(el.matches('input[type=checkbox]') ? el.labels[0] : el) })) : [],
      state: window.__PI05__?.state() ?? window.__PI05_COMPONENT__?.state() }
  })
}
try {
  await mkdir(out, { recursive: true }); const disk = await statfs(out)
  assert(disk.bavail * disk.bsize >= 8 * 2 ** 30, 'Continuous 8GiB floor; external dual guards are also mandatory')
  await server.listen(); browser = await chromium.launch({ channel: 'chrome', headless: true })
  report.browser = browser.version(); report.node = process.version
  const party = '00000000-0000-4000-8000-000000000010'
  const beacon = '00000000-0000-4000-8000-000000000015'
  const cases = [360, 390, 430].flatMap(width => [
    [width, 844, '/meet', 'normal'], [width, 844, `/meet/parties/${party}`, 'normal'],
    [width, 844, '/meet/parties/new', 'normal'], [width, 844, `/meet/parties/${party}/manage`, 'owner'],
    [width, 844, `/meet/parties/${party}/rally`, 'owner'], [width, 844, '/meet/people', 'normal'],
    [width, 844, '/meet/intro/edit', 'normal'], [width, 844, 'M06_COMPONENT', 'current'],
    [width, 844, `/meet/beacons/${beacon}`, 'private'],
  ]).concat([
    [390, 420, '/meet', 'long'], [390, 844, '/meet', 'full'], [390, 844, '/meet', 'closed'],
    [390, 844, '/meet', 'empty'], [390, 844, '/meet', 'error'],
    [390, 844, 'M06_COMPONENT', 'private'], [390, 844, 'M06_COMPONENT', 'expired'], [390, 420, 'M06_COMPONENT', 'long'],
  ])
  const indices = process.env.PI05_NATIVE_CASES ? new Set(process.env.PI05_NATIVE_CASES.split(',').map(Number)) : null
  report.plannedCaseCount = cases.length
  for (const [index, [width, height, entry, scenario]] of cases.entries()) {
    if (indices && !indices.has(index + 1)) continue
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce', serviceWorkers: 'block' })
    const page = await context.newPage(), errors = [], resourceErrors = []; page.on('pageerror', error => errors.push(error.message))
    page.on('response', response => { if (response.status() >= 400 && resourceErrors.length < 30) resourceErrors.push({ url: response.url(), status: response.status() }) })
    page.setDefaultTimeout(15000); await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
    const result = { caseIndex: index + 1, width, height, entry, scenario, errors, resourceErrors, classification: entry === 'M06_COMPONENT' ? 'TEST_ONLY_COMPONENT_NOT_DEFAULT_APP' : 'TEST_ONLY_DEFAULT_APP' }; report.viewportCases.push(result)
    try {
      const fixture = entry === 'M06_COMPONENT' ? 'meet-beacon' : 'party-discovery'
      await page.goto(`http://127.0.0.1:${port}/src/features/${fixture}/__tests__/parade-ink/preview.html?entry=${encodeURIComponent(entry)}&case=${scenario}`)
      await page.waitForFunction(() => !!window.__PI05__ || !!window.__PI05_COMPONENT__)
      if (entry.includes('/beacons/')) await expect(page.getByText('Real-world meeting guidance is unavailable until current place, audience, and expiry information is verified.')).toBeVisible()
      else await expect(page.locator('[data-presentation="parade-ink-meet"]').first()).toBeVisible()
      await page.evaluate(() => document.fonts.ready); await page.waitForLoadState('networkidle')
      result.initial = await measure(page)
      result.capture = `${String(index + 1).padStart(2, '0')}-${width}x${height}-${scenario}.png`
      await page.screenshot({ path: resolve(out, result.capture) })
      await pngBudget()
      await writeFile(resolve(out, 'report.json'), `${JSON.stringify(report, null, 2)}\n`)
      assert.equal(result.initial.overflow, false)
      assert(result.initial.controls.every(control => control.width >= 43.5 && control.height >= 43.5), '44px controls')
      if (entry === '/meet' && scenario === 'normal' && width === 390) await compareReference(page, result.capture, result.initial.panel)
      if (entry === '/meet' && scenario === 'normal') {
        await expect(page.locator('.meet-party')).toHaveCount(3)
        await page.getByRole('button', { name: 'Refresh parties' }).click()
        await page.getByRole('button', { name: 'MY PLAN', exact: true }).click()
        await expect(page.getByRole('button', { name: 'Refresh MY PLAN' })).toBeVisible()
        await page.getByRole('button', { name: 'Parties', exact: true }).click()
        await page.locator('.party-filters summary').click()
        await page.getByLabel('Ticket section', { exact: true }).fill('Area A')
        await page.getByRole('button', { name: 'Show parties' }).click()
        await page.locator('.party-filters summary').click()
        await page.keyboard.press('Tab'); result.focus = await page.evaluate(() => ({ tag: document.activeElement.tagName, outline: getComputedStyle(document.activeElement).outlineStyle }))
        assert.equal(result.focus.outline, 'solid')
      }
      assert.equal(errors.length, 0); result.status = 'PASS'
    } catch (error) {
      result.status = 'FAIL'; result.error = error.stack
      if (!result.capture) { result.failureCapture = `failed-${index + 1}.png`; await page.screenshot({ path: resolve(out, result.failureCapture) }).catch(() => {}) }
      throw error
    }
    finally { await page.evaluate(() => { window.__PI05__?.dispose(); window.__PI05_COMPONENT__?.dispose() }).catch(() => {}); await context.close() }
  }
  report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.error = error.stack; throw error }
finally {
  report.cssReads = [...cssReads.values()]
  await writeFile(resolve(out, 'report.json'), `${JSON.stringify(report, null, 2)}\n`)
  await browser?.close(); await server.close(); await rm(cacheDir, { recursive: true, force: true })
}
