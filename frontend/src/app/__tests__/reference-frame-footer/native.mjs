// Bounded shared-decoration validation. Existing QA06 DEV fixture, never live-service proof.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile, statfs, rm } from 'node:fs/promises'
import { createServer as netServer } from 'node:net'
import { tmpdir } from 'node:os'
import { resolve, extname } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { CASES, CLOCK, viewportFor } from '../../../../e2e/reference-integrated/cases.mjs'

assert(process.env.MCR_FRAME_ACCEPTED_JOB, 'Fresh21d offer/explicit acceptance required before invoking this runner')
const repo=resolve(import.meta.dirname,'../../../../..'), frontend=resolve(repo,'frontend')
const output=process.env.MCR_FRAME_OUTPUT
assert(output?.startsWith(resolve(repo,'docs/mcr/evidence/reference-frame-footer/')+'/'))
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:64*1024*1024})
const hash=b=>createHash('sha256').update(b).digest('hex')
const owned=['frontend/src/shared/ui/concert-window/concert-window.css','frontend/src/app/shell.css','frontend/src/app/shell-chrome/shell-chrome.css','frontend/src/app/shell-chrome/parade-footer.tsx']
const base='6f2d289bdce8154a3aa42b981c6ee6d9f5923448'
const reference=process.env.MCR_FRAME_REFERENCE
assert(reference?.startsWith('/'),'Exact human JPEG required')
assert.equal(hash(await readFile(reference)),'2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23')
const space=async()=>{const s=await statfs(repo);assert(s.bavail*s.bsize>=8*1024**3,'8GiB disk floor')}
await space();await mkdir(output,{recursive:false,mode:0o700})
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n')),assets=new Map(),assetReads=[]
const report={task:'QA06-HUMAN-FRAME-FOOTER-01',status:'IN_PROGRESS',startedAt:new Date().toISOString(),job:process.env.MCR_FRAME_ACCEPTED_JOB,
  base,candidate:git('rev-parse','HEAD').toString().trim(),workingDiffSha256:hash(git('diff','HEAD','--',...owned)),
  sources:Object.fromEntries(await Promise.all(owned.map(async p=>[p,hash(await readFile(resolve(repo,p)))]))),
  fixture:{path:'frontend/e2e/visual/qa06/preview.tsx',sha256:hash(await readFile(resolve(frontend,'e2e/visual/qa06/preview.tsx'))),clock:CLOCK,
    qualification:'DEV_ONLY_SYNTHETIC_HTTP_C05_LOCAL_INPUT_DEFAULT_ROUTER; TEST_ONLY_LEGACY_MAP_ADMISSION',
    privacy:'fixture@example.test / fixture-password only; requests path/method only; no live account, token dump or private handles'},
  reference:{sha256:hash(await readFile(reference)),dimensions:[1280,853],aspect:'Earlier registered ten-panel aspect guide scaled to exact JPEG; no stretching/pixel oracle'},
  limits:['Desktop Chrome touch viewports only; physical iOS/Android/keyboard/safe-area NOT_RUN','Old QA06 test-map fixture, not current semantic MAP architecture acceptance','Wall unavailable guard remains truthful; not mural editor acceptance','No real multiuser/API/authority or public save proof','Single PARADE cohort; historical three-era coverage is qualified reuse, not fresh corrected three-era acceptance'],
  baseline:[],cases:[],findings:[],assetReads}
let browser,server,cacheDir
async function metrics(page){return page.evaluate(()=>{
  const r=e=>{if(!e)return null;const b=e.getBoundingClientRect();return {x:b.x,y:b.y,width:b.width,height:b.height}}
  const visible=e=>!!e.getClientRects().length&&getComputedStyle(e).visibility!=='hidden'
  const chrome=[...document.querySelectorAll('.mcr-chrome-header a,.mcr-chrome-eras button,.mcr-chrome-bottom a,.mcr-chrome-panel-header button')].filter(visible)
  const footer=document.querySelector('.mcr-chrome-parade-footer'),window=document.querySelector('.world-shell')
  return {overflow:document.documentElement.scrollWidth>innerWidth+1,viewport:{width:innerWidth,height:innerHeight},
    shell:r(window),panel:r(document.querySelector('.shell-panel')),header:r(document.querySelector('.mcr-chrome-panel-header')),body:r(document.querySelector('.shell-panel-body')),footer:r(footer),
    footerHidden:footer?.getAttribute('aria-hidden'),footerPointerEvents:footer&&getComputedStyle(footer).pointerEvents,
    smallChromeControls:chrome.filter(e=>{const b=e.getBoundingClientRect();return b.width<43.5||b.height<43.5}).map(e=>({label:e.getAttribute('aria-label')||e.textContent,rect:r(e)})),
    focused:{tag:document.activeElement?.tagName,outline:document.activeElement&&getComputedStyle(document.activeElement).outlineStyle},
    fonts:document.fonts.status}
  })}
async function start(mode){
  const reserve=netServer();await new Promise(done=>reserve.listen(0,'127.0.0.1',done));const port=reserve.address().port;await new Promise(done=>reserve.close(done))
  cacheDir=resolve(tmpdir(),`mcr-frame-footer-${process.pid}-${mode}`)
  server=await createServer({root:frontend,configLoader:'runner',cacheDir,logLevel:'error',resolve:{dedupe:['react','react-dom']},
    plugins:[{name:'frame-footer-readonly-source',enforce:'pre',load(id){const path=id.split('?')[0];const p=owned.find(p=>resolve(repo,p)===path);if(mode==='baseline'&&p)return git('show',base+':'+p).toString()},
      configureServer(s){s.middlewares.use((req,res,next)=>{const p='frontend/public'+decodeURIComponent(new URL(req.url,'http://localhost').pathname)
        if(!tracked.has(p)||!['.png','.jpg','.webp','.ttf'].includes(extname(p)))return next()
        try{if(!assets.has(p)){const b=git('show','HEAD:'+p);assets.set(p,b);assetReads.push({path:p,sha256:hash(b),bytes:b.length})}
          res.setHeader('Content-Type',extname(p)==='.ttf'?'font/ttf':extname(p)==='.jpg'?'image/jpeg':`image/${extname(p).slice(1)}`);res.end(assets.get(p))}catch(e){next(e)}
      })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false}})
  await server.listen();return server.resolvedUrls.local[0]
}
async function stop(){await server?.close();server=null;if(cacheDir)await rm(cacheDir,{recursive:true,force:true});cacheDir=null}
async function capture(url,item,width,mode){
  await space();const viewport=viewportFor(item,width),context=await browser.newContext({viewport,hasTouch:true,isMobile:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),errors=[]
  page.on('pageerror',e=>errors.push(e.message));page.setDefaultTimeout(15000)
  const entry={id:item.id,route:item.route,viewport,era:'PARADE',mode,startedAt:new Date().toISOString(),errors,status:'IN_PROGRESS'}
  try{await page.clock.setFixedTime(new Date(CLOCK));await page.goto(url+'e2e/visual/qa06/preview.html?'+new URLSearchParams({entry:item.route,era:'PARADE'}))
    await page.waitForFunction(()=>window.__QA06__);if(!item.public)await expect.poll(()=>page.evaluate(()=>window.__QA06__.state().published),{timeout:25000}).toBe(true)
    await expect(page.getByText(/^Opening (this section|your account|your profile|your avatar|settings|map|wall|parties|gifts|account controls|this page)…?$/)).toHaveCount(0)
    await page.waitForLoadState('networkidle');await page.evaluate(()=>document.fonts.ready)
    entry.state=await page.evaluate(()=>window.__QA06__.state());entry.metrics=await metrics(page)
    entry.screenshot=`${mode}-${item.id}-${width}.png`;await page.screenshot({path:resolve(output,entry.screenshot)})
    assert.deepEqual(errors,[]);assert.equal(entry.metrics.overflow,false);assert.deepEqual(entry.metrics.smallChromeControls,[])
    if(entry.metrics.footer){assert.equal(entry.metrics.footerHidden,'true');assert.equal(entry.metrics.footerPointerEvents,'none')}
    if(mode==='candidate'&&width===390&&['world','meet','me','login'].includes(item.id)){
      await page.keyboard.press('Tab');entry.focus=await page.evaluate(()=>({tag:document.activeElement?.tagName,outline:getComputedStyle(document.activeElement).outlineStyle,offset:getComputedStyle(document.activeElement).outlineOffset}))
      assert.notEqual(entry.focus.outline,'none')
      await page.emulateMedia({forcedColors:'active'});entry.forcedColors=await page.evaluate(()=>({active:matchMedia('(forced-colors: active)').matches,outline:getComputedStyle(document.activeElement).outlineStyle}))
      assert.equal(entry.forcedColors.active,true);assert.notEqual(entry.forcedColors.outline,'none');await page.emulateMedia({forcedColors:'none'})
    }
    if(mode==='candidate'&&width===390&&item.id==='world'){
      await page.evaluate(()=>{window.__frameHost=document.querySelector('[data-world-host]')})
      for(const route of ['/feed','/meet','/world'])await page.evaluate(path=>window.__QA06__.navigate(path),route)
      entry.persistence=await page.evaluate(()=>({sameHost:window.__frameHost===document.querySelector('[data-world-host]'),state:window.__QA06__.state()}))
      assert.equal(entry.persistence.sameHost,true);assert.equal(entry.persistence.state.activeSockets,1);assert.equal(entry.persistence.state.socketPeak,1)
    }
    const before=report.baseline.find(c=>c.id===item.id&&c.viewport.width===width)
    if(mode==='candidate'&&before){entry.geometryPreserved=['shell','panel','header','body','footer'].every(key=>JSON.stringify(before.metrics[key])===JSON.stringify(entry.metrics[key]));assert(entry.geometryPreserved,'Base/candidate frame geometry differs')}
    entry.status='CAPTURED_FOR_HUMAN_COMPARISON'
  }catch(e){entry.status='FAIL';entry.error=e.message;report.findings.push({id:item.id,width,mode,error:e.message})}
  finally{await page.evaluate(()=>window.__QA06__?.dispose()).catch(()=>{});entry.cleanup=await page.evaluate(()=>window.__QA06__?.state()).catch(()=>null);await context.close();entry.finishedAt=new Date().toISOString()}
  report[mode==='baseline'?'baseline':'cases'].push(entry);await writeFile(resolve(output,'report.partial.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({mode,id:item.id,width,status:entry.status,error:entry.error}))
}
try{
  browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version();report.node=process.version
  let url=await start('baseline');for(const item of CASES.filter(c=>['world','meet','gifts','me','login'].includes(c.id)))await capture(url,item,390,'baseline');await stop()
  url=await start('candidate');for(const width of [360,390,430])for(const item of CASES)await capture(url,item,width,'candidate')
  report.status=[...report.baseline,...report.cases].every(c=>c.status==='CAPTURED_FOR_HUMAN_COMPARISON')?'CAPTURED_PENDING_OWNER_VISUAL_COMPARISON':'OPEN_FAILURES'
}finally{await browser?.close();await stop();report.finishedAt=new Date().toISOString();await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n');await rm(resolve(output,'report.partial.json'),{force:true})}
if(report.status==='OPEN_FAILURES')process.exitCode=1
