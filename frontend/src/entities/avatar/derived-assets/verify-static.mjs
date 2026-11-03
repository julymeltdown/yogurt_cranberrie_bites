import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {readFile,writeFile} from 'node:fs/promises'
import {fileURLToPath,pathToFileURL} from 'node:url'
import {createArtReviewServer} from './serve.mjs'
const root=fileURLToPath(new URL('../../../../../',import.meta.url))
const json=async path=>JSON.parse(await readFile(`${root}${path}`,'utf8'))
const hash=bytes=>createHash('sha256').update(bytes).digest('hex')
const artifact=await json('frontend/src/entities/avatar/derived-assets/static-package.json')
const release=await json('frontend/public/world/avatars/derived-v1/static-release.json')
const layout=await json('frontend/src/entities/avatar/derived-assets/source-layout.json')
const catalog=await json('frontend/src/entities/avatar/character-source-pack.json')
assert.equal(release.format,'mcr-selected-static-v1');assert.equal(release.pose,'front_idle_only')
assert.equal(release.source_pack_id,catalog.source_pack_id);assert.equal(release.source_pack_revision,catalog.revision)
assert.equal(release.characters.length,6);assert.equal(artifact.c06Compatible,false);assert.equal(artifact.wireBinding,null)
const originalChecks=[]
for(const source of catalog.files) {
  const bytes=await readFile(`${root}frontend/public/world/avatars/supplied-v1/${source.file}`)
  assert.equal(hash(bytes),source.sha256);assert.equal(bytes.length,source.byte_length)
  originalChecks.push({sourceId:source.source_id,sha256:source.sha256,status:'PASS'})
}
assert.equal(originalChecks.length,20)
const publicAssets=[artifact.texture,artifact.detailTexture,...artifact.provenance.map(frame=>frame.native)]
for(const resource of publicAssets) {
  const bytes=await readFile(`${root}frontend/public${resource.path}`)
  assert.equal(hash(bytes),resource.sha256);assert.equal(bytes.length,resource.byte_length)
  assert.equal(bytes.readUInt32BE(16),resource.width_px);assert.equal(bytes.readUInt32BE(20),resource.height_px)
  assert.equal(bytes[25],6,'Expected real RGBA PNG')
}
assert.equal(release.atlas.path,artifact.texture.path.slice(1));assert.equal(release.atlas.sha256,`sha256:${artifact.texture.sha256}`)
assert(/^world\/avatars\/derived-v1\/[A-Za-z0-9_-]+\.png$/.test(release.atlas.path),'Consumer requires a filename stem without internal dots')
for(const [index,character]of release.characters.entries()) {
  assert.equal(character.id,`photo01-front-${String(index+1).padStart(2,'0')}`)
  assert.equal(character.source_id,'photo_01');assert.equal(character.source_sha256,layout.source_sha256)
  assert.deepEqual(character.anchor,{x:32,y:60});assert.deepEqual(character.frame,{x:index*64,y:0,width_px:64,height_px:64})
}
assert.equal(hash(await readFile(`${root}frontend/public/world/avatars/derived-v1/static-release.json`)),artifact.descriptor.sha256)
assert(process.env.MCR_ART_PLAYWRIGHT,'Set MCR_ART_PLAYWRIGHT to installed @playwright/test/index.mjs')
const {chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href)
const server=createArtReviewServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve))
let browser
try {
  browser=await chromium.launch({executablePath:process.env.MCR_ART_CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
  const base=`http://127.0.0.1:${server.address().port}`,proofPage=await browser.newPage()
  await proofPage.goto(`${base}/src/entities/avatar/derived-assets/static-review.html`)
  await proofPage.waitForFunction(()=>window.__MCR_STATIC_REVIEW__?.figures===6)
  // Reviewed interior eye highlights, collars/bars, and pale-hair regions in original pixels.
  // Exterior JPEG ringing can also exceed RGB230; it is not an intentional white feature.
  const protectedFeatures=[
    [{x:190,y:260,width:130,height:80},{x:210,y:350,width:65,height:30}],
    [{x:570,y:275,width:140,height:65},{x:540,y:140,width:190,height:75},{x:590,y:360,width:95,height:60}],
    [{x:960,y:265,width:175,height:65},{x:990,y:365,width:65,height:45}],
    [{x:180,y:645,width:145,height:65},{x:180,y:745,width:125,height:30}],
    [{x:585,y:650,width:125,height:65},{x:595,y:730,width:100,height:55}],
    [{x:975,y:650,width:155,height:65},{x:1000,y:745,width:100,height:45}],
  ]
  const pixelProof=await proofPage.evaluate(async({artifact,layout,protectedFeatures})=>{
    const decode=async path=>{
      const image=new Image();image.src=path;await image.decode()
      const canvas=document.createElement('canvas');canvas.width=image.naturalWidth;canvas.height=image.naturalHeight
      const ctx=canvas.getContext('2d',{willReadFrequently:true});ctx.drawImage(image,0,0)
      return {width:canvas.width,height:canvas.height,pixels:ctx.getImageData(0,0,canvas.width,canvas.height).data}
    }
    const source=await decode(layout.source_path),atlas=await decode(artifact.texture.path),detail=await decode(artifact.detailTexture.path),results=[]
    for(const[index,frame]of artifact.provenance.entries()) {
      const native=await decode(frame.native.path),rect=frame.sourceRect,origin=frame.native.sourceOrigin
      let kept=0,rgbMismatches=0,semi=0,white=0,sourceWhite=0,removedDark=0
      const removedWhite=[]
      let minX=native.width,minY=native.height,maxX=-1,maxY=-1
      for(let y=0;y<native.height;y++)for(let x=0;x<native.width;x++) {
        const ni=(y*native.width+x)*4,alpha=native.pixels[ni+3]
        if(alpha>0&&alpha<255)semi++
        if(!alpha)continue
        kept++;minX=Math.min(minX,x);minY=Math.min(minY,y);maxX=Math.max(maxX,x);maxY=Math.max(maxY,y)
        const si=((y+origin.y)*source.width+x+origin.x)*4
        for(let channel=0;channel<3;channel++)if(native.pixels[ni+channel]!==source.pixels[si+channel])rgbMismatches++
        if(Math.min(...native.pixels.subarray(ni,ni+3))>=230)white++
      }
      for(let y=rect.y;y<rect.y+rect.height;y++)for(let x=rect.x;x<rect.x+rect.width;x++) {
        const si=(y*source.width+x)*4,nx=x-origin.x,ny=y-origin.y
        const alpha=nx>=0&&ny>=0&&nx<native.width&&ny<native.height?native.pixels[(ny*native.width+nx)*4+3]:0
        if(Math.max(...source.pixels.subarray(si,si+3))<=112&&!alpha)removedDark++
        if(Math.min(...source.pixels.subarray(si,si+3))>=230){sourceWhite++;if(!alpha)removedWhite.push({x,y,rgb:[...source.pixels.subarray(si,si+3)]})}
      }
      let protectedPixels=0,protectedMissing=0,protectedWhite=0
      for(const feature of protectedFeatures[index])for(let y=feature.y;y<feature.y+feature.height;y++)for(let x=feature.x;x<feature.x+feature.width;x++) {
        const ni=((y-origin.y)*native.width+x-origin.x)*4,si=(y*source.width+x)*4
        protectedPixels++;if(!native.pixels[ni+3])protectedMissing++
        if(Math.min(...source.pixels.subarray(si,si+3))>=230)protectedWhite++
      }
      const registered=[]
      for(const [ratio,texture]of [[1,atlas],[4,detail]]) {
        const size=64*ratio,scale=frame.transform.scale64*ratio,foot=frame.transform.sourceFoot
        let mismatches=0,opaque=0,cellMinX=size,cellMinY=size,cellMaxX=-1,cellMaxY=-1
        for(let y=0;y<size;y++)for(let x=0;x<size;x++) {
          const i=(y*texture.width+index*size+x)*4
          const sx=Math.floor((x+0.5-32*ratio)/scale+foot.x)-origin.x,sy=Math.floor((y+0.5-60*ratio)/scale+foot.y)-origin.y
          const ni=(sy*native.width+sx)*4,expected=sx>=0&&sy>=0&&sx<native.width&&sy<native.height?native.pixels[ni+3]:0
          if(texture.pixels[i+3]!==expected)mismatches++
          if(!texture.pixels[i+3])continue
          opaque++;cellMinX=Math.min(cellMinX,x);cellMinY=Math.min(cellMinY,y);cellMaxX=Math.max(cellMaxX,x);cellMaxY=Math.max(cellMaxY,y)
          for(let channel=0;channel<3;channel++)if(texture.pixels[i+channel]!==native.pixels[ni+channel])mismatches++
        }
        registered.push({ratio,mismatches,opaque,bounds:{minX:cellMinX,minY:cellMinY,maxX:cellMaxX,maxY:cellMaxY}})
      }
      results.push({sourceLocation:frame.sourceLocation,kept,rgbMismatches,semi,white,sourceWhite,removedDark,removedWhite,
        protectedFeatures:protectedFeatures[index],protectedPixels,protectedMissing,protectedWhite,
        nativeBounds:{minX,minY,maxX,maxY},nativeDimensions:{width:native.width,height:native.height},registered})
    }
    return results
  },{artifact,layout,protectedFeatures})
  for(const proof of pixelProof) {
    assert.equal(proof.rgbMismatches,0);assert.equal(proof.semi,0);assert.equal(proof.removedDark,0)
    assert(proof.white>0);assert(proof.protectedWhite>0);assert.equal(proof.protectedMissing,0,'Lost an interior source feature')
    assert.equal(proof.nativeBounds.minX,2);assert.equal(proof.nativeBounds.minY,2)
    assert.equal(proof.nativeBounds.maxX,proof.nativeDimensions.width-3);assert.equal(proof.nativeBounds.maxY,proof.nativeDimensions.height-3)
    for(const variant of proof.registered){assert.equal(variant.mismatches,0);assert(variant.bounds.minX>=variant.ratio&&variant.bounds.maxX<63*variant.ratio);assert(variant.bounds.minY>=3*variant.ratio&&variant.bounds.maxY<60*variant.ratio)}
  }
  // Inspect all native-resolution contours on light, dark and checker surfaces.
  await proofPage.setViewportSize({width:1200,height:980})
  const nativeMarkup=artifact.provenance.map(frame=>`<figure><img src="${frame.native.path}" alt="${frame.label}"><figcaption>${frame.label}</figcaption></figure>`).join('')
  for(const [surface,bg,ink]of [['light','#ffffff','#24191d'],['dark','#1b1b20','#fff5ef'],['checker','#ddd','#24191d']]) {
    await proofPage.setContent(`<style>body{margin:16px;font:16px system-ui;color:${ink};background:${bg};${surface==='checker'?'background-image:conic-gradient(#aaa 25%,transparent 0 50%,#aaa 0 75%,transparent 0);background-size:16px 16px;':''}}main{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:16px}figure{margin:0;min-width:0}img{max-width:100%;height:auto;image-rendering:pixelated}figcaption{line-height:2}</style><h1>Native source-pixel cutouts · ${surface}</h1><main>${nativeMarkup}</main>`)
    await proofPage.locator('img').evaluateAll(images=>Promise.all(images.map(image=>image.decode())))
    await proofPage.screenshot({path:`${root}docs/mcr/art/evidence/static-native-${surface}.png`,fullPage:true})
  }
  await proofPage.close()
  const browserResults=[],errors=[]
  for(const width of [360,390,430]) {
    const context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce'}),page=await context.newPage(),requests=[]
    page.on('pageerror',error=>errors.push(error.message));page.on('request',request=>requests.push(new URL(request.url()).pathname))
    await page.goto(`${base}/src/entities/avatar/derived-assets/static-review.html`)
    await page.waitForFunction(()=>window.__MCR_STATIC_REVIEW__?.figures===6)
    assert.equal(await page.locator('.sprite').count(),18)
    const identity=await page.locator('.sprite').evaluateAll(sprites=>sprites.map(sprite=>sprite.style.cssText))
    for(const theme of ['REVENGE','PARADE','DANGER','CHECKER']) {
      await page.getByRole('button',{name:theme,exact:true}).click()
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth))
      assert.deepEqual(await page.locator('.sprite').evaluateAll(sprites=>sprites.map(sprite=>sprite.style.cssText)),identity)
      if(width===390||theme==='PARADE')await page.screenshot({path:`${root}docs/mcr/art/evidence/static-${theme.toLowerCase()}-${width}.png`,fullPage:true})
    }
    for(const label of ['Large','Compact','Large','Compact'])await page.getByRole('button',{name:label,exact:true}).click()
    assert.equal(requests.filter(path=>path===artifact.texture.path).length,1,'Atlas refetched during view/surface changes')
    assert.equal(requests.filter(path=>path==='/world/avatars/derived-v1/static-release.json').length,1)
    assert.equal(requests.filter(path=>path.endsWith('.png')).length,1,'Native/detail/per-item image fetched by default')
    browserResults.push({width,figures:6,views:18,atlasRequests:1,descriptorRequests:1,viewAndEraRefetches:0,overflow:false})
    await context.close()
  }
  assert.deepEqual(errors,[])
  const pngResponse=await fetch(`${base}${artifact.texture.path}`)
  assert.equal(pngResponse.headers.get('cache-control'),'public, max-age=31536000, immutable')
  const revalidated=await fetch(`${base}${artifact.texture.path}`,{headers:{'If-None-Match':pngResponse.headers.get('etag')}})
  assert.equal(revalidated.status,304)
  const report={status:'IN_PROGRESS',task:'AR03',sourceBase:artifact.sourceBase,sourceChecks:originalChecks,pixelProof,
    browser:{version:await browser.version(),results:browserResults,pageErrors:errors},
    transfer:{descriptorBytes:artifact.descriptor.byte_length,atlasBytes:artifact.texture.byte_length,initialPayloadBytes:artifact.descriptor.byte_length+artifact.texture.byte_length,
      immutableContentTexture:true,etagRevalidation:304,scope:'Isolated static-art fixture. Production consumer integration is owned separately.'},
    visualEvidence:['static-native-light.png','static-native-dark.png','static-native-checker.png','static-revenge-390.png','static-parade-390.png','static-danger-390.png','static-checker-390.png'],
    c06ProductionComposition:'NOT_RUN',layerSeparation:'NOT_RUN',directionWalkFrames:'NOT_PROVIDED',realDeviceAcceptance:'NOT_RUN',multiuserAcceptance:'NOT_RUN'}
  await writeFile(`${root}docs/mcr/art/evidence/static-validation.json`,`${JSON.stringify(report,null,2)}\n`)
  console.log(JSON.stringify({status:report.status,frames:pixelProof.map(p=>({sourceLocation:p.sourceLocation,rgbMismatches:p.rgbMismatches,removedDark:p.removedDark,white:p.white,sourceWhite:p.sourceWhite})),transfer:report.transfer,browser:report.browser},null,2))
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve))}
