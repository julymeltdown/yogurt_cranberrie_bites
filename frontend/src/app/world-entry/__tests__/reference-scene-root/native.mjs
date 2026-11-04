import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {mkdir,readFile,writeFile,statfs,rm} from 'node:fs/promises'
import {resolve} from 'node:path'
import {execFileSync} from 'node:child_process'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../../../..'),frontend=resolve(root,'frontend'),output=process.env.MCR_SCENE_ROOT_OUTPUT
assert(output?.startsWith('/'),'Set MCR_SCENE_ROOT_OUTPUT to a new absolute directory')
const disk=await statfs(root);assert(disk.bavail*disk.bsize>=8*1024**3,'8GiB disk floor');await mkdir(output,{mode:0o700})
const hash=bytes=>createHash('sha256').update(bytes).digest('hex')
const version=process.env.MCR_SCENE_VERSION==='legacy'?'legacy':'spatial',revision=version==='legacy'?2:4
const report={status:'IN_PROGRESS',version,qualification:'MOCK_READY_SOURCE_FIXTURE_ONLY',candidate:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),sourcePins:{},checks:[],limits:['Explicit nonauthoritative source resolver for positive journeys; no fabricated REVIEWED metadata','Actual default MapScene/Phaser, production runtime/composition/root/Places views','Default production resolver separately rejects current UNREVIEWED top-level source','No production/live/multiuser/device/fidelity acceptance']}
for(const path of ['src/world/runtime.ts','src/world/composition/index.ts','src/world/reference-scene/model.ts','src/world/reference-scene/overview.ts','src/world/scenes/MapScene.ts','src/features/places/ui/IllustratedMap.tsx','src/app/world-entry/production.ts','src/app/world-entry/resolver.ts','src/app/world-entry/reference-scene-overview.ts',...['data.ts','preview.tsx','native.mjs'].map(p=>'src/app/world-entry/__tests__/reference-scene-root/'+p),'public/world/reference-scene-v2/runtime/canonical-map-v2.json',...['parade-ground.png','parade-atlas.png'].map(p=>'public/world/reference-scene-v2/runtime-v4/'+p)])report.sourcePins['frontend/'+path]=hash(await readFile(resolve(frontend,path)))
for(const path of ['src/world/reference-scene/spatial-v3/model.ts','src/world/reference-scene/spatial-v3/source-inputs.json','src/world/reference-scene/spatial-v3/tile-pin.json','src/world/theme/compatibility.ts','public/world/reference-scene-v2/runtime/spatial-v3/canonical-map-v4.json','public/world/reference-scene-v2/runtime/spatial-v3/source-binding-v4.json','public/world/reference-scene-v2/quality-v6/manifest.json',...['revenge','parade','danger'].flatMap(id=>['scenery.json','layout.json','map-metadata.json','resource-inputs.json','provenance.json','atlas.png'].map(p=>`public/world/reference-scene-v2/quality-v6/${id}/${p}`)),'public/world/reference-scene-v2/quality-v6/ground.png'])report.sourcePins['frontend/'+path]=hash(await readFile(resolve(frontend,path)))
const server=await createServer({root:frontend,cacheDir:resolve(output,'vite-cache'),logLevel:'error',server:{host:'127.0.0.1',port:0,strictPort:false}})
let browser
const state=page=>page.evaluate(()=>window.__SCENE_ROOT__?.state()),world=page=>page.evaluate(()=>window.__SCENE_ROOT__.inspectWorld())
const navigate=(page,path)=>page.evaluate(path=>window.__SCENE_ROOT__.navigate(path),path)
const resources=network=>network.filter(p=>p.includes('/world/reference-scene-v2/runtime-v4/')||p.includes('/world/reference-scene-v2/runtime/canonical-map-v2.json')||p.includes('/world/reference-scene-v2/quality-v6/')||p.includes('/world/reference-scene-v2/runtime/spatial-v3/'))
try{
 await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const width of (process.env.QA_WIDTH?[Number(process.env.QA_WIDTH)]:[360,390,430])) for(const era of (version==='legacy'?['PARADE']:['REVENGE','PARADE','DANGER'])){
  const context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),errors=[],network=[]
  page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>network.push(new URL(r.url()).pathname));await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
  const result={width,era,mode:'source-resolver',status:'IN_PROGRESS',samples:[]}
  const sample=async name=>{const entry={name,state:await state(page),world:await world(page),sceneResourceRequests:resources(network).length};result.samples.push(entry);return entry}
  try{
   await page.goto(server.resolvedUrls.local[0]+'src/app/world-entry/__tests__/reference-scene-root/preview.html?version='+version+'&era='+era)
   await expect.poll(async()=>(await state(page))?.overview?.imageUrl,{timeout:25000}).toBeTruthy()
   await expect.poll(async()=>(await world(page)).sprites,{timeout:15000}).toBe(6)
   const cold=await sample('world-cold');assert.equal(cold.state.factoryCalls,1);assert.equal(cold.world.games,1);assert.equal(cold.world.referenceObjects,23)
   assert.equal(cold.state.map.mapId,'mcr_reference_scene_r3');assert.equal(cold.state.map.revision,revision);assert.equal(cold.state.overview.mapRevision,revision)
   assert.equal(cold.state.metadata.mapRevision,revision);assert.equal(cold.state.metadata.mapId,cold.state.map.mapId)
   assert.equal(cold.state.overview.mapHash,cold.state.map.contentHash);assert.equal(cold.state.overview.geometryHash,cold.state.map.geometryHash)
   assert(cold.state.originalPlaces.every(p=>p.place_kind==='VIRTUAL_CONCEPT'&&p.verification_state==='CONCEPT_ONLY'&&p.provenance.review_state==='UNREVIEWED'))
   assert.equal(cold.state.originalMapProvenance.review_state,'UNREVIEWED')
   result.pixels=await page.evaluate(()=>window.__SCENE_ROOT__.overviewPixels());assert.equal(result.pixels.width,768);assert.equal(result.pixels.height,1296);assert(result.pixels.bytes>0&&result.pixels.bytes<=4*1024**2)
   await page.screenshot({path:resolve(output,`world-${era.toLowerCase()}-${width}.png`)})
   await navigate(page,'/world/map');await expect(page.locator('[data-map-presentation="reference-scene"] img')).toBeVisible({timeout:15000});await sample('map-first')
   await expect(page.locator('.places-map-plaque')).toHaveCount(4);await page.screenshot({path:resolve(output,`map-${era.toLowerCase()}-${width}.png`)})
   const image=await page.locator('.places-map-plane img').evaluate(e=>({src:e.getAttribute('src'),width:e.naturalWidth,height:e.naturalHeight}));assert.equal(image.src,cold.state.overview.imageUrl);assert.equal(image.width,768);assert.equal(image.height,1296)
   const cameraBefore=(await state(page)).camera;await page.getByRole('button',{name:'Zoom in map view',exact:true}).click();await page.locator('.places-map-viewport').press('ArrowDown');assert.deepEqual((await state(page)).camera,cameraBefore)
   await page.locator('.places-map-plaque').filter({hasText:/stage/i}).click();await expect(page.getByText('This landmark exists in the virtual world only. Do not use it as a physical meeting destination.',{exact:true})).toBeVisible();await sample('virtual-place')
   await navigate(page,'/world');await expect.poll(async()=>(await state(page)).phase).toBe('live');await sample('world-return')
   await navigate(page,'/world/map');await expect(page.locator('[data-map-presentation="reference-scene"] img')).toBeVisible();const warm=await sample('map-warm')
   assert.equal(warm.sceneResourceRequests,cold.sceneResourceRequests);assert.equal(warm.state.overview.imageUrl,cold.state.overview.imageUrl)
   if(version==='spatial'){
    assert.equal(cold.state.overview.themeId,era);assert.equal(cold.state.metadata.themeId,era)
    await navigate(page,'/world');const other=era==='REVENGE'?'DANGER':'REVENGE'
    await page.getByRole('button',{name:new RegExp('^'+other+'$','i')}).click()
    await expect.poll(async()=>(await state(page))?.theme?.phase).toBe('error')
    const denied=await sample('unreviewed-optional-era-denied');assert.equal(denied.state.theme.selected,era)
    assert.equal(denied.state.overview.imageUrl,cold.state.overview.imageUrl);assert.equal(denied.sceneResourceRequests,cold.sceneResourceRequests)
    result.optionalEraDenial={requested:other,status:'PASS',reason:'No reviewed derived optional ThemeManifest supplied'}
    await page.getByRole('button',{name:new RegExp('^'+era+'$','i')}).click()
    await expect.poll(async()=>(await state(page))?.theme?.phase).toBe('ready')
    await navigate(page,'/world/map');await expect(page.getByAltText(`${era} illustrated virtual map`,{exact:true})).toBeVisible()
   }
   for(const entry of result.samples){assert.equal(entry.world.games,1);assert.equal(entry.world.totalGames,1);assert.equal(entry.world.peakGames,1);assert.equal(entry.world.sameGame,true);assert.equal(entry.state.sameCamera,true);assert.equal(entry.state.factoryCalls,1);assert.equal(entry.state.canvasPeak,1);assert.equal(entry.state.socketPeak,1);assert.equal(entry.state.overview.mapHash,cold.state.overview.mapHash)}
   result.sceneRequests=resources(network);assert.equal(result.sceneRequests.filter(p=>p.endsWith('.png')).length,2);assert.equal(result.sceneRequests.filter(p=>p.endsWith('.json')).length,1)
   await page.evaluate(()=>window.__SCENE_ROOT__.suspend());await expect.poll(async()=>(await world(page)).games).toBe(0);await expect.poll(()=>page.locator('[data-map-presentation="reference-scene"]').count()).toBe(0)
   const withdrawn=await state(page);assert.equal(withdrawn.published,false);assert.equal(withdrawn.activeSockets,0);assert.equal(withdrawn.canvases,0);assert.equal(withdrawn.overview,undefined)
   result.withdrawn=withdrawn;await page.evaluate(()=>window.__SCENE_ROOT__.dispose());const closed=await state(page);assert.deepEqual(new Set(closed.urls.revoked),new Set(closed.urls.created));assert(closed.sourceCaches.every(c=>c.disposed&&c.residentBytes===0&&c.entries===0));result.cleanup=closed
   assert.equal(errors.length,0);result.status='PASS'
  }catch(error){result.status='FAIL';result.error=String(error.stack);result.failureState=await state(page).catch(()=>null);result.failureWorld=await world(page).catch(()=>null);result.body=await page.locator('body').innerText();await page.screenshot({path:resolve(output,`failure-${era.toLowerCase()}-${width}.png`)}).catch(()=>{})}
  finally{result.errors=errors;await page.evaluate(()=>window.__SCENE_ROOT__?.dispose()).catch(()=>{});await context.close();report.checks.push(result);console.log(JSON.stringify({width,era,status:result.status,error:result.error?.slice(0,600)}))}
  if(result.status!=='PASS')break
 }
 if(report.checks.every(c=>c.status==='PASS')){
  const context=await browser.newContext({viewport:{width:390,height:844}}),page=await context.newPage(),network=[];page.on('request',r=>network.push(new URL(r.url()).pathname))
  try{await page.goto(server.resolvedUrls.local[0]+'src/app/world-entry/__tests__/reference-scene-root/preview.html?mode=production-denied&version='+version);await expect(page.getByText('The world is unavailable. You can try opening it again.',{exact:true})).toBeVisible({timeout:20000});const s=await state(page),w=await world(page);assert.equal(s.sourceOnly,false);assert.equal(s.published,false);assert.equal(w.games,0);assert.equal(s.activeSockets,0);assert.equal(resources(network).length,0);report.productionDenial={status:'PASS',sceneAssetRequests:0,world:w,state:s}}finally{await page.evaluate(()=>window.__SCENE_ROOT__?.dispose()).catch(()=>{});await context.close()}
 }
 report.status=report.checks.every(c=>c.status==='PASS')&&report.productionDenial?.status==='PASS'?'PASS':'FAIL'
}finally{await writeFile(resolve(output,'native.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await rm(resolve(output,'vite-cache'),{recursive:true,force:true})}
console.log(JSON.stringify({status:report.status,output}));if(report.status!=='PASS')process.exitCode=1
