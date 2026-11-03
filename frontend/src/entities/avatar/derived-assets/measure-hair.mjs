import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { createArtReviewServer } from './serve.mjs'

const root = fileURLToPath(new URL('../../../../../', import.meta.url))
const catalog = JSON.parse(await readFile(`${root}frontend/src/entities/avatar/character-source-pack.json`, 'utf8'))
assert(process.env.MCR_ART_PLAYWRIGHT, 'Set MCR_ART_PLAYWRIGHT to installed @playwright/test/index.mjs')
const { chromium } = await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href)
const server = createArtReviewServer(); await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
let browser
try {
  browser = await chromium.launch({ executablePath: process.env.MCR_ART_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const page = await browser.newPage()
  await page.goto(`http://127.0.0.1:${server.address().port}/src/entities/avatar/derived-assets/review.html`)
  const sources = []
  for (const sourceId of ['photo_04', 'photo_05']) {
    const source = catalog.files.find(entry => entry.source_id === sourceId)
    const bytes = await readFile(`${root}frontend/public/world/avatars/supplied-v1/${source.file}`)
    assert.equal(createHash('sha256').update(bytes).digest('hex'), source.sha256)
    const path = `/world/avatars/supplied-v1/${source.file}`
    const components = await page.evaluate(async path => {
      const image = new Image(); image.src = path; await image.decode()
      const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight
      if (canvas.width !== 1280 || canvas.height !== 960) throw Error('Unexpected original dimensions')
      const ctx = canvas.getContext('2d'); ctx.drawImage(image,0,0)
      const pixels = ctx.getImageData(0,0,1280,960).data, visited = new Uint8Array(1280*960), found = []
      const dark = index => Math.max(pixels[index*4],pixels[index*4+1],pixels[index*4+2]) <= 112
      // A temporary visited array reads connectivity; no pixel is edited or exported.
      for(let index=0;index<visited.length;index++) {
        if(visited[index] || !dark(index)) continue
        const queue=[index]; visited[index]=1; let minX=1280,minY=960,maxX=-1,maxY=-1
        for(let cursor=0;cursor<queue.length;cursor++) {
          const pixel=queue[cursor], x=pixel%1280, y=Math.floor(pixel/1280)
          minX=Math.min(minX,x); minY=Math.min(minY,y); maxX=Math.max(maxX,x); maxY=Math.max(maxY,y)
          for(const neighbor of [x>0?pixel-1:-1,x<1279?pixel+1:-1,y>0?pixel-1280:-1,y<959?pixel+1280:-1]) {
            if(neighbor>=0 && !visited[neighbor] && dark(neighbor)) { visited[neighbor]=1; queue.push(neighbor) }
          }
        }
        if(queue.length>=5000) found.push({ dark_pixels:queue.length,min_x:minX,min_y:minY,max_x:maxX,max_y:maxY })
      }
      return found
    },path)
    assert.equal(components.length,16, `Expected sixteen principal outlines on ${sourceId}`)
    const shapes = components.map(bounds => {
      const row=Math.floor(((bounds.min_y+bounds.max_y)/2)/240)+1, column=Math.floor(((bounds.min_x+bounds.max_x)/2)/320)+1
      const x=Math.max(0,bounds.min_x-8), y=Math.max(0,bounds.min_y-8)
      return { source_location:`row${row}_col${column}`, source_row:row, source_column:column,
        rect:{ x,y,width:Math.min(1280,bounds.max_x+9)-x,height:Math.min(960,bounds.max_y+9)-y },
        measured_dark_outline:bounds,
        estimated_attachment:{ kind:'cranium_center',x:(bounds.min_x+bounds.max_x+1)/2,y:Math.round(bounds.min_y+(bounds.max_y-bounds.min_y+1)*0.45),
          status:'NOT_VERIFIED',basis:'Geometric centerline at 45% of principal dark-outline height; not fitted to any body, face, skull or C06 cell.' },
        transparent_cutout:'NOT_PREPARED',layer_alignment:'NOT_VERIFIED',direction_frames:'UNKNOWN',walk_frames:'NOT_PROVIDED',
        paired_source_location:null,composition_order:null,wire_item_id:null }
    }).sort((a,b) => a.source_row-b.source_row || a.source_column-b.source_column)
    assert.equal(new Set(shapes.map(shape => shape.source_location)).size,16)
    sources.push({ source_id:sourceId,source_path:path,source_sha256:source.sha256,width_px:1280,height_px:960,
      supplied_view:sourceId==='photo_04'?'front_hair_reference':'back_hair_reference',
      notes:sourceId==='photo_04'?'White face openings and pale hair panels are opaque JPEG pixels; transparency intent needs separate review. Accessories can be baked into hair.':'Back-view hair references do not establish a rear full character, animation, back composition layer or pairing with any front shape.',shapes })
  }
  const result={ kind:'selected_source_hair_registration_review',revision:1,status:'IN_PROGRESS',production_eligible:false,
    source_pack_id:catalog.source_pack_id,source_pack_revision:catalog.revision,
    method:{ dark_outline:'Four-neighbor component of maximum RGB channel <=112; principal components >=5000 pixels; read-only JPEG decode.',
      review_window:'Principal outline bounds plus eight original pixels of margin. Not an alpha silhouette or extracted sprite rectangle.',
      pairing:'All front/back mappings explicitly unknown; source positions do not imply matching designs.',
      image_editing:'NONE. No source pixels or raster outputs are written.',inventory:'No product IDs reserved or accepted.',skin:'No recoloring; retain supplied pale panels, highlights and ornaments.' },
    preview_cell:{width_px:128,height_px:128},preview_attachment:{x:64,y:64},
    sources,wire_binding:null,limitations:['Attachment estimates are not approved composition anchors.','No alpha, layer separation, body fit or directional animation established.','No C06 schema/catalog change, production renderer or product fallback.'] }
  await writeFile(`${root}frontend/src/entities/avatar/derived-assets/hair-layout.json`,`${JSON.stringify(result,null,2)}\n`)
  console.log(JSON.stringify({sources:sources.map(source=>({sourceId:source.source_id,shapes:source.shapes.length,sha256:source.source_sha256})),imageEditing:'NONE'},null,2))
} finally { await browser?.close(); await new Promise(resolve => server.close(resolve)) }
