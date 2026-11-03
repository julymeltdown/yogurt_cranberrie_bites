import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { createArtReviewServer } from './serve.mjs'

const root=fileURLToPath(new URL('../../../../../',import.meta.url))
const readJson=async path=>JSON.parse(await readFile(`${root}${path}`,'utf8'))
const hash=bytes=>createHash('sha256').update(bytes).digest('hex')
const metadata=await readJson('frontend/src/entities/avatar/derived-assets/hair-layout.json')
const catalog=await readJson('frontend/src/entities/avatar/character-source-pack.json')
assert.equal(metadata.production_eligible,false);assert.equal(metadata.wire_binding,null)
assert.equal(metadata.source_pack_id,catalog.source_pack_id);assert.equal(metadata.source_pack_revision,catalog.revision)
assert.deepEqual(metadata.sources.map(source=>source.source_id),['photo_04','photo_05'])
const measurements=[]
for(const source of metadata.sources) {
  const original=catalog.files.find(entry=>entry.source_id===source.source_id)
  const bytes=await readFile(`${root}frontend/public/world/avatars/supplied-v1/${original.file}`)
  assert.equal(hash(bytes),original.sha256);assert.equal(source.source_sha256,original.sha256);assert.equal(bytes.length,original.byte_length)
  assert.equal(source.shapes.length,16)
  assert.equal(new Set(source.shapes.map(shape=>shape.source_location)).size,16)
  for(const shape of source.shapes) {
    const {rect,estimated_attachment:a,measured_dark_outline:b}=shape
    assert(Number.isInteger(rect.x)&&Number.isInteger(rect.y)&&Number.isInteger(rect.width)&&Number.isInteger(rect.height))
    assert(rect.x>=0&&rect.y>=0&&rect.x+rect.width<=1280&&rect.y+rect.height<=960)
    assert(b.min_x>rect.x&&b.min_y>rect.y&&b.max_x<rect.x+rect.width-1&&b.max_y<rect.y+rect.height-1)
    assert(a.x>rect.x&&a.x<rect.x+rect.width&&a.y>rect.y&&a.y<rect.y+rect.height)
    assert.equal(a.status,'NOT_VERIFIED');assert.equal(shape.paired_source_location,null);assert.equal(shape.composition_order,null);assert.equal(shape.wire_item_id,null)
    assert.equal(shape.transparent_cutout,'NOT_PREPARED');assert.equal(shape.direction_frames,'UNKNOWN');assert.equal(shape.walk_frames,'NOT_PROVIDED')
  }
  measurements.push({sourceId:source.source_id,sha256:source.source_sha256,regions:16,
    extendsBeyondRegularGrid:source.shapes.filter(shape=>{
      const b=shape.measured_dark_outline,col=shape.source_column-1,row=shape.source_row-1
      return b.min_x<col*320||b.max_x>=(col+1)*320||b.min_y<row*240||b.max_y>=(row+1)*240
    }).map(shape=>shape.source_location),
    bodyFit:'NOT_VERIFIED',pairing:'UNKNOWN',alpha:'NOT_PREPARED',directionAnimation:'NOT_PROVIDED'})
}
assert(process.env.MCR_ART_PLAYWRIGHT,'Set MCR_ART_PLAYWRIGHT to installed @playwright/test/index.mjs')
const {chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href)
const server=createArtReviewServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve))
let browser
try {
  browser=await chromium.launch({executablePath:process.env.MCR_ART_CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
  const errors=[],results=[]
  for(const width of [360,390,430]) {
    const context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce'}),page=await context.newPage()
    page.on('pageerror',error=>errors.push(error.message))
    await page.goto(`http://127.0.0.1:${server.address().port}/src/entities/avatar/derived-assets/hair-review.html`)
    await page.waitForFunction(()=>window.__MCR_HAIR_REVIEW__?.shapes===32)
    assert.equal(await page.locator('article').count(),32);assert.equal(await page.locator('article img').count(),64)
    const imageIdentity=await page.locator('article img').evaluateAll(images=>images.map(image=>({src:image.src,style:image.style.cssText})))
    for(const source of ['photo_04','photo_05']) {
      const label=source==='photo_04'?'Front hair · 16':'Back hair · 16'
      await page.getByRole('button',{name:label,exact:true}).click()
      assert.equal(await page.locator('article:visible').count(),16)
      assert.equal(await page.locator(`section[data-sheet="${source}"]`).isVisible(),true)
      assert.equal(await page.locator(`button[data-source="${source}"]`).getAttribute('aria-pressed'),'true')
      for(const theme of ['PARADE','DANGER','REVENGE']) {
        await page.getByRole('button',{name:theme,exact:true}).click()
        assert.deepEqual(await page.locator('article img').evaluateAll(images=>images.map(image=>({src:image.src,style:image.style.cssText}))),imageIdentity)
      }
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'Horizontal overflow')
      assert(await page.locator('article img').evaluateAll(images=>images.every(image=>image.complete&&image.naturalWidth===1280&&image.naturalHeight===960)))
      await page.screenshot({path:`${root}docs/mcr/art/evidence/hair-${source}-${width}.png`,fullPage:true})
      // Tighter captures preserve inspectable detail without resizing/editing any raster.
      if(width===390) for(const location of ['row1_col1','row4_col4']) await page.locator(`section[data-sheet="${source}"] article[data-location="${location}"]`).screenshot({path:`${root}docs/mcr/art/evidence/hair-${source}-${location}-390.png`})
      results.push({width,sourceId:source,visibleShapes:16,decodedOriginalViews:32,overflow:false,themePixelsAndGeometryUnchanged:true})
    }
    // Returning to the first sheet reuses its original images and estimates, without assigning a pair.
    await page.getByRole('button',{name:'Front hair · 16',exact:true}).click()
    assert.equal(await page.locator('section[data-sheet="photo_04"] article:visible').count(),16)
    await context.close()
  }
  assert.deepEqual(errors,[])
  const report={status:'IN_PROGRESS',task:'AR03',sourceBase:'f0e44bd1bba943b02319f9d9a42433e253e8de6f',
    metadataSha256:hash(await readFile(`${root}frontend/src/entities/avatar/derived-assets/hair-layout.json`)),measurements,
    browser:{version:await browser.version(),results,pageErrors:errors},
    imageEditing:'NONE',transparentDerivatives:'NOT_RUN',productionCompositing:'NOT_RUN',actualMobileDevices:'NOT_RUN',directionWalkAcceptance:'NOT_RUN'}
  await writeFile(`${root}docs/mcr/art/evidence/hair-registration.json`,`${JSON.stringify(report,null,2)}\n`)
  console.log(JSON.stringify(report,null,2))
} finally {await browser?.close();await new Promise(resolve=>server.close(resolve))}
