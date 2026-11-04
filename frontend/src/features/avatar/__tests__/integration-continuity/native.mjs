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
const { aliases } = await import('../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs')
const frontend = resolve(import.meta.dirname, '../../../../..')
const fixture = '/src/features/avatar/__tests__/integration-continuity/preview.html'
const manifest = JSON.parse(await readFile(resolve(frontend, '../contracts/assets/v2/reviewed/avatar-manifest.schema.json'), 'utf8')).$defs.ReviewedMotionAvatarManifest.const
const participant = JSON.parse(await readFile(resolve(frontend, '../contracts/fixtures/core/bootstrap-open.json'), 'utf8')).participant
const input = { revision: 'TEST_ONLY_REVIEWED_ROOT_not_C05_authority', reviewedBy: 'Synthetic continuity fixture',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
await mkdir(output, { recursive: true })
const report = { result: 'FAIL', qualification: 'ACTUAL_MAIN_CONTROLLED_HTTP_C05_NOT_LIVE', viewport: { width: 390, height: 844 }, errors: [], screenshots: [] }
const server = await createServer({ root: frontend, configFile: false, plugins: [react(), tailwind(), {
  name: 'avatar-continuity-fixture', configureServer(server) { server.middlewares.use(async (request, response, next) => {
    if (request.url === '/__reviewed_persistence_motion/state') {
      assert.equal(request.method, 'GET'); response.setHeader('Content-Type', 'application/json')
      response.end(JSON.stringify({ avatar: { revision: 7, config: manifest.presets[0].config }, participantRevision: participant.revision })); return
    }
    if (!request.headers.accept?.includes('text/html')) return next()
    response.setHeader('Content-Type', 'text/html')
    response.end(await server.transformIndexHtml(request.url, await readFile(resolve(frontend, `.${fixture}`), 'utf8')))
  }) },
}], cacheDir: resolve(scratch, 'vite'), logLevel: 'warn',
  resolve: { alias: Object.entries(aliases).sort(([a], [b]) => b.length - a.length).map(([find, replacement]) => ({ find, replacement })) },
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
    'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)) },
  server: { host: '127.0.0.1', port: 0, strictPort: false, watch: null, fs: { allow: [frontend, dependencies] } } })
let browser, page, stage = 'startup'
try {
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  const context = await browser.newContext({ viewport: report.viewport, hasTouch: true, reducedMotion: 'reduce' })
  page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/login`)
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  const nav = page.getByRole('navigation', { name: 'Main navigation' })
  await nav.getByRole('link', { name: 'Me', exact: true }).click()
  await page.getByRole('link', { name: 'Dress up', exact: true }).click()
  const editor = page.getByRole('region', { name: 'Avatar editor' })
  const digest = async canvas => {
    await expect(canvas).toBeVisible()
    return canvas.evaluate(async canvas => {
      const bytes = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
      if (!bytes.some((value, index) => index % 4 === 3 && value > 0)) throw Error('Preview must contain visible artwork')
      return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(value => value.toString(16).padStart(2, '0')).join('')
    })
  }
  const previewDigest = () => digest(editor.getByRole('img', { name: 'Draft outfit preview', exact: true }))
  const shot = async name => { await page.screenshot({ path: resolve(output, `${name}.png`), fullPage: true }); report.screenshots.push(`${name}.png`) }
  await expect(editor.getByRole('button', { name: 'Cancel changes', exact: true })).toBeEnabled()
  await expect(editor.locator('canvas').first()).toBeVisible()
  const original = await previewDigest()
  assert.equal(original, await digest(editor.getByRole('button', { name: 'Choose Preset 1', exact: true }).locator('canvas')))
  const expectedDraft = await digest(editor.getByRole('button', { name: 'Choose Preset 2', exact: true }).locator('canvas'))
  const expectedLatest = await digest(editor.getByRole('button', { name: 'Choose Preset 5', exact: true }).locator('canvas'))
  stage = 'cancel-draft'
  await editor.getByRole('button', { name: 'Choose Preset 2', exact: true }).click()
  await expect.poll(previewDigest).toBe(expectedDraft)
  await editor.getByRole('button', { name: 'Cancel changes', exact: true }).click()
  await expect.poll(previewDigest).toBe(original)
  await expect(editor.getByRole('button', { name: 'Save outfit', exact: true })).toBeDisabled()
  const baseline = await page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.state())
  assert.equal(baseline.requests.filter(value => value.method === 'PUT').length, 0)
  await shot('cancel-restores-persisted')
  stage = 'optimistic-conflict'
  await editor.getByRole('button', { name: 'Choose Preset 2', exact: true }).click()
  await expect.poll(previewDigest).toBe(expectedDraft); const draft = await previewDigest()
  report.remote = await page.evaluate(() => window.__AVATAR_CONTINUITY__.changedElsewhere())
  await editor.getByRole('button', { name: 'Save outfit', exact: true }).click()
  await expect(editor.getByRole('status')).toContainText('changed elsewhere')
  assert.equal(await previewDigest(), draft)
  await editor.getByRole('button', { name: 'Check latest saved outfit', exact: true }).click()
  await expect(editor.getByRole('region', { name: 'Saved outfit reconciliation' })).toBeVisible()
  await expect(editor.getByRole('button', { name: 'Cancel changes', exact: true })).toBeDisabled()
  await expect(editor.getByRole('button', { name: 'Save outfit', exact: true })).toBeDisabled()
  await shot('conflict-preserves-draft')
  await editor.getByRole('button', { name: 'Keep my draft', exact: true }).click()
  assert.equal(await previewDigest(), draft)
  await editor.getByRole('button', { name: 'Cancel changes', exact: true }).click()
  await expect.poll(previewDigest).toBe(expectedLatest)
  const adopted = await previewDigest(); assert.notEqual(adopted, original)
  await expect(editor.getByRole('button', { name: 'Save outfit', exact: true })).toBeDisabled()
  await shot('cancel-uses-latest-server-outfit')
  report.requests = await page.evaluate(() => window.__AVATAR_CONTINUITY__.state())
  assert.equal(report.requests.requests.filter(value => value.method === 'PUT').length, 1)
  assert.equal(report.requests.requests.find(value => value.method === 'PUT').expectedRevision, 7)
  assert.equal(report.requests.requests.filter(value => value.method === 'GET').length, 1)
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
  report.pixels = { original, draft, adopted, expectedDraft, expectedLatest }
  report.checks = { cancelWithoutWrite: true, conflictDraftRetained: true, explicitLatestRead: true, keepDraft: true, cancelUsesLatest: true, noDuplicateWrite: true }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { stage, message: String(error), stack: error.stack }; if (page) await page.screenshot({ path: resolve(output, 'failure.png') }).catch(() => {})
} finally {
  await browser?.close(); await server.close(); report.resourcesClosed = true
  await writeFile(resolve(output, 'native.json'), `${JSON.stringify(report, null, 2)}\n`)
}
assert.equal(report.result, 'PASS', JSON.stringify(report.failure))
