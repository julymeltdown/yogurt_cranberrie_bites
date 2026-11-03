import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const frontend=resolve(import.meta.dirname,'../../../..'),evidence=process.env.MCR_DEFAULT_EVIDENCE??resolve(import.meta.dirname,'evidence')
assert(evidence===resolve(import.meta.dirname,'evidence')||evidence.startsWith(resolve(import.meta.dirname,'evidence')+'/'))
await mkdir(evidence,{recursive:true})
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:'/tmp/mcr-default-qualified-cache',server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'})
await server.listen()
const report={result:'FAIL',qualification:'Actual default createAppRoutes, canonical Auth login/bootstrap/HTTP, production lazy owner/resolver/composition, actual single Phaser, deterministic in-process C04 reads and C05 MockSocket, geometric TEST ONLY resources/review metadata. Default Wall unavailable: no approved anchor/write policy. No live service, approved artwork, two-user, physical device or load acceptance.',cases:[],directEntries:[],screenshots:[],errors:[]}
let browser,currentPage,stage='start'
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 const base=`http://127.0.0.1:${server.httpServer.address().port}`
 for(const [width,height] of [[360,800],[390,844],[430,932]]){
  const context=await browser.newContext({viewport:{width,height},hasTouch:true,reducedMotion:'reduce'})
  const page=await context.newPage();currentPage=page;page.on('pageerror',e=>report.errors.push(e.message))
  const state=()=>page.evaluate(()=>window.__DEFAULT_QUALIFIED__?.state())
  const mode=value=>page.evaluate(v=>window.__DEFAULT_QUALIFIED__.mode(v),value)
  const navigate=path=>page.evaluate(p=>window.__DEFAULT_QUALIFIED__.navigate(p),path)
  const ready=async()=>{await expect.poll(async()=>(await state())?.published).toBe(true);await expect.poll(async()=>(await state()).room).toBeTruthy()}
  const shot=async name=>{await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));const file=`qualified-${name}-${width}.png`;await page.screenshot({path:resolve(evidence,file),fullPage:true});report.screenshots.push(file);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false)}
  const colors=()=>page.locator('.shell-world-host canvas').evaluate(async canvas=>{await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));const pixels=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data,colors=new Set();for(let i=0;i<pixels.length;i+=4)colors.add(`${pixels[i]},${pixels[i+1]},${pixels[i+2]}`);return [...colors]})
  const stable=async initial=>{const s=await state();assert.equal(s.mounts,1);assert.equal(s.games,1);assert.equal(s.peak,1);assert.equal(s.sockets,1);assert.equal(s.activeSockets,1);assert.equal(s.sameCamera,true);assert.equal(s.canvases,1);assert.equal(s.geometry,initial.geometry);assert.equal(s.mapHash,initial.mapHash);assert.equal(s.room,initial.room);assert.deepEqual(s.outfit,initial.outfit);return s}
  stage=`${width}/open`;await page.goto(`${base}/src/app/__tests__/default-shell/qualified.html`);await ready();const initial=await state();await shot('world')
  const beforeColors=await colors()
  stage=`${width}/pending`;await mode('pending');await page.getByRole('button',{name:'Parade',exact:true}).click();await expect.poll(async()=>(await state()).theme?.phase).toBe('loading');await expect.poll(async()=>(await state()).gates).toBeGreaterThan(0)
  assert.equal((await state()).theme.selected,'REVENGE');assert.equal((await state()).writes,0);await shot('pending')
  await mode('valid');await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.release());await expect.poll(async()=>(await state()).theme?.selected).toBe('PARADE');await expect.poll(async()=>(await state()).theme?.phase).toBe('ready')
  const paradeColors=await colors();assert.notDeepEqual(paradeColors,beforeColors);await stable(initial);assert.deepEqual((await state()).camera,initial.camera);await shot('parade')
  stage=`${width}/rollback`;await mode('fail');await page.getByRole('button',{name:'Danger',exact:true}).click();await expect.poll(async()=>(await state()).theme?.phase).toBe('error');assert.equal((await state()).theme.selected,'PARADE');assert.deepEqual(await colors(),paradeColors);await shot('rollback');await mode('valid')
  stage=`${width}/map`;await page.getByRole('link',{name:'Map',exact:true}).click();await expect(page.getByRole('heading',{name:'Map',exact:true,level:1})).toBeVisible();await expect(page.getByRole('region',{name:'Map overview'})).toBeVisible()
  const place=page.locator('[id^="place-"]').filter({hasText:'View'}).first();const placeId=await place.getAttribute('id');await place.click();await expect(page.getByRole('heading',{name:'Place',exact:true,level:1})).toBeVisible();await expect(page.getByRole('region',{name:'Place information'})).toBeVisible();await shot('place')
  await page.getByRole('button',{name:'Back to previous panel'}).click();await expect(page.getByRole('heading',{name:'Map',exact:true,level:1})).toBeVisible();await expect(page.locator(`[id="${placeId}"]`)).toBeFocused();await shot('map')
  const mapRegion=page.getByRole('region',{name:'Map overview'})
  await expect(page.locator('.mcr-root-map .concert-window__titlebar')).toBeHidden();await expect(page.getByRole('heading',{name:'Map',exact:true,level:1})).toBeVisible()
  await mapRegion.getByRole('button',{name:/Show .+ on virtual map/}).first().click();assert.equal((await state()).camera.following,false)
  await mapRegion.getByRole('button',{name:'Recenter avatar',exact:true}).click();assert.equal((await state()).camera.following,true)
  const search=page.getByRole('searchbox',{name:'Find a place',exact:true});await search.focus();await page.keyboard.press('ArrowLeft');await page.keyboard.press('Escape');assert.equal((await state()).path,'/world/map')
  await page.setViewportSize({width,height:430});await search.scrollIntoViewIfNeeded();await expect(search).toBeInViewport();await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport();await page.setViewportSize({width,height})
  await page.getByRole('region',{name:'Map overview'}).getByRole('button',{name:'Zoom in',exact:true}).click();await expect.poll(async()=>(await state()).camera.camera.zoom).toBe(2)
  stage=`${width}/nav`;for(const label of ['Feed','Meet','Gifts','Wall','Me']){await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:label,exact:true}).click();await expect(page.getByRole('heading',{name:label,exact:true,level:1})).toBeVisible();await stable(initial)}
  await page.getByRole('link',{name:'World',exact:true}).click();await page.getByRole('link',{name:'Inbox',exact:true}).click();await expect(page.getByRole('heading',{name:'Inbox',exact:true,level:1})).toBeVisible();await page.getByRole('button',{name:'Back to previous panel'}).click();await expect(page.getByRole('link',{name:'Inbox',exact:true})).toBeFocused()
  stage=`${width}/direct`;for(const [path,title] of [['/world/map','Map'],[`/world/places/${placeId.slice(6)}`,'Place'],['/wall','Wall'],['/world/theme','Choose an era']]){await navigate(path);await expect(page.getByRole('heading',{name:title,exact:true,level:1})).toBeVisible();await stable(initial)}
  await navigate('/wall');await expect(page.getByText('Wall unavailable.',{exact:false})).toBeVisible();assert.equal((await state()).requests.filter(r=>r.method==='POST'&&r.path.endsWith('/mural/pixels')).length,0);await shot('wall-unavailable')
  await page.getByRole('button',{name:'Expand panel'}).click();await shot('full-panel');await page.getByRole('button',{name:'Reduce panel'}).click();await page.getByRole('button',{name:'Minimize panel'}).click();await page.getByRole('button',{name:'Show panel content'}).click()
  await page.setViewportSize({width,height:430});await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport();await shot('short-viewport');await page.setViewportSize({width,height})
  stage=`${width}/short-world`;await navigate('/world');await page.setViewportSize({width,height:430})
  const visibleScene=await page.locator('.shell-world-host canvas').evaluate(canvas=>{
   const c=canvas.getBoundingClientRect(),w=document.querySelector('.shell-world').getBoundingClientRect(),s=document.querySelector('.world-composition').getBoundingClientRect()
   return Math.max(0,Math.min(c.bottom,w.bottom,s.bottom,innerHeight)-Math.max(c.top,w.top,s.top,0))
  });assert(visibleScene>=100,`Visible short world scene: ${visibleScene}`);await shot('short-world')
  const recenter=page.getByRole('button',{name:'Recenter',exact:true});await recenter.focus();await expect(recenter).toBeInViewport();await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport();await shot('short-world-controls')
  await page.setViewportSize({width,height})
  stage=`${width}/withdraw`;await mode('pending');await page.getByRole('button',{name:'Danger',exact:true}).click();await expect.poll(async()=>(await state()).gates).toBeGreaterThan(0);const writes=(await state()).writes
  await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.withdraw());await expect(page.getByText('Eras unavailable.',{exact:true})).toBeVisible();await mode('valid');await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.release());await shot('withdrawn')
  const end=await state();assert.equal(end.writes,writes);assert.equal(end.games,0);assert.equal(end.activeSockets,0);assert.equal(end.sceneClosed,1);assert.equal(end.canvases,0)
  await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.dispose());report.cases.push({width,height,mounts:end.mounts,peak:end.peak,disposals:end.disposals,sceneClosed:end.sceneClosed,lateWrites:end.writes-writes,defaultMuralPosts:0,visibleSceneAt430:visibleScene})
  // Fresh documents prove initial-entry registration and local preference restore, not only navigation.
  for(const [path,title] of [['/world/map','Map'],[`/world/places/${placeId.slice(6)}`,'Place'],['/wall','Wall'],['/world/theme','Choose an era']]){
   stage=`${width}/entry${path}`;await page.goto(`${base}/src/app/__tests__/default-shell/qualified.html?path=${encodeURIComponent(path)}`);await ready()
   await expect(page.getByRole('heading',{name:title,exact:true,level:1})).toBeVisible();await expect.poll(async()=>(await state()).theme?.phase).toBe('ready');assert.equal((await state()).theme.selected,'PARADE')
   assert.equal((await state()).mounts,1);assert.equal((await state()).activeSockets,1)
   await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.dispose());const disposed=await state();assert.equal(disposed.games,0);assert.equal(disposed.activeSockets,0);assert.equal(disposed.sceneClosed,1)
   report.directEntries.push({width,path,restored:'PARADE',mounts:disposed.mounts,disposals:disposed.disposals})
  }
  await context.close()
 }
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(e){report.failure={stage,error:String(e),state:await currentPage?.evaluate(()=>window.__DEFAULT_QUALIFIED__?.state()),text:await currentPage?.locator('body').innerText()};await currentPage?.screenshot({path:resolve(evidence,'qualified-failure.png')});throw e}finally{await writeFile(resolve(evidence,'qualified-native.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close()}
