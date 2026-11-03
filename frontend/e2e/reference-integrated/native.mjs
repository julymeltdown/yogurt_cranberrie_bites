import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createServer as createNetServer } from 'node:net'
import { mkdir, readFile, writeFile, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { CASES, WIDTHS, CLOCK, REFERENCE_SHA256, viewportFor, summarizeNetwork } from './cases.mjs'
import { writeComparison } from './comparison.mjs'
const root=resolve(import.meta.dirname,'../../..'),frontend=resolve(root,'frontend')
const output=process.env.MCR_REFERENCE_INTEGRATED_OUTPUT
assert(output?.startsWith('/'),'Set MCR_REFERENCE_INTEGRATED_OUTPUT to a NEW absolute owned output directory')
const floor=async()=>{const s=await statfs(root);assert(s.bavail*s.bsize>=8*1024**3,'Disk floor below8GiB: no capture permitted')}
await floor();await mkdir(output,{mode:0o700})
const hash=b=>createHash('sha256').update(b).digest('hex'),target=await readFile(resolve(root,'docs/mcr/design/reference-target-2026-11-04.png'))
assert.equal(hash(target),REFERENCE_SHA256)
const report={kind:'mcr-reference-integrated-v1',candidate:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),clock:CLOCK,
 reference:{sha256:REFERENCE_SHA256,width:1536,height:1024,crops:CASES.map(({id,rect})=>({id,rect}))},
 evidenceStatus:'IN_PROGRESS',visualAcceptance:'NOT_RUN',liveServices:'NOT_RUN',devices:'NOT_RUN',fixtureMetadataIsApproval:false,
 sourceHashes:{},approvedBytePins:{},cases:[],warm:[],findings:[],browser:''}
for(const path of ['AGENTS.md','frontend/src/app/router.tsx','frontend/src/app/CommunityFeatures.tsx','frontend/src/app/WorldShell.tsx','frontend/src/app/shell.css','frontend/src/app/__tests__/default-shell/illustrated-overview/fixture.tsx','frontend/src/entities/avatar/character-source-pack.json','contracts/generated/mcr-core.openapi.json','contracts/generated/mcr-domain.openapi.json']){
 report.sourceHashes[path]=hash(await readFile(resolve(root,path)))
}
const reservation=createNetServer();await new Promise(done=>reservation.listen(0,'127.0.0.1',done));const port=reservation.address().port;await new Promise(done=>reservation.close(done))
const server=await createServer({root:frontend,logLevel:'error',cacheDir:resolve(output,'vite-cache'),server:{host:'127.0.0.1',port,strictPort:true}})
server.middlewares.use('/__qa_reference_target.png',(_q,r)=>{r.setHeader('Content-Type','image/png');r.end(target)})
let browser
const state=page=>page.evaluate(()=>window.__REFERENCE_INTEGRATED__?.state())
const navigate=(page,path)=>page.evaluate(p=>window.__REFERENCE_INTEGRATED__.navigate(p),path)
async function settled(page,item){
 await expect.poll(()=>state(page)).toBeTruthy()
 await expect.poll(async()=>(await state(page)).path).toBe(item.route)
 if(!item.public){await expect.poll(async()=>(await state(page)).published,{timeout:20000}).toBe(true);await expect.poll(async()=>(await state(page)).room).toBeTruthy()}
 await expect(page.getByText(/^Opening (this section|your account|your profile|your avatar|settings|map|wall|parties|gifts|account controls)…$/)).toHaveCount(0,{timeout:12000})
 await page.evaluate(()=>document.fonts.ready)
}
async function instrument(context,page){
 const records=[],byId=new Map(),errors=[];page.on('pageerror',e=>errors.push(e.message.slice(0,300)))
 await page.clock.setFixedTime(new Date(CLOCK))
 await page.addInitScript(()=>{
  const decode=HTMLImageElement.prototype.decode,metrics={calls:0,resolved:0,rejected:0}
  HTMLImageElement.prototype.decode=function(...args){metrics.calls++;return decode.apply(this,args).then(v=>{metrics.resolved++;return v},e=>{metrics.rejected++;throw e})}
  Object.defineProperty(window,'__QA_IMAGE_DECODE__',{value:metrics})
 })
 const cdp=await context.newCDPSession(page);await cdp.send('Network.enable');await cdp.send('Network.setCacheDisabled',{cacheDisabled:false})
 cdp.on('Network.requestWillBeSent',e=>{if(!/^https?:/.test(e.request.url))return;const row={path:new URL(e.request.url).pathname,type:e.type,encodedBytes:0,fromDiskCache:false,fromMemoryCache:false,fromServiceWorker:false};records.push(row);byId.set(e.requestId,row)})
 cdp.on('Network.requestServedFromCache',e=>{const r=byId.get(e.requestId);if(r)r.fromMemoryCache=true})
 cdp.on('Network.responseReceived',e=>{const r=byId.get(e.requestId);if(r){r.status=e.response.status;r.fromDiskCache=!!e.response.fromDiskCache;r.fromServiceWorker=!!e.response.fromServiceWorker;if(r.fromDiskCache)r.fromMemoryCache=false}})
 cdp.on('Network.loadingFinished',e=>{const r=byId.get(e.requestId);if(r)r.encodedBytes=e.encodedDataLength})
 return {records,errors,cdp}
}
async function measures(page){return page.evaluate(()=>{
 const box=el=>{if(!el)return null;const r=el.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height}}
 const visible=el=>el.checkVisibility()&&el.getBoundingClientRect().width>1&&el.getBoundingClientRect().height>1
 const controls=[...document.querySelectorAll('button,a,input,select,textarea,summary')].filter(visible).map(el=>{
  const measure=el.matches('input[type=checkbox],input[type=radio]')?el.closest('label')??el:el
  return {name:(el.getAttribute('aria-label')??el.textContent??'').trim().slice(0,100),tag:el.tagName,disabled:!!el.disabled,rect:box(measure)}
 })
 const select=s=>box(document.querySelector(s)),images=[...document.querySelectorAll('img,canvas')].filter(visible).map(el=>({tag:el.tagName,source:el.tagName==='IMG'?el.src.startsWith('data:')?'DATA_URL':el.src.startsWith('blob:')?'BLOB':new URL(el.src).pathname:'CANVAS',rect:box(el),complete:el.tagName==='IMG'?el.complete:null,naturalWidth:el.naturalWidth??null}))
 return {viewport:{width:innerWidth,height:innerHeight},scroll:{width:document.documentElement.scrollWidth,height:document.documentElement.scrollHeight},overflow:document.documentElement.scrollWidth>innerWidth,
 bounds:{shell:select('.world-shell'),title:select('.mcr-chrome-panel-header,.auth-titlebar,.public-entry-title'),panel:select('.shell-panel'),body:select('.shell-panel-body'),tabs:select('.party-tabs,.gift-tabs,.mcr-profile-tabs'),footer:select('.mcr-parade-footer,.meet-create,.gift-create'),world:select('.shell-world')},
 rows:[...document.querySelectorAll('.meet-party,.gift-discovery-row,.gift-row,.feed-post,.concert-row')].filter(visible).map(box),controls,smallControls:controls.filter(c=>c.rect.width<43.9||c.rect.height<43.9),images,
 decode:{...window.__QA_IMAGE_DECODE__},headings:[...document.querySelectorAll('h1,h2,h3')].filter(visible).map(el=>el.textContent),actions:controls.filter(c=>/join|create|reserve|save|enter|sign in/i.test(c.name))}
 })}
async function simpleAction(page,item){
 if(item.id==='landing'){
  const enter=page.getByRole('link',{name:/Enter the world/i});if(await enter.count()){await enter.click();return {status:'PASS',kind:'entry-route',path:(await state(page)).path}}return {status:'NOT_RUN',code:'ENTRY_CONTROL_NOT_FOUND'}
 }
 if(item.id==='login'){
  await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password')
  const before=(await state(page)).requests.filter(r=>r.path==='/api/auth/login').length
  await page.getByRole('button',{name:'Sign in with email',exact:true}).click();await expect.poll(async()=>(await state(page)).path).toBe('/world')
  assert.equal((await state(page)).requests.filter(r=>r.path==='/api/auth/login').length,before+1);return {status:'PASS',kind:'one-canonical-login'}
 }
 if(item.id==='meet'||item.id==='gifts'){
  const link=page.getByRole('link',{name:item.id==='meet'?/^View details for /:/^View .*details|^View giveaway|^View /}).first()
  if(!await link.count())return {status:'NOT_RUN',code:'DETAIL_CONTROL_NOT_FOUND'}
  await link.focus();const focus=await link.evaluate(el=>getComputedStyle(el).outlineStyle!=='none');const before=(await state(page)).requests.filter(r=>r.method!=='GET').length
  await link.press('Enter');await expect.poll(async()=>(await state(page)).path!==item.route).toBe(true)
  assert.equal((await state(page)).requests.filter(r=>r.method!=='GET').length,before);await page.evaluate(()=>window.__REFERENCE_INTEGRATED__.back())
  await expect.poll(async()=>(await state(page)).path).toBe(item.route);return {status:'PASS',kind:'detail-read-no-mutation',focus,backFocus:await link.evaluate(el=>el===document.activeElement)}
 }
 if(!item.public){
  const back=page.getByRole('button',{name:'Back to previous panel',exact:true});if(await back.count()){await back.focus();const focus=await back.evaluate(el=>getComputedStyle(el).outlineStyle!=='none');await back.press('Enter');return {status:'PASS',kind:'native-panel-back',focus,path:(await state(page)).path}}
 }
 return {status:'NOT_RUN',code:'NO_SUPPORTED_ACTION_IN_CURRENT_COMPOSITION'}
}
async function snapshotCounters(page,records){const s=await state(page);return {loaderCalls:s.loaderCalls.length,fixtureHttpCalls:s.requests.length,avatarPuts:s.requests.filter(r=>r.method==='PUT'&&r.path.endsWith('/me/avatar')).length,decode:await page.evaluate(()=>({...window.__QA_IMAGE_DECODE__})),network:summarizeNetwork(records),recordCount:records.length}}
try{
 await server.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 for(const width of WIDTHS)for(const item of CASES){
  await floor();const viewport=viewportFor(item,width),context=await browser.newContext({viewport,hasTouch:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage()
  const i=await instrument(context,page),entry={id:item.id,route:item.route,width,viewport,fixture:true,status:'IN_PROGRESS',screenshot:`${item.id}-${width}.png`}
  try{
   await page.goto(`http://127.0.0.1:${port}/e2e/reference-integrated/preview.html?path=${encodeURIComponent(item.route)}`);await settled(page,item)
   entry.measurements=await measures(page);entry.counters=await snapshotCounters(page,i.records)
   await page.screenshot({path:resolve(output,entry.screenshot),clip:{x:0,y:0,...viewport}})
   entry.action=await simpleAction(page,item);entry.errors=i.errors
   if(entry.measurements.overflow)report.findings.push({id:item.id,width,code:'HORIZONTAL_OVERFLOW'})
   if(entry.measurements.smallControls.length)report.findings.push({id:item.id,width,code:'TARGETS_UNDER_44',controls:entry.measurements.smallControls})
   if(i.errors.length)report.findings.push({id:item.id,width,code:'PAGE_ERRORS',errors:i.errors})
   entry.status='CAPTURED'
  }catch(error){entry.status='FAIL';entry.code='HARNESS_OR_COMPOSITION_ASSERTION';entry.error=String(error.message).slice(0,1200)}
  finally{report.cases.push(entry);await page.evaluate(()=>window.__REFERENCE_INTEGRATED__?.dispose()).catch(()=>{});await context.close()}
 }
 // Separate warm-navigation/era/cache experiment; never infer decode reuse from HTTP cache alone.
 for(const width of WIDTHS){
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
   report.warm.push({width,status:'PASS',steps,avatar,errors:i.errors})
  }catch(error){report.warm.push({width,status:'FAIL',steps,code:'WARM_ASSERTION',error:String(error.message).slice(0,1200)})}
  finally{await page.evaluate(()=>window.__REFERENCE_INTEGRATED__?.dispose()).catch(()=>{});await context.close()}
 }
 report.evidenceStatus=report.cases.every(c=>c.status==='CAPTURED')&&report.warm.every(c=>c.status==='PASS')?'CAPTURED':'PARTIAL'
 // Pin actual immutable public scenery/selected-art files that were requested; never synthetic bytes as approved art.
 for(const path of [...new Set(report.cases.flatMap(c=>c.measurements?.images??[]).map(x=>x.source).filter(p=>p.startsWith('/world/'))) ]){
  try{report.approvedBytePins[path]={sha256:hash(await readFile(resolve(frontend,'public',path.slice(1)))),authority:'SOURCE_BYTES_ONLY_NOT_IDENTITY_MAPPING'}}catch{report.approvedBytePins[path]={status:'UNREADABLE'}}
 }
 await writeComparison(output,report)
}finally{
 await browser?.close();await server.close();await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n',{mode:0o600})
}
console.log(JSON.stringify({evidenceStatus:report.evidenceStatus,cases:report.cases.length,warm:report.warm.length,findings:report.findings.length,output}))
if(report.evidenceStatus!=='CAPTURED')process.exitCode=1
