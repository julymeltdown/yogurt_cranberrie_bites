import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { mkdir, mkdtemp, readFile, writeFile, rm } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { execFileSync, spawnSync } from 'node:child_process'
import { statfsSync } from 'node:fs'
import { aliases } from '../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs'
import { readPinnedProducer } from '../../../../world/avatar/__tests__/motion-adoption/producer-inputs.ts'

const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies || !process.env.MCR_PORTRAIT_CAPTURE_SLOT) throw Error('Set retained dependencies and the explicitly allocated portrait capture slot')
const root = resolve(import.meta.dirname, '../../../../..'), checkout = resolve(root, '..')
const disk = statfsSync(checkout), minimumFreeBytes = 8.2 * 1024 ** 3
assert(disk.bavail * disk.bsize >= minimumFreeBytes, 'Capture preflight requires 8 GiB floor plus the 0.2 GiB whole-workflow cap')
const require = createRequire(resolve(dependencies, 'package.json'))
const { createServer } = require('vite'), { chromium, expect } = require('@playwright/test')
const output = resolve(checkout, 'docs/mcr/evidence/avatar-motion-adoption/portrait-responsive')
await mkdir(output, { recursive: true })
const cache = await mkdtemp('/private/tmp/mcr-portrait-native-'), hash = bytes => createHash('sha256').update(bytes).digest('hex')
// Keep browser profiles/artifacts and bundler temporaries inside the same measured workflow directory.
process.env.TMPDIR = cache
const candidate = readPinnedProducer('backend/releases/selected-character-motion-v2/manifest.json')
const config = JSON.parse(new TextDecoder().decode(readPinnedProducer('contracts/fixtures/assets/motion-v2/valid/preset-01.json')))
const report = { result: 'FAIL', qualification: 'TEST_ONLY actual MotionEditor/Phaser component geometry, not production Root/service/device admission',
  sourceCommit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: checkout, encoding: 'utf8' }).trim(),
  slot: process.env.MCR_PORTRAIT_CAPTURE_SLOT, minimumFreeBytes, initialFreeBytes: disk.bavail * disk.bsize,
  cases: [], artifacts: {}, errors: [], httpWrites: 0, peakProcessTreeRssBytes: 0, peakWorkflowBytes: 0,
  workflowBudgetBytes: 192 * 1024 ** 2, stopAtBytes: 128 * 1024 ** 2, sourcePins: {},
  httpObservation: 'All page HTTP requests except GET/HEAD are counted and blocked before network; server rejects non-GET/HEAD on every path. No service-save proof.' }
for (const path of ['frontend/src/features/avatar/motion-adoption/MotionEditor.tsx', 'frontend/src/features/avatar/motion-adoption/MotionView.tsx',
  'frontend/src/features/avatar/motion-adoption/motion.css', 'frontend/src/app/character-entry/__tests__/motion-adoption/fixture.tsx',
  'frontend/src/app/character-entry/__tests__/motion-adoption/portrait-native.mjs']) report.sourcePins[path] = hash(await readFile(resolve(checkout, path)))
const sampleRss = () => {
  const rows = execFileSync('ps', ['-axo', 'pid=,ppid=,rss='], { encoding: 'utf8' }).trim().split('\n').map(row => row.trim().split(/\s+/).map(Number))
  const owned = new Set([process.pid]); let previous
  do { previous = owned.size; for (const [pid, ppid] of rows) if (owned.has(ppid)) owned.add(pid) } while (owned.size !== previous)
  report.peakProcessTreeRssBytes = Math.max(report.peakProcessTreeRssBytes, rows.reduce((sum, [pid, , rss]) => sum + (owned.has(pid) ? rss * 1024 : 0), 0))
  const usage = spawnSync('du', ['-sk', cache, output], { encoding: 'utf8' })
  const totals = usage.stdout.trim().split('\n').map(row => Number(row.trim().split(/\s+/)[0]))
  if (totals.length !== 2 || totals.some(value => !Number.isFinite(value))) {
    report.resourceStop = 'Whole-workflow disk usage could not be measured'; void browser?.close().catch(() => {}); return
  }
  const blocks = totals.reduce((sum, value) => sum + value * 1024, 0)
  report.peakWorkflowBytes = Math.max(report.peakWorkflowBytes, blocks)
  if (blocks >= report.stopAtBytes) { report.resourceStop = 'Observed workflow reached 128 MiB; stopped with 64 MiB reserve before the 192 MiB budget'; void browser?.close().catch(() => {}) }
}
const server = await createServer({ configFile: false, root, cacheDir: cache, logLevel: 'error',
  optimizeDeps: { entries: ['src/app/character-entry/__tests__/motion-adoption/fixture.html'] },
  resolve: { alias: Object.entries(aliases).sort(([a], [b]) => b.length - a.length).map(([find, replacement]) => ({ find, replacement })) },
  server: { host: '127.0.0.1', port: 0, fs: { allow: [root, checkout, dependencies] } },
  plugins: [{ name: 'read-only-portrait-geometry', configureServer(server) {
    server.middlewares.use((req, res, next) => {
      if (!['GET', 'HEAD'].includes(req.method)) { report.httpWrites++; res.writeHead(405); res.end(); return }
      if (!req.url?.startsWith('/__motion/')) return next()
      res.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' })
      res.end(req.url === '/__motion/candidate' ? candidate : JSON.stringify({ revision: 1, config }))
    })
  } }],
})
let browser, page, timer
try {
  await server.listen(); const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version(); timer = setInterval(sampleRss, 250); sampleRss()
  for (const [width, height] of [[360, 698], [390, 698], [430, 698], [320, 554], [390, 420]]) {
    page = await browser.newPage({ viewport: { width, height }, reducedMotion: 'reduce', serviceWorkers: 'block' }); page.setDefaultTimeout(12000)
    await page.route('**/*', route => {
      if (!['GET', 'HEAD'].includes(route.request().method())) { report.httpWrites++; return route.abort('blockedbyclient') }
      return route.continue()
    })
    page.on('pageerror', error => report.errors.push(error.message))
    const state = () => page.evaluate(() => window.motionJourney.state())
    await page.goto(`${origin}/src/app/character-entry/__tests__/motion-adoption/fixture.html`)
    await expect.poll(async () => (await state()).scene.sprites).toBe(2)
    await page.getByRole('button', { name: 'CUSTOMIZE', exact: true }).click()
    const preview = page.getByRole('img', { name: 'Draft outfit preview', exact: true })
    await expect(preview).toBeVisible()
    await expect(page.getByRole('group', { name: 'Preset choices' }).locator('canvas:visible')).toHaveCount(6)
    const fixedPresets = await page.getByRole('group', { name: 'Preset choices' }).locator('canvas').evaluateAll(canvases => canvases.map(c => ({ width: c.getBoundingClientRect().width, height: c.getBoundingClientRect().height })))
    assert.equal(fixedPresets.length, 6); assert(fixedPresets.every(c => c.width === 64 && c.height === 64))
    await page.getByRole('button', { name: 'Hair', exact: true }).click()
    await page.getByLabel('Choose hair', { exact: true }).selectOption('hair-5')
    await expect(page.getByRole('button', { name: 'Choose hair 6', exact: true })).toHaveAttribute('aria-pressed', 'true')
    await expect(page.locator('.mcr-motion-option-grid canvas:visible')).toHaveCount(3)
    const geometry = await page.getByRole('region', { name: 'Avatar editor' }).evaluate(panel => {
      const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } }
      const canvas = panel.querySelector('.mcr-motion-preview canvas'), frame = panel.querySelector('.mcr-motion-preview')
      const controls = [...panel.querySelectorAll('button,select')].filter(e => e.getClientRects().length).map(e => ({ label: e.getAttribute('aria-label') ?? e.textContent.trim(), ...rect(e) }))
      return { panel: rect(panel), figure: rect(panel.querySelector('.mcr-motion-figure')), frame: rect(frame), canvas: rect(canvas),
        buffer: { width: canvas.width, height: canvas.height }, smoothing: canvas.getContext('2d').imageSmoothingEnabled,
        categories: [...panel.querySelectorAll('.mcr-motion-categories button')].map(rect),
        options: [...panel.querySelectorAll('.mcr-motion-option-grid canvas')].map(rect), controls,
        page: { width: innerWidth, height: innerHeight, scrollWidth: document.documentElement.scrollWidth, scrollHeight: document.documentElement.scrollHeight } }
    })
    const g = geometry
    assert.equal(g.buffer.width, 64); assert.equal(g.buffer.height, 64); assert.equal(g.smoothing, false)
    assert(Math.abs(g.canvas.width - g.canvas.height) < 0.1)
    assert(Math.abs(g.canvas.width - (g.frame.width - 2)) < 0.1)
    assert(g.canvas.x >= g.frame.x && g.canvas.x + g.canvas.width <= g.frame.x + g.frame.width)
    assert(g.canvas.y >= g.frame.y && g.canvas.y + g.canvas.height <= g.frame.y + g.frame.height)
    assert.equal(g.categories.length, 5); assert(g.categories.every(c => c.width >= 44 && c.height >= 44 && c.x >= g.canvas.x + g.canvas.width))
    assert.equal(g.options.length, 3); assert(g.options.every(c => c.width === 56 && c.height === 56)); assert(g.controls.every(c => c.width >= 44 && c.height >= 44))
    assert(g.page.scrollWidth <= width)
    await page.getByLabel('View', { exact: true }).selectOption('up'); await expect(preview).toHaveAttribute('data-frame', '15')
    await page.getByLabel('View', { exact: true }).selectOption('down'); await expect(preview).toHaveAttribute('data-frame', '0')
    const poseFrames = new Set(); await page.getByRole('button', { name: 'Preview walk', exact: true }).click()
    await expect.poll(async () => { poseFrames.add(await preview.getAttribute('data-frame')); return poseFrames.size }).toBe(4)
    assert.deepEqual([...poseFrames].sort(), ['1', '2', '3', '4']); await page.getByRole('button', { name: 'Stop walking', exact: true }).click()
    const save = page.getByRole('button', { name: 'Save outfit', exact: true })
    await save.scrollIntoViewIfNeeded(); await save.focus(); await expect(save).toBeFocused(); await expect(save).toBeEnabled()
    const saveBox = await save.boundingBox(); assert(saveBox.y >= 0 && saveBox.y + saveBox.height <= height)
    await page.getByRole('button', { name: 'Reset draft', exact: true }).click(); await expect(save).toBeDisabled()
    await page.getByRole('button', { name: 'Hair', exact: true }).click(); await page.getByLabel('Choose hair', { exact: true }).selectOption('hair-5')
    await expect(save).toBeEnabled()
    const panel = page.getByRole('region', { name: 'Avatar editor' }), file = `component-${width}x${height}.png`
    await panel.screenshot({ path: resolve(output, file) }); report.artifacts[file] = hash(await readFile(resolve(output, file)))
    await page.getByRole('button', { name: 'PROFILE', exact: true }).click()
    await expect(page.getByRole('img', { name: 'Saved profile avatar', exact: true })).toBeVisible()
    const profile = await page.getByRole('img', { name: 'Saved profile avatar', exact: true }).boundingBox()
    assert.equal(profile.width, 72); assert.equal(profile.height, 72)
    const alive = await state(); assert.equal(alive.mounts, 1); assert.equal(alive.sockets, 0); assert.equal(alive.requests.length, 2)
    const cleanup = await page.evaluate(() => window.motionJourney.dispose())
    assert.equal(cleanup.canvases, 0); assert.equal(cleanup.destroys, 1); assert.equal(cleanup.owner.leases, 0)
    assert.deepEqual(cleanup.scene, { textures: 0, sprites: 0, listeners: 0 })
    report.cases.push({ width, height, geometry, fixedPresets, profile, poseFrames: [...poseFrames], saveAfterScroll: saveBox,
      lifecycle: { mounts: alive.mounts, worldSockets: alive.sockets, artRequests: alive.requests.length }, cleanup })
    await page.close(); page = null
  }
  assert.equal(report.httpWrites, 0); assert.deepEqual(report.errors, []); assert(!report.resourceStop); report.result = 'PASS'
} catch (error) {
  report.failure = String(error)
  if (page) { await page.screenshot({ path: resolve(output, 'failure.png'), fullPage: true }).catch(() => {}); report.failureDom = await page.locator('body').innerText().catch(() => '') }
  throw error
} finally {
  clearInterval(timer); await browser?.close(); await server.close(); await rm(cache, { recursive: true, force: true })
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(JSON.stringify({ result: report.result, cases: report.cases.length, qualification: report.qualification }))
