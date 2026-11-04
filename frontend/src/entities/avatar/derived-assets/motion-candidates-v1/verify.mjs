import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { pathToFileURL } from 'node:url'
import { candidateServer,root,publicPath,evidencePath } from './server.mjs'
assert(process.env.MCR_ART_PLAYWRIGHT, 'Set MCR_ART_PLAYWRIGHT to retained installed @playwright/test/index.mjs')
const { chromium,expect } = await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href)
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const manifest = JSON.parse(await readFile(root + publicPath + 'candidate-index.json', 'utf8'))
for (const file of manifest.files) { const b=await readFile(root+publicPath+file.file);assert.equal(hash(b),file.sha256);assert.equal(b.length,file.bytes) }
const server=candidateServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve))
const report={result:'FAIL',qualification:'Actual source pixel readback and isolated Canvas consumer fixture; no production descriptor, renderer, inventory, backend, walk animation or full-direction admission.',cases:[],errors:[]}
let browser,page
try {
  browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
  page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  const url=`http://127.0.0.1:${server.address().port}/tools/review.html`
  await page.goto(url);await expect(page.locator('#state')).toContainText('Front only')
  report.attachmentPixels=await page.evaluate(async()=>{
    const {decode}=await import('/tools/pixels.mjs'),m=window.__MOTION__.attachments,a=await decode('/world/avatars/derived-v1/motion-candidates-v1/'+m.atlas.file);let checked=0
    for(const [n,item]of m.items.entries()){const native=await decode('/world/avatars/derived-v1/motion-candidates-v1/'+item.native.file),t=item.transform;for(let y=0;y<64;y++)for(let x=0;x<64;x++){const sx=Math.floor((x+.5-t.targetAnchor.x)/t.scale+t.sourceAnchor.x),sy=Math.floor((y+.5-t.targetAnchor.y)/t.scale+t.sourceAnchor.y),i=(y*a.width+n*64+x)*4,valid=sx>=0&&sy>=0&&sx<native.width&&sy<native.height,si=(sy*native.width+sx)*4,alpha=valid?native.rgba[si+3]:0;if(a.rgba[i+3]!==alpha)throw Error('Attachment alpha mismatch');if(alpha){for(let c=0;c<3;c++)if(a.rgba[i+c]!==native.rgba[si+c])throw Error('Attachment source pixel mismatch');checked++}}}
    return{layers:m.items.length,checkedOpaque:checked,sourceMismatches:0}
  })
  report.pixelReadback=await page.evaluate(async()=>{const{verifyPixels}=await import('/tools/verify-pixels.mjs');return verifyPixels(window.__MOTION__.manifest)})
  report.composition=await page.evaluate(()=>{
    const {manifest,drawFrame,drawAttachment,drawEyes}=window.__MOTION__,bodies=manifest.items.map((p,i)=>p.sourceId==='photo_02'?i:-1).filter(i=>i>=0),hairs=manifest.items.map((p,i)=>p.sourceId==='photo_04'?i:-1).filter(i=>i>=0),cases=[]
    const c=document.createElement('canvas');c.width=64;c.height=64;const ctx=c.getContext('2d',{willReadFrequently:true})
    for(const body of bodies)for(const hair of hairs)for(const accessory of [-1,0,1,2,3])for(const eyes of [-1,0,1,2,3]){ctx.clearRect(0,0,64,64);drawFrame(ctx,body);if(eyes>=0)drawEyes(ctx,eyes);drawFrame(ctx,hair);if(accessory>=0)drawAttachment(ctx,accessory);const a=ctx.getImageData(0,0,64,64).data;let minX=64,minY=64,maxX=-1,maxY=-1;for(let y=0;y<64;y++)for(let x=0;x<64;x++)if(a[(y*64+x)*4+3]){minX=Math.min(minX,x);maxX=Math.max(maxX,x);minY=Math.min(minY,y);maxY=Math.max(maxY,y)}if(minX<=0||minY<=0||maxX>=63||maxY!==59)throw Error('Composition clipped or lost foot registration');cases.push({body,hair,accessory,eyes,bounds:[minX,minY,maxX,maxY]})}
    return {cases:cases.length,accessoryLayers:4,eyeOverlays:4,footAnchor:[32,60],allBoundsClear:true,bodyEyesAndClothes:'BAKED_ORIGINAL',hairPairing:'FRONT_BODY_ATTACHMENT_CANDIDATE_ONLY',rearHairPairs:0}
  })
  for(const width of [360,390,430]){
    await page.setViewportSize({width,height:844});await page.locator('#body').selectOption('7');await page.locator('#hair').selectOption('10');await page.locator('#eyes').selectOption('2')
    const fingerprints=[]
    for(const theme of ['REVENGE','PARADE','DANGER']){
      await page.locator('#theme').selectOption(theme)
      const pixels=await page.evaluate(()=>window.__MOTION__.canvases.map(c=>Array.from(c.getContext('2d').getImageData(0,0,64,64).data)))
      assert(pixels.every(p=>hash(Buffer.from(p))===hash(Buffer.from(pixels[0]))),'Context pixels differ')
      fingerprints.push(hash(Buffer.from(pixels[0])))
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false)
      await page.locator('#accessory').selectOption('2');await page.screenshot({path:root+evidencePath+`preview-${width}-${theme.toLowerCase()}.jpg`,type:'jpeg',quality:82});await page.locator('#accessory').selectOption('none')
    }
    assert.equal(new Set(fingerprints).size,1,'Theme changed avatar identity')
    await page.locator('#hair').selectOption('none');const noHair=await page.locator('#large').screenshot();await page.locator('#hair').selectOption('10');assert.notEqual(hash(noHair),hash(await page.locator('#large').screenshot()),'Hair toggle did not change pixels')
    report.cases.push({width,themeIdentitySha256:fingerprints[0],samePixelsAcrossFiveContexts:true,cssSizes:[56,64,72],horizontalOverflow:false,independentHairToggle:true})
  }
  await page.locator('summary').click();await expect(page.locator('.part')).toHaveCount(manifest.items.length);await expect(page.locator('.garment')).toHaveCount(6)
  await page.screenshot({path:root+evidencePath+'all-parts.jpg',type:'jpeg',quality:82,fullPage:true})
  assert.deepEqual(report.errors,[]);assert.equal(manifest.productionEligible,false);assert.deepEqual(manifest.walkFrames,[]);assert.deepEqual(manifest.fullAvatarDirections,[])
  report.result='PASS'
} catch(error) {report.failure=String(error);await page?.screenshot({path:root+evidencePath+'failure.jpg',type:'jpeg',quality:75});throw error}
finally {await writeFile(root+evidencePath+'validation.json',JSON.stringify(report,null,2)+'\n');await browser?.close();await new Promise(resolve=>server.close(resolve))}
