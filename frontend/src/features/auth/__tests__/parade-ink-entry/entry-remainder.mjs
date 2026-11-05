import assert from 'node:assert/strict'
import { pathToFileURL } from 'node:url'
import { mkdir, writeFile, readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { compareHeaders } from './compare.mjs'
import { aliases, retained, frontend, compositePlugin } from './runtime.mjs'

assert.equal(process.env.PI10_GUARDED, '1', 'Finite admission/guard required')
const { createServer } = await import(pathToFileURL(retained.resolve('vite')).href)
const { chromium, expect } = retained('@playwright/test')
const output = process.env.PI10_OUTPUT, cache = process.env.PI10_CACHE
assert(output && cache && process.env.PI10_OVERLAY)
const priorBytes = await readFile(resolve(frontend, '../docs/mcr/evidence/parade-ink-public-auth/native10/native.json'))
assert.equal(createHash('sha256').update(priorBytes).digest('hex'), '01587b09490a9074c68a87d307446641aae17fc9531cea727db195e946354dc9')
const prior = JSON.parse(priorBytes)
assert.equal(prior.failure.stage, 'actual-default-entry')
assert.equal(prior.screenshots.length, 34)
assert.deepEqual(prior.errors, [])
assert.deepEqual(prior.states.map(row => row.width), [360, 390, 430])
assert(prior.states.every(row => Object.entries(row).every(([key, value]) => key === 'width' || value === true)))
await mkdir(output, { recursive: true })
const server = await createServer({ root: frontend, configFile: false, cacheDir: cache,
  plugins: [compositePlugin()], resolve: { alias: aliases },
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('11111111-1111-4111-8111-111111111111') },
  optimizeDeps: { include: ['react', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client', 'react-router-dom', 'react-hook-form', '@hookform/resolvers/zod', 'browser-image-compression', '@tanstack/react-query', 'zod', 'zustand', 'ky', 'phaser'] },
  server: { host: '127.0.0.1', port: Number(process.env.PI10_PORT), strictPort: true,
    fs: { allow: [resolve(frontend, '..'), '/Users/lhs/Desktop/mcr/frontend', process.env.PI10_OVERLAY] } }, logLevel: 'warn' })
const report = { result: 'FAIL', kind: 'COMPOSITE_SOURCE_FIXTURE',
  shared: '77cfe94af06cac6b292ad7528352e4750e929e71', sharedReadset: 'eb80d722f513ce794fe35317a87702dd2878321a2d76f313d36265b3d593d5ef',
  retainedCore: { source: '0cd18bf66f3cf0d981af26daefe508f0702dbd92', reportSHA: createHash('sha256').update(priorBytes).digest('hex'), captures: 34, states: prior.states },
  limits: 'Actual index.html/main.tsx/default ports with explicit unavailable HTTP fixtures and exact PI01 source. Desktop Chromium only; real Auth/World, live backend, physical devices and release remain OPEN/NOT_RUN.',
  entries: [], headers: [], screenshots: [], errors: [], console: [], failedRequests: [] }
const headers = []
let browser, page, stage = 'server'
async function requireFonts() {
  const fonts = await page.evaluate(async () => {
    const requests = ['400 18px "Parade Ink Text"', '700 26px "Parade Ink Display"']
    const loaded = await Promise.all(requests.map(request => document.fonts.load(request)))
    await document.fonts.ready
    return requests.map((request, index) => ({ request, loaded: loaded[index].length, ready: document.fonts.check(request) }))
  })
  assert(fonts.every(font => font.loaded > 0 && font.ready), JSON.stringify(fonts))
  return fonts
}
async function header(name, width, selector) {
  const bounds = await page.locator(selector).evaluate(e => e.getBoundingClientRect().toJSON())
  assert.equal(bounds.height, 48)
  report.headers.push({ name, width, bounds })
  headers.push({ name, width, bytes: await page.locator(selector).screenshot() })
}
try {
  await server.listen()
  const base = `http://127.0.0.1:${process.env.PI10_PORT}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    stage = `actual-default-entry:${width}`
    page = await browser.newPage({ viewport: { width, height: 844 }, reducedMotion: 'reduce' })
    page.on('pageerror', error => report.errors.push(error.message))
    page.on('console', message => { if (message.type() === 'error' && report.console.length < 20) report.console.push(message.text().slice(0, 500)) })
    page.on('requestfailed', request => { if (report.failedRequests.length < 20) report.failedRequests.push({ url: request.url(), reason: request.failure()?.errorText }) })
    // Exact endpoint prefix: /src/shared/api/* modules must load normally.
    await page.route(`${base}/api/**`, route => route.fulfill({ status: 503, contentType: 'application/json', body: '{}' }))
    await page.goto(base)
    await expect(page.getByRole('heading', { name: 'One world. Every era.', exact: true })).toBeVisible()
    const fonts = await requireFonts()
    await expect(page.getByRole('link', { name: 'Sign in to enter', exact: true })).toBeVisible()
    await expect(page.locator('.public-poster')).toHaveCSS('background-image', 'none')
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    assert.equal(await page.locator('canvas,[data-world-host]').count(), 0)
    await header('landing', width, '.public-header')
    if (width === 390) {
      const name = 'actual-default-entry-390.png'
      await page.screenshot({ path: resolve(output, name), fullPage: true }); report.screenshots.push(name)
    }
    await page.getByRole('link', { name: 'Sign in to enter', exact: true }).click()
    await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible()
    await requireFonts()
    await expect(page.locator('.concert-window')).toHaveCSS('background-image', 'none')
    assert.equal(await page.locator('canvas,[data-world-host]').count(), 0)
    await header('sign-in', width, '.concert-window__titlebar')
    report.entries.push({ width, actualIndex: true, defaultPorts: true, signInNavigation: true, noWorld: true, fonts })
    await page.close(); page = undefined
  }
  stage = 'fixed-reference-comparison'
  const reference = await readFile(resolve(frontend, '../docs/mcr/design/parade-ink-2026-11-05/parade-ink.jpg'))
  assert.equal(createHash('sha256').update(reference).digest('hex'), '9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27')
  assert.equal(headers.length, 6)
  report.comparisons = await compareHeaders(browser, reference, headers, output)
  report.screenshots.push('header-comparison.png')
  assert.deepEqual(report.errors, [])
  report.result = 'PASS'
} catch (error) {
  report.failure = { stage, message: String(error), stack: error.stack }
  if (page && !page.isClosed()) report.diagnostic = { url: page.url(), text: (await page.locator('body').innerText().catch(() => '')).slice(0, 2500) }
  throw error
} finally {
  await browser?.close(); await server.close()
  await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n')
}
