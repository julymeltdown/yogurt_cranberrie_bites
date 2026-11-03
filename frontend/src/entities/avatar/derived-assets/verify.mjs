import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { createArtReviewServer } from './serve.mjs'

const root = fileURLToPath(new URL('../../../../../', import.meta.url))
const readJson = async path => JSON.parse(await readFile(`${root}${path}`, 'utf8'))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const catalog = await readJson('frontend/src/entities/avatar/character-source-pack.json')
const layout = await readJson('frontend/src/entities/avatar/derived-assets/source-layout.json')
const sourceChecks = []
for (const source of catalog.files) {
  const bytes = await readFile(`${root}frontend/public/world/avatars/supplied-v1/${source.file}`)
  assert.equal(hash(bytes), source.sha256, `Original changed: ${source.source_id}`)
  assert.equal(bytes.length, source.byte_length)
  sourceChecks.push({ sourceId: source.source_id, sha256: source.sha256, bytes: bytes.length, status: 'PASS' })
}
assert.equal(sourceChecks.length, 20)
assert.equal(layout.production_eligible, false)
assert.equal(layout.wire_binding, null)
assert.equal(layout.inventory_ids, null)
assert.equal(layout.regions.length, 6)
assert.deepEqual(layout.transparent_derivatives, [])
assert.equal(layout.source_sha256, sourceChecks.find(s => s.sourceId === 'photo_01').sha256)
for (const { rect, foot } of layout.regions) {
  assert(rect.x >= 0 && rect.y >= 80 && rect.x + rect.width <= 1280 && rect.y + rect.height <= 853)
  assert(foot.x > rect.x && foot.x < rect.x + rect.width && foot.y > rect.y && foot.y < rect.y + rect.height)
}
const rejectedPath = 'docs/mcr/art/evidence/rejected/imagegen-photo01-r1.png'
const rejected = await readFile(`${root}${rejectedPath}`)
assert(rejected.subarray(0, 8).equals(Buffer.from([137,80,78,71,13,10,26,10])))
const report = { status: 'IN_PROGRESS', task: 'AR03', sourceBase: 'f0e44bd1bba943b02319f9d9a42433e253e8de6f',
  sourcePack: { id: catalog.source_pack_id, revision: catalog.revision }, sourceChecks,
  rejectedImage: { path: rejectedPath, sha256: hash(rejected), bytes: rejected.length,
    width: rejected.readUInt32BE(16), height: rejected.readUInt32BE(20), pngColorType: rejected[25], productEligible: false,
    reason: 'Imagegen redrew source pixel contours and interior layout, including fringe/eye arrangement and collar. Extraction identity fidelity rejected.' },
  runtimeC06: 'NOT_RUN', transparentDerivatives: 'NOT_RUN', directionalWalkAtlas: 'NOT_RUN', productionAcceptance: 'NOT_RUN' }

// Reuse the installed locked Playwright only for evidence; no dependency/lockfile mutation.
const playwright = process.env.MCR_ART_PLAYWRIGHT
if (playwright) {
  const { chromium } = await import(pathToFileURL(playwright).href)
  const server = createArtReviewServer(); await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  let browser
  try {
    browser = await chromium.launch({ executablePath: process.env.MCR_ART_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
    const errors = [], results = []
    for (const width of [360, 390, 430]) {
      const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' })
      const page = await context.newPage(); page.on('pageerror', error => errors.push(error.message))
      await page.goto(`http://127.0.0.1:${server.address().port}/src/entities/avatar/derived-assets/review.html`)
      await page.waitForFunction(() => window.__MCR_ART_REVIEW__?.regions === 6)
      assert.equal(await page.locator('article').count(),6)
      assert.equal(await page.locator('article img').count(),18)
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Horizontal overflow')
      const pixels = await page.locator('article img').evaluateAll(images => images.map(img => ({ src: img.src, style: img.style.cssText })))
      for (const theme of ['PARADE','DANGER','REVENGE']) {
        await page.getByRole('button', { name: theme, exact: true }).click()
        assert.equal(await page.locator(`[data-theme="${theme}"]`).getAttribute('aria-pressed'), 'true')
        assert.deepEqual(await page.locator('article img').evaluateAll(images => images.map(img => ({ src: img.src, style: img.style.cssText }))),pixels)
      }
      await page.screenshot({ path: `${root}docs/mcr/art/evidence/source-registration-${width}.png`, fullPage: true })
      results.push({ width, originalRegionCount: 6, sourceViews: 18, overflow: false, themeImageAndGeometryUnchanged: true })
      if (width === 390) {
        report.sourceRegistrationAnalysis = await page.evaluate(async regions => {
          // Read-only source measurements, deliberately not an alpha mask or extracted bitmap.
          const source = new Image(); source.src = '/world/avatars/supplied-v1/photo-01.jpg'; await source.decode()
          const canvas = document.createElement('canvas'); canvas.width = source.naturalWidth; canvas.height = source.naturalHeight
          const ctx = canvas.getContext('2d'); ctx.drawImage(source,0,0)
          const rgba = ctx.getImageData(0,0,canvas.width,canvas.height).data
          const threshold = 112
          const dark = (x,y) => { const i = (y*canvas.width+x)*4; return Math.max(rgba[i],rgba[i+1],rgba[i+2]) <= threshold }
          return { method: 'Maximum RGB channel <=112, inside reviewed source windows. Measures dark outlines only; not an alpha silhouette or skin mask.',
            regions: regions.map(({ source_location, rect, foot }) => {
              let minX=1280,minY=853,maxX=-1,maxY=-1,darkPixels=0
              for(let y=rect.y;y<rect.y+rect.height;y++) for(let x=rect.x;x<rect.x+rect.width;x++) if(dark(x,y)) {
                minX=Math.min(minX,x); minY=Math.min(minY,y); maxX=Math.max(maxX,x); maxY=Math.max(maxY,y); darkPixels++
              }
              let shoeMinX=1280,shoeMaxX=-1
              for(let y=maxY-7;y<=maxY;y++) for(let x=minX;x<=maxX;x++) if(dark(x,y)) { shoeMinX=Math.min(shoeMinX,x); shoeMaxX=Math.max(shoeMaxX,x) }
              const estimatedShoeFoot={ x:(shoeMinX+shoeMaxX+1)/2,y:maxY+1 }
              return { source_location, darkPixels, darkOutlineBounds:{minX,minY,maxX,maxY}, bottomEightRows:{minX:shoeMinX,maxX:shoeMaxX},
                estimatedShoeFoot, descriptorFoot:foot,
                descriptorDelta:{x:foot.x-estimatedShoeFoot.x,y:foot.y-estimatedShoeFoot.y},
                darkOutlineTouchesReviewBoundary:minX===rect.x||minY===rect.y||maxX===rect.x+rect.width-1||maxY===rect.y+rect.height-1 }
            }) }
        },layout.regions)
        assert(report.sourceRegistrationAnalysis.regions.every(region => region.darkPixels > 1000 && !region.darkOutlineTouchesReviewBoundary), 'Source outline clipped by review window')
        report.rejectedImage.pixelInspection = await page.evaluate(async () => {
          const image = new Image(); image.src = '/evidence/rejected.png'; await image.decode()
          const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight
          const ctx = canvas.getContext('2d'); ctx.drawImage(image,0,0)
          const rgba = ctx.getImageData(0,0,canvas.width,canvas.height).data
          let transparent = 0, semitransparent = 0, opaqueWhite = 0, minX = canvas.width, minY = canvas.height, maxX = -1, maxY = -1
          for(let i = 0; i < rgba.length; i += 4) {
            const alpha = rgba[i+3], pixel = i/4, x = pixel%canvas.width, y = Math.floor(pixel/canvas.width)
            if(alpha === 0) transparent++; else { if(alpha < 255) semitransparent++; minX=Math.min(minX,x); minY=Math.min(minY,y); maxX=Math.max(maxX,x); maxY=Math.max(maxY,y) }
            if(alpha === 255 && rgba[i] >= 235 && rgba[i+1] >= 235 && rgba[i+2] >= 235) opaqueWhite++
          }
          return { transparent, semitransparent, opaqueWhite, alphaBounds: { minX,minY,maxX,maxY }, note: 'Read-only decode of rejected candidate; alpha presence does not establish identity fidelity.' }
        })
      }
      await context.close()
    }
    assert.deepEqual(errors,[])
    report.browser = { version: await browser.version(), results, pageErrors: errors, actualMobileDevice: 'NOT_RUN' }
  } finally { await browser?.close(); await new Promise(resolve => server.close(resolve)) }
} else report.browser = { status: 'NOT_RUN', reason: 'Set MCR_ART_PLAYWRIGHT to an installed @playwright/test/index.mjs to run native Chrome evidence.' }
await mkdir(`${root}docs/mcr/art/evidence`, { recursive: true })
await writeFile(`${root}docs/mcr/art/evidence/source-registration.json`,`${JSON.stringify(report,null,2)}\n`)
console.log(JSON.stringify(report,null,2))
