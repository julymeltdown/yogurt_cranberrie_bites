import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {readFile,writeFile,mkdir} from 'node:fs/promises'
import {fileURLToPath,pathToFileURL} from 'node:url'
import {createArtReviewServer} from './serve.mjs'
const root=fileURLToPath(new URL('../../../../../',import.meta.url))
const sourceBase='37e0e0369443c3365c94d8dcaef99eaec06a4038'
const reviewedAt=process.env.MCR_ART_REVIEWED_AT
assert(reviewedAt&&/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/.test(reviewedAt),'Set fixed MCR_ART_REVIEWED_AT to the actual contour review time')
const hash=bytes=>createHash('sha256').update(bytes).digest('hex')
const layout=JSON.parse(await readFile(`${root}frontend/src/entities/avatar/derived-assets/source-layout.json`,'utf8'))
assert.equal(hash(await readFile(`${root}frontend/public${layout.source_path}`)),layout.source_sha256)
assert(process.env.MCR_ART_PLAYWRIGHT,'Set MCR_ART_PLAYWRIGHT to an installed @playwright/test/index.mjs')
const {chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href)
const server=createArtReviewServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve))
let browser
try {
  browser=await chromium.launch({executablePath:process.env.MCR_ART_CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
  const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/src/entities/avatar/derived-assets/review.html`)
  const assets=await page.evaluate(async layout=>{const {buildPresetPixels}=await import('./mask-presets.mjs');return buildPresetPixels(layout)},layout)
  const dir='frontend/public/world/avatars/derived-v1',url='/world/avatars/derived-v1/'
  await mkdir(`${root}${dir}`,{recursive:true})
  const save=async (prefix,base64,width,height)=>{
    const bytes=Buffer.from(base64,'base64'),sha256=hash(bytes),file=`${prefix}-${sha256.slice(0,16)}.png`
    await writeFile(`${root}${dir}/${file}`,bytes)
    return {path:`${url}${file}`,sha256,byte_length:bytes.length,width_px:width,height_px:height}
  }
  const texture=await save('photo01-front-idle-64',assets.atlas64,384,64)
  const detailTexture=await save('photo01-front-idle-256',assets.atlas256,1536,256)
  const provenance=[]
  for(const frame of assets.frames) {
    assert.equal(frame.evidence.retainedRgbMismatches,0);assert.equal(frame.evidence.registeredRgbMismatches,0)
    assert.equal(frame.evidence.removedDarkPixels,0,`Lost source outline pixels: ${frame.sourceLocation}`)
    assert(frame.evidence.discardedDetachedComponents.every(size=>size<64),`Detached source feature requires review: ${frame.sourceLocation}`)
    const native=await save(`photo01-${frame.sourceLocation}`,frame.native.png,frame.native.width,frame.native.height)
    provenance.push({...frame,native:{...native,sourceOrigin:frame.native.sourceOrigin,foot:frame.native.foot}})
  }
  const descriptor={format:'mcr-selected-static-v1',source_pack_id:layout.source_pack_id,source_pack_revision:layout.source_pack_revision,pose:'front_idle_only',
    reviewed_by:'Codex AR03 selected-source asset owner',reviewed_at:reviewedAt,
    atlas:{path:texture.path.slice(1),sha256:`sha256:${texture.sha256}`,width_px:384,height_px:64},
    characters:assets.frames.map((frame,index)=>({id:`photo01-front-${String(index+1).padStart(2,'0')}`,label:frame.label,source_id:layout.source_id,source_sha256:layout.source_sha256,
      anchor:{x:32,y:60},frame:{x:index*64,y:0,width_px:64,height_px:64}}))}
  const descriptorBytes=Buffer.from(`${JSON.stringify(descriptor)}\n`),descriptorSha=hash(descriptorBytes),descriptorFile='static-release.json'
  await writeFile(`${root}${dir}/${descriptorFile}`,descriptorBytes)
  const binding={kind:'selected_source_static_art_package',status:'IN_PROGRESS',sourceBase,
    descriptor:{path:`${url}${descriptorFile}`,sha256:descriptorSha,byte_length:descriptorBytes.length},texture,detailTexture,
    provenance,method:{authorization:'Human source-preserving reconstruction answer relayed by Main on 2026-11-04; no redraw or substitutions.',
      masking:'Exterior-connected near-neutral pixels minRGB>=118/channelSpread<=18; keep largest connected foreground; alpha binary. No RGB edits or all-white key.',
      registration:'Explicit uniform nearest-neighbor source sampling into64x64 at32,60; optional4x atlas. Native crops retain source-resolution colors.',
      consumer:'Choose one static atlas for all surfaces/eras and cache its immutable content URL; compact genuine server config IDs only.'},
    sourcePackId:layout.source_pack_id,sourcePackRevision:layout.source_pack_revision,sourceId:layout.source_id,sourceSha256:layout.source_sha256,
    supportedPose:'front_idle_only',logicalCell:{width:64,height:64},anchor:{x:32,y:60},c06Compatible:false,wireBinding:null,
    browser:await browser.version(),visualReview:{reviewedBy:descriptor.reviewed_by,reviewedAt,scope:'Supplied identities, full figures, pale hair, whites and outlines inspected in native cutout/atlas; mobile fixture qualification recorded separately.'},
    productionBinding:'NOT_RUN',fullAr03Acceptance:'NOT_RUN'}
  await writeFile(`${root}frontend/src/entities/avatar/derived-assets/static-package.json`,`${JSON.stringify(binding,null,2)}\n`)
  await writeFile(`${root}docs/mcr/art/evidence/static-extraction.json`,`${JSON.stringify(binding,null,2)}\n`)
  console.log(JSON.stringify({descriptor:binding.descriptor,texture,detailTexture,frames:provenance.map(f=>({sourceLocation:f.sourceLocation,native:f.native,evidence:f.evidence}))},null,2))
} finally {await browser?.close();await new Promise(resolve=>server.close(resolve))}
