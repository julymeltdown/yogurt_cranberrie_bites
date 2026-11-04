import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const followup = process.env.MCR_COMMUNITY_NATIVE_FOLLOWUP === 'settled'
const directory = process.env.MCR_COMMUNITY_NATIVE_OUTPUT
if (!directory) throw Error('Explicit allocated evidence directory required')
await mkdir(directory, { recursive: true })
const dev = await createServer({ configLoader: 'runner', cacheDir: `${directory}/vite-cache`,
  server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false } })
const captures = [], errors = [], cases = [], report = { classification: 'TEST_ONLY', realService: false, status: 'NOT_RUN', errors, cases }
let browser
try {
  await dev.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const origin = `http://127.0.0.1:${dev.httpServer.address().port}`
  report.listenerPort = dev.httpServer.address().port
  for (const width of [360, 390, 430]) for (const scenario of followup ? ['enabled'] : ['world-only', 'no-policy', 'virtual', 'enabled']) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' })
    const page = await context.newPage()
    page.on('pageerror', error => errors.push({ width, scenario, error: error.message }))
    try {
      await page.goto(`${origin}/src/app/community-adoption/__tests__/native/preview.html?entry=/world&scenario=${scenario}`)
      await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
      await expect(page.getByRole('navigation', { name: 'Main navigation', includeHidden: true })).toBeAttached()
      const state = () => page.evaluate(() => window.__COMMUNITY_AUTHORITY_NATIVE__.state())
      const navigate = path => page.evaluate(path => window.__COMMUNITY_AUTHORITY_NATIVE__.navigate(path), path)
      if (scenario !== 'world-only') await expect.poll(async () => (await state()).published, { timeout: 25000 }).toBe(true)
      else await expect(page.getByRole('heading', { name: 'World entry is being prepared' })).toBeVisible()
      await navigate('/meet')
      await expect(page.getByRole('main', { name: 'Meet', exact: true })).toBeVisible()
      await page.evaluate(() => document.fonts.ready)
      await expect(page.locator('.party-browse')).toBeVisible()
      if (scenario !== 'world-only') await expect(page.getByRole('button', { name: 'Refresh parties', exact: true })).toBeEnabled()
      if (scenario === 'enabled') await expect(page.locator('#party-create')).toBeVisible()
      else await expect(page.getByRole('button', { name: 'Create party', exact: true })).toBeDisabled()
      const initial = await state()
      assert.equal(initial.requests.filter(r => r.path.endsWith('/bootstrap')).length, 1)
      assert.equal(initial.canvases, scenario === 'world-only' ? 0 : 1); assert.ok(initial.socketPeak <= 1)
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
      captures.push({ name: `${scenario}-${width}.png`, bytes: await page.screenshot() })
      if (scenario === 'enabled') {
        await page.locator('#party-create').click()
        const title = page.getByLabel('Party title')
        await expect(title).toBeVisible(); await title.fill(`Retained TEST_ONLY draft ${width}`)
        await page.getByRole('button', { name: 'Save session draft' }).click()
        await navigate('/gifts/new'); await expect(page.getByText('Start a giveaway', { exact: true })).toBeVisible()
        await navigate('/meet/parties/new'); await expect(title).toHaveValue(`Retained TEST_ONLY draft ${width}`)
        captures.push({ name: `retained-form-${width}.png`, bytes: await page.screenshot() })
        await page.evaluate(() => window.__COMMUNITY_AUTHORITY_NATIVE__.interruptClock())
        await expect(page.getByRole('heading', { name: 'Community action unavailable', exact: true })).toBeVisible()
        await expect(title).toHaveCount(0)
        await navigate('/meet'); await expect(page.getByRole('button', { name: 'Create party', exact: true })).toBeDisabled()
      } else if (scenario === 'virtual') {
        await navigate('/meet/parties/new'); await expect(page.getByLabel('Party title')).toBeVisible()
        await page.getByLabel('Party title').fill(`Private TEST_ONLY draft ${width}`)
        await page.evaluate(() => window.__COMMUNITY_AUTHORITY_NATIVE__.suspendPrivacy())
        await expect(page.getByLabel('Party title')).toHaveCount(0)
        await expect(page.getByText(`Private TEST_ONLY draft ${width}`, { exact: true })).toHaveCount(0)
      } else {
        await navigate('/meet/parties/new'); await expect(page.getByLabel('Party title')).toHaveCount(0)
      }
      const final = await state()
      assert.equal(final.requests.filter(r => /\/(?:parties|giveaways|reservations|beacons)(?:\/|$)/.test(r.path) && r.method !== 'GET').length, 0)
      if (scenario === 'enabled') assert.equal(final.requests.filter(r => r.path.endsWith('/bootstrap')).length, 1)
      await page.evaluate(() => window.__COMMUNITY_AUTHORITY_NATIVE__.dispose())
      const disposed = await state()
      assert.equal(disposed.canvases, 0); assert.equal(disposed.activeSockets, 0)
      cases.push({ width, scenario, initial, final, disposed, status: 'PASS' })
      console.log(JSON.stringify({ width, scenario, status: 'PASS' }))
    } catch (error) {
      report.diagnostic = { width, scenario, state: await page.evaluate(() => window.__COMMUNITY_AUTHORITY_NATIVE__?.state()).catch(() => null), text: await page.locator('body').innerText().catch(() => '') }
      throw error
    } finally { await context.close() }
  }
  assert.deepEqual(errors, [])
  assert.equal(captures.length, followup ? 6 : 15)
  const selectedCaptures = followup ? captures.filter(c => /^enabled-/.test(c.name)) : captures
  report.screenshotNames = selectedCaptures.map(c => c.name)
  for (const capture of selectedCaptures) await writeFile(`${directory}/${capture.name}`, capture.bytes)
  Object.assign(report, { status: 'PASS', browser: browser.version(), limits: [
    'Synthetic HTTP/C05 and explicitly TEST_ONLY clock policy, not measured platform guarantees',
    'Lifecycle interruption is a synthetic visibility event through the actual browser adapter',
    'No real-service mutation, physical arrival, member roster or full FE21/FE22 acceptance',
  ] })
} catch (error) { Object.assign(report, { status: 'FAIL', failure: error.stack }); throw error }
finally {
  await browser?.close(); await dev.close()
  await writeFile(`${directory}/native.json`, `${JSON.stringify(report, null, 2)}\n`)
}
console.log(JSON.stringify(report))
