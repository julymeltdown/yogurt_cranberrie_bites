import assert from 'node:assert/strict'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'
import { mkdir, writeFile, readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
const frontend=resolve(import.meta.dirname,'../../../..'),evidence=resolve(import.meta.dirname,'evidence');await mkdir(evidence,{recursive:true})
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:'/tmp/mcr-illustrated-native-cache',server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'});await server.listen()
const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
const paths=['src/world/map/manifest.ts','src/world/map/resources.ts','src/world/scenes/MapScene.ts','src/world/illustrated/model.ts','src/world/illustrated/phaser.ts','src/world/theme/compatibility.ts','src/app/world-entry/resolver.ts','public/world/illustrated-runtime/bindings.json']
const report={result:'FAIL',qualification:'Actual loadMap, bounded same-origin resource reader, single incumbent Phaser/runtime/camera; original source scenery and AR02 geometry. TEST ONLY rectangular depth marker, no avatar fallback/art, canonical Auth/C03, socket/live multiplayer, devices or product acceptance.',browser:browser.version(),pins:Object.fromEntries(await Promise.all(paths.map(async p=>[p,createHash('sha256').update(await readFile(resolve(frontend,p))).digest('hex')]))),cases:[],screenshots:[],errors:[]}
const assertParity = p => { assert.equal(p.different, 0, JSON.stringify(p)) }
let page,stage='start'
try{
 for(const width of [360,390,430]){
  page=await browser.newPage({viewport:{width,height:800},deviceScaleFactor:1,reducedMotion:'reduce'});page.on('pageerror',e=>report.errors.push(e.message))
  await page.addInitScript(()=>{const urls=new Set(),create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL);URL.createObjectURL=(blob)=>{const url=create(blob);urls.add(url);return url};URL.revokeObjectURL=url=>{urls.delete(url);return revoke(url)};window.__liveImageUrls=urls})
  stage=`${width}/start`;await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/world/__tests__/illustrated/native.html`)
  await expect.poll(()=>page.evaluate(()=>Boolean(window.illustrated))).toBe(true)
  await expect(page.locator('#world canvas')).toBeInViewport({ratio:1})
  await expect(page.getByRole('button',{name:'Next era'})).toBeInViewport({ratio:1})
  const invoke=(method,...args)=>page.evaluate(async({method,args})=>window.illustrated[method](...args),{method,args})
  const shot=async name=>{const path=`${name}-${width}.png`;await page.screenshot({path:resolve(evidence,path)});report.screenshots.push(path)}
  const cases=[]
  for(const era of [0,1,2]){
   stage=`${width}/era${era}`;await invoke('select',era)
   for(let landmark=0;landmark<5;landmark++){
    await invoke('focus',landmark);const state=await invoke('state'),parity=await invoke('parity')
    assert.equal(state.canvases,1);assert.equal(state.mounts,1);assert.equal(state.skinTextures,3);assert.equal(state.textureBytes,6225920);assert.equal(state.decodedImages,3);assert.equal(state.objects,29)
    assert.equal(await page.evaluate(()=>window.__liveImageUrls.size),4)
    assertParity(parity)
    cases.push({theme:state.theme,landmark,state,parity});await shot(`${state.theme.toLowerCase()}-landmark${landmark}`)
   }
  }
  stage=`${width}/foot-depth`;await invoke('marker',148,255);const behind=await invoke('parity');await shot('marker-behind')
  await invoke('marker',148,257);const front=await invoke('parity');await shot('marker-front');assertParity(behind);assertParity(front);assert(front.magenta>behind.magenta,JSON.stringify({behind,front}))
  stage=`${width}/atomic`;const atomic=await invoke('atomic');assert.equal(atomic.before.signature,atomic.held.signature);assert.equal(atomic.before.theme,atomic.held.theme);assert.notEqual(atomic.before.theme,atomic.after.theme);assert.equal(atomic.after.skinTextures,3);assert.equal(atomic.peakTextures,6);assert.equal(atomic.peakBytes,12451840)
  stage=`${width}/failures`;const before=await invoke('state'),negative=await invoke('negatives'),after=await invoke('state');assert.equal(negative.length,5);assert.equal(before.signature,after.signature);assert.equal(after.skinTextures,3);assert.equal(await page.evaluate(()=>window.__liveImageUrls.size),4)
  stage=`${width}/cancel`;const cancelled=await invoke('cancelled');assert.equal(cancelled.before.signature,cancelled.after.signature);assert.equal(cancelled.after.skinTextures,3);assert.equal(await page.evaluate(()=>window.__liveImageUrls.size),4)
  assert(!after.requests.some(url=>/original|scene\.png|landmarks\.png|\.svg/.test(url)))
  const cleanup=await invoke('dispose');assert.deepEqual(cleanup,{canvases:0,decodedImages:0,disposals:1,sceneTextures:0});assert.equal(await page.evaluate(()=>window.__liveImageUrls.size),0)
  report.cases.push({width,cases,behind,front,atomic,negative,cancelled,cleanup});await page.close()
 }
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(error){report.failure={stage,error:String(error),text:await page?.locator('body').innerText()};await page?.screenshot({path:resolve(evidence,'failure.png')});throw error}
finally{await writeFile(resolve(evidence,'native.json'),JSON.stringify(report,null,2)+'\n');await browser.close();await server.close()}
