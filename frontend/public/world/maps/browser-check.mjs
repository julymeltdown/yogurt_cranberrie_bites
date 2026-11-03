// Native installed Chrome, original AR02 geometry and actual AR04 PNG resources.
import { createRequire } from 'node:module'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve, dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, '../../../..')
const require = createRequire(resolve(process.env.MCR_NODE_MODULE_ROOT ?? '/Users/lhs/Desktop/mcr/frontend/node_modules', '../package.json'))
const { chromium } = require('@playwright/test')
const evidence = join(root, 'docs/mcr/art/evidence/maps')
const themeRoot = resolve(process.env.MCR_AR04_THEME_ROOT ?? join(root, 'frontend/public/world/themes'))
const hashes = {}
for (const name of ['revenge', 'parade', 'danger']) hashes[name] = 'sha256:' + createHash('sha256').update(await readFile(join(themeRoot, `${name}.png`))).digest('hex')
const themeCheckout = process.env.MCR_AR04_CHECKOUT
const themeResourceCommit = process.env.MCR_AR04_RESOURCE_COMMIT ?? (themeCheckout ? execFileSync('git', ['-C', themeCheckout, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() : null)
if (themeCheckout && themeResourceCommit) for (const name of ['revenge', 'parade', 'danger']) {
  const committed = execFileSync('git', ['-C', themeCheckout, 'show', `${themeResourceCommit}:frontend/public/world/themes/${name}.png`])
  assert.equal('sha256:' + createHash('sha256').update(committed).digest('hex'), hashes[name], 'uncommitted theme pixels differ from pinned source commit')
}
const browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
const errors = [], results = []
try {
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, deviceScaleFactor: 1 })
    page.on('pageerror', e => errors.push(e.message))
    await page.goto('http://127.0.0.1:8873/world/maps/review.html')
    await page.waitForFunction(() => window.ar02Review?.ready)
    for (const theme of ['revenge', 'parade', 'danger']) {
      await page.selectOption('#theme', theme)
      await page.waitForFunction(t => window.ar02Review?.ready && window.ar02Review.theme === t, theme)
      for (let i = 0; i < 5; i++) {
        await page.selectOption('#target', String(i))
        await page.waitForFunction(id => window.ar02Review?.ready && window.ar02Review.point === id, ['studio_info', 'park_info', 'stage_info', 'gifts_info', 'wall_info'][i])
        assert.equal(await page.locator('#current').isVisible(), true)
      }
      await page.selectOption('#target', '1')
      await page.waitForFunction(() => window.ar02Review?.ready && window.ar02Review.point === 'park_info')
      const metrics = await page.evaluate(() => {
        const focus = document.querySelector('#focus'), ctx = focus.getContext('2d')
        const colors = new Set(), pixels = ctx.getImageData(0, 0, focus.width, focus.height).data
        for (let i = 0; i < pixels.length; i += 4) if (pixels[i+3]) colors.add(pixels.slice(i, i+4).join(','))
        const selects = [...document.querySelectorAll('select')].map(e => ({ height: e.getBoundingClientRect().height, width: e.getBoundingClientRect().width }))
        return { pageWidth: document.documentElement.scrollWidth, viewportWidth: innerWidth, canvasWidth: focus.width, canvasCssWidth: focus.getBoundingClientRect().width, colors: colors.size,
          fontSize: parseFloat(getComputedStyle(document.querySelector('#current')).fontSize), selects, imageSmoothing: ctx.imageSmoothingEnabled, labels: document.querySelectorAll('#legend li').length }
      })
      assert.equal(metrics.pageWidth, width, 'horizontal overflow')
      assert.equal(metrics.canvasWidth, metrics.canvasCssWidth, 'not1:1 pixel rendering')
      assert.equal(metrics.imageSmoothing, false)
      assert.ok(metrics.colors > 8, 'actual textured map pixels missing')
      assert.ok(metrics.fontSize >= 16 && metrics.selects.every(e => e.height >= 44 && e.width <= width))
      assert.equal(metrics.labels, 5)
      const screenshot = `review-${width}-${theme}.png`
      await page.screenshot({ path: join(evidence, screenshot), fullPage: true })
      results.push({ width, theme, metrics, fiveLandmarkSelections: 'PASS', screenshot })
    }
    await page.close()
  }
  assert.deepEqual(errors, [])
  const report = { status: 'MOCK_READY', scope: 'Original AR02 virtual geometry with actual AR04 texture bytes in native Canvas; asset review only',
    browser: await browser.version(), node: process.version, playwright: require('@playwright/test/package.json').version,
    themeSourceCheckout: themeRoot, themeCheckoutHeadAtCapture: themeCheckout ? execFileSync('git', ['-C', themeCheckout, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() : null,
    themeResourceCommit,
    resourceHashes: hashes, results, pageErrors: errors,
    limits: ['Native Canvas review harness, not application Phaser composition', 'No character artwork or avatar readability claim', 'No actual iOS/Android hardware or QA06 acceptance', 'No real server/multiuser/IN01 acceptance', 'No operational venue map or physical destinations', 'No final release manifest binding'] }
  await writeFile(join(evidence, 'browser-validation.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify({ status: report.status, browser: report.browser, cases: results.length, resourceHashes: hashes, errors }, null, 2))
} finally { await browser.close() }
