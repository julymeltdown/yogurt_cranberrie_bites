import {requireCaptureSpace,reservePort,temporaryCache,clearTemporaryCache} from './resources.mjs'
// QA06 desktop evidence only. Never a physical iOS/Android qualification.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve, extname } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { CASES, REFERENCE_SHA256, CLOCK } from '../../reference-integrated/cases.mjs'
const repo=resolve(import.meta.dirname,'../../../..'), frontend=resolve(repo,'frontend')
const out=process.env.QA06_OUTPUT
assert(out?.startsWith(resolve(repo,'docs/mcr/evidence/visual/qa06/')+'/'),'QA06_OUTPUT must be a new owned evidence subdirectory')
await requireCaptureSpace(repo);await mkdir(out,{mode:0o700})

const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:64*1024*1024}), hash=b=>createHash('sha256').update(b).digest('hex')
const target=await readFile(resolve(repo,'docs/mcr/evidence/visual/qa06/reference-target.png'));assert.equal(hash(target),REFERENCE_SHA256)
// Sparse checkout omits tracked PNGs. Serve exact HEAD Git bytes, never restore/change source originals.
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n'))
const assets=new Map(),assetReads=[]
const cacheDir=temporaryCache('matrix'),port=await reservePort()
const server=await createServer({root:frontend,configLoader:'runner',cacheDir,logLevel:'error',
 define:{'import.meta.env.VITE_MCR_APPLICATION_RELEASE':JSON.stringify({version:1,applicationVersion:'0.1.0',buildId:'a'.repeat(64),sourceRevision:'b'.repeat(40)})},
 plugins:[{name:'qa06-readonly-sparse-assets',configureServer(s){s.middlewares.use((req,res,next)=>{
 const url=new URL(req.url,'http://localhost'), path='frontend/public'+decodeURIComponent(url.pathname)
 if(!tracked.has(path)||!['.png','.jpg','.webp','.ttf'].includes(extname(path)))return next()
 try{if(!assets.has(path))assets.set(path,git('show',`HEAD:${path}`));const bytes=assets.get(path)
 if(!assetReads.some(a=>a.path===path))assetReads.push({path,sha256:hash(bytes),bytes:bytes.length})
 res.setHeader('Content-Type',extname(path)==='.ttf'?'font/ttf':`image/${extname(path).slice(1)}`);res.end(bytes)
 }catch(e){next(e)}
 })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false}})
const report={task:'QA06',status:'IN_PROGRESS',startedAt:new Date().toISOString(),candidate:git('rev-parse','HEAD').toString().trim(),
 qualification:'DESKTOP_CHROME_TOUCH_VIEWPORTS_ONLY',physicalIOS:'NOT_RUN',physicalAndroid:'NOT_RUN',softwareKeyboard:'NOT_RUN',hardwareSafeArea:'NOT_RUN',
 reference:{sha256:REFERENCE_SHA256,crops:CASES.map(({id,rect})=>({id,rect}))},clock:CLOCK,viewportHeight:844,
 sourceTrees:Object.fromEntries(['frontend/src','frontend/public','contracts'].map(p=>[p,git('rev-parse',`HEAD:${p}`).toString().trim()])),harnessHashes:{},cases:[],sourceCases:[],findings:[],assetReads,
 limits:['Synthetic HTTP/C05 and local input; no live server/multiuser/physical arrival proof','Default-loader ten-screen fixture uses test-only legacy map/theme admission and current immutable selected whole-character identity','Versioned spatial-v4 source uses explicit resolver; unchanged UNREVIEWED map7004, no product or backend revision5 admission','Desktop focus/reduced height does not prove software keyboard, mobile browser chrome or hardware safe-area behavior']}
for(const file of ['preview.tsx','preview.html','native.mjs','comparison.mjs','tsconfig.json'])report.harnessHashes[file]=hash(await readFile(resolve(import.meta.dirname,file)))
const widths=process.env.QA06_WIDTHS?process.env.QA06_WIDTHS.split(',').map(Number):[360,390,430]
const eras=process.env.QA06_ERAS?process.env.QA06_ERAS.split(','):['REVENGE','PARADE','DANGER']
let browser
const state=p=>p.evaluate(()=>window.__QA06__?.state())
async function metrics(page){return page.evaluate(()=>{
 const box=r=>({x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right})
 const controls=[...document.querySelectorAll('button,a[href],input,textarea,select,summary,[tabindex="0"]')].filter(e=>e.getClientRects().length&&getComputedStyle(e).visibility!=='hidden')
 const target=e=>e.matches('input[type=checkbox],input[type=radio]')?(e.labels?.[0]??e):e
 const label=e=>(e.getAttribute('aria-label')||e.textContent||e.getAttribute('placeholder')||e.tagName).trim().slice(0,100)
 return {viewport:{width:innerWidth,height:innerHeight,visualWidth:visualViewport?.width,visualHeight:visualViewport?.height},overflow:document.documentElement.scrollWidth>innerWidth+1,
 documentWidth:document.documentElement.scrollWidth,world:document.querySelector('.shell-world-host')?box(document.querySelector('.shell-world-host').getBoundingClientRect()):null,
 panel:document.querySelector('.shell-panel')?box(document.querySelector('.shell-panel').getBoundingClientRect()):null,
 controls:controls.map(e=>({label:label(e),tag:e.tagName,disabled:e.matches(':disabled'),rect:box(e.getBoundingClientRect()),outline:getComputedStyle(e).outlineStyle})),
 smallControls:controls.filter(e=>{const r=target(e).getBoundingClientRect();return !e.matches(':disabled')&&(r.width<43.5||r.height<43.5)}).map(e=>({label:label(e),tag:e.tagName,rect:box(target(e).getBoundingClientRect())})),
 reducedMotion:matchMedia('(prefers-reduced-motion: reduce)').matches,activeElement:document.activeElement?.tagName,
 safeArea:{cssSupported:CSS.supports('padding-bottom','env(safe-area-inset-bottom)'),meta:document.querySelector('meta[name="viewport"]')?.content,qualification:'DESKTOP_ZERO_INSET_ONLY'},
 headings:[...document.querySelectorAll('h1,h2')].map(e=>e.textContent),images:[...document.images].map(e=>({alt:e.alt,loaded:e.complete&&e.naturalWidth>0,src:e.currentSrc.startsWith('blob:')?'blob':new URL(e.currentSrc||location.href).pathname}))}
})}
async function reachability(page){
 const list=page.locator('button:visible:not(:disabled),a[href]:visible,input:visible:not(:disabled),textarea:visible:not(:disabled),select:visible:not(:disabled),summary:visible')
 const results=[]
 for(let i=0;i<await list.count();i++){
  const el=list.nth(i);await el.focus();await el.scrollIntoViewIfNeeded().catch(()=>{})
  results.push(await el.evaluate(e=>{const target=e.matches('input[type=checkbox],input[type=radio]')?(e.labels?.[0]??e):e,r=target.getBoundingClientRect(),x=r.left+r.width/2,y=r.top+r.height/2,hit=document.elementFromPoint(x,y)
   const reachable=[.15,.5,.85].some(dx=>[.15,.5,.85].some(dy=>{const px=r.left+r.width*dx,py=r.top+r.height*dy,h=document.elementFromPoint(px,py);return px>=0&&px<innerWidth&&py>=0&&py<innerHeight&&(h===target||target.contains(h))}))
   return {label:(e.getAttribute('aria-label')||e.textContent||e.tagName).trim().slice(0,100),centerVisible:x>=0&&x<innerWidth&&y>=0&&y<innerHeight,centerHit:hit===target||target.contains(hit),reachable}}))
 }
 return results
}
async function settle(page,item){
 await page.waitForFunction(()=>window.__QA06__,{},{timeout:20000})
 await expect.poll(async()=>(await state(page)).path).toBe(item.route)
 if(!item.public)await expect.poll(async()=>(await state(page)).published,{timeout:20000}).toBe(true)
 await expect(page.getByText(/^Opening (this section|your account|your profile|your avatar|settings|map|wall|parties|gifts|account controls|this page)…?$/)).toHaveCount(0,{timeout:15000})
 await page.evaluate(()=>document.fonts.ready);await page.waitForLoadState('networkidle');await page.waitForTimeout(150)
}
async function shot(page,name){await page.screenshot({path:resolve(out,name+'.png')});return name+'.png'}
try{
 await server.listen();const base=server.resolvedUrls.local[0]
 browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version();report.node=process.version
 for(const width of widths)for(const era of eras)for(const item of CASES.filter(c=>!c.public||era===eras[0])){
  const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,isMobile:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),errors=[]
  page.on('pageerror',e=>errors.push(e.message));page.setDefaultTimeout(12000);await page.clock.setFixedTime(new Date(CLOCK))
  const entry={id:item.id,route:item.route,width,era:item.public?'N/A':era,status:'IN_PROGRESS',errors}
  try{
   await page.goto(base+'e2e/visual/qa06/preview.html?'+new URLSearchParams({entry:item.route,era}));await settle(page,item)
   entry.state=await state(page);if(!item.public)assert.equal(entry.state.theme.selected,era);entry.metrics=await metrics(page);entry.screenshot=await shot(page,`${item.id}-${era.toLowerCase()}-${width}`)
   entry.reachability=await reachability(page)
   const focusable=page.locator('input:visible:not(:disabled),textarea:visible:not(:disabled),button:visible:not(:disabled),a[href]:visible').first()
   if(await focusable.count()){await focusable.focus();entry.keyboard={focusReached:await focusable.evaluate(e=>e===document.activeElement)};await page.keyboard.press('Tab');entry.keyboard.afterTab=await page.evaluate(()=>document.activeElement?.tagName)}
   if(item.id==='login'||item.id==='map'||item.id==='feed'||item.id==='customize'){
    await page.setViewportSize({width,height:420});entry.reducedHeight=await metrics(page);entry.reducedReachability=await reachability(page)
    entry.reducedScreenshot=await shot(page,`${item.id}-${era.toLowerCase()}-${width}-reduced420`)
   }
   if(entry.metrics.overflow||entry.reducedHeight?.overflow)report.findings.push({id:item.id,width,era,code:'HORIZONTAL_OVERFLOW'})
   if(entry.metrics.smallControls.length)report.findings.push({id:item.id,width,era,code:'TARGET_UNDER_44',controls:entry.metrics.smallControls})
   if(entry.reachability.some(x=>!x.reachable))report.findings.push({id:item.id,width,era,code:'CONTROL_CENTER_UNREACHABLE',controls:entry.reachability.filter(x=>!x.reachable)})
   if(errors.length)report.findings.push({id:item.id,width,era,code:'PAGE_ERROR',errors})
   entry.status=errors.length?'FAIL':'CAPTURED'
  }catch(e){entry.status='FAIL';entry.error=e.message;report.findings.push({id:item.id,width,era,code:'HARNESS_OR_PRODUCT_FAILURE',error:e.message})}
  finally{await page.evaluate(()=>window.__QA06__?.dispose()).catch(()=>{});await context.close();report.cases.push(entry);console.log(JSON.stringify({id:item.id,width,era,status:entry.status,error:entry.error}));await writeFile(resolve(out,'report.partial.json'),JSON.stringify(report,null,2)+'\n')}
 }
 for(const width of widths)for(const era of eras){
  const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,isMobile:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),errors=[]
  page.on('pageerror',e=>errors.push(e.message));await page.clock.setFixedTime(new Date(CLOCK))
  const entry={width,era,qualification:'EXPLICIT_UNREVIEWED_SPATIAL_V4_SOURCE_FIXTURE',status:'IN_PROGRESS',errors}
  try{
   await page.goto(base+`src/app/world-entry/__tests__/reference-scene-v4/preview.html?era=${era}`)
   await expect.poll(()=>page.evaluate(()=>window.__SCENE_V4_ROOT__?.state().overview?.themeId),{timeout:25000}).toBe(era)
   await expect.poll(()=>page.evaluate(()=>window.__SCENE_V4_ROOT__.inspectWorld()).then(w=>w.sprites)).toBe(6)
   entry.state=await page.evaluate(()=>window.__SCENE_V4_ROOT__.state());entry.world=await page.evaluate(()=>window.__SCENE_V4_ROOT__.inspectWorld())
   assert.equal(entry.state.map.revision,7004);assert(entry.state.originalMapProvenance.review_state==='UNREVIEWED');assert.equal(entry.world.games,1)
   entry.worldMetrics=await metrics(page);entry.worldScreenshot=await shot(page,`spatial-v4-world-${era.toLowerCase()}-${width}`)
   await page.evaluate(()=>window.__SCENE_V4_ROOT__.navigate('/world/map'));await expect(page.getByAltText(`${era} illustrated virtual map`,{exact:true})).toBeVisible()
   entry.mapMetrics=await metrics(page);entry.mapScreenshot=await shot(page,`spatial-v4-map-${era.toLowerCase()}-${width}`)
   await page.getByRole('button',{name:'Places',exact:true}).click();const search=page.getByRole('searchbox',{name:'Find a place',exact:true});await search.focus();assert(await search.evaluate(e=>e===document.activeElement));await page.setViewportSize({width,height:420})
   entry.reducedMetrics=await metrics(page);entry.reducedReachability=await reachability(page);entry.reducedScreenshot=await shot(page,`spatial-v4-map-${era.toLowerCase()}-${width}-reduced420`)
   assert.equal(entry.worldMetrics.overflow,false);assert.equal(entry.mapMetrics.overflow,false);assert.equal(entry.reducedMetrics.overflow,false)
   entry.status='CAPTURED'
  }catch(e){entry.status='FAIL';entry.error=e.message}
  finally{await page.evaluate(()=>window.__SCENE_V4_ROOT__?.dispose()).catch(()=>{});await context.close()}
  const denied=await browser.newPage({viewport:{width,height:844}})
  try{await denied.clock.setFixedTime(new Date(CLOCK));await denied.goto(base+`src/app/world-entry/__tests__/reference-scene-v4/preview.html?era=${era}&mode=production-denied`)
   await expect.poll(()=>denied.evaluate(()=>window.__SCENE_V4_ROOT__?.state().sourceOnly)).toBe(false)
   await expect(denied.getByText('The world is unavailable. You can try opening it again.',{exact:true})).toBeVisible({timeout:20000})
   entry.productionDenied=await denied.evaluate(()=>window.__SCENE_V4_ROOT__.state());assert.equal(entry.productionDenied.canvases,0);assert.equal(entry.productionDenied.sockets,0)
  }catch(e){entry.productionDeniedError=e.message}finally{await denied.evaluate(()=>window.__SCENE_V4_ROOT__?.dispose()).catch(()=>{});await denied.close()}
  report.sourceCases.push(entry);console.log(JSON.stringify({source:'spatial-v4',width,era,status:entry.status,error:entry.error,denial:entry.productionDeniedError??'PASS'}));await writeFile(resolve(out,'report.partial.json'),JSON.stringify(report,null,2)+'\n')
 }
 report.status=report.cases.some(c=>c.status==='FAIL')||report.sourceCases.some(c=>c.status==='FAIL'||c.productionDeniedError)?'OPEN_FAILURES':'DESKTOP_CAPTURED_WITH_FINDINGS'
}finally{report.finishedAt=new Date().toISOString();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await clearTemporaryCache(cacheDir)}
console.log(JSON.stringify({status:report.status,cases:report.cases.length,sourceCases:report.sourceCases.length,findings:report.findings.length}))
if(report.status==='OPEN_FAILURES')process.exitCode=1
