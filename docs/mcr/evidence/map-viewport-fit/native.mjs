// MAP-VIEWPORT-FOREGROUND-FIT-01. Actual App/Auth/Phaser; controlled HTTP/C05 only.
// Fixture/public serving adapted from Map owner's 7468f1fe roof/street native driver.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {execFileSync} from 'node:child_process'
import {readFile,writeFile,mkdir,statfs} from 'node:fs/promises'
import {resolve,extname} from 'node:path'
import {createRequire} from 'node:module'
import {pathToFileURL} from 'node:url'
const root=resolve(import.meta.dirname,'../../../..'),front=resolve(root,'frontend')
const require=createRequire(resolve(front,'package.json'))
const {createServer}=await import(pathToFileURL(require.resolve('vite')).href)
const {chromium,expect}=require('@playwright/test')
const output=process.env.MCR_MAP_FIT_OUTPUT,tmp=process.env.MCR_MAP_FIT_TMP
const deadline=Date.parse(process.env.MCR_MAP_FIT_WORK_DEADLINE??''),port=Number(process.env.MCR_MAP_FIT_PORT)
assert(output?.startsWith(root+'/docs/mcr/evidence/map-viewport-fit/')&&tmp?.startsWith('/private/tmp/mcr-map-viewport-fit-')&&Number.isFinite(deadline)&&port>1023)
const hash=b=>'sha256:'+createHash('sha256').update(b).digest('hex')
const candidate=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim()
const registration=await readFile(resolve(front,'src/world/reference-scene/map4-service-v2/registration.ts'),'utf8')
const expectedHash=registration.match(/presentationHash:\s*'(sha256:[a-f0-9]{64})'/)?.[1]
assert.equal(expectedHash,'sha256:348f1969deeaca05c16087f7ae5c7060f49c3c8fbd934ffc7aef195c937f2d24','Published qualified v2 input required; no older cohort/overlay')
const input=JSON.parse(await readFile(resolve(front,'src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'),'utf8'))
const localInput=JSON.parse(await readFile(resolve(front,'src/app/world-entry/__tests__/map-presentation-v1/source-inputs/local-input.json'),'utf8'))
const fixture=resolve(front,'src/app/world-entry/__tests__/map-roof-street-uptake/preview.html')
const paths=new Set(execFileSync('git',['ls-tree','-r','--name-only',candidate,'frontend/public'],{cwd:root,encoding:'utf8'}).trim().split('\n'))
const report={task:'MAP-VIEWPORT-FOREGROUND-FIT-01',candidate,expectedHash,status:'IN_PROGRESS',lease:process.env.MCR_MAP_FIT_LEASE,
  qualification:'ACTUAL_MAIN_AUTH_PHASER_CONTROLLED_HTTP_C05_NOT_LIVE_OR_DEVICE',checks:[],assetPins:{},
  limits:['No source-art, registration, C06, geometry, identity or World change.','Changed local framing/focus/controls only; unchanged source pixel/permission/photo-refusal cohorts reused.','Chrome mobile/touch emulation, not physical iOS/Android. Full reference/whole-product acceptance remains open.']}
const state=p=>p.evaluate(()=>window.__ART06_DEFAULT__.state())
const settle=async p=>{await p.evaluate(()=>document.fonts.ready);await p.waitForTimeout(160)}
async function guard(){const s=await statfs(root);assert(s.bavail*s.bsize>=8*1024**3&&Date.now()<deadline)}
async function geometry(page){return page.evaluate(()=>{
  const v=document.querySelector('.places-map-viewport'),im=v.querySelector('img'),z=document.querySelector('.places-map-zoom'),c=document.querySelector('.places-map-caption')
  const rect=e=>{const r=e.getBoundingClientRect();return {left:r.left,top:r.top,right:r.right,bottom:r.bottom,width:r.width,height:r.height}}
  return {mode:document.querySelector('.places-map-surface').dataset.mapFraming,image:rect(im),viewport:rect(v),zoom:rect(z),caption:rect(c),
    source:{width:im.naturalWidth,height:im.naturalHeight},scroll:{left:v.scrollLeft,top:v.scrollTop,width:v.scrollWidth,height:v.scrollHeight},
    client:{width:v.clientWidth,height:v.clientHeight},page:{x:scrollX,y:scrollY,overflow:document.documentElement.scrollWidth>innerWidth},
    controls:[...document.querySelectorAll('.places-map-tools button,.places-map-zoom button')].map(e=>({name:e.getAttribute('aria-label')||e.textContent,...rect(e)}))}
})}
async function raster(page){return page.locator('.places-map-plane > img').evaluate(async image=>{
  const c=document.createElement('canvas');c.width=image.naturalWidth;c.height=image.naturalHeight;c.getContext('2d').drawImage(image,0,0)
  const bytes=c.getContext('2d').getImageData(0,0,c.width,c.height).data
  const digest=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(v=>v.toString(16).padStart(2,'0')).join('');c.width=c.height=0;return digest
})}
async function capture(page,result,name){await guard();const file=`${result.era.toLowerCase()}-${result.width}-${result.height}-${name}.png`;await page.screenshot({path:resolve(output,file),fullPage:false});result.images??=[];result.images.push({file,sha256:hash(await readFile(resolve(output,file)))})}
async function focusProof(page,target){await target.focus();await settle(page);const proof=await target.evaluate(element=>{
  const r=element.getBoundingClientRect(),v=element.closest('.places-map-viewport').getBoundingClientRect()
  const hit=document.elementFromPoint(r.left+r.width/2,r.top+r.height/2)
  return {id:element.id,href:element.getAttribute('href'),width:r.width,height:r.height,
    fullyVisible:r.left>=v.left-.5&&r.right<=v.right+.5&&r.top>=v.top-.5&&r.bottom<=v.bottom+.5,
    centerHit:hit===element||element.contains(hit),focused:document.activeElement===element}
});assert(proof.width>=44&&proof.height>=44&&proof.fullyVisible&&proof.centerHit&&proof.focused,JSON.stringify(proof));return proof}
let server,browser,context
await mkdir(output,{recursive:true})
try{
 await guard()
 server=await createServer({root:front,configLoader:'runner',cacheDir:resolve(tmp,'vite'),logLevel:'error',
  define:{'import.meta.env.VITE_MCR_EVENT_ID':JSON.stringify(input.event.event_id),'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG':JSON.stringify(JSON.stringify(localInput))},
  plugins:[{name:'map-fit-actual-main-frozen-public',configureServer(s){s.middlewares.use(async(req,res,next)=>{
   const path='frontend/public'+decodeURIComponent(new URL(req.url,'http://localhost').pathname)
   if(paths.has(path)){const bytes=execFileSync('git',['show',`${candidate}:${path}`],{cwd:root,maxBuffer:64*1024**2});report.assetPins[path]={sha256:hash(bytes),bytes:bytes.length};const mime={'.png':'image/png','.svg':'image/svg+xml','.ttf':'font/ttf','.json':'application/json'}[extname(path)];if(mime)res.setHeader('Content-Type',mime);res.end(bytes);return}
   if(!req.headers.accept?.includes('text/html'))return next()
   res.setHeader('Content-Type','text/html');res.end(await s.transformIndexHtml(req.url,await readFile(fixture,'utf8')))
  })}}],server:{host:'127.0.0.1',port,strictPort:true,watch:null}})
 await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const [era,width,height] of [['REVENGE',360,844],['PARADE',390,844],['DANGER',430,844],['PARADE',390,420]]){
  await guard();context=await browser.newContext({viewport:{width,height},hasTouch:true,isMobile:true,reducedMotion:'reduce',serviceWorkers:'block'})
  const page=await context.newPage();page.setDefaultTimeout(10000)
  const result={era,width,height,errors:[],focus:[]};report.checks.push(result)
  page.on('pageerror',e=>result.errors.push(e.message))
  await page.goto(`http://127.0.0.1:${port}/login?`+new URLSearchParams({era,scenario:'eligible-attendee-contract-fixture'}))
  await page.getByLabel('Email',{exact:true}).fill('map-fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password');await page.getByRole('button',{name:'Sign in with email'}).click()
  await expect.poll(async()=>(await state(page)).activeSockets).toBe(1)
  await expect(page.locator('.shell-world-host canvas')).toHaveCount(1);await settle(page)
  const before=await page.evaluate(()=>window.__ART06_DEFAULT__.worldProof())
  result.worldBefore=before
  await page.locator('.shell-world-host canvas').evaluate(e=>Reflect.set(e,'__mapFitPersistentCanvas',true))
  await page.getByRole('button',{name:'World tools',exact:true}).click();await page.locator('a[href="/world/map"]').click()
  await expect(page.getByRole('img',{name:`${era} illustrated virtual map`,exact:true})).toBeVisible()
  await expect(page.getByRole('button',{name:'Fit map',exact:true})).toBeEnabled();await settle(page)
  const initial=await state(page);assert.equal(await page.evaluate(async()=>{const {currentComposition}=await import('/src/world/reference-scene/map4-service-v2/current.ts');return currentComposition()?.registration.presentationHash}),expectedHash);assert.equal(initial.canvases,1);assert.equal(initial.sockets,1)
  result.initial=await geometry(page);const g=result.initial
  assert.equal(g.mode,'readable');assert.equal(g.source.width,768);assert.equal(g.source.height,1296)
  assert(Math.abs(g.image.width/g.image.height-768/1296)<1e-7)
  assert(g.image.width>=g.client.width-.5&&g.image.height>=g.client.height-.5,'Reading scale fills local slot')
  assert(g.image.width*40/768>=16,'Readable original label scale')
  assert(g.zoom.top>=g.viewport.bottom-.5,'Docked zoom never obscures source pixels')
  assert(!g.page.overflow);for(const c of g.controls)assert(c.width>=44&&c.height>=44,c.name)
  const originalRaster=await raster(page)
  result.rasterBefore=originalRaster
  await capture(page,result,'reading')
  const region=page.locator('.places-map-viewport')
  // Local keyboard edges provide a direct path to the entire image, not just its initial crop.
  await region.focus();await page.keyboard.press('End');await settle(page)
  let end=await geometry(page);assert(Math.abs(end.scroll.left-Math.max(0,end.scroll.width-end.client.width))<2);assert(Math.abs(end.scroll.top-Math.max(0,end.scroll.height-end.client.height))<2)
  await page.keyboard.press('Home');await settle(page);let start=await geometry(page);assert(start.scroll.left<1&&start.scroll.top<1)
  const vector=start.scroll.width>start.client.width+1?{x:60,y:0}:{x:0,y:60}
  const point=await page.evaluate(()=>{const v=document.querySelector('.places-map-viewport'),r=v.getBoundingClientRect();for(let y=r.top+30;y<r.bottom-30;y+=30)for(let x=r.left+80;x<r.right-30;x+=30)if(document.elementFromPoint(x,y)===v.querySelector('img'))return {x,y};throw Error('No exposed image point for drag')})
  await page.mouse.move(point.x,point.y);await page.mouse.down();await page.mouse.move(point.x-vector.x,point.y-vector.y,{steps:8});await page.mouse.up();await settle(page)
  const dragged=await geometry(page);assert(dragged.scroll.left>0||dragged.scroll.top>0,'Mouse drag pans local image')
  result.mouse=dragged
  await region.focus();await page.keyboard.press('Home');await settle(page)
  const cdp=await context.newCDPSession(page)
  await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:point.x,y:point.y,id:1}]})
  for(let step=1;step<=8;step++){await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:point.x-vector.x*step/8,y:point.y-vector.y*step/8,id:1}]});await page.waitForTimeout(16)}
  await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});await cdp.detach();await settle(page)
  result.touch=await geometry(page);assert(result.touch.scroll.left>0||result.touch.scroll.top>0,'Emulated touch pans local image')
  // Every actual landmark is focusable, fully revealed, centre-hittable and still canonical.
  const links=page.locator('.places-map-plane a');assert(await links.count()>=8)
  for(let i=0;i<await links.count();i++)result.focus.push(await focusProof(page,links.nth(i)))
  const focused=await geometry(page);assert.equal(focused.page.y,g.page.y,'Focus reveal does not scroll the page')
  await page.getByRole('button',{name:'Fit map',exact:true}).click();await settle(page)
  result.overview=await geometry(page);assert.equal(result.overview.mode,'overview');assert(result.overview.image.height>=448-.5)
  await page.getByRole('button',{name:'Reset view',exact:true}).click();await settle(page)
  assert.equal((await geometry(page)).mode,'readable')
  const oldView=await geometry(page)
  await page.getByRole('button',{name:'Zoom in map view',exact:true}).click();await settle(page)
  const newView=await geometry(page);result.zoom={before:oldView,after:newView}
  assert(newView.image.width>oldView.image.width)
  await page.getByRole('button',{name:'Reset view',exact:true}).click();await settle(page)
  if(era==='PARADE'){
   const fit=page.getByRole('button',{name:'Fit map',exact:true});await page.emulateMedia({forcedColors:'active'})
   await fit.focus();await page.keyboard.press('Tab');await page.keyboard.press('Shift+Tab');await expect(fit).toBeFocused()
   result.forced=await fit.evaluate(e=>{const s=getComputedStyle(e);return {style:s.outlineStyle,width:s.outlineWidth,offset:s.outlineOffset}})
   assert.equal(result.forced.style,'solid');assert(parseFloat(result.forced.width)>=3);await capture(page,result,'forced-focus');await page.emulateMedia({forcedColors:'none'})
  }
  if(era==='PARADE'&&height===844){
   // Navigation itself is unchanged; qualify changed focus restoration with one feature and one canonical place.
   for(const selector of ['[data-map-feature="feed"]','[data-map-compact-marker="stage_info"]']){
    const link=page.locator(selector);const id=await link.getAttribute('id');await focusProof(page,link);await page.keyboard.press('Enter')
    await expect(page).not.toHaveURL(/\/world\/map(?:\?|$)/);await page.getByRole('button',{name:'Back to previous panel',exact:true}).click()
    await expect(page).toHaveURL(/\/world\/map(?:\?|$)/);await settle(page);await expect(page.locator('#'+id)).toBeFocused();result.focus.push(await focusProof(page,page.locator('#'+id)))
   }
  }
  result.rasterAfter=await raster(page)
  assert.equal(result.rasterAfter,originalRaster,'Viewport operations preserve every decoded source/label pixel')
  const stable=await state(page);assert.equal(stable.canvases,1);assert.equal(stable.sockets,1)
  await expect(page.getByText('No physical arrival is implied.',{exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Back to previous panel',exact:true}).click();await settle(page)
  await expect(page).toHaveURL(/\/world(?:\?|$)/)
  result.worldAfter=await page.evaluate(()=>window.__ART06_DEFAULT__.worldProof())
  assert.deepEqual(result.worldAfter,before,'Underlying World RGBA/objects/cameras unchanged')
  assert(await page.locator('.shell-world-host canvas').evaluate(e=>Reflect.get(e,'__mapFitPersistentCanvas')),'Same game canvas survives navigation')
  if(era==='PARADE'&&height===844){
   await page.getByRole('button',{name:'World tools',exact:true}).click();await page.locator('a[href="/world/map"]').click()
   await expect(page.getByRole('button',{name:'Fit map',exact:true})).toBeEnabled();await settle(page)
   await page.getByRole('button',{name:'Reset view',exact:true}).click();await settle(page)
   const old=await geometry(page);await page.setViewportSize({width:430,height:620});await settle(page)
   const resized=await geometry(page)
   const expectedLeft=Math.max(0,Math.min(resized.scroll.width-resized.client.width,(old.scroll.left+old.client.width/2)/old.image.width*resized.image.width-resized.client.width/2))
   const expectedTop=Math.max(0,Math.min(resized.scroll.height-resized.client.height,(old.scroll.top+old.client.height/2)/old.image.height*resized.image.height-resized.client.height/2))
   assert(Math.abs(resized.scroll.left-expectedLeft)<2&&Math.abs(resized.scroll.top-expectedTop)<2,'Resize preserves the clamped source centre')
   const resizedWorld=await page.evaluate(()=>window.__ART06_DEFAULT__.worldProof())
   await page.getByRole('button',{name:'Fit map',exact:true}).click();await page.getByRole('button',{name:'Reset view',exact:true}).click();await settle(page)
   assert.deepEqual(await page.evaluate(()=>window.__ART06_DEFAULT__.worldProof()),resizedWorld,'Map controls preserve current resized World')
   result.resize={before:old,after:resized,qualification:'Intentional browser resize may resize World; invariance is measured separately within each size'}
   await page.setViewportSize({width,height});await settle(page);await page.getByRole('button',{name:'Back to previous panel',exact:true}).click();await settle(page)
  }
  await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Me',exact:true}).click();await page.getByText('Profile options',{exact:true}).click();await page.getByRole('link',{name:'Account',exact:true}).click();await page.getByRole('button',{name:'Sign out',exact:true}).click();await page.getByRole('button',{name:'Confirm sign out',exact:true}).click()
  await expect.poll(async()=>{const s=await state(page);return s.activeSockets===0&&s.canvases===0&&s.urls.created.every(u=>s.urls.revoked.includes(u))}).toBe(true)
  assert.deepEqual(result.errors,[]);result.status='PASS';await context.close();context=null
 }
 report.status='PASS'
}catch(error){report.status='FAIL';report.error=String(error.stack??error);process.exitCode=1}
finally{await context?.close();await browser?.close();await server?.close();report.finishedAt=new Date().toISOString();await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n')}
