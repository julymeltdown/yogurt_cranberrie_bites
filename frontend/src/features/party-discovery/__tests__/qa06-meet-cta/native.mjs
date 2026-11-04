// Bounded QA06-MEET-CTA desktop Chrome harness. Synthetic HTTP/C05, actual app/router.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {execFileSync} from 'node:child_process'
import {mkdir,readFile,writeFile,statfs,rm} from 'node:fs/promises'
import {resolve,extname} from 'node:path'
import {createServer as netServer} from 'node:net'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const repo=resolve(import.meta.dirname,'../../../../../..'),frontend=resolve(repo,'frontend')
const phase=process.argv[2]??'after';assert(['before','after'].includes(phase))
const out=resolve(repo,'docs/mcr/evidence/visual/qa06-meet-cta',phase)
const disk=await statfs(repo);assert(disk.bavail*disk.bsize>=8*2**30,'Native run requires >=8 GiB free')
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:64*1024*1024})
const hash=x=>createHash('sha256').update(x).digest('hex')
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n')),assets=new Map(),assetReads=[]
const reservation=netServer();await new Promise(r=>reservation.listen(0,'127.0.0.1',r));const port=reservation.address().port;await new Promise(r=>reservation.close(r))
const cacheDir=`/tmp/mcr-qa06-meet-cta-${process.pid}`
const server=await createServer({root:frontend,cacheDir,configLoader:'runner',logLevel:'error',plugins:[{name:'owned-readonly-sparse-assets',configureServer(s){s.middlewares.use((req,res,next)=>{
 const path='frontend/public'+decodeURIComponent(new URL(req.url,'http://localhost').pathname)
 if(!tracked.has(path)||!['.png','.jpg','.webp','.ttf'].includes(extname(path)))return next()
 try{if(!assets.has(path)){const bytes=git('show',`HEAD:${path}`);assets.set(path,bytes);assetReads.push({path,sha256:hash(bytes),bytes:bytes.length})}res.setHeader('Content-Type',extname(path)==='.ttf'?'font/ttf':`image/${extname(path).slice(1)}`);res.end(assets.get(path))}catch(e){next(e)}
 })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false}})
const report={phase,base:git('rev-parse','HEAD').toString().trim(),clock:'2026-11-07T07:00:00.000Z',diskFreeGiB:disk.bavail*disk.bsize/2**30,qualification:'DESKTOP_CHROME_SYNTHETIC_HTTP_C05_ONLY',physicalIOS:'NOT_RUN',physicalAndroid:'NOT_RUN',sourceHashes:{},cases:[],assetReads}
for(const p of ['src/features/party-discovery/ui/party-discovery.css','src/features/party-discovery/ui/PartyDiscovery.tsx','e2e/reference-integrated/data.ts','e2e/reference-integrated/whole/data.ts','src/app/WorldShell.tsx','src/app/shell.css','src/app/shell-chrome/shell-chrome.css'])report.sourceHashes[p]=hash(await readFile(resolve(frontend,p)))
for(const p of ['preview.tsx','preview.html','native.mjs'])report.sourceHashes[p]=hash(await readFile(resolve(import.meta.dirname,p)))
let browser
async function metrics(page){return page.evaluate(()=>{
 const box=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right}}
 const discovery=document.querySelector('.meet-discovery'),body=document.querySelector('.shell-panel-body'),action=document.querySelector('.meet-create__action'),copy=document.querySelector('.meet-create__unavailable'),feature=document.querySelector('.party-browse')
 const rect=box(body),within=e=>{if(!e)return null;const r=box(e);return {...r,visible:r.y>=rect.y&&r.bottom<=rect.bottom,hit:e.contains(document.elementFromPoint(r.x+r.width/2,r.y+r.height/2))}}
 return {discovery:discovery?{...box(discovery),scrollTop:discovery.scrollTop,scrollHeight:discovery.scrollHeight,clientHeight:discovery.clientHeight}:null,controls:[...feature.querySelectorAll('button,a,summary,input,select,textarea')].filter(e=>e.getClientRects().length).map(e=>({label:e.getAttribute('aria-label')??e.textContent.trim(),...box(e.matches('input[type=checkbox]')?e.labels[0]:e)})),body:{...rect,scrollTop:body.scrollTop,scrollHeight:body.scrollHeight,clientHeight:body.clientHeight},feature:box(feature),action:within(action),copy:within(copy),disabled:action.matches(':disabled'),rows:[...document.querySelectorAll('.meet-party')].map(box),overflow:document.documentElement.scrollWidth>innerWidth,footer:box(document.querySelector('.mcr-chrome-parade-footer')),state:window.__QA06__.state()}
 })}
try{
 await mkdir(out,{recursive:true});await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version();report.node=process.version
 const sizes=phase==='before'?[[390,637]]:[[390,637],[360,844],[390,844],[430,844],[360,420],[390,420],[430,420]]
 for(const [width,height] of sizes){
  const context=await browser.newContext({viewport:{width,height},isMobile:true,hasTouch:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),errors=[]
  page.on('pageerror',e=>errors.push(e.message));page.setDefaultTimeout(15000);await page.clock.setFixedTime(new Date(report.clock))
  try{
   await page.goto(`http://127.0.0.1:${port}/src/features/party-discovery/__tests__/qa06-meet-cta/preview.html?entry=%2Fmeet&era=REVENGE`)
   await page.waitForFunction(()=>window.__QA06__?.state().published);await expect(page.locator('.meet-party')).toHaveCount(3);await page.evaluate(()=>document.fonts.ready);await page.waitForLoadState('networkidle');await page.waitForTimeout(150)
   const initial=await metrics(page);await page.screenshot({path:resolve(out,`meet-${width}x${height}.png`)})
   const entry={width,height,initial,errors};report.cases.push(entry);console.log(JSON.stringify({width,height,action:initial.action,copy:initial.copy,scrollTop:initial.body.scrollTop,overflow:initial.overflow}))
   assert.equal(initial.body.scrollTop,0);assert.equal(initial.disabled,true);assert.equal(initial.overflow,false);assert.equal(initial.state.canvases,1);assert(initial.state.socketPeak<=1)
   if(phase==='before'){assert.equal(initial.action.visible,false);continue}
   assert(initial.action.visible&&initial.copy.visible,'Entire action and copy must be initially visible')
   assert(initial.action.width>=44&&initial.action.height>=44)
   assert(initial.controls.every(c=>c.width>=43.5&&c.height>=43.5),'Feature controls must meet 44px targets')
   if(initial.discovery.scrollHeight>initial.discovery.clientHeight){const cdp=await context.newCDPSession(page);await cdp.send('Input.synthesizeScrollGesture',{x:Math.round(initial.discovery.x+initial.discovery.width/2),y:Math.round(initial.discovery.y+initial.discovery.height/2),yDistance:-200,gestureSourceType:'touch',speed:600});await cdp.detach();entry.afterTouchScroll=await metrics(page);assert(entry.afterTouchScroll.discovery.scrollTop>0);assert(entry.afterTouchScroll.action.visible&&entry.afterTouchScroll.copy.visible)}
   await page.locator('.party-filters summary').tap();await expect(page.getByLabel('Ticket section',{exact:true})).toBeVisible()
   entry.expanded=await metrics(page);assert.equal(entry.expanded.overflow,false);assert(entry.expanded.controls.every(c=>c.width>=43.5&&c.height>=43.5),'Expanded controls must meet 44px targets');await page.getByLabel('Ticket section',{exact:true}).fill('Area A');await page.getByRole('button',{name:'Show parties',exact:true}).tap();await expect.poll(()=>page.evaluate(()=>window.__QA06__.state().requests.some(r=>r.path.endsWith('/parties')))).toBe(true)
   await page.locator('.party-filters summary').tap()
   await page.locator('#party-people').tap();await expect(page.getByRole('heading',{name:'People',exact:true}).first()).toBeVisible();await page.goBack();await expect(page.locator('.meet-party')).toHaveCount(3)
   await page.getByRole('button',{name:'MY PLAN',exact:true}).tap();await expect(page.getByRole('button',{name:'MY PLAN',exact:true})).toHaveAttribute('aria-pressed','true');await page.getByRole('button',{name:'Parties',exact:true}).tap()
   const join=page.locator('.meet-party__join').last();await join.scrollIntoViewIfNeeded();const bounds=await join.boundingBox();assert(bounds.width>=44&&bounds.height>=44);await join.tap();await expect.poll(()=>page.evaluate(()=>window.__QA06__.state().path)).toMatch(/^\/meet\/parties\//);await page.goBack();await expect(page.locator('.meet-party')).toHaveCount(3)
   entry.final=await metrics(page);assert.equal(entry.final.disabled,true);assert.equal(entry.final.overflow,false);assert(entry.final.state.socketPeak<=1);assert.equal(errors.length,0);entry.interactions='PASS: filter touch/input/submit, People + browser Back, MY PLAN + Parties, last JOIN + browser Back; disabled creation preserved'
  }finally{await page.evaluate(()=>window.__QA06__?.dispose()).catch(()=>{});await context.close()}
 }
 report.status='PASS'
}catch(e){report.status='FAIL';report.error=e.stack;throw e}
finally{await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await rm(cacheDir,{recursive:true,force:true})}
