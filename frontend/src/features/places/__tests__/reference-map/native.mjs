import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { resolve } from 'node:path'
import { mkdir,writeFile } from 'node:fs/promises'
const frontend=resolve(import.meta.dirname,'../../../../..'),evidence=resolve(import.meta.dirname,process.env.MCR_MAP_EVIDENCE_DIR??'evidence')
await mkdir(evidence,{recursive:true})
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:'/tmp/mcr-ref-map-vite-cache',server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'})
await server.listen()
const report={result:'FAIL',qualification:'Actual production Places/router/Auth/owner/Phaser with existing TEST ONLY in-process HTTP/C05/review metadata; no preview layout override. Current root/module byte pins are recorded by the associated qualification. Original orthogonal1152x896 scenery; diagonal portrait reference fidelity remains incomplete. Not live/device/character acceptance.',cases:[],errors:[]}
let browser,page,stage='start'
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 for(const width of [360,390,430]){
  const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,reducedMotion:'reduce'})
  await context.addInitScript(()=>{
   const state={mode:'valid',jobs:[],created:[],revoked:[]},encode=HTMLCanvasElement.prototype.toBlob
   HTMLCanvasElement.prototype.toBlob=function(callback,...args){
    if(this.width!==1152||this.height!==896)return encode.call(this,callback,...args)
    const mode=state.mode
    if(mode==='broken'){callback(new Blob(['invalid image'],{type:'image/png'}));return}
    return encode.call(this,blob=>mode==='pending'?state.jobs.push(()=>callback(blob)):callback(blob),...args)
   }
   const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL)
   URL.createObjectURL=blob=>{const url=create(blob);state.created.push(url);return url}
   URL.revokeObjectURL=url=>{state.revoked.push(url);revoke(url)}
   window.__MAP_ENCODING__=state
  })
  page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message))
  const state=()=>page.evaluate(()=>window.__ILLUSTRATED_MAP__?.state())
  const encode=mode=>page.evaluate(m=>window.__MAP_ENCODING__.mode=m,mode)
  const zoomIn=page.getByRole('button',{name:'Zoom in map view'}),zoomOut=page.getByRole('button',{name:'Zoom out map view'})
  const map=page.getByRole('region',{name:'Illustrated virtual map; drag or use arrow keys to pan'})
  const shot=async name=>page.screenshot({path:resolve(evidence,`${name}-${width}.jpg`),type:'jpeg',quality:76})
  const theme=async id=>{await page.evaluate(()=>window.__ILLUSTRATED_MAP__.navigate('/world'));await page.getByRole('button',{name:id,exact:true}).click();await expect.poll(async()=>(await state()).theme?.selected).toBe(id.toUpperCase());await page.evaluate(()=>window.__ILLUSTRATED_MAP__.navigate('/world/map'))}
  stage=`${width}/open`;await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/features/places/__tests__/reference-map/fixture.html?path=/world/map`)
  await expect(page.locator('.places-map-plaque')).toHaveCount(5);await expect(map).toBeVisible();await shot('portrait')
  const initial=await state(),bounds=await map.boundingBox();report.geometry=await page.evaluate(()=>['.world-shell','.shell-header','.shell-era-region','.shell-stage','.shell-panel','.shell-panel-body','.mcr-root-map','.places-map-full','.places-map-surface','.places-map-viewport','.shell-tabs'].map(s=>{const e=document.querySelector(s),r=e?.getBoundingClientRect(),c=e&&getComputedStyle(e);return {s,r:r?.toJSON(),display:c?.display,grid:c?.gridTemplateRows,height:c?.height,flex:c?.flex,classes:e?.className}}));assert(bounds.height>500);assert(bounds.width>=(await page.locator('.mcr-root-map').boundingBox()).width-4)
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false)
  assert.equal(await page.getByRole('img',{name:/Selected virtual location/}).count(),0)
  stage=`${width}/local`;const requests=initial.requests.length,camera=initial.camera
  await map.focus();const before=await map.evaluate(e=>e.scrollLeft);await page.keyboard.press('ArrowRight');assert((await map.evaluate(e=>e.scrollLeft))>before)
  for(let i=0;i<8;i++)await zoomIn.click();await expect(zoomIn).toBeDisabled()
  await page.getByRole('button',{name:'Fit map',exact:true}).click();await expect(zoomOut).toBeDisabled()
  const checkLabels=async()=>{const boxes=await page.locator('.places-map-plaque').evaluateAll(elements=>elements.map(e=>e.getBoundingClientRect().toJSON()));report.fitBoxes=boxes;for(let i=0;i<boxes.length;i++)for(const b of boxes.slice(i+1)){const a=boxes[i];assert(a.right<=b.left||b.right<=a.left||a.bottom<=b.top||b.bottom<=a.top,'Fit map plaque labels overlap')}};await checkLabels()
  assert.deepEqual((await state()).camera,camera);assert.equal((await state()).requests.length,requests)
  const plaques=page.locator('.places-map-plaque'),label=await plaques.first().getAttribute('aria-label'),id=await plaques.first().getAttribute('id')
  await plaques.first().focus();await expect(page.getByRole('img',{name:/Selected virtual location/})).toBeVisible()
  stage=`${width}/reset`;const fitWidth=await page.locator('.places-map-plane').evaluate(e=>e.getBoundingClientRect().width);await page.getByRole('button',{name:'Reset view',exact:true}).click();await expect(page.getByRole('img',{name:/Selected virtual location/})).toHaveCount(0);assert((await page.locator('.places-map-plane').evaluate(e=>e.getBoundingClientRect().width))>fitWidth);assert.equal((await state()).requests.length,requests);assert.deepEqual((await state()).camera,camera);await page.getByRole('button',{name:'Fit map',exact:true}).click();await plaques.first().focus();await expect(page.getByRole('img',{name:/Selected virtual location/})).toBeVisible()
  stage=`${width}/details`;await page.keyboard.press('Enter');await expect(page.getByRole('region',{name:'Place information'})).toBeVisible();await expect(page.getByRole('heading',{name:'Source and review'})).toBeVisible();await expect(page.getByText('No authorized place photo is available in this place information.',{exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Back to previous panel'}).click();await expect(page.locator(`[id="${id}"]`)).toBeFocused()
  await expect(page.getByRole('img',{name:/Selected virtual location/})).toBeVisible()
  stage=`${width}/dialogs`;await page.getByRole('button',{name:'Places',exact:true}).click();await expect(page.getByRole('dialog')).toBeVisible()
  await page.getByRole('searchbox',{name:'Find a place'}).fill('no result');await expect(page.getByText('No places match.',{exact:true})).toBeVisible()
  await page.keyboard.press('Escape');await expect(page.getByRole('button',{name:'Places',exact:true})).toBeFocused()
  await page.getByRole('button',{name:'Legend',exact:true}).click();await expect(page.getByRole('dialog')).toBeVisible();await expect(page.getByText('No GPS, travel time or safe route is inferred.',{exact:false})).toBeVisible()
  await page.getByRole('button',{name:'Close map information'}).click();await expect(page.getByRole('button',{name:'Legend',exact:true})).toBeFocused()
  if(width===390){await page.getByRole('button',{name:'Fit map',exact:true}).click();await shot('fit-selected');await page.setViewportSize({width,height:430});await expect(zoomIn).toBeInViewport();await expect(page.getByRole('button',{name:'Places',exact:true})).toBeInViewport();await expect.poll(()=>page.locator('.places-map-plane').evaluate(e=>e.getBoundingClientRect().width)).toBeGreaterThanOrEqual(bounds.width-1);await checkLabels();const zoomBox=await page.locator('.places-map-zoom').boundingBox();for(const b of await plaques.evaluateAll(es=>es.map(e=>e.getBoundingClientRect().toJSON())))assert(b.right<=zoomBox.x||zoomBox.x+zoomBox.width<=b.left||b.bottom<=zoomBox.y||zoomBox.y+zoomBox.height<=b.top,'Short fit label is obscured by zoom controls');await shot('short');await page.setViewportSize({width,height:844})}
  stage=`${width}/root-close`;await page.getByRole('button',{name:'Back to previous panel'}).click();await expect.poll(async()=>(await state()).path).toBe('/world');await page.evaluate(()=>window.__ILLUSTRATED_MAP__.navigate('/world/map'));await expect(plaques).toHaveCount(5);
  stage=`${width}/replacement`;await encode('broken');await theme('Parade');await expect(page.getByText('Illustration unavailable. Schematic and place list remain available.')).toBeVisible()
  await encode('valid');await theme('Danger');await expect(plaques).toHaveCount(5);await expect(page.getByRole('img',{name:'DANGER illustrated virtual map'})).toBeVisible()
  stage=`${width}/pending`;await encode('pending');await theme('Revenge');await expect.poll(()=>page.evaluate(()=>window.__MAP_ENCODING__.jobs.length)).toBe(1);assert.equal((await state()).overview,null);await expect(plaques).toHaveCount(0);await encode('valid');await page.evaluate(()=>window.__MAP_ENCODING__.jobs.splice(0).forEach(job=>job()));await expect(plaques).toHaveCount(5);
  // Revalidate through an explicit catalog refresh; no navigation/gesture fetch loop.
  const refresh=async mode=>{await page.evaluate(m=>window.__ILLUSTRATED_MAP__.mode(m),mode);await page.getByRole('button',{name:'Places',exact:true}).click();await page.getByRole('button',{name:'Refresh place information',exact:true}).click()}
  stage=`${width}/closed`;await refresh('closed');await expect(page.getByRole('dialog')).toHaveCount(0);await expect(plaques.first()).toHaveAttribute('data-status','closed');await plaques.first().focus();await expect(page.getByRole('img',{name:/Selected virtual location/})).toHaveCount(0)
  stage=`${width}/removed`;await refresh('removed');await expect(plaques).toHaveCount(4)
  stage=`${width}/mismatch`;await refresh('mismatch');await expect(page.getByRole('heading',{name:'Your map and places do not match'})).toBeVisible();await expect(plaques).toHaveCount(0)
  stage=`${width}/late-withdraw`;await encode('pending');await theme('Parade');await expect.poll(()=>page.evaluate(()=>window.__MAP_ENCODING__.jobs.length)).toBe(1);await page.evaluate(()=>window.__ILLUSTRATED_MAP__.account());await page.evaluate(()=>window.__MAP_ENCODING__.jobs.splice(0).forEach(job=>job()));await expect(plaques).toHaveCount(0);
  stage=`${width}/dispose`;await page.evaluate(()=>window.__ILLUSTRATED_MAP__.dispose());const end=await state();assert.equal(end.mounts,1);assert.equal(end.peak,1);assert.equal(end.games,0);assert.equal(end.activeSockets,0);assert(end.socketPeak<=1)
  const urls=await page.evaluate(()=>({created:window.__MAP_ENCODING__.created,revoked:window.__MAP_ENCODING__.revoked}));assert.deepEqual([...urls.created].sort(),[...urls.revoked].sort())
  report.cases.push({width,mapBounds:bounds,firstPlaque:label,localGesturesRequests:0,incumbentCameraUnchanged:true,mounts:end.mounts,peak:end.peak,disposals:end.disposals,createdURLs:urls.created.length,revokedURLs:urls.revoked.length});await context.close()
 }
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(error){await page?.screenshot({path:resolve(evidence,'failure.jpg'),type:'jpeg',quality:70});report.failure={stage,error:String(error),text:await page?.locator('body').innerText()};throw error}
finally{await writeFile(resolve(evidence,'native.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close()}
