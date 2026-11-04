import assert from 'node:assert/strict'
import{readFile,writeFile}from'node:fs/promises'
import{createHash}from'node:crypto'
import{pathToFileURL}from'node:url'
import{candidateServer,root,publicPath,evidencePath}from'./server.mjs'
const hash=b=>createHash('sha256').update(b).digest('hex'),manifest=JSON.parse(await readFile(root+publicPath+'front-gait-index.json','utf8'))
for(const f of manifest.files){const b=await readFile(root+publicPath+f.file);assert.equal(hash(b),f.sha256);assert.equal(b.length,f.bytes)}
const{chromium,expect}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href)
const server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser,page
const report={result:'FAIL',qualification:'Original-pixel articulated FRONT-only gait, isolated Canvas consumer, no full-direction/product admission.',errors:[],cases:[]}
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.message));await page.goto(`http://127.0.0.1:${server.address().port}/tools/gait-review.html`);await expect(page.locator('#state')).toContainText('idle')
 report.pixels=await page.evaluate(async()=>{
  const{manifest:m}=window.__GAIT__,{decode}=await import('/tools/pixels.mjs'),source=await decode('/world/avatars/supplied-v1/'+m.source.file),atlas=await decode('/world/avatars/derived-v1/motion-candidates-v1/'+m.atlas.file)
  const b=await(await fetch('/world/avatars/derived-v1/motion-candidates-v1/'+m.provenance.file)).arrayBuffer(),v=new DataView(b),frames=[]
  for(let i=0;i<30;i++){let opaque=0,moving=0,minX=64,minY=64,maxX=-1,maxY=-1;const f=m.frames[i],rgba=[]
   for(let y=0;y<64;y++)for(let x=0;x<64;x++){const ai=((Math.floor(i/5)*64+y)*atlas.width+i%5*64+x)*4,pi=(i*4096+y*64+x)*4,sx=v.getInt16(pi,true),sy=v.getInt16(pi+2,true),a=atlas.rgba[ai+3];rgba.push(...atlas.rgba.subarray(ai,ai+4))
    if(a===0){if(sx!==-1||sy!==-1)throw Error('Transparent pixel carries source coordinate');continue}
    if(a!==255||sx<f.rig.rect.x||sx>=f.rig.rect.x+f.rig.rect.width||sy<f.rig.rect.y||sy>=f.rig.rect.y+f.rig.rect.height)throw Error('Alpha/provenance bounds')
    const si=(sy*source.width+sx)*4;for(let c=0;c<3;c++)if(atlas.rgba[ai+c]!==source.rgba[si+c])throw Error('Source pixel changed')
    // Prove each mapping uses one declared limb translation, never another
    // design, reflected coordinate, palette repaint or whole-sprite displacement.
    let explained=false;for(const offset of [[0,0],f.offsets.leftArm,f.offsets.rightArm,f.offsets.leftLeg,f.offsets.rightLeg]){const ex=Math.floor((x+.5-32)/f.scale+f.sourceAnchor.x-offset[0])+f.rig.rect.x,ey=Math.floor((y+.5-60)/f.scale+f.sourceAnchor.y-offset[1])+f.rig.rect.y;if(ex===sx&&ey===sy)explained=true}
    if(!explained)throw Error('Undeclared pixel transform')
    opaque++;if(y>=46)moving++;minX=Math.min(minX,x);maxX=Math.max(maxX,x);minY=Math.min(minY,y);maxY=Math.max(maxY,y)
   }
   if(minX<=0||minY<=0||maxX>=63||maxY!==59)throw Error('Clipping or foot registration')
   frames.push({preset:f.preset,pose:f.pose,opaque,limbRegionPixels:moving,bounds:[minX,minY,maxX,maxY],rgba})
  }
  // Preserve all head/upper-body pixels exactly through all four gait phases.
  for(let p=0;p<6;p++)for(let phase=1;phase<5;phase++)for(let n=0;n<42*64*4;n++)if(frames[p*5].rgba[n]!==frames[p*5+phase].rgba[n])throw Error('Head identity moved')
  return{frames,sourceRgbMismatches:0,undeclaredTransforms:0,headIdentityStable:true,footRegistration:true}
 })
 for(let p=0;p<6;p++){const set=new Set(report.pixels.frames.slice(p*5,p*5+5).map(f=>hash(Buffer.from(f.rgba))));assert.equal(set.size,5,'Repeated frame masquerades as gait')}
 for(const f of report.pixels.frames){f.pixelSha256=hash(Buffer.from(f.rgba));delete f.rgba}
 for(const width of[360,390,430]){await page.setViewportSize({width,height:844});const fingerprints=[]
  for(const theme of['REVENGE','PARADE','DANGER']){await page.locator('#theme').selectOption(theme)
   for(let p=0;p<6;p++){await page.locator('#preset').selectOption(String(p));for(let phase=0;phase<5;phase++){await page.evaluate(p=>window.__GAIT__.setPhase(p),phase);const pixels=await page.evaluate(()=>window.__GAIT__.canvases.map(c=>Array.from(c.getContext('2d').getImageData(0,0,64,64).data)));assert(pixels.every(a=>hash(Buffer.from(a))===hash(Buffer.from(pixels[0]))));fingerprints.push(hash(Buffer.from(pixels[0])))} }
   assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.screenshot({path:root+evidencePath+`gait-${width}-${theme.toLowerCase()}.jpg`,type:'jpeg',quality:82,fullPage:true})
  }
  assert.deepEqual(fingerprints.slice(0,30),fingerprints.slice(30,60));assert.deepEqual(fingerprints.slice(0,30),fingerprints.slice(60));report.cases.push({width,frames:30,themes:3,contexts:5,themeIdentityInvariant:true,horizontalOverflow:false})
 }
 await page.locator('#play').click();const before=await page.evaluate(()=>window.__GAIT__.draws);await page.waitForFunction(n=>window.__GAIT__.draws>=n+8,before);await page.locator('#play').click();assert.equal(await page.evaluate(()=>window.__GAIT__.phase),0);report.animation={actualRafPhaseChanges:8,resetToIdle:true}
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(e){report.failure=String(e);await page?.screenshot({path:root+evidencePath+'gait-failure.jpg',type:'jpeg'});throw e}
finally{await writeFile(root+evidencePath+'gait-validation.json',JSON.stringify(report,null,2)+'\n');await browser?.close();await new Promise(r=>server.close(r))}
