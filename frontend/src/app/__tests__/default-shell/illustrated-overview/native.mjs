import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const frontend=resolve(import.meta.dirname,'../../../../..'), evidence=resolve(import.meta.dirname,'evidence')
await mkdir(evidence,{recursive:true})
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:'/tmp/mcr-map-overview-native-cache',server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'})
await server.listen()
const report={result:'FAIL',qualification:'Actual default router/Auth/resolver/production composition/Phaser/Places. Original scenery pixels, immutable orthogonal geometry; TEST ONLY simulated admission metadata and geometric avatar resource inherited from qualified fixture. Deterministic HTTP/C05. No approved character, live backend, physical venue, device or complete visual acceptance.',cases:[],errors:[]}
let browser,page,stage='start'
try {
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 for(const [width,height] of [[360,800],[390,844],[430,932]]) {
  const context=await browser.newContext({viewport:{width,height},hasTouch:true,reducedMotion:'reduce'})
  await context.addInitScript(()=>{
   const state={mode:'valid',jobs:[],created:[],revoked:[]}, original=HTMLCanvasElement.prototype.toBlob
   HTMLCanvasElement.prototype.toBlob=function(callback,...args){
    if(this.width!==1152||this.height!==896)return original.call(this,callback,...args)
    const mode=state.mode
    if(mode==='missing'){callback(null);return}
    if(mode==='invalid'){callback(new Blob(['invalid'],{type:'text/plain'}));return}
    if(mode==='broken'){callback(new Blob(['invalid PNG'],{type:'image/png'}));return}
    return original.call(this,blob=>mode==='pending'?state.jobs.push(()=>callback(blob)):callback(blob),...args)
   }
   const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL)
   URL.createObjectURL=blob=>{const url=create(blob);state.created.push(url);return url}
   URL.revokeObjectURL=url=>{state.revoked.push(url);revoke(url)}
   window.__ENCODING__=state
  })
  page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  const state=()=>page.evaluate(()=>window.__ILLUSTRATED_MAP__?.state())
  const encoding=mode=>page.evaluate(m=>window.__ENCODING__.mode=m,mode)
  const flush=()=>page.evaluate(()=>{const jobs=window.__ENCODING__.jobs.splice(0);jobs.forEach(job=>job())})
  const art=page.locator('.places-illustrated'), schematic=page.locator('.places-diagram:not(.places-illustrated)')
  const parity=()=>page.evaluate(async()=>{
   const snapshot=window.__ILLUSTRATED_MAP__.state().overview
   const binding=await fetch('/world/illustrated-runtime/bindings.json').then(r=>r.json())
   const s=binding.themes.find(t=>t.theme_id===snapshot.themeId).scenery
   const image=async url=>{const i=new Image();i.src=url;await i.decode();return i}
   const assets=new Map(await Promise.all([s.ground,...s.atlases].map(async t=>[t.id,await image('/'+t.path)])))
   const expected=document.createElement('canvas');expected.width=snapshot.width;expected.height=snapshot.height
   const ctx=expected.getContext('2d');ctx.imageSmoothingEnabled=false;ctx.drawImage(assets.get(s.ground.id),0,0)
   const depth=y=>y===-1?.5:1+y/(snapshot.height+1)*.9
   for(const o of [...s.objects].sort((a,b)=>depth(a.depth_y)-depth(b.depth_y))){const f=o.frame;ctx.drawImage(assets.get(o.atlas_id),f.x,f.y,f.width,f.height,o.x,o.y,o.width,o.height)}
   const actual=document.createElement('canvas');actual.width=snapshot.width;actual.height=snapshot.height;actual.getContext('2d').drawImage(await image(snapshot.imageUrl),0,0)
   const hash=async canvas=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data))).map(b=>b.toString(16).padStart(2,'0')).join('')
   return {theme:snapshot.themeId,actual:await hash(actual),expected:await hash(expected)}
  })
  const pixels=[]
  const checkPixels=async()=>{const p=await parity();assert.equal(p.actual,p.expected);pixels.push(p)}
  const shot=async name=>{await page.screenshot({path:resolve(evidence,`${name}-${width}.png`),fullPage:true});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false)}
  const theme=async name=>{await page.getByRole('button',{name,exact:true}).click();await expect.poll(async()=>(await state()).theme?.selected).toBe(name.toUpperCase());await expect.poll(async()=>(await state()).theme?.phase).toBe('ready')}
  stage=`${width}/default`;await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/app/__tests__/default-shell/illustrated-overview/fixture.html?path=/world/map`)
  await expect.poll(async()=>(await state())?.published).toBe(true)
  await expect(art).toBeVisible();await expect(page.locator('.places-map-marker')).toHaveCount(5)
  const initial=await state();assert(initial.overview?.imageUrl.startsWith('blob:'));await checkPixels();await shot('illustrated-revenge')
  for(const box of await page.locator('.places-map-marker').evaluateAll(elements=>elements.map(e=>{const r=e.getBoundingClientRect();return {width:r.width,height:r.height,left:r.left,right:r.right}}))){assert.equal(box.width,44);assert.equal(box.height,44);assert(box.left>=0&&box.right<=width)}
  stage=`${width}/keyboard`;const marker=page.locator('.places-map-marker').first();await marker.focus();await page.keyboard.press('Enter');assert.equal((await state()).camera.following,false)
  await page.getByRole('button',{name:'Recenter avatar',exact:true}).click();assert.equal((await state()).camera.following,true)
  await page.getByRole('region',{name:'Map overview'}).getByRole('button',{name:'Zoom in',exact:true}).click();await expect.poll(async()=>(await state()).camera.camera.zoom).toBe(2)
  const link=page.locator('[id^="place-"]').first(),id=await link.getAttribute('id');await link.click();await expect(page.getByRole('region',{name:'Place information'})).toBeVisible()
  await page.getByRole('button',{name:'Back to previous panel'}).click();await expect(page.locator(`[id="${id}"]`)).toBeFocused();await expect(art).toBeVisible()
  await page.getByRole('searchbox',{name:'Find a place'}).fill('no such place');await expect(page.locator('.places-list li')).toHaveCount(0);await page.getByRole('searchbox',{name:'Find a place'}).fill('')
  stage=`${width}/theme`;await theme('Parade');await expect(art.locator('img')).toHaveAttribute('alt',/^PARADE/);await expect(art).toBeVisible();await checkPixels();await shot('illustrated-parade')
  // Browser encoding failure modes retain a usable catalog and the honest schematic.
  for(const [mode,name] of [['missing','Danger'],['invalid','Revenge'],['broken','Parade']]){
   stage=`${width}/${mode}`;await encoding(mode);await theme(name);await expect(schematic).toBeVisible();await expect(art).toBeHidden();await expect(page.locator('.places-list li')).toHaveCount(5);await shot(`fallback-${mode}`)
  }
  stage=`${width}/late`;await encoding('pending');await theme('Danger');await expect.poll(()=>page.evaluate(()=>window.__ENCODING__.jobs.length)).toBe(1);await expect(schematic).toBeVisible()
  await encoding('valid');await theme('Revenge');assert.equal((await state()).overview,null);await flush();await expect(art).toBeVisible();await expect(art.locator('img')).toHaveAttribute('alt',/^REVENGE/)
  await theme('Danger');await expect(art).toBeVisible();await expect(art.locator('img')).toHaveAttribute('alt',/^DANGER/);await checkPixels()
  await page.getByRole('button',{name:'Expand panel',exact:true}).click();await page.locator('.places-view').evaluate(e=>{for(let p=e.parentElement;p;p=p.parentElement)p.scrollTop=0});await shot('illustrated-danger-expanded');await page.getByRole('button',{name:'Reduce panel',exact:true}).click()
  stage=`${width}/closed`;await page.evaluate(()=>window.__ILLUSTRATED_MAP__.mode('closed'));await page.getByRole('button',{name:'Refresh place information',exact:true}).click();await expect(page.locator('.places-map-marker').first()).toBeDisabled();await expect(page.locator('.places-map-marker').first()).toHaveText('×')
  stage=`${width}/removed`;await page.evaluate(()=>window.__ILLUSTRATED_MAP__.mode('removed'));await page.getByRole('button',{name:'Refresh place information',exact:true}).click();await expect(page.locator('.places-map-marker')).toHaveCount(4)
  stage=`${width}/mismatch`;await page.evaluate(()=>window.__ILLUSTRATED_MAP__.mode('mismatch'));await page.getByRole('button',{name:'Refresh place information',exact:true}).click();await expect(page.getByText('Your map and places do not match',{exact:true})).toBeVisible();await expect(art).toHaveCount(0);await expect(schematic).toHaveCount(0);await shot('map-mismatch')
  const stable=await state();assert.equal(stable.mounts,1);assert.equal(stable.games,1);assert.equal(stable.peak,1);assert(stable.sockets>=1);assert.equal(stable.socketPeak,1);assert.equal(stable.activeSockets,1);assert.equal(stable.canvases,1);assert.equal(stable.sameCamera,true);assert.equal(stable.geometry,initial.geometry);assert.equal(stable.room,initial.room)
  stage=`${width}/withdraw`;await encoding('pending');await theme('Parade');await expect.poll(()=>page.evaluate(()=>window.__ENCODING__.jobs.length)).toBe(1)
  await page.evaluate(()=>window.__ILLUSTRATED_MAP__.account());assert.equal((await state()).overview,undefined);await flush();await expect(art).toHaveCount(0)
  await page.evaluate(()=>window.__ILLUSTRATED_MAP__.dispose());const end=await state();assert.equal(end.games,0);assert.equal(end.activeSockets,0);assert.equal(end.canvases,0)
  const urls=await page.evaluate(()=>({created:window.__ENCODING__.created,revoked:window.__ENCODING__.revoked}));assert.deepEqual([...urls.created].sort(),[...urls.revoked].sort())
  report.cases.push({width,height,pixels,socketPeak:stable.socketPeak,socketConnections:stable.sockets,mounts:end.mounts,peak:end.peak,disposals:end.disposals,createdURLs:urls.created.length,revokedURLs:urls.revoked.length,checks:['default illustrated map','keyboard camera','zoom/recenter','Back focus','search/list','theme replacement','missing/invalid/broken encoding fallback','late/superseded encoding','closed/removed/mismatched target','withdrawal/late result','one world/connection','URL cleanup']});
  stage=`${width}/legacy-default`;await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/app/__tests__/default-shell/qualified.html?path=/world/map`);await expect(schematic).toBeVisible();await expect(art).toHaveCount(0);await expect(page.locator('.places-list li')).toHaveCount(1);await shot('legacy-schematic');await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.dispose());assert.equal(await page.locator('.shell-world-host canvas').count(),0);
  await context.close()
 }
 assert.deepEqual(report.errors,[]);report.result='PASS'
} catch(error) {
 report.failure={stage,error:String(error),state:await page?.evaluate(()=>window.__ILLUSTRATED_MAP__?.state()),text:await page?.locator('body').innerText()};await page?.screenshot({path:resolve(evidence,'failure.png')});throw error
} finally {await writeFile(resolve(evidence,'native.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close()}
