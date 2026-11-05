import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { writeFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import baseline from '../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs'
const require = createRequire(resolve(process.env.MCR_FRONTEND_DEPENDENCIES, 'package.json'))
const { createServer } = await import(require.resolve('vite'))
const { default: react } = await import(require.resolve('@vitejs/plugin-react'))
const { chromium, expect } = require('@playwright/test')
const root = resolve(import.meta.dirname, '../../../../../..'), frontend = resolve(root, 'frontend')
const output = process.env.MCR_PART_OUTPUT, temporary = process.env.MCR_PART_TMP, port = Number(process.env.MCR_PART_PORT)
assert(output && temporary && port, 'Fresh finite browser allocation required')
const report = { status: 'FAIL', source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  qualification: 'Native Chrome pixels, verified atlas and minted reviewed authority from controlled protected bootstrap; no real backend, device, whole-PI07 or reference-parity claim.',
  errors: [], captures: [], cases: [], cleanup: {} }
const server = await createServer({ root: frontend, configFile: false, logLevel: 'error', plugins: [react()],
  cacheDir: resolve(temporary, 'vite'), resolve: { alias: baseline.resolve.alias },
  server: { host: '127.0.0.1', port, strictPort: true, fs: { allow: [root, process.env.MCR_FRONTEND_DEPENDENCIES] } },
  optimizeDeps: { include: ['react', 'react-dom', 'react-dom/client', 'react/jsx-runtime', '@tanstack/react-query', 'ky', 'zod'] } })
let browser, context, page
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    args: ['--no-sandbox', '--disable-dev-shm-usage'], downloadsPath: resolve(temporary, 'downloads') })
  context = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true })
  page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
  const artRequests = [], atlasResponses = []
  page.on('request', request => { if (request.url().includes('/world/avatars/')) artRequests.push(request.url()) })
  page.on('response', response => { if (/avatar-candidates-64-.*\.png$/.test(response.url())) atlasResponses.push(response) })
  await page.goto(`http://127.0.0.1:${port}/src/features/avatar/__tests__/parade-ink-part-preview/preview.html`)
  await page.waitForFunction(() => !!window.partPreviewFixture)
  assert.equal(atlasResponses.length, 1)
  const reference = 'data:image/png;base64,' + (await atlasResponses[0].body()).toString('base64')
  const pixelCases = await page.evaluate(async dataUrl => {
    const f = window.partPreviewFixture, image = new Image(); image.src = dataUrl; await image.decode()
    const expected = document.createElement('canvas'); expected.width = expected.height = 64
    const ctx = expected.getContext('2d', { willReadFrequently: true }), cases = []
    for (const item of f.catalog.items) {
      const lease = await f.runtime.acquirePartPreview({ kind: item.kind, key: item.key }, { signal: new AbortController().signal })
      const snapshot = lease.read(), canvas = snapshot.canvas, frame = item.directions.find(direction => direction.direction === 0).frames[0]
      ctx.clearRect(0, 0, 64, 64); ctx.drawImage(image, frame % 20 * 64, Math.floor(frame / 20) * 64, 64, 64, 0, 0, 64, 64)
      const actual = canvas.getContext('2d').getImageData(0, 0, 64, 64).data, wanted = ctx.getImageData(0, 0, 64, 64).data
      let mismatches = 0, transparent = 0, visible = 0
      for (let i = 0; i < actual.length; i++) { if (actual[i] !== wanted[i]) mismatches++; if (i % 4 === 3) { if (actual[i] === 0) transparent++; else visible++ } }
      const sha256 = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', actual)), byte => byte.toString(16).padStart(2, '0')).join('')
      cases.push({ kind: item.kind, key: item.key, frame, width: canvas.width, height: canvas.height, mismatches, transparent, visible, sha256, composition: snapshot.composition })
      lease.dispose(); if (canvas.width !== 0 || lease.read() !== null) throw Error('Late isolated pixels after dispose')
    }
    expected.width = expected.height = 0; image.src = ''
    return cases
  }, reference)
  assert.equal(pixelCases.length, 28)
  for (const item of pixelCases) { assert.equal(item.mismatches, 0); assert.equal(item.width, 64); assert.equal(item.height, 64); assert(item.transparent > 0 && item.visible > 0); assert.equal(item.composition, 'raw-item-cell') }
  report.cases.push({ name: 'all-28-native-raw-cell-pixels', items: pixelCases })
  for (const kind of ['hair', 'outfit', 'eyes']) {
    await page.evaluate(kind => window.partPreviewFixture.show(kind), kind)
    await expect(page.locator('[data-motion-part-state="ready"]')).toHaveCount(4)
    const diagnostics = await page.evaluate(() => window.partPreviewFixture.runtime.diagnostics())
    assert.equal(diagnostics.partPreviews, 4); assert.equal(diagnostics.composition.partPreviews, 4)
    assert.equal(diagnostics.composition.created, 0); assert.equal(diagnostics.clockListeners, 0)
    const file = `${kind}-390.png`; await page.screenshot({ path: resolve(output, file) }); report.captures.push(file)
  }
  const withdrew = await page.evaluate(() => {
    const canvases = [...document.querySelectorAll('#cards canvas')]
    window.partPreviewFixture.revoke()
    return { allErased: canvases.every(canvas => canvas.width === 0 && canvas.height === 0),
      attached: document.querySelectorAll('#cards canvas').length, diagnostics: window.partPreviewFixture.runtime.diagnostics() }
  })
  assert.equal(withdrew.allErased, true); assert.equal(withdrew.attached, 0); assert.equal(withdrew.diagnostics.partPreviews, 0)
  report.cases.push({ name: 'four-card-category-change-and-synchronous-withdrawal', ...withdrew })
  assert.equal(artRequests.filter(url => url.endsWith('.png')).length, 1)
  assert.equal(artRequests.filter(url => url.endsWith('.json')).length, 1)
  report.artRequests = artRequests; assert.deepEqual(report.errors, []); report.status = 'PASS'
} catch (error) { report.failure = error.stack; process.exitCode = 1 }
finally {
  if (page) await page.evaluate(() => window.partPreviewFixture?.dispose()).catch(() => {})
  if (context) await context.close(); if (browser) await browser.close(); await server.close()
  report.cleanup = { browserClosed: !browser?.isConnected(), serverClosed: !server.httpServer?.listening }
  await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify({ status: report.status, captures: report.captures, failure: report.failure, cleanup: report.cleanup }))
}
