// Actual retained Gifts component fixture. No production-auth or inventory claim.
import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { chromium } from '@playwright/test'
import { serve } from './serve.mjs'

const env = await serve()
const out = path.join(env.packet, process.env.MCR_ART_ATTEMPT ?? 'native-01')
await mkdir(out, { recursive: true })
let browser
const report = { usage: 'TEST_ONLY_REFERENCE_FIXTURE', sourceCommit: env.sourceCommit,
  productionBinding: null, actualConsumer: 'GiftDiscovery -> ReferenceGiftRow',
  sourceFormatChecksSeparate: true, fullTypeCheck: 'NOT_RUN', result: 'IN_PROGRESS', scenarios: [], errors: [] }
try {
  if (!process.env.MCR_ART_BROWSER) throw new Error('Explicit existing retained browser executable is required; no download')
  browser = await chromium.launch({ headless: true, executablePath: process.env.MCR_ART_BROWSER })
  report.browserVersion = browser.version()
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, deviceScaleFactor: 1 })
    page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(`${env.baseUrl}/native.html`, { waitUntil: 'networkidle' })
    assert.deepEqual(report.errors, [], 'Fixture must load without runtime/import errors')
    await page.waitForFunction(() => document.querySelectorAll('img[data-art-fixture]').length === 3
      && [...document.querySelectorAll('img[data-art-fixture]')].every(img => img.complete && img.naturalWidth === 256))
    const metrics = await page.evaluate(() => ({
      viewport: innerWidth, scrollWidth: document.documentElement.scrollWidth,
      usage: window.referenceGiftsArt.usage, productionBinding: window.referenceGiftsArt.productionBinding,
      images: [...document.querySelectorAll('img[data-art-fixture]')].map(img => {
        const r = img.getBoundingClientRect(), frame = img.closest('.gift-reference-image').getBoundingClientRect()
        return { file: img.dataset.artFixture, naturalWidth: img.naturalWidth, naturalHeight: img.naturalHeight,
          fit: getComputedStyle(img).objectFit, image: { x: r.x, y: r.y, width: r.width, height: r.height },
          frame: { x: frame.x, y: frame.y, width: frame.width, height: frame.height } }
      }),
      fixture: window.referenceGiftsArt.state(),
    }))
    assert.equal(metrics.usage, 'TEST_ONLY_REFERENCE_FIXTURE'); assert.equal(metrics.productionBinding, null)
    assert.equal(metrics.scrollWidth, width); assert.equal(metrics.images.length, 3)
    for (const image of metrics.images) {
      assert.equal(image.fit, 'contain'); assert.equal(image.naturalWidth, 256); assert.equal(image.naturalHeight, 256)
      assert.ok(Math.abs(image.frame.width - image.frame.height) < 1)
      assert.ok(image.image.x >= 0 && image.image.x + image.image.width <= width)
      assert.ok(image.image.width <= image.frame.width && image.image.height <= image.frame.height)
    }
    await page.screenshot({ path: path.join(out, `gifts-${width}.png`), fullPage: true })
    report.scenarios.push({ name: `actual-list-${width}`, result: 'PASS', ...metrics })
    if (width === 390) {
      await page.keyboard.press('Tab')
      const focus = await page.evaluate(() => ({ tag: document.activeElement?.tagName,
        outlineStyle: getComputedStyle(document.activeElement).outlineStyle }))
      report.scenarios.push({ name: 'native-keyboard-focus-observation', ...focus })
      await page.evaluate(() => window.referenceGiftsArt.invalidate())
      await page.waitForFunction(() => document.querySelectorAll('img[data-art-fixture]').length === 0)
      await page.screenshot({ path: path.join(out, 'context-withdrawn-390.png'), fullPage: true })
      report.scenarios.push({ name: 'fixture-context-withdrawal', result: 'PASS', imagesRemaining: 0 })
    }
    await page.evaluate(() => window.referenceGiftsArt.dispose())
    await page.close()
  }
  const specimen = await browser.newPage({ viewport: { width: 900, height: 844 }, deviceScaleFactor: 1 })
  specimen.on('pageerror', error => report.errors.push(error.message))
  await specimen.goto(`${env.baseUrl}/preview.html`, { waitUntil: 'networkidle' })
  await specimen.waitForFunction(() => document.images.length === 6 && [...document.images].every(image => image.complete && image.naturalWidth === 256))
  await specimen.screenshot({ path: path.join(out, 'source-specimen.png'), fullPage: true })
  await specimen.close()
  assert.deepEqual(report.errors, [])
  report.result = 'PASS'
} catch (error) {
  report.result = 'FAIL'
  report.errors.push(error.message)
  throw error
} finally {
  report.consumerInputsRead = [...env.readPaths].sort()
  await browser?.close()
  await env.server.close()
  await writeFile(path.join(out, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(JSON.stringify({ result: report.result, scenarios: report.scenarios.length,
  report: path.relative(env.packet, path.join(out, 'report.json')), productionBinding: null }))
