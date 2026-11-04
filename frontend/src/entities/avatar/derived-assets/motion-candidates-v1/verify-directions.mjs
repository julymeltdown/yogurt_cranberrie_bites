import assert from'node:assert/strict'
import{readFile,writeFile}from'node:fs/promises'
import{createHash}from'node:crypto'
import{pathToFileURL}from'node:url'
import{candidateServer,root,publicPath,evidencePath}from'./server.mjs'
const hash=b=>createHash('sha256').update(b).digest('hex'),m=JSON.parse(await readFile(root+publicPath+'combined-release-candidate.json','utf8')).presets
for(const f of m.files){const b=await readFile(root+publicPath+f.file);assert.equal(hash(b),f.sha256);assert.equal(b.length,f.bytes)}
const{chromium,expect}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href),server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser,page
const report={result:'FAIL',qualification:'120 candidate art frames, actual source/authored pixel attribution, isolated Canvas consumer. Not production/wardrobe/C06/Phaser admission.',errors:[],cases:[]},requests=[]
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version();page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.message));page.on('request',r=>{const p=new URL(r.url()).pathname;if(p.startsWith('/world/avatars/'))requests.push(p)});await page.goto(`http://127.0.0.1:${server.address().port}/tools/direction-review.html`);await expect(page.locator('#state')).toContainText('idle')
 assert.equal(requests.length,2);report.initialAvatarRequests=[...requests]
 report.pixels=await page.evaluate(async()=>{
  const{decode}=await import('/tools/pixels.mjs'),{manifest:m}=window.__DIRECTION__,base='/world/avatars/derived-v1/motion-candidates-v1/',atlas=await decode(base+m.atlas.file),front=await decode('/world/avatars/supplied-v1/photo-01.jpg'),authored=await decode(base+m.authoredSource.file),map=new DataView(await(await fetch(base+m.provenance.file)).arrayBuffer()),frames=[]
  for(let n=0;n<m.frames.length;n++){const f=m.frames[n],source=f.direction==='front'?front:authored,rgba=[];let opaque=0,minX=64,minY=64,maxX=-1,maxY=-1
   for(let y=0;y<64;y++)for(let x=0;x<64;x++){const ai=((Math.floor(n/20)*64+y)*atlas.width+n%20*64+x)*4,pi=(n*4096+y*64+x)*4,sx=map.getInt16(pi,true),sy=map.getInt16(pi+2,true),alpha=atlas.rgba[ai+3];rgba.push(...atlas.rgba.subarray(ai,ai+4));if(!alpha){if(sx!==-1||sy!==-1)throw Error('Transparent pixel provenance');continue}if(alpha!==255||sx<0||sy<0||sx>=source.width||sy>=source.height)throw Error('Bad source bounds/alpha');const si=(sy*source.width+sx)*4;if(source.rgba[si+3]<128)throw Error('Paper used as figure');for(let c=0;c<3;c++)if(atlas.rgba[ai+c]!==source.rgba[si+c])throw Error('Wrong source attribution/RGB');opaque++;minX=Math.min(minX,x);maxX=Math.max(maxX,x);minY=Math.min(minY,y);maxY=Math.max(maxY,y)}
   if(minX<=0||minY<=0||maxX>=63||maxY!==59)throw Error('Clipped or unregistered '+n+' '+JSON.stringify([minX,minY,maxX,maxY]));frames.push({preset:f.preset,direction:f.direction,phase:f.phase,attribution:f.attribution,opaque,bounds:[minX,minY,maxX,maxY],rgba})
  }
  for(let p=0;p<6;p++)for(let d=0;d<4;d++)for(let phase=1;phase<5;phase++)for(let n=0;n<42*64*4;n++)if(frames[p*20+d*5].rgba[n]!==frames[p*20+d*5+phase].rgba[n])throw Error('Head moved with limbs')
  return{frames,originalFrontFrames:30,authoredDirectionFrames:90,sourceRgbMismatches:0,sourceAttributionMismatches:0,headIdentityStable:true,footRegistration:true}
 })
 for(let p=0;p<6;p++)for(let d=0;d<4;d++)assert.equal(new Set(report.pixels.frames.slice(p*20+d*5,p*20+d*5+5).map(f=>hash(Buffer.from(f.rgba)))).size,5,'Repeated gait phase')
 for(const f of report.pixels.frames){f.pixelSha256=hash(Buffer.from(f.rgba));delete f.rgba}
 // Ignore deliberate debug decodes above; actual consumer preview must not fetch
 // any more avatar data on changing identities, directions, themes or phases.
 const consumerRequestBaseline=requests.length
 for(const width of[360,390,430]){await page.setViewportSize({width,height:844});const fingerprints=[]
  for(const theme of['REVENGE','PARADE','DANGER']){await page.locator('#theme').selectOption(theme)
   const hashes=await page.evaluate(async()=>{const all=[];for(let p=0;p<6;p++)for(let d=0;d<4;d++)for(let phase=0;phase<5;phase++){for(const[id,value]of[['preset',p],['direction',d]]){const el=document.getElementById(id);el.value=String(value);el.dispatchEvent(new Event('change'))}window.__DIRECTION__.setPhase(phase);const arrays=window.__DIRECTION__.canvases.map(c=>c.getContext('2d').getImageData(0,0,64,64).data),first=arrays[0];for(const a of arrays)for(let n=0;n<a.length;n++)if(a[n]!==first[n])throw Error('Context identity mismatch');const digest=await crypto.subtle.digest('SHA-256',first);all.push(Array.from(new Uint8Array(digest)).map(n=>n.toString(16).padStart(2,'0')).join(''))}return all})
   fingerprints.push(hashes);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false)
   for(const direction of[0,1,2,3]){await page.locator('#preset').selectOption(direction===3?'2':'5');await page.locator('#direction').selectOption(String(direction));await page.evaluate(()=>window.__DIRECTION__.setPhase(1));await page.screenshot({path:root+evidencePath+`direction-${width}-${theme.toLowerCase()}-${m.directions[direction]}.jpg`,fullPage:true,type:'jpeg',quality:78})}
  }
  assert.deepEqual(fingerprints[0],fingerprints[1]);assert.deepEqual(fingerprints[0],fingerprints[2]);report.cases.push({width,frames:120,themes:3,contexts:5,cssSizes:[56,64,72],themeIdentityInvariant:true,horizontalOverflow:false})
 }
 await page.locator('#play').click();const before=await page.evaluate(()=>window.__DIRECTION__.draws);await page.waitForFunction(n=>window.__DIRECTION__.draws>=n+8,before);await page.locator('#play').click();assert.equal(await page.evaluate(()=>window.__DIRECTION__.phase),0)
 assert.equal(requests.length,consumerRequestBaseline);report.network={initialAvatarFetches:2,gestureAnimationAvatarFetches:0,debugVerificationRequests:consumerRequestBaseline-2,atlasBytes:m.atlas.bytes};report.animation={actualRafPhaseChanges:8,resetToIdle:true}
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(e){report.failure=String(e);await page?.screenshot({path:root+evidencePath+'direction-failure.jpg',type:'jpeg'});throw e}
finally{await writeFile(root+evidencePath+'direction-validation.json',JSON.stringify(report,null,2)+'\n');await browser?.close();await new Promise(r=>server.close(r))}
