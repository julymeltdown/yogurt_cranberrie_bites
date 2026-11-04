// One finite native workflow. Actual default root composition, synthetic public HTTP only.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { readFile, writeFile, mkdir, mkdtemp, rm, readdir, lstat, statfs } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve, relative } from 'node:path'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { chromium, expect } from '@playwright/test'

const slot = process.env.MCR_NATIVE_SLOT
assert(typeof slot === 'string' && slot.trim().length > 0, 'Record the explicitly accepted named workflow')
const repo = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(repo, 'frontend')
const evidence = resolve(repo, 'docs/mcr/evidence/archive-root-composition')
const out = resolve(evidence, 'native')
const git = (...args) => execFileSync('git', args, { cwd: repo, encoding: 'utf8' }).trim()
const head = git('rev-parse', 'HEAD')
assert.equal(head, process.env.MCR_ARCHIVE_ROOT_HEAD, 'Exact qualified combined source HEAD required')
assert((await readFile(resolve(frontend, 'src/app/PublicBoundary.tsx'), 'utf8')).includes("import('./archive-entry')"), 'Actual Root-owned PublicBoundary hook required; no transform/override')
const hash = value => createHash('sha256').update(value).digest('hex')
async function freeGiB() { const s = await statfs(repo); return s.bavail * s.bsize / 2 ** 30 }
async function size(path) {
  let total = 0
  for (const e of await readdir(path, { withFileTypes: true }).catch(() => [])) {
    const p = resolve(path, e.name)
    total += e.isDirectory() ? await size(p) : await lstat(p).then(s => s.size, error => {
      if (error.code === 'ENOENT') return 0 // Browser cache entries may disappear during a sample.
      throw error
    })
  }
  return total
}
assert(await freeGiB() >= 16, '16GiB start floor')
await mkdir(out, { recursive: true })
const temporary = await mkdtemp(resolve(tmpdir(), 'mcr-fe40-archive-root-'))
// Playwright's owned browser profile and subprocess temporary files belong in
// the same accounted tree as Vite's cache, rather than an uncounted OS temp path.
const previousTemporary = process.env.TMPDIR
process.env.TMPDIR = temporary
const inputs = [
  'frontend/src/app/archive-entry/index.tsx', 'frontend/src/app/PublicBoundary.tsx', 'frontend/src/app/router.tsx',
  'frontend/src/app/ports.ts', 'frontend/src/main.tsx', 'frontend/src/app/access.tsx', 'frontend/src/app/routes.ts',
  'frontend/src/app/__tests__/archive-entry/preview.tsx', 'frontend/src/app/__tests__/archive-entry/native.mjs',
  'frontend/src/app/__tests__/fixtures/ports.ts', 'frontend/src/features/event-feed/contract.ts',
  'frontend/src/shared/api/domain.ts', 'frontend/src/shared/api/domain-operations.generated.ts',
  'contracts/generated/mcr-domain.openapi.json', 'frontend/pnpm-lock.yaml',
]
for (const name of await readdir(resolve(frontend, 'src/features/archive'))) if (/\.(ts|tsx|css)$/.test(name)) inputs.push(`frontend/src/features/archive/${name}`)
const report = { result: 'FAIL', qualification: 'ACTUAL_DEFAULT_ROOT_SYNTHETIC_PUBLIC_HTTP_NO_QUALIFIED_MEDIA',
  slot, sourceHead: head, node: process.version, browser: null, sourceHashes: {}, cases: [], physicalDevices: 'NOT_RUN', BE41: 'NOT_RUN',
  resourceSamples: 0, sampledPeakWholeBytes: 0, sampledPeakTemporaryBytes: 0, minimumFreeGiB: await freeGiB() }
for (const p of inputs) report.sourceHashes[p] = hash(await readFile(resolve(repo, p)))
let server, browser, resourceTimer, resourceFailure
let resourceTask = Promise.resolve()
async function checkResources() {
  const temporaryBytes = await size(temporary) + await size(resolve(frontend, 'node_modules'))
  const durableBytes = await size(evidence), free = await freeGiB()
  report.resourceSamples++
  report.sampledPeakWholeBytes = Math.max(report.sampledPeakWholeBytes, temporaryBytes + durableBytes)
  report.sampledPeakTemporaryBytes = Math.max(report.sampledPeakTemporaryBytes, temporaryBytes)
  report.minimumFreeGiB = Math.min(report.minimumFreeGiB, free)
  assert(free >= 8, '8GiB continuous stop floor')
  assert(temporaryBytes <= .15 * 2 ** 30, '0.15GiB temporary ceiling including owned dependency links/cache')
  assert(durableBytes <= .01 * 2 ** 30, '0.01GiB durable ceiling')
  assert(temporaryBytes + durableBytes <= 214748364, '0.2GiB whole-job output ceiling')
}
try {
  await checkResources()
  resourceTimer = setInterval(() => {
    resourceTask = resourceTask.then(async () => {
      if (resourceFailure) return
      try { await checkResources() } catch (error) { resourceFailure = error; await browser?.close(); await server?.close() }
    })
  }, 1000)
  server = await createServer({ root: frontend, configFile: false, plugins: [react(), tailwindcss()], publicDir: false,
    cacheDir: resolve(temporary, 'cache'), logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false } })
  await server.listen()
  const address = server.httpServer.address()
  assert(address && typeof address === 'object')
  const url = `http://127.0.0.1:${address.port}/src/app/__tests__/archive-entry/preview.html`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const [width, height] of [[360, 844], [390, 844], [430, 844], [390, 420]]) {
    await checkResources()
    const context = await browser.newContext({ viewport: { width, height }, isMobile: true, hasTouch: true, reducedMotion: 'reduce' })
    const page = await context.newPage(), errors = []
    page.on('pageerror', e => errors.push(e.message))
    await page.addInitScript(() => {
      window.__archiveSocketProtocols = []
      const Original = window.WebSocket
      window.WebSocket = class extends Original { constructor(...args) {
        super(...args)
        // Vite's explicit development transport is fixture infrastructure.
        // Record only protocols, never its URL/token; reject every other socket.
        window.__archiveSocketProtocols.push(args[1] ?? null)
      } }
    })
    await page.goto(url)
    await expect(page.getByText('We sang every word.', { exact: false })).toBeVisible()
    assert.equal(await page.evaluate(() => window.__ARCHIVE_ROOT__.worldMounts()), 0)
    const socketProtocols = await page.evaluate(() => window.__archiveSocketProtocols)
    assert.deepEqual(socketProtocols.filter(protocol => protocol !== 'vite-hmr'), [])
    assert.equal(await page.locator('canvas').count(), 1)
    const pixel = () => page.locator('canvas').evaluate(c => Array.from(c.getContext('2d').getImageData(1, 1, 1, 1).data))
    assert.deepEqual(await pixel(), [21, 23, 28, 255])
    await page.locator('canvas').tap({ position: { x: 15, y: 15 } })
    assert.deepEqual(await pixel(), [21, 23, 28, 255])
    assert.equal(await page.locator('form,input,textarea,select,[data-world-host]').count(), 0)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    const badTargets = await page.locator('button:enabled').evaluateAll(nodes => nodes.filter(n => {
      const r = n.getBoundingClientRect(); return r.width < 44 || r.height < 44
    }).map(n => n.textContent))
    assert.deepEqual(badTargets, [])
    const file = `${width}x${height}.png`
    await page.screenshot({ path: resolve(out, file), fullPage: true })
    await page.getByRole('button', { name: /View archived image 1/ }).tap()
    await expect(page.getByText('Archived image unavailable. No live or private image is substituted.')).toBeVisible()
    assert.equal(await page.locator('img.archive-image').count(), 0)
    await page.getByRole('button', { name: 'Next page' }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByText('The last chorus stayed with us.')).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Posts after the show' })).toBeFocused()
    await page.evaluate(() => window.__ARCHIVE_ROOT__.generation())
    await expect(page.getByText('We sang every word.', { exact: false })).toBeVisible()
    for (const mode of ['removed', 'offline', 'private']) {
      await page.evaluate(value => window.__ARCHIVE_ROOT__.mode(value), mode)
      await page.getByRole('button', { name: 'Reload archive' }).tap()
      await expect(page.getByRole('alert')).toBeVisible()
      assert.equal(await page.locator('canvas').count(), 0)
      assert(!await page.locator('body').innerText().then(t => t.includes('DO_NOT_DISPLAY')))
    }
    await page.evaluate(() => window.__ARCHIVE_ROOT__.mode('empty'))
    await page.getByRole('button', { name: 'Reload archive' }).tap()
    await expect(page.getByText('No permitted posts on this page.')).toBeVisible()
    const requests = await page.evaluate(() => window.__ARCHIVE_ROOT__.requests())
    assert(requests.every(r => r.method === 'GET' && r.credentials === 'omit' && r.cache === 'no-store' && !r.authorization && !r.cookie
      && r.path === '/api/concert/events/00000000-0000-4000-8000-000000000001/archive'))
    assert.deepEqual(errors, [])
    assert.deepEqual((await page.evaluate(() => window.__archiveSocketProtocols)).filter(protocol => protocol !== 'vite-hmr'), [])
    report.cases.push({ width, height, result: 'PASS', errors, requests, fixtureSocketProtocols: socketProtocols, applicationSockets: 0, screenshot: file,
      screenshotSha256: hash(await readFile(resolve(out, file))), checks: ['actual default PublicBoundary', 'public-only GET', 'frozen pixels', '44px targets', 'pagination/focus', 'media unavailable', 'generation reload', 'withdrawal/privacy/offline', 'empty', 'zero world/application socket; Vite fixture socket classified separately'] })
    await page.evaluate(() => window.__ARCHIVE_ROOT__.dispose())
    await context.close()
  }
  await checkResources()
  if (resourceFailure) throw resourceFailure
  report.result = 'PASS'
} finally {
  clearInterval(resourceTimer)
  await resourceTask
  if (resourceFailure) { report.result = 'FAIL'; report.resourceFailure = String(resourceFailure) }
  await browser?.close()
  await server?.close()
  if (previousTemporary === undefined) delete process.env.TMPDIR
  else process.env.TMPDIR = previousTemporary
  await rm(temporary, { recursive: true, force: true })
  report.outputBytes = await size(out)
  report.finalFreeGiB = await freeGiB()
  await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
assert.equal(report.result, 'PASS')
process.stdout.write(JSON.stringify({ result: report.result, cases: report.cases.length, sourceHead: head, output: relative(repo, out) }) + '\n')
