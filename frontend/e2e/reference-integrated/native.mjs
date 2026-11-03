import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createServer as createNetServer } from 'node:net'
import { mkdir, readFile, writeFile, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { CASES, WIDTHS, CLOCK, REFERENCE_SHA256, viewportFor, summarizeNetwork, counterDelta } from './cases.mjs'
import { writeComparison } from './comparison.mjs'
const harnessFiles=['native.mjs','cases.mjs','comparison.mjs','preview.tsx','data.ts','preview.html','metrics.test.mjs']
const root=resolve(import.meta.dirname,'../../..'),frontend=resolve(root,'frontend')
const output=process.env.MCR_REFERENCE_INTEGRATED_OUTPUT
assert(output?.startsWith('/'),'Set MCR_REFERENCE_INTEGRATED_OUTPUT to a NEW absolute owned output directory')
const floor=async()=>{const s=await statfs(root);assert(s.bavail*s.bsize>=8*1024**3,'Disk floor below8GiB: no capture permitted')}
await floor();await mkdir(output,{mode:0o700})
const hash=b=>createHash('sha256').update(b).digest('hex'),target=await readFile(resolve(root,'docs/mcr/design/reference-target-2026-11-04.png'))
assert.equal(hash(target),REFERENCE_SHA256)
const report={kind:'mcr-reference-integrated-v1',candidate:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),clock:CLOCK,
 reference:{sha256:REFERENCE_SHA256,width:1536,height:1024,crops:CASES.map(({id,rect})=>({id,rect}))},
 productBase:execFileSync('git',['merge-base','HEAD','origin/master'],{cwd:root,encoding:'utf8'}).trim(),
 evidenceStatus:'IN_PROGRESS',visualAcceptance:'NOT_RUN',liveServices:'NOT_RUN',devices:'NOT_RUN',fixtureMetadataIsApproval:false,
 harnessHashes:{},sourceHashes:{},approvedBytePins:{},cases:[],warm:[],findings:[],browser:''}
for(const file of harnessFiles)report.harnessHashes[file]=hash(await readFile(resolve(import.meta.dirname,file)))
for(const path of ['AGENTS.md','frontend/src/app/__tests__/default-shell/qualified-data.ts','frontend/src/app/world-entry/__tests__/data.ts','contracts/fixtures/core/bootstrap-open.json','contracts/fixtures/assets/avatar-config.json','contracts/assets/v1/avatar-config.schema.json','contracts/assets/v1/avatar-manifest.schema.json','frontend/src/app/router.tsx','frontend/src/app/CommunityFeatures.tsx','frontend/src/app/WorldShell.tsx','frontend/src/app/shell.css','frontend/src/world/scenes/MapScene.ts','frontend/src/world/input/controller.ts','frontend/src/world/input/controls.css','frontend/src/entities/avatar/selected-static/release.ts','frontend/src/world/map/resource-cache.ts','frontend/src/app/__tests__/default-shell/illustrated-overview/fixture.tsx','frontend/src/entities/avatar/character-source-pack.json','contracts/generated/mcr-core.openapi.json','contracts/generated/mcr-domain.openapi.json']){
 report.sourceHashes[path]=hash(await readFile(resolve(root,path)))
}
const releasePath='world/avatars/derived-v1/static-release.json',releaseBytes=await readFile(resolve(frontend,'public',releasePath)),release=JSON.parse(releaseBytes)
report.approvedBytePins[releasePath]={sha256:hash(releaseBytes),authority:'AR03_REVIEWED_STATIC_RELEASE_NOT_AVATAR_CONFIG'}
const pack=JSON.parse(await readFile(resolve(frontend,'src/entities/avatar/character-source-pack.json'))),source=pack.files.find(f=>f.source_id==='photo_01')
for(const [path,expected] of [[release.atlas.path,release.atlas.sha256.replace('sha256:','')],[pack.base_url.slice(1)+source.file,source.sha256]]){const bytes=await readFile(resolve(frontend,'public',path));assert.equal(hash(bytes),expected);report.approvedBytePins[path]={sha256:hash(bytes),bytes:bytes.length,authority:'SELECTED_STATIC_ONLY_NOT_IDENTITY_OR_WALKING'} }
const reservation=createNetServer();await new Promise(done=>reservation.listen(0,'127.0.0.1',done));const port=reservation.address().port;await new Promise(done=>reservation.close(done))
const server=await createServer({root:frontend,logLevel:'error',cacheDir:resolve(output,'vite-cache'),server:{host:'127.0.0.1',port,strictPort:true}})
server.middlewares.use('/__qa_reference_target.png',(_q,r)=>{r.setHeader('Content-Type','image/png');r.end(target)})
let browser
const state=page=>page.evaluate(()=>window.__REFERENCE_INTEGRATED__?.state())
const navigate=(page,path)=>page.evaluate(p=>window.__REFERENCE_INTEGRATED__.navigate(p),path)
async function settled(page,item){
 await expect.poll(async()=>{const value=await state(page);if(!value){const error=await page.locator('vite-error-overlay').count();assert.equal(error,0,'Published root import failed before QA bridge initialized')}return value},{timeout:15000}).toBeTruthy()
 await expect.poll(async()=>(await state(page)).path).toBe(item.route)
 if(!item.public){await expect.poll(async()=>(await state(page)).published,{timeout:20000}).toBe(true);if(item.id==='world')await expect.poll(async()=>(await state(page)).room).toBeTruthy()}
 await expect(page.getByText(/^Opening (this section|your account|your profile|your avatar|settings|map|wall|parties|gifts|account controls)…$/)).toHaveCount(0,{timeout:12000})
 await expect(page.getByText('Opening this page',{exact:true})).toHaveCount(0,{timeout:15000})
 if(item.public)await expect(page.locator(item.id==='login'?'.auth-sign-in':'.public-poster')).toBeVisible()
 const gallery=page.locator('.selected-static-gallery');if(await gallery.count())await expect(gallery).toHaveAttribute('data-art-state','ready',{timeout:8000})
 await page.evaluate(()=>document.fonts.ready);await page.waitForLoadState('networkidle');if(item.route==='/meet')await expect(page.locator('.meet-party')).toHaveCount(3);if(item.route==='/gifts')await expect(page.locator('.gift-reference-row')).toHaveCount(3)
}
async function instrument(context,page){
 const records=[],byId=new Map(),errors=[];page.on('pageerror',e=>errors.push(e.message.slice(0,300)))
 await page.clock.setFixedTime(new Date(CLOCK))
 await page.addInitScript(()=>{
  const decode=HTMLImageElement.prototype.decode,metrics={calls:0,resolved:0,rejected:0}
  HTMLImageElement.prototype.decode=function(...args){metrics.calls++;return decode.apply(this,args).then(v=>{metrics.resolved++;return v},e=>{metrics.rejected++;throw e})}
  Object.defineProperty(window,'__QA_IMAGE_DECODE__',{value:metrics})
  const original=URL.createObjectURL,blobs=[],jobs=[]
  URL.createObjectURL=function(blob){const url=original.call(this,blob);if(blob instanceof Blob&&blob.type.startsWith('image/'))jobs.push(blob.arrayBuffer().then(bytes=>crypto.subtle.digest('SHA-256',bytes)).then(value=>blobs.push({url,sha256:[...new Uint8Array(value)].map(v=>v.toString(16).padStart(2,'0')).join(''),bytes:blob.size,type:blob.type})));return url}
  Object.defineProperty(window,'__QA_IMAGE_BLOBS__',{value:blobs});Object.defineProperty(window,'__QA_IMAGE_BLOB_JOBS__',{value:jobs})
 })
 const cdp=await context.newCDPSession(page);await cdp.send('Network.enable');await cdp.send('Network.setCacheDisabled',{cacheDisabled:false})
 cdp.on('Network.requestWillBeSent',e=>{if(!/^https?:/.test(e.request.url))return;const row={path:new URL(e.request.url).pathname.replace(root,'$CHECKOUT').replace('/Users/lhs/Desktop/mcr/frontend/node_modules','$DEPENDENCIES').replace(output,'$EVIDENCE'),type:e.type,encodedBytes:0,fromDiskCache:false,fromMemoryCache:false,fromServiceWorker:false};records.push(row);byId.set(e.requestId,row)})
 cdp.on('Network.requestServedFromCache',e=>{const r=byId.get(e.requestId);if(r)r.fromMemoryCache=true})
 cdp.on('Network.responseReceived',e=>{const r=byId.get(e.requestId);if(r){r.status=e.response.status;r.fromDiskCache=!!e.response.fromDiskCache;r.fromServiceWorker=!!e.response.fromServiceWorker;if(r.fromDiskCache)r.fromMemoryCache=false}})
 cdp.on('Network.loadingFinished',e=>{const r=byId.get(e.requestId);if(r)r.encodedBytes=e.encodedDataLength})
 return {records,errors,cdp}
}
async function measures(page){return page.evaluate(async()=>{
 await Promise.all(window.__QA_IMAGE_BLOB_JOBS__)
 const box=el=>{if(!el)return null;const r=el.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height}}
 const visible=el=>el.checkVisibility()&&el.getBoundingClientRect().width>1&&el.getBoundingClientRect().height>1
 const controls=[...document.querySelectorAll('button,a,input,select,textarea,summary')].filter(visible).map(el=>{
  const measure=el.matches('input[type=checkbox],input[type=radio]')?el.closest('label')??el:el
  return {name:(el.getAttribute('aria-label')??el.textContent??'').trim().slice(0,100),tag:el.tagName,disabled:!!el.disabled,withinViewport:el.getBoundingClientRect().bottom<=innerHeight&&el.getBoundingClientRect().top>=0,rect:box(measure)}
 })
 const select=s=>box([...document.querySelectorAll(s)].find(visible)),images=[...document.querySelectorAll('img,canvas')].filter(visible).map(el=>({tag:el.tagName,source:el.tagName==='IMG'?el.src.startsWith('data:')?'DATA_URL':el.src.startsWith('blob:')?'BLOB':new URL(el.src).pathname:'CANVAS',rect:box(el),complete:el.tagName==='IMG'?el.complete:null,naturalWidth:el.naturalWidth??null}))
 return {viewport:{width:innerWidth,height:innerHeight},scroll:{width:document.documentElement.scrollWidth,height:document.documentElement.scrollHeight},overflow:document.documentElement.scrollWidth>innerWidth,
 bounds:{frame:select('.public-entry,.auth-sign-in .concert-window,.world-shell'),shell:select('.world-shell'),header:select('.mcr-chrome-header'),eraTabs:select('.mcr-chrome-eras'),title:select('.mcr-chrome-panel-header,.concert-window__titlebar,.public-header'),panel:select('.shell-panel'),body:select('.shell-panel-body,.auth-paper-content,.public-poster'),tabs:select('.party-tabs,.gifts-reference [role=tablist],.feed-tabs,.mcr-profile-tabs'),footer:select('.mcr-chrome-parade-footer,.feed-footer,.public-entry-action'),primaryAction:select('.meet-create,.gift-reference-action,.auth-submit,.public-primary'),navigation:select('.mcr-chrome-bottom'),world:select('.shell-world')},
 rows:[...document.querySelectorAll('.meet-party,.gift-reference-row,.gift-discovery-row,.gift-row,.feed-post,.concert-row')].filter(visible).map(box),controls,smallControls:controls.filter(c=>c.rect.width<43.9||c.rect.height<43.9),images,
 cssArtwork:[...document.querySelectorAll('.selected-static-gallery span')].filter(visible).map(el=>{const bg=getComputedStyle(el).backgroundImage,blob=window.__QA_IMAGE_BLOBS__.find(b=>bg.includes(b.url));return {rect:box(el),bytes:blob?.bytes??null,sha256:blob?.sha256??null}}),
 decode:{...window.__QA_IMAGE_DECODE__},headings:[...document.querySelectorAll('h1,h2,h3')].filter(visible).map(el=>el.textContent),actions:controls.filter(c=>/join|create|reserve|save|enter|sign in/i.test(c.name))}
 })}
async function simpleAction(page,item){
 if(item.id==='world'){const map=page.getByRole('link',{name:'Map',exact:true});await map.focus();const focus=await map.evaluate(el=>getComputedStyle(el).outlineStyle!=='none');await map.press('Enter');await expect.poll(async()=>(await state(page)).path).toBe('/world/map');await page.evaluate(()=>window.__REFERENCE_INTEGRATED__.back());await expect.poll(async()=>(await state(page)).path).toBe('/world');return {status:'PASS',kind:'map-open-back',focus}}
 if(item.id==='landing'){
  const enter=page.getByRole('link',{name:'Sign in to enter',exact:true});if(await enter.count()){await enter.click();return {status:'PASS',kind:'entry-route',path:(await state(page)).path}}return {status:'NOT_RUN',code:'ENTRY_CONTROL_NOT_FOUND'}
 }
 if(item.id==='login'){
  await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password')
  const before=(await state(page)).requests.filter(r=>r.path==='/api/auth/login').length
  await page.getByRole('button',{name:'Sign in with email',exact:true}).click();await expect.poll(async()=>(await state(page)).path).toBe('/world')
  assert.equal((await state(page)).requests.filter(r=>r.path==='/api/auth/login').length,before+1);return {status:'PASS',kind:'one-canonical-login'}
 }
 if(item.id==='meet'||item.id==='gifts'){
  const link=page.getByRole('link',{name:item.id==='meet'?/^View details for /:/^Review reservation for |^View pickup for /}).first()
  if(!await link.count())return {status:'NOT_RUN',code:'DETAIL_CONTROL_NOT_FOUND'}
  await link.focus();const focus=await link.evaluate(el=>getComputedStyle(el).outlineStyle!=='none');const before=(await state(page)).requests.filter(r=>r.method!=='GET').length
  await link.press('Enter');await expect.poll(async()=>(await state(page)).path!==item.route).toBe(true)
  assert.equal((await state(page)).requests.filter(r=>r.method!=='GET').length,before);await page.evaluate(()=>window.__REFERENCE_INTEGRATED__.back())
  await expect.poll(async()=>(await state(page)).path).toBe(item.route);return {status:'PASS',kind:'detail-read-no-mutation',focus,backFocus:await link.evaluate(el=>el===document.activeElement)}
 }
 if(!item.public){
  const back=page.getByRole('button',{name:'Back to previous panel',exact:true});if(await back.count()){await back.focus();const focus=await back.evaluate(el=>getComputedStyle(el).outlineStyle!=='none');await back.press('Enter');await expect.poll(async()=>(await state(page)).path).toBe(item.id==='customize'?'/me':'/world');return {status:'PASS',kind:'native-panel-back',focus,path:(await state(page)).path}}
 }
 return {status:'NOT_RUN',code:'NO_SUPPORTED_ACTION_IN_CURRENT_COMPOSITION'}
}
async function snapshotCounters(page,records){const s=await state(page);return {singletons:{mounts:s.mounts,games:s.games,peak:s.peak,sockets:s.sockets,socketPeak:s.socketPeak,activeSockets:s.activeSockets,sameCamera:s.sameCamera,geometry:s.geometry,outfit:s.outfit},loaderCalls:s.loaderCalls.length,fixtureHttpCalls:s.requests.length,loaderByPath:Object.fromEntries([...new Set(s.loaderCalls.map(r=>r.path))].sort().map(path=>[path,s.loaderCalls.filter(r=>r.path===path).length])),fixtureHttpByPath:Object.fromEntries([...new Set(s.requests.map(r=>r.method+' '+r.path))].sort().map(path=>[path,s.requests.filter(r=>r.method+' '+r.path===path).length])),avatarPuts:s.requests.filter(r=>r.method==='PUT'&&r.path.endsWith('/me/avatar')).length,decode:await page.evaluate(()=>({...window.__QA_IMAGE_DECODE__})),network:summarizeNetwork(records),recordCount:records.length}}
try{
 await server.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 for(const width of (process.env.QA_WIDTH?[Number(process.env.QA_WIDTH)]:WIDTHS))for(const item of CASES.filter(c=>!process.env.QA_CASE||process.env.QA_CASE.split(',').includes(c.id))){
  await floor();const viewport=viewportFor(item,width),context=await browser.newContext({viewport,hasTouch:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage()
  const i=await instrument(context,page),entry={id:item.id,route:item.route,width,viewport,fixture:true,status:'IN_PROGRESS',screenshot:`${item.id}-${width}.png`}
  try{
   await page.goto(`http://127.0.0.1:${port}/e2e/reference-integrated/preview.html?path=${encodeURIComponent(item.route)}`);await settled(page,item)
   entry.measurements=await measures(page);for(const art of entry.measurements.cssArtwork){assert.equal(art.sha256,release.atlas.sha256.replace('sha256:',''));assert.equal(art.bytes,report.approvedBytePins[release.atlas.path].bytes)}entry.counters=await snapshotCounters(page,i.records)
   await page.screenshot({path:resolve(output,entry.screenshot),clip:{x:0,y:0,...viewport}})
   entry.action=await simpleAction(page,item);entry.errors=i.errors
   if(entry.measurements.overflow)report.findings.push({id:item.id,width,code:'HORIZONTAL_OVERFLOW'})
   if(entry.measurements.smallControls.length)report.findings.push({id:item.id,width,code:'TARGETS_UNDER_44',controls:entry.measurements.smallControls})
   if(i.errors.length)report.findings.push({id:item.id,width,code:'PAGE_ERRORS',errors:i.errors})
   entry.status='CAPTURED'
  }catch(error){entry.status='FAIL';entry.code=i.errors.length?'PUBLISHED_ROOT_RUNTIME_ERROR':'HARNESS_OR_COMPOSITION_ASSERTION';entry.errors=i.errors;entry.error=String(error.message).slice(0,1200)}
  finally{console.log(JSON.stringify({route:item.route,width,status:entry.status,code:entry.code}));report.cases.push(entry);await writeFile(resolve(output,'report.partial.json'),JSON.stringify(report,null,2)+'\n',{mode:0o600});await page.evaluate(()=>window.__REFERENCE_INTEGRATED__?.dispose()).catch(()=>{});await context.close()}
 }
 // Separate warm-navigation/era/cache experiment; never infer decode reuse from HTTP cache alone.
 for(const width of (process.env.QA_CASE?[]:WIDTHS)){
  await floor();const context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),i=await instrument(context,page),steps=[]
  try{
   await page.goto(`http://127.0.0.1:${port}/e2e/reference-integrated/preview.html?path=%2Fworld`);await settled(page,CASES[2]);steps.push({name:'cold-world',...(await snapshotCounters(page,i.records))})
   for(const [name,path] of [['me','/me'],['customize-first','/me/avatar'],['world-return','/world'],['customize-return','/me/avatar']]){await navigate(page,path);await settled(page,{route:path,public:false});steps.push({name,...(await snapshotCounters(page,i.records))})}
   await navigate(page,'/world');await settled(page,CASES[2])
   for(const era of ['Parade','Danger','Revenge']){await page.getByRole('button',{name:era,exact:true}).click();await expect.poll(async()=>(await state(page)).theme?.selected).toBe(era.toUpperCase());steps.push({name:`era-${era}`,...(await snapshotCounters(page,i.records))})}
   await navigate(page,'/me/avatar');await settled(page,{route:'/me/avatar',public:false})
   const before=await snapshotCounters(page,i.records),random=page.getByRole('button',{name:'Random outfit',exact:true}),save=page.getByRole('button',{name:'Save outfit',exact:true})
   const avatar={status:'NOT_RUN',code:'CURRENT_APPROVED_CANONICAL_AVATAR_AUTHORITY_UNAVAILABLE'}
   if(await random.count()&&await random.isEnabled()){
    await random.click();const drafted=await snapshotCounters(page,i.records);assert.equal(drafted.avatarPuts,before.avatarPuts);avatar.draftWrites=0
    if(await save.isEnabled()){await save.click();await expect.poll(async()=>(await snapshotCounters(page,i.records)).avatarPuts).toBe(before.avatarPuts+1);avatar.status='PASS';avatar.code='ONE_CANONICAL_SAVE_REQUEST';avatar.savePuts=1}
   }
   for(let index=1;index<steps.length;index++)steps[index].delta=counterDelta(steps[index-1],steps[index]);for(const step of steps){assert.equal(step.singletons.games,1);assert.equal(step.singletons.mounts,1);assert.equal(step.singletons.peak,1);assert(step.singletons.sockets>=1);assert.equal(step.singletons.socketPeak,1);assert.equal(step.singletons.activeSockets,['me','customize-first','customize-return'].includes(step.name)?0:1);assert.equal(step.singletons.sameCamera,true);assert.equal(step.singletons.geometry,steps[0].singletons.geometry);if(step.singletons.outfit)assert.deepEqual(step.singletons.outfit,steps[0].singletons.outfit)}assert.equal(i.errors.length,0);report.warm.push({width,status:'PASS',steps,avatar,errors:i.errors})
  }catch(error){report.warm.push({width,status:'FAIL',steps,code:'WARM_ASSERTION',error:String(error.message).slice(0,1200)})}
  finally{await page.evaluate(()=>window.__REFERENCE_INTEGRATED__?.dispose()).catch(()=>{});await context.close()}
 }
 report.evidenceStatus=report.cases.every(c=>c.status==='CAPTURED')&&report.warm.every(c=>c.status==='PASS')?'CAPTURED':'PARTIAL'
 // Pin actual immutable public scenery/selected-art files that were requested; never synthetic bytes as approved art.
 for(const path of new Set([...report.cases.flatMap(c=>c.measurements?.images??[]).map(x=>x.source),...report.cases.flatMap(c=>Object.keys(c.counters?.network.byPath??{})),...report.warm.flatMap(w=>w.steps.flatMap(step=>Object.keys(step.network.byPath)))].filter(p=>p.startsWith('/world/')))){
  try{report.approvedBytePins[path]={sha256:hash(await readFile(resolve(frontend,'public',path.slice(1)))),authority:'SOURCE_BYTES_ONLY_NOT_IDENTITY_MAPPING'}}catch{report.approvedBytePins[path]={status:'UNREADABLE'}}
 }
 report.selection={widths:process.env.QA_WIDTH?[Number(process.env.QA_WIDTH)]:WIDTHS,cases:report.cases.map(c=>c.id),full:!process.env.QA_WIDTH&&!process.env.QA_CASE};if(!report.selection.full)report.evidenceStatus='PARTIAL';await writeComparison(output,report)
}finally{
 await browser?.close();await server.close();await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n',{mode:0o600})
}
console.log(JSON.stringify({evidenceStatus:report.evidenceStatus,cases:report.cases.length,warm:report.warm.length,findings:report.findings.length,output}))
if(report.evidenceStatus!=='CAPTURED')process.exitCode=1
