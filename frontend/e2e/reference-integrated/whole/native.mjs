// Developer-owned default-root assistance. No independent approval gate or live-service claim.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createServer as netServer } from 'node:net'
import { readFile, writeFile, mkdir, statfs, rm } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../..'),frontend=resolve(root,'frontend'),output=process.env.MCR_WHOLE_ROOT_OUTPUT
assert(output?.startsWith('/'),'Set MCR_WHOLE_ROOT_OUTPUT to a NEW absolute owned directory')
const floor=async()=>{const s=await statfs(root);assert(s.bavail*s.bsize>=8*1024**3,'Disk floor below 8 GiB')}
await floor();await mkdir(output,{mode:0o700})
const hash=bytes=>createHash('sha256').update(bytes).digest('hex'),selected=JSON.parse(await readFile(resolve(root,'contracts/fixtures/assets/selected-static/avatar-manifest.json'))),release=JSON.parse(await readFile(resolve(frontend,'public/world/avatars/derived-v1/static-release.json')))
const report={kind:'GENUINE_SELECTED_ROOT_FIXTURE',status:'IN_PROGRESS',qualification:'MOCK_READY_ONLY',candidate:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),clock:'2026-11-07T07:00:00.000Z',defaultProductionLoad:true,compositionOverride:false,liveServices:'NOT_RUN',devices:'NOT_RUN',fullVisualAcceptance:'NOT_RUN',sourceHashes:{},assetBytes:{},requestedWidths:process.env.QA_WIDTH?[Number(process.env.QA_WIDTH)]:[360,390,430],cases:[]}
for(const path of ['frontend/src/app/ports.ts','frontend/src/app/WorldShell.tsx','frontend/src/app/world-entry/production.ts','frontend/src/app/world-entry/resolver.ts','frontend/src/app/account-entry/owner.ts','frontend/src/world/avatar/whole-character.ts','frontend/src/world/scenes/MapScene.ts','frontend/src/features/avatar/whole-character/controller.ts','contracts/fixtures/assets/selected-static/avatar-manifest.json','frontend/e2e/reference-integrated/whole/preview.tsx','frontend/e2e/reference-integrated/whole/data.ts','frontend/e2e/reference-integrated/whole/native.mjs'])report.sourceHashes[path]=hash(await readFile(resolve(root,path)))
for(const path of [selected.source.release_path,selected.atlas.texture.path])report.assetBytes['/'+path]={sha256:hash(await readFile(resolve(frontend,'public',path)))}
const reservation=netServer();await new Promise(done=>reservation.listen(0,'127.0.0.1',done));const port=reservation.address().port;await new Promise(done=>reservation.close(done))
const server=await createServer({root:frontend,cacheDir:resolve(output,'vite-cache'),logLevel:'error',server:{host:'127.0.0.1',port,strictPort:true}})
let browser
const state=page=>page.evaluate(()=>window.__MCR_WHOLE_ROOT__?.state()),world=page=>page.evaluate(()=>window.__MCR_WHOLE_ROOT__.inspectWorld()),navigate=(page,path)=>page.evaluate(path=>window.__MCR_WHOLE_ROOT__.navigate(path),path)
const puts=s=>s.requests.filter(r=>r.method==='PUT'&&r.path.endsWith('/me/avatar'))
async function pixel(page,selector){return page.evaluate(selector=>window.__MCR_WHOLE_ROOT__.canvasPixels(selector),selector)}
const header='.mcr-chrome-header [data-whole-character-state="ready"] canvas',me='.me-identity [data-whole-character-state="ready"] canvas',preview='.mcr-whole-preview [data-whole-character-state="ready"] canvas'
try{
 await server.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 for(const width of (process.env.QA_WIDTH?[Number(process.env.QA_WIDTH)]:[360,390,430])){
  await floor();const context=await browser.newContext({viewport:{width,height:844},serviceWorkers:'block',reducedMotion:'reduce'}),page=await context.newPage(),errors=[],network=[],bodies=[],jobs=[]
  page.on('pageerror',error=>errors.push(error.message.slice(0,500)));await page.clock.setFixedTime(new Date(report.clock))
  await page.addInitScript(()=>{const original=HTMLImageElement.prototype.decode,counts={calls:0,resolved:0,rejected:0};HTMLImageElement.prototype.decode=function(...args){counts.calls++;return original.apply(this,args).then(value=>{counts.resolved++;return value},error=>{counts.rejected++;throw error})};Object.defineProperty(window,'__WHOLE_DECODE__',{value:counts})})
  const cdp=await context.newCDPSession(page);await cdp.send('Network.enable');const ids=new Map()
  cdp.on('Network.requestWillBeSent',e=>{if(!/^https?:/.test(e.request.url))return;const path=new URL(e.request.url).pathname.replace(root,'$CHECKOUT').replace('/Users/lhs/Desktop/mcr/frontend/node_modules','$DEPENDENCIES'),row={path,bytes:0,cache:false};network.push(row);ids.set(e.requestId,row)})
  cdp.on('Network.requestServedFromCache',e=>{const row=ids.get(e.requestId);if(row)row.cache=true})
  cdp.on('Network.responseReceived',e=>{const row=ids.get(e.requestId);if(row)row.cache||=!!e.response.fromDiskCache||!!e.response.fromServiceWorker})
  cdp.on('Network.loadingFinished',e=>{const row=ids.get(e.requestId);if(!row)return;row.bytes=e.encodedDataLength;if(report.assetBytes[row.path])jobs.push(cdp.send('Network.getResponseBody',{requestId:e.requestId}).then(body=>bodies.push({path:row.path,sha256:hash(Buffer.from(body.body,body.base64Encoded?'base64':'utf8'))}),error=>bodies.push({path:row.path,error:String(error.message)})))})
  const samples=[],sample=async name=>{await page.waitForLoadState('networkidle');const s=await state(page),w=await world(page),decode=await page.evaluate(()=>({...window.__WHOLE_DECODE__}));const entry={name,state:s,world:w,decode,network:{requests:network.length,transferredBytes:network.reduce((n,r)=>n+r.bytes,0),cacheHits:network.filter(r=>r.cache).length,assetRequests:network.filter(r=>r.path.startsWith('/world/avatars/')).length},loaderCalls:'NOT_INSTRUMENTED_DEFAULT_LOADER_NOT_OVERRIDDEN'};samples.push(entry);return entry}
  const result={width,status:'IN_PROGRESS',samples,errors}
  try{
   await page.goto(`http://127.0.0.1:${port}/e2e/reference-integrated/whole/preview.html`)
   await expect.poll(async()=>(await state(page))?.published,{timeout:25000}).toBe(true)
   await expect.poll(async()=>(await world(page)).sprites.length,{timeout:20000}).toBe(6)
   const initial=await state(page);assert.equal(initial.bootstrapVersion,'1.1.0');assert.equal(initial.currentRuntime,true);assert.equal(initial.compositionOverride,false);assert.equal(new Set(initial.entities.map(e=>e.avatar.config.preset_id)).size,6);assert.equal(puts(initial).length,0)
   const approved=await page.evaluate(()=>window.__MCR_WHOLE_ROOT__.approvedPixels()),initialWorld=await world(page)
   assert.deepEqual(initialWorld.sprites.map(s=>s.hash).sort(),approved.map(a=>a.hash).sort());assert(initialWorld.sprites.every(s=>s.visible&&s.frame===0))
   await expect.poll(()=>pixel(page,header)).toBe(approved[0].hash);result.approvedPixels=approved;await sample('cold-world')
   await page.screenshot({path:resolve(output,`world-before-${width}.png`)})
   await navigate(page,'/me');await expect.poll(()=>pixel(page,me)).toBe(approved[0].hash);await sample('me-first');await page.screenshot({path:resolve(output,`me-before-${width}.png`)})
   await navigate(page,'/me/avatar');await expect(page.getByRole('button',{name:'Random character',exact:true})).toBeEnabled({timeout:20000});await expect.poll(()=>pixel(page,preview)).toBe(approved[0].hash);await sample('customize-first')
   const choose=()=>page.getByRole('button',{name:`Choose ${release.characters[5].label}`,exact:true})
   await page.getByRole('button',{name:'Next characters',exact:true}).click();await choose().click();await expect.poll(()=>pixel(page,preview)).toBe(approved[5].hash);assert.equal(puts(await state(page)).length,0)
   await page.getByRole('button',{name:'Reset draft',exact:true}).click();await expect.poll(()=>pixel(page,preview)).toBe(approved[0].hash);assert.equal(puts(await state(page)).length,0)
   await choose().click();await page.screenshot({path:resolve(output,`customize-draft-${width}.png`)});await page.getByRole('button',{name:'Save character',exact:true}).click()
   await expect(page.getByText('Character saved.',{exact:true})).toBeVisible({timeout:12000});await expect.poll(async()=>(await state(page)).server.revision).toBe(8)
   const saved=await state(page),writes=puts(saved);assert.equal(writes.length,1);assert.deepEqual(writes[0].body,{expected_revision:7,config:selected.presets[5].config});assert.deepEqual(saved.server,{revision:8,config:selected.presets[5].config})
   const writeIndex=saved.requests.findIndex(r=>r.method==='PUT'&&r.path.endsWith('/me/avatar'));assert(saved.requests.slice(writeIndex+1).some(r=>r.method==='GET'&&r.path.endsWith('/me')))
   await navigate(page,'/world');await expect.poll(async()=>(await state(page)).entities?.find(e=>e.self)?.avatar.revision,{timeout:15000}).toBe(8);await expect.poll(async()=>(await world(page)).sprites.length).toBe(6);await expect.poll(()=>pixel(page,header)).toBe(approved[5].hash)
   const returned=await state(page),rendered=await world(page);assert(returned.deltaCount>=1);assert.deepEqual(returned.entities.find(e=>e.self).avatar,saved.server);assert.equal(rendered.sprites.find(s=>s.x===140&&s.y===520)?.hash,approved[5].hash);await sample('world-after-save');await page.screenshot({path:resolve(output,`world-saved-${width}.png`)})
   await navigate(page,'/me');await expect.poll(()=>pixel(page,me)).toBe(approved[5].hash);await sample('me-saved');await page.screenshot({path:resolve(output,`me-saved-${width}.png`)})
   await navigate(page,'/me/avatar');await expect.poll(()=>pixel(page,preview)).toBe(approved[5].hash);await expect(page.getByRole('button',{name:'Save character',exact:true})).toBeDisabled();assert.equal(puts(await state(page)).length,1);await sample('customize-warm')
   await navigate(page,'/world');for(const era of ['Parade','Danger','Revenge']){await page.getByRole('button',{name:era,exact:true}).click();await expect.poll(async()=>(await state(page)).theme?.selected).toBe(era.toUpperCase());await sample(`era-${era}`)}
   for(const entry of samples){assert.equal(entry.world.games,1);assert.equal(entry.world.peakGames,1);assert.equal(entry.world.totalObservedGames,1);assert.equal(entry.world.sameGame,true);assert.equal(entry.state.sameRuntime,true);assert.equal(entry.state.sameCamera,true);assert.equal(entry.state.geometry,initial.geometry);assert.equal(entry.state.socketPeak,1);assert(entry.state.activeSockets<=1);assert.equal(entry.state.canvasPeak,1)}
   result.prePrivacy=await state(page);await page.evaluate(()=>window.__MCR_WHOLE_ROOT__.privacy());await expect.poll(async()=>(await state(page)).retainedCurrent).toBe(false);await expect.poll(async()=>(await world(page)).games).toBe(0);await expect.poll(()=>page.locator('[data-whole-character-state="ready"]').count()).toBe(0)
   const cleared=await state(page);assert.equal(cleared.runtime.disposed,true);assert.equal(cleared.runtime.leases,0);assert.equal(cleared.runtime.decodedAtlases,0);assert.equal(cleared.activeSockets,0);assert.equal(cleared.canvases,0);assert.equal(cleared.privateQueries,0);result.privacy=cleared
   await Promise.all(jobs);assert(bodies.length>=2);for(const body of bodies)assert.equal(body.sha256,report.assetBytes[body.path].sha256);result.actualNetworkAssetBodies=bodies
   assert.equal(errors.length,0);result.status='PASS'
  }catch(error){result.status='FAIL';result.error=String(error.message).slice(0,1800);result.failureState=await state(page).catch(()=>null);result.failureWorld=await world(page).catch(()=>null);await page.screenshot({path:resolve(output,`failure-${width}.png`)}).catch(()=>{})}
  finally{await page.evaluate(()=>window.__MCR_WHOLE_ROOT__?.dispose()).catch(()=>{});result.cleanup=await state(page).catch(()=>null);await context.close();report.cases.push(result);await writeFile(resolve(output,'report.partial.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({width,status:result.status,error:result.error}))}
 }
 report.status=report.cases.every(c=>c.status==='PASS')?'PASS':'FAIL'
}finally{await browser?.close();await server.close();await rm(resolve(output,'vite-cache'),{recursive:true,force:true});await rm(resolve(output,'report.partial.json'),{force:true});await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n')}
console.log(JSON.stringify({status:report.status,cases:report.cases.length,output}));if(report.status!=='PASS')process.exitCode=1
