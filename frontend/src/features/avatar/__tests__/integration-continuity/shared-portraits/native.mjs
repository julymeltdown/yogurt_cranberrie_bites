import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
const scratch = process.env.MCR_CONTINUITY_SCRATCH
const output = process.env.MCR_CONTINUITY_OUTPUT
assert(dependencies && scratch && output, 'Allocated dependencies, scratch and output are required')
const require = createRequire(resolve(dependencies, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')))
const { chromium, expect } = require('@playwright/test')
const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
const { default: tailwind } = await import(pathToFileURL(require.resolve('@tailwindcss/vite')))
const { aliases } = await import('../../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs')
const browserAliases = { ...aliases, '@hookform/resolvers/zod': require.resolve('@hookform/resolvers/zod') }
const frontend = resolve(import.meta.dirname, '../../../../../..')
const fixture = '/src/features/avatar/__tests__/integration-continuity/shared-portraits/preview.html'
const mainFixture = '/src/features/avatar/__tests__/integration-continuity/shared-portraits/main-preview.html'
const manifest = JSON.parse(await readFile(resolve(frontend, '../contracts/assets/v2/reviewed/avatar-manifest.schema.json'), 'utf8')).$defs.ReviewedMotionAvatarManifest.const
const participant = JSON.parse(await readFile(resolve(frontend, '../contracts/fixtures/core/bootstrap-open.json'), 'utf8')).participant
const input = { revision: 'TEST_ONLY_REVIEWED_ROOT_not_C05_authority', reviewedBy: 'Synthetic continuity fixture',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
await mkdir(output, { recursive: true })
const report = { result: 'FAIL', qualification: 'ACTUAL_APP_ROUTER_PRODUCTION_PORTS_TEST_CLOCK_CONTROLLED_HTTP_C05_NOT_MAIN_NOT_LIVE', viewport: { width: 390, height: 844 }, errors: [], screenshots: [] }
const server = await createServer({ root: frontend, configFile: false, plugins: [react(), tailwind(), {
  name: 'shared-portraits-fixture', configureServer(server) { server.middlewares.use(async (request, response, next) => {
    if (request.url === '/__reviewed_persistence_motion/state') {
      assert.equal(request.method, 'GET'); response.setHeader('Content-Type', 'application/json')
      response.end(JSON.stringify({ avatar: { revision: 7, config: manifest.presets[0].config }, participantRevision: participant.revision })); return
    }
    if (!request.headers.accept?.includes('text/html')) return next()
    response.setHeader('Content-Type', 'text/html')
    const documentFixture = request.url.includes('caller=main') ? mainFixture : fixture
    response.end(await server.transformIndexHtml(request.url, await readFile(resolve(frontend, `.${documentFixture}`), 'utf8')))
  }) },
}], cacheDir: resolve(scratch, 'vite'), logLevel: 'warn',
  resolve: { alias: Object.entries(browserAliases).map(([name, replacement]) => ({
    find: new RegExp('^' + name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '$'), replacement,
  })) },
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
    'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)) },
  server: { host: '127.0.0.1', port: 0, strictPort: false, watch: null, fs: { allow: [frontend, dependencies] } } })
let browser, page, stage = 'startup', caller = 'ACTUAL_MAIN'
try {
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  page = await browser.newPage({ viewport: report.viewport, deviceScaleFactor: 1 })
  page.setDefaultTimeout(10000); page.setDefaultNavigationTimeout(20000)
  page.on('pageerror', error => report.errors.push(error.message))
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/login?caller=main`)
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  // Feed's real readiness borrows the incumbent admitted World policy baseline.
  // Wait for that prerequisite before opening a panel that pauses its viewport.
  await expect.poll(async () => {
    try { return (await page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.worldProof())).actors.length } catch { return 0 }
  }, { timeout: 15000 }).toBeGreaterThan(0)
  const nav = page.getByRole('navigation', { name: 'Main navigation' })
  // Retained, admitted preset2's exact nonempty idle pixels from the prior qualified native report.
  const expected = '04ad48d25587aa0cfe7fd3c1ab4444493914d5fadea1372d18ce24567b9493c4'
  report.expected = { preset: 2, digest: expected, provenance: 'avatar-integration-continuity/native-retry05/native/native.json expectedDraft', manifest: manifest.content_hash }
  report.portraits = {}
  const inspect = async (name, root) => {
    const portraits = root.locator('[data-motion-state="ready"] canvas')
    await expect(portraits.first()).toBeVisible()
    const values = await portraits.evaluateAll(async canvases => Promise.all(canvases.map(async canvas => {
      const bytes = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
      if (!bytes.some((value, index) => index % 4 === 3 && value)) throw Error('Empty portrait')
      return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(value => value.toString(16).padStart(2, '0')).join('')
    })))
    assert(values.length > 0); assert(values.every(value => value === expected), name + ': wrong selected owner pixels')
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), name + ': horizontal overflow')
    const state = await page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.state())
    assert.equal(state.active, 1); assert.equal(state.canvases, 1); assert.equal(state.peak, 1)
    report.portraits[name] = { caller, digests: values, worldCanvases: state.canvases, activeSockets: state.active, socketPeak: state.peak }
    await page.screenshot({ path: resolve(output, name + '.png') }); report.screenshots.push(name + '.png')
  }
  stage = 'feed'
  await nav.getByRole('link', { name: 'Feed', exact: true }).click()
  await inspect('feed', page.locator('.social-composition'))
  stage = 'thread'
  await page.getByRole('button', { name: /^Open thread/ }).first().click()
  await expect(page).toHaveURL(/\/feed\/[0-9a-f-]{36}$/)
  await inspect('thread', page.locator('.social-composition'))
  report.actualMain = { surfaces: ['feed', 'thread'], state: await page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.state()) }
  caller = 'ACTUAL_APP_ROUTER_PRODUCTION_PORTS_TEST_CLOCK'
  // A new document ends the actual-main shell. Community uses an explicit test clock input;
  // it cannot certify the default entry point's absent production clock configuration.
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/login`)
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  await expect.poll(async () => {
    try { return (await page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.worldProof())).actors.length } catch { return 0 }
  }, { timeout: 15000 }).toBeGreaterThan(0)
  stage = 'gifts'
  await nav.getByRole('link', { name: 'Gifts', exact: true }).click()
  await inspect('gift-list', page.getByRole('region', { name: 'Giveaway discovery' }))
  await page.locator('.gift-reference-copy h3 a').first().click()
  await inspect('gift-detail', page.getByRole('region', { name: 'Giveaway detail' }))
  stage = 'party'
  // Detail is a blocking panel; leave it through its real parent link before
  // addressing the shell navigation, which is correctly inert while blocked.
  await page.getByRole('link', { name: /All giveaways/ }).click()
  await nav.getByRole('link', { name: 'Meet', exact: true }).click()
  await inspect('party-list', page.locator('.meet-parties'))
  await expect(page.getByText('Member portraits unavailable').first()).toBeVisible()
  await page.locator('.meet-party__copy h3 a').first().click()
  await inspect('party-detail', page.locator('.party-view'))
  stage = 'denial'
  await page.evaluate(() => window.__SHARED_PORTRAITS__.denyGift())
  await page.getByRole('link', { name: 'All parties', exact: true }).click()
  await nav.getByRole('link', { name: 'Gifts', exact: true }).click()
  await page.locator('.gift-reference-copy h3 a').nth(1).click()
  await expect(page.getByRole('region', { name: 'Giveaway detail' }).getByText('This giveaway is unavailable')).toBeVisible()
  await expect(page.getByRole('region', { name: 'Giveaway detail' }).locator('[data-motion-state="ready"]')).toHaveCount(0)
  report.control = await page.evaluate(() => window.__SHARED_PORTRAITS__.state())
  assert(report.control.calls.some(call => call.status === 404))
  assert(report.control.calls.every(call => call.method === 'GET'))
  report.checks = { sharedOwnerPixels: true, distinctFromSelfPreset: true, actualAppRouter: true, explicitTestClock: true, defaultMainQualified: false, oneWorldAndSocket: true,
    memberPortraitsUnavailable: true, deniedOwnerWithdrawn: true, noMutations: true }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { stage, message: String(error), stack: error.stack }; if (page) await page.screenshot({ path: resolve(output, 'failure.png') }).catch(() => {})
} finally {
  await browser?.close(); await server.close(); report.resourcesClosed = true
  await writeFile(resolve(output, 'native.json'), `${JSON.stringify(report, null, 2)}\n`)
}
assert.equal(report.result, 'PASS', JSON.stringify(report.failure))
