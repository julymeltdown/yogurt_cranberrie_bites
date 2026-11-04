import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { resolve } from 'node:path'
import { mkdir, writeFile } from 'node:fs/promises'
const root=resolve(import.meta.dirname,'../../../../..'),out=resolve(import.meta.dirname,'scene-r3-evidence/portrait')
await mkdir(out,{recursive:true})
const server=await createServer({root,configLoader:'runner',cacheDir:'/tmp/mcr-scene-r3-map-vite',server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'})
await server.listen()
const url=`http://127.0.0.1:${server.httpServer.address().port}/src/features/places/__tests__/reference-map/scene-fixture.html`
const report={result:'FAIL',qualification:'Actual production Map/Places/controller and router/window, coherent exact candidate geometry and shared MAP producer. In-process HTTP and instrumented test world host, NOT Phaser/C05/live admission. Separate actual canonical production bootstrap must reject this PARADE-only unreviewed proposal. Legacy actual-Phaser regression recorded separately. Partial art fidelity is not acceptance.',cases:[],errors:[]}
let browser,page,stage='start'
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 for(const width of process.argv.includes('--admission-only')?[]:[360,390,430]){
  const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,reducedMotion:'reduce'})
  await context.addInitScript(()=>{
   const state={mode:'valid',jobs:[],created:[],revoked:[]},encode=HTMLCanvasElement.prototype.toBlob
   HTMLCanvasElement.prototype.toBlob=function(callback,...args){
    if(this.width!==768||this.height!==1296)return encode.call(this,callback,...args)
    const mode=state.mode
    if(mode==='broken'){callback(new Blob(['TEST ONLY bad PNG'],{type:'image/png'}));return}
    return encode.call(this,blob=>mode==='pending'?state.jobs.push(()=>callback(blob)):callback(blob),...args)
   }
   const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL)
   URL.createObjectURL=blob=>{const value=create(blob);state.created.push(value);return value}
   URL.revokeObjectURL=value=>{state.revoked.push(value);revoke(value)}
   Object.assign(window,{__SCENE_ENCODING__:state})
  })
  page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  const textures=[];page.on('request',r=>{if(/runtime-v4\/.*\.png$/.test(r.url()))textures.push(new URL(r.url()).pathname)})
  const state=()=>page.evaluate(()=>window.__SCENE_MAP__?.state())
  const encode=mode=>page.evaluate(m=>window.__SCENE_ENCODING__.mode=m,mode)
  const plaques=page.locator('.places-map-plaque'),plane=page.locator('.places-map-plane'),map=page.getByRole('region',{name:'Illustrated virtual map; drag or use arrow keys to pan'})
  const plus=page.getByRole('button',{name:'Zoom in map view'}),minus=page.getByRole('button',{name:'Zoom out map view'}),fit=page.getByRole('button',{name:'Fit map',exact:true})
  const labelsClear=async()=>{
   const labels=await plaques.evaluateAll(es=>es.map(e=>e.getBoundingClientRect().toJSON())),controls=await page.locator('.places-map-zoom').boundingBox()
   for(const [i,a] of labels.entries()){
    for(const b of labels.slice(i+1))assert(a.right<=b.left||b.right<=a.left||a.bottom<=b.top||b.bottom<=a.top,'Labels overlap')
    assert(a.right<=controls.x||controls.x+controls.width<=a.left||a.bottom<=controls.y||controls.y+controls.height<=a.top,'Zoom controls obscure a label')
   }
  }
  const shot=name=>page.screenshot({path:resolve(out,`${name}-${width}.jpg`),type:'jpeg',quality:76})
  stage=`${width}/open`;await page.goto(url);await expect(plaques).toHaveCount(4);await expect(page.locator('[data-map-presentation="reference-scene"]')).toBeVisible()
  assert.equal((await state()).geometryHash,'sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5')
  assert.equal(new Set(textures).size,2);assert.equal(textures.length,2)
  await labelsClear();await shot('portrait')
  const initial=await state(),bounds=await map.boundingBox(),requests=initial.requests.length
  assert(bounds.height>500);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false)
  stage=`${width}/pan-zoom`;await map.focus();const left=await map.evaluate(e=>e.scrollLeft);await page.keyboard.press('ArrowRight');assert((await map.evaluate(e=>e.scrollLeft))>left)
  for(let i=0;i<8;i++)await plus.click();await expect(plus).toBeDisabled();await fit.click();await expect(minus).toBeDisabled();await labelsClear()
  const fitted=await plane.boundingBox();assert(fitted.width<=bounds.width+.5&&fitted.height<=bounds.height+.5);assert(Math.abs(fitted.width/fitted.height-768/1296)<.001)
  const stageLink=page.getByRole('link',{name:'View Virtual stage',exact:true})
  await stageLink.focus();await expect(page.getByRole('img',{name:'Selected virtual location: Virtual stage'})).toBeVisible()
  const pin=page.getByRole('img',{name:'Selected virtual location: Virtual stage'});assert(Math.abs(Number.parseFloat(await pin.evaluate(e=>e.style.left))-30.5/48*100)<.001)
  assert.equal((await state()).requests.length,requests);assert.equal((await state()).writes,0)
  stage=`${width}/reset`;await page.getByRole('button',{name:'Reset view',exact:true}).click();await expect(page.getByRole('img',{name:/Selected virtual location/})).toHaveCount(0);assert((await plane.boundingBox()).width>fitted.width)
  await fit.click();await stageLink.focus();await page.keyboard.press('Enter');await expect(page.getByRole('heading',{name:'Source and review'})).toBeVisible();await expect(page.getByText('No authorized place photo is available in this place information.',{exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Back to previous panel'}).click();await expect(stageLink).toBeFocused()
  stage=`${width}/dialogs`;await page.getByRole('button',{name:'Places',exact:true}).click();await page.getByRole('searchbox',{name:'Find a place'}).fill('no result');await expect(page.getByText('No places match.',{exact:true})).toBeVisible();await page.keyboard.press('Escape');await expect(page.getByRole('button',{name:'Places',exact:true})).toBeFocused()
  await page.getByRole('button',{name:'Legend',exact:true}).click();await expect(page.getByText('No GPS, travel time or safe route is inferred.',{exact:false})).toBeVisible();await page.getByRole('button',{name:'Close map information'}).click();await expect(page.getByRole('button',{name:'Legend',exact:true})).toBeFocused()
  if(width===390){
   await fit.click();await shot('fit-selected');await page.setViewportSize({width,height:430});await expect.poll(async()=>(await map.boundingBox()).height).toBeLessThan(400);await expect(minus).toBeDisabled();await labelsClear()
   await expect.poll(async()=>{const m=await map.boundingBox(),p=await plane.boundingBox();return p.width<=m.width+.5&&p.height<=m.height+.5&&p.x>=m.x-.5&&p.y>=m.y-.5}).toBe(true);await shot('short-fit');await page.setViewportSize({width,height:844})
  }
  stage=`${width}/root-close`;await page.getByRole('button',{name:'Back to previous panel'}).click();await expect.poll(async()=>(await state()).path).toBe('/world');await page.evaluate(()=>window.__SCENE_MAP__.navigate('/world/map'));await expect(plaques).toHaveCount(4)
  stage=`${width}/bad-config`;await page.evaluate(()=>window.__SCENE_MAP__.config('mismatch'));await expect(plaques).toHaveCount(0);await page.evaluate(()=>window.__SCENE_MAP__.config('valid'));await expect(plaques).toHaveCount(4)
  stage=`${width}/broken`;await encode('broken');await page.evaluate(()=>window.__SCENE_MAP__.projection());await expect(page.getByText('Illustration unavailable. Schematic and place list remain available.')).toBeVisible()
  await encode('valid');await page.evaluate(()=>window.__SCENE_MAP__.projection());await expect(plaques).toHaveCount(4)
  stage=`${width}/pending`;await encode('pending');await page.evaluate(()=>window.__SCENE_MAP__.projection());await expect.poll(()=>page.evaluate(()=>window.__SCENE_ENCODING__.jobs.length)).toBe(1);assert.equal((await state()).overview,null);await expect(plaques).toHaveCount(0);await encode('valid');await page.evaluate(()=>window.__SCENE_ENCODING__.jobs.splice(0).forEach(job=>job()));await expect(plaques).toHaveCount(4)
  const refresh=async mode=>{await page.evaluate(m=>{window.__SCENE_MAP__.mode(m);return window.__SCENE_MAP__.refresh()},mode)}
  stage=`${width}/catalog`;await refresh('closed');await expect(plaques.first()).toHaveAttribute('data-status','closed');await plaques.first().focus();await expect(page.getByRole('img',{name:/Selected virtual location/})).toHaveCount(0);await refresh('removed');await expect(plaques).toHaveCount(3);await refresh('mismatch');await expect(page.getByRole('heading',{name:'Your map and places do not match'})).toBeVisible();await expect(plaques).toHaveCount(0)
  stage=`${width}/late-account`;await encode('pending');await page.evaluate(()=>window.__SCENE_MAP__.projection());await expect.poll(()=>page.evaluate(()=>window.__SCENE_ENCODING__.jobs.length)).toBe(1);await page.evaluate(()=>window.__SCENE_MAP__.account());await page.evaluate(()=>window.__SCENE_ENCODING__.jobs.splice(0).forEach(job=>job()));await expect(plaques).toHaveCount(0)
  stage=`${width}/dispose`;await page.evaluate(()=>window.__SCENE_MAP__.dispose());const end=await state();assert.equal(end.writes,0);assert.equal(end.stub.games,0);assert.equal(end.stub.connections,0);assert.equal(end.stub.peakGames,1);assert.equal(end.stub.peakConnections,1)
  const urls=await page.evaluate(()=>({created:window.__SCENE_ENCODING__.created,revoked:window.__SCENE_ENCODING__.revoked}));assert.deepEqual([...urls.created].sort(),[...urls.revoked].sort());assert.equal(textures.length,2)
  report.cases.push({width,mapBounds:bounds,fitBounds:fitted,textureRequests:textures.length,textureURLs:[...new Set(textures)],gestureRequests:0,cameraWrites:end.writes,createdURLs:urls.created.length,revokedURLs:urls.revoked.length,instrumentedHost:end.stub});await context.close()
 }
 stage='production-admission';page=await browser.newPage({viewport:{width:390,height:844}});let productionTextures=0;page.on('request',r=>{if(/runtime-v4\/.*\.png$/.test(r.url()))productionTextures++});page.on('pageerror',e=>report.errors.push(e.message))
 await page.goto(url.replace('scene-fixture.html','scene-admission.html'));await expect.poll(()=>page.evaluate(()=>window.__SCENE_MAP__?.state().denied)).toBe(true);const admission=await page.evaluate(()=>window.__SCENE_MAP__.state());assert.equal(admission.baselineAccepted,true);assert.equal(admission.requests.filter(p=>p.endsWith('/bootstrap')).length,2);await expect(page.locator('.places-map-plaque')).toHaveCount(0);assert.equal(productionTextures,0);report.productionAdmission={baselineAccepted:true,denied:true,denial:admission.denial,bootstrapRequests:2,textureRequests:productionTextures,qualification:'Actual canonical Auth/createProductionPorts accepts coherent incumbent baseline, then rejects partial scene candidate; no injected acceptance'};await page.evaluate(()=>window.__SCENE_MAP__.dispose())
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(error){report.failure={stage,error:String(error),text:await page?.locator('body').innerText()};await page?.screenshot({path:resolve(out,'failure.jpg'),type:'jpeg',quality:70});throw error}
finally{await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close()}
