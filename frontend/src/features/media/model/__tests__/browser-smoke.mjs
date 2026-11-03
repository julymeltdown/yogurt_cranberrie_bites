import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { createServer, build, preview } from 'vite'
import { chromium } from '@playwright/test'
const evidence = process.env.MCR_FE04_EVIDENCE ?? '/tmp/mcr-fe04-evidence'
await mkdir(evidence, { recursive: true })
const input = 'src/features/media/model/__tests__/browser-fixture.html'
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser, production
const results = []
try {
  await dev.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  async function exercise(base, mode) {
    const context = await browser.newContext(), page = await context.newPage(), errors = []
    page.on('pageerror', error => errors.push(error.message))
    await context.addCookies([{ name: 'must-not-upload', value: 'fixture', url: 'https://storage.mcr-fixture.invalid' }])
    let uploads = 0
    await page.route('https://storage.mcr-fixture.invalid/**', async route => {
      const headers = await route.request().allHeaders()
      assert(!headers.cookie); assert(!headers.authorization); assert(!headers['x-mcr-intent']); assert(!headers['idempotency-key'])
      assert(!headers['x-client-id']); assert.equal(headers['content-type'], 'image/png')
      uploads++
      await route.fulfill({ status: 200, headers: { 'Access-Control-Allow-Origin': '*' } })
    })
    await page.goto(`${base}/${input}`)
    await page.waitForFunction(() => window.mediaFixture)
    const compression = await page.evaluate(() => window.mediaFixture.compression())
    assert.equal(compression.width, 2048); assert.equal(compression.height, 1152); assert.equal(compression.type, 'image/webp')
    assert(compression.bytes > 0 && compression.bytes < compression.originalBytes)
    assert(compression.sha256Matches); assert(compression.metadataAbsent); assert(compression.progressCompleted); assert(compression.revoked)
    assert.equal(compression.liveWorkers, 0); assert.equal(compression.peakWorkers, 1)
    const abort = await page.evaluate(() => window.mediaFixture.abort())
    assert.equal(abort.result, 'aborted'); assert.equal(abort.liveWorkers, 0)
    await page.evaluate(() => window.mediaFixture.presigned()); assert.equal(uploads, 1)
    assert.deepEqual(errors, [])
    results.push({ mode, compression, abort, presigned: 'cookie/token/session-header free' }); await context.close()
  }
  await exercise(`http://127.0.0.1:${dev.httpServer.address().port}`, 'development')
  await build({ build: { outDir: `${evidence}/dist`, emptyOutDir: true, rolldownOptions: { input } } })
  production = await preview({ build: { outDir: `${evidence}/dist` }, preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  await exercise(`http://127.0.0.1:${production.httpServer.address().port}`, 'production-bundle')
  const report = { browser: browser.version(), status: 'PASS', results, limitation: 'Synthetic native-browser fixture; BE24/live media flow and actual devices NOT_RUN.' }
  await writeFile(`${evidence}/results.json`, JSON.stringify(report, null, 2)); console.log(JSON.stringify(report))
} finally { await browser?.close(); await dev.close(); await new Promise(resolve => production ? production.httpServer.close(resolve) : resolve()) }
