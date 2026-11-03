// AR04 original source-art review. No product map/avatar manifests or release pins.
const { createHash } = require('node:crypto')
const { readFileSync, writeFileSync } = require('node:fs')
const path = require('node:path')
const root = path.resolve(__dirname, '../../../../..')
const { chromium } = require(process.env.MCR_PLAYWRIGHT_MODULE || path.join(root, 'frontend/node_modules/@playwright/test'))
const origin = process.env.MCR_ART_PREVIEW_ORIGIN || 'http://127.0.0.1:41864'
const inputs = JSON.parse(readFileSync(path.join(root, 'frontend/public/world/themes/binding-inputs.json')))
const hash = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`
function assert(condition, message) { if (!condition) throw new Error(message) }

;(async () => {
  const browser = await chromium.launch({ channel: 'chrome', headless: true })
  const results = [], resourceChecks = []
  try {
    for (const width of [360, 390, 430]) {
      for (const entry of inputs.themes) {
        const theme = entry.theme_id
        const page = await browser.newPage({ viewport: { width, height: 844 }, deviceScaleFactor: 1 })
        const errors = []
        page.on('pageerror', error => errors.push(error.message))
        await page.goto(`${origin}/docs/mcr/art/evidence/themes/preview.html?theme=${theme}`)
        await page.waitForFunction(() => window.review?.ready)
        const initial = await page.evaluate(() => JSON.stringify([window.review.terrain, window.review.decorations]))
        assert(await page.title() === 'AR04 original world textures · source review', 'preview context')
        for (const target of ['REVENGE', 'PARADE', 'DANGER', theme]) {
          await page.getByRole('button', { name: target, exact: true }).click()
          await page.waitForFunction(t => window.review.theme === t, target)
          assert(initial === await page.evaluate(() => JSON.stringify([window.review.terrain, window.review.decorations])), 'art-board geometry changed across themes')
        }
        const metrics = await page.evaluate(() => ({
          width: innerWidth, scrollWidth: document.documentElement.scrollWidth,
          canvasWidth: document.querySelector('#world').width,
          decoded: window.review.decoded,
          buttonBounds: [...document.querySelectorAll('button')].map(b => ({ width: b.getBoundingClientRect().width, height: b.getBoundingClientRect().height })),
          smoothing: document.querySelector('#world').getContext('2d').imageSmoothingEnabled,
        }))
        assert(errors.length === 0, errors.join('\n'))
        assert(metrics.width === width && metrics.scrollWidth === width, 'horizontal page clipping')
        assert(metrics.decoded.join(',') === '256,128', 'browser PNG dimensions')
        assert(metrics.canvasWidth === width-32 && metrics.smoothing === false, '1:1 pixel viewport')
        assert(metrics.buttonBounds.every(b => b.width >= 44 && b.height >= 44), 'touch target <44px')
        const screenshot = `${theme.toLowerCase()}-${width}.png`
        await page.screenshot({ path: path.join(__dirname, screenshot), fullPage: true })
        results.push({ theme, width, height: 844, deviceScaleFactor: 1, ...metrics, screenshot,
          screenshot_sha256: hash(readFileSync(path.join(__dirname, screenshot))),
          theme_switch_same_art_board: 'PASS', browser_page_errors: errors })
        if (width === 360) {
          const response = await page.request.get(`${origin}/frontend/public/${entry.texture.path}`)
          const bytes = await response.body()
          assert(response.ok() && response.headers()['content-type'] === 'image/png', 'resource MIME/status')
          assert(hash(bytes) === entry.texture.sha256 && bytes.length < 16384, 'served PNG byte identity/size')
          resourceChecks.push({ path: entry.texture.path, status: response.status(),
            mime: response.headers()['content-type'], sha256: hash(bytes), bytes: bytes.length })
        }
        await page.close()
      }
    }
    const report = { scope: 'Native desktop Chrome source-art preview at mobile CSS widths; no device emulation or product acceptance',
      result: 'PASS', browser: `Chrome ${browser.version()}`, playwright: require(path.join(process.env.MCR_PLAYWRIGHT_MODULE || path.join(root, 'frontend/node_modules/@playwright/test'), 'package.json')).version,
      resourceChecks, previews: results,
      selected_avatar_compatibility: 'NOT_RUN', production_map: 'NOT_RUN', real_ios_safari_android_chrome: 'NOT_RUN', qa06_in01: 'NOT_RUN' }
    writeFileSync(path.join(__dirname, 'browser-validation.json'), `${JSON.stringify(report, null, 2)}\n`)
    console.log(`PASS: ${results.length} native Chrome previews, theme switches preserve art-board positions; 3 exact PNG resources`)
  } finally { await browser.close() }
})().catch(error => { console.error(error); process.exitCode = 1 })
