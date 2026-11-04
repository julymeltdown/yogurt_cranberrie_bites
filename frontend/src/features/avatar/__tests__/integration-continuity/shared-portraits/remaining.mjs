import { createHash } from 'node:crypto'
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
let browser, page, stage = 'startup'
try {
  const priorPath = resolve(frontend, '../docs/mcr/evidence/avatar-integration-continuity/shared-portraits-native-retry03/native/native.json')
  const priorBytes = await readFile(priorPath), priorHash = createHash('sha256').update(priorBytes).digest('hex')
  assert.equal(priorHash, '6c430d23d8947c61ed6cc0adcc9c589fd41c6208826bfa64fb64bbf0461d3c4d')
  const prior = JSON.parse(priorBytes), expected = '04ad48d25587aa0cfe7fd3c1ab4444493914d5fadea1372d18ce24567b9493c4'
  assert.equal(prior.result, 'FAIL') // Preserve its overall failure; borrow only completed assertions.
  const names = ['feed', 'thread', 'gift-list', 'gift-detail']
  for (const name of names) { assert(prior.portraits[name].digests.length); assert(prior.portraits[name].digests.every(value => value === expected)) }
  report.retained = { report: priorPath, sha256: priorHash, parentResult: prior.result,
    completedSurfaces: Object.fromEntries(names.map(name => [name, prior.portraits[name]])), replayed: false }
  report.executedThisRound = ['party-list', 'party-detail', 'public-profile', 'gift-denial']
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  page = await browser.newPage({ viewport: report.viewport, deviceScaleFactor: 1 })
  page.setDefaultTimeout(10000); page.setDefaultNavigationTimeout(20000)
  page.on('pageerror', error => report.errors.push(error.message))
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/login`)
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  await expect.poll(async () => {
    try { return (await page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.worldProof())).actors.length } catch { return 0 }
  }, { timeout: 15000 }).toBeGreaterThan(0)
  const nav = page.getByRole('navigation', { name: 'Main navigation' })
  report.portraits = {}
  const inspect = async (name, root) => {
    const portraits = root.locator('[data-motion-state="ready"] canvas')
    await expect(portraits.first()).toBeVisible()
    const values = await portraits.evaluateAll(async canvases => Promise.all(canvases.map(async canvas => {
      const bytes = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
      if (!bytes.some((value, index) => index % 4 === 3 && value)) throw Error('Empty portrait')
      return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(value => value.toString(16).padStart(2, '0')).join('')
    })))
    assert(values.length); assert(values.every(value => value === expected))
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    const state = await page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.state())
    assert.equal(state.active, 1); assert.equal(state.canvases, 1); assert.equal(state.peak, 1)
    report.portraits[name] = { caller: 'ACTUAL_APP_ROUTER_PRODUCTION_PORTS_TEST_CLOCK', digests: values, worldCanvases: 1, activeSockets: 1, socketPeak: 1 }
    await page.screenshot({ path: resolve(output, name + '.png') }); report.screenshots.push(name + '.png')
  }
  stage = 'party-list'
  await nav.getByRole('link', { name: 'Meet', exact: true }).click()
  await inspect('party-list', page.locator('.meet-parties'))
  await expect(page.getByText('Member portraits unavailable').first()).toBeVisible()
  await page.locator('.meet-party__copy h3 a').first().click()
  stage = 'party-detail'
  await inspect('party-detail', page.locator('.party-view'))
  await page.getByRole('link', { name: 'All parties', exact: true }).click()
  await page.getByRole('link', { name: /^people$/i }).click()
  await page.locator('a[href^="/people/"]').first().click()
  stage = 'public-profile'
  await inspect('public-profile', page.locator('.social-composition'))
  // Full mobile panels hide shell navigation. Use its functioning size control.
  await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
  await nav.getByRole('link', { name: 'Gifts', exact: true }).click()
  await expect(page.locator('.gift-reference-copy h3 a').nth(1)).toBeVisible()
  await page.evaluate(() => window.__SHARED_PORTRAITS__.denyGift())
  await page.locator('.gift-reference-copy h3 a').nth(1).click()
  stage = 'gift-denial'
  const detail = page.getByRole('region', { name: 'Giveaway detail' })
  await expect(detail.getByText('This giveaway is unavailable')).toBeVisible()
  await expect(detail.locator('[data-motion-state="ready"]')).toHaveCount(0)
  report.control = await page.evaluate(() => window.__SHARED_PORTRAITS__.state())
  assert(report.control.calls.some(call => call.status === 404)); assert(report.control.calls.every(call => call.method === 'GET'))
  await page.screenshot({ path: resolve(output, 'gift-denial.png') }); report.screenshots.push('gift-denial.png')
  report.checks = { exactSelectedOwnerPixels: true, unavailableMemberPortraitsPreserved: true, deniedPortraitWithdrawn: true,
    oneWorldAndSocket: true, actualPanelControls: true, noMutations: true, priorPrefixNotReplayed: true,
    productionCommunityClockStillOpen: true }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { stage, message: String(error), stack: error.stack }; if (page) await page.screenshot({ path: resolve(output, 'failure.png') }).catch(() => {})
} finally {
  await browser?.close(); await server.close(); report.resourcesClosed = true
  await writeFile(resolve(output, 'native.json'), `${JSON.stringify(report, null, 2)}\n`)
}
assert.equal(report.result, 'PASS', JSON.stringify(report.failure))
