// QA06 original project: current editable-panel resize/input qualification, desktop only.
// Reuses the unmodified community-adoption default-App fixture; no app/source overlays.
import assert from 'node:assert/strict'
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {readFile,writeFile} from 'node:fs/promises'
import {resolve,extname} from 'node:path'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const repo=resolve(import.meta.dirname,'../../../..'), frontend=resolve(repo,'frontend')
const out=process.env.QA06_REFLOW_OUTPUT,temp=process.env.QA06_REFLOW_TEMP
const job=process.env.QA06_REFLOW_JOB,pin=process.env.QA06_REFLOW_CANDIDATE,port=Number(process.env.QA06_REFLOW_PORT)
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:32*1024**2})
const hash=b=>createHash('sha256').update(b).digest('hex')
assert.equal(process.version,'v24.22.2')
assert(job&&/^[a-f0-9]{40}$/.test(pin??'')&&out?.startsWith(repo+'/docs/mcr/evidence/visual/qa06/current-reflow/'))
assert(temp?.startsWith('/private/tmp/mcr-qa06-reflow-')&&port>1024)
assert.equal(git('rev-parse','HEAD').toString().trim(),pin)
git('diff','--quiet','HEAD','--','frontend/src','frontend/public','contracts','frontend/package.json','frontend/pnpm-lock.yaml')
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n')),assets=new Map()
const fixture='/src/app/community-adoption/__tests__/native/preview.html'
const origin=`http://127.0.0.1:${port}`
const report={task:'QA06',job,candidate:pin,startedAt:new Date().toISOString(),status:'IN_PROGRESS',
 qualification:'DESKTOP_CHROME_CURRENT_EDITABLE_PANEL_RESIZE_ONLY',
 physicalIOS:'NOT_RUN',physicalAndroid:'NOT_RUN',realSoftwareKeyboard:'NOT_RUN',hardwareSafeArea:'NOT_RUN',
 fixture:{path:fixture,sha256:hash(await readFile(resolve(frontend,fixture.slice(1))))},
 runnerSha256:hash(await readFile(new URL(import.meta.url))),reference:{path:'docs/mcr/design/reference-target-2026-11-04.png',sha256:hash(await readFile(resolve(repo,'docs/mcr/design/reference-target-2026-11-04.png')))},
 sourceReadset:{},assetReadset:{},cases:[],errors:[],limits:[
 'Unmodified existing actual Root/Auth/router/providers/Phaser with explicit synthetic HTTP/C05/clock fixture, not live service/physical arrival.',
 'PARADE-only six editable-panel cases at360/390/430; unchanged ART06 three-skin/width scene and shared-chrome list evidence reused separately.',
 'Viewport resize420 is NOT an actual mobile software keyboard or hardware safe-area check.',
 'No form qualification for an unavailable/disabled guard. No provider/age/assets/admission fabricated by this driver.',
 'Draft input only; no submit/save/upload/stock mutation. Primary action check is reachability/geometry, not committed-write acceptance.' ]}
let server,browser
const state=p=>p.evaluate(()=>window.__COMMUNITY_AUTHORITY_NATIVE__.state())
const checkpoint=()=>writeFile(resolve(out,'report.partial.json'),JSON.stringify(report,null,2)+'\n')
async function hit(locator){
 await locator.scrollIntoViewIfNeeded()
 const r=await locator.boundingBox();assert(r&&r.width>=43.5&&r.height>=43.5,'44px target')
 assert(await locator.evaluate(e=>{const r=e.getBoundingClientRect(),h=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return h===e||e.contains(h)}),'Visible target center reachable')
 return r
}
async function metrics(page){return page.evaluate(()=>{
 const shell=document.querySelector('.world-shell'), panel=document.querySelector('.shell-panel'), body=document.querySelector('.shell-panel-body'),s=getComputedStyle(shell)
 const box=e=>e?.getBoundingClientRect().toJSON()??null
 return {width:innerWidth,height:innerHeight,visualHeight:visualViewport?.height,viewportVariable:parseFloat(shell.style.getPropertyValue('--shell-viewport-height')),overflow:document.documentElement.scrollWidth>innerWidth+1,
 shell:box(shell),panel:box(panel),body:box(body),active:{tag:document.activeElement?.tagName,id:document.activeElement?.id},
 resolvedShellPadding:{top:s.paddingTop,right:s.paddingRight,bottom:s.paddingBottom,left:s.paddingLeft}}
})}
async function shot(page,name){
 // The first diagnostic captured Profile360 before its incorrect Back assertion.
 // Reuse those explicitly labelled same-source captures; the entire job stays <=12 PNGs.
 if(out.endsWith('/round-02')&&name.startsWith('profile-360-')){
  const previous=JSON.parse(await readFile(resolve(out,'../round-01/report.json'),'utf8'))
  assert.equal(previous.candidate,pin);assert.equal(previous.job,job)
  const b=await readFile(resolve(out,'../round-01',name))
  return {file:'../round-01/'+name,sha256:hash(b),bytes:b.length,reusedCapture:true,
   limit:'Prior same-candidate resize capture before owned Back assertion repair; current round re-executes geometry/input checks without another PNG.'}
 }
 const b=await page.screenshot({path:resolve(out,name),fullPage:false});return {file:name,sha256:hash(b),bytes:b.length}
}
try{
 server=await createServer({root:frontend,configLoader:'runner',cacheDir:resolve(temp,'vite'),logLevel:'error',resolve:{dedupe:['react','react-dom']},
 plugins:[{name:'qa06-source-bound-readonly',async transform(_code,id){const path=id.split('?')[0];if(path.startsWith(frontend+'/src/')&&['.ts','.tsx','.css','.json'].includes(extname(path)))report.sourceReadset[path.slice(repo.length+1)]=hash(await readFile(path))},configureServer(s){s.middlewares.use((req,res,next)=>{
 const p='frontend/public'+decodeURIComponent(new URL(req.url,origin).pathname)
 if(!tracked.has(p))return next()
 try{if(!assets.has(p))assets.set(p,git('show',pin+':'+p));const b=assets.get(p);report.assetReadset[p]={sha256:hash(b),bytes:b.length}
 const types={'.png':'image/png','.svg':'image/svg+xml','.json':'application/json','.ttf':'font/ttf','.woff2':'font/woff2','.jpg':'image/jpeg','.jpeg':'image/jpeg','.webp':'image/webp'}
 res.setHeader('Content-Type',types[extname(p)]??'application/octet-stream');res.end(b)}catch(e){next(e)}
 })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false}})
 await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 cases:for(const width of [360,390,430])for(const route of ['/me/profile','/gifts/new']){
  const c={width,era:'PARADE',route,status:'IN_PROGRESS',formQualification:'NOT_RUN',errors:[]};const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,isMobile:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage()
  page.setDefaultTimeout(12000);page.on('pageerror',e=>c.errors.push(e.message))
  try{
   await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
   await page.goto(origin+fixture+'?'+new URLSearchParams({entry:'/world',era:'PARADE',scenario:'enabled'}))
   await expect.poll(async()=>(await state(page)).published,{timeout:25000}).toBe(true)
   await page.evaluate(()=>window.__QA06_INITIAL_CANVAS__=document.querySelector('.shell-world-host canvas'))
   await page.evaluate(path=>window.__COMMUNITY_AUTHORITY_NATIVE__.navigate(path),route)
   await expect.poll(async()=>(await state(page)).path).toBe(route)
   await expect(page.locator('.shell-panel')).toBeVisible()
   const field=page.getByRole('textbox',{name:route==='/me/profile'?'Display name':'Item title',exact:true})
   await expect.poll(async()=>await field.count()===1&&await field.isVisible()&&await field.isEnabled(),{timeout:18000}).toBe(true)
   await page.evaluate(()=>document.fonts.ready)
   const before=await state(page);c.initialMutations=before.requests.filter(r=>r.method!=='GET').length
   await field.scrollIntoViewIfNeeded();await field.fill('Viewport draft');await field.focus();await expect(field).toBeFocused()
   await page.setViewportSize({width,height:420})
   await expect.poll(()=>page.evaluate(()=>parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height')))).toBe(420)
   await hit(field);await expect(field).toHaveValue('Viewport draft');await expect(field).toBeFocused()
   c.reduced=await metrics(page);assert(!c.reduced.overflow,'No horizontal page overflow');assert(c.reduced.panel.y>=-1&&c.reduced.panel.bottom<=421,'Panel stays inside viewport')
   c.reducedScreenshot=await shot(page,route==='/me/profile'?`profile-${width}-420.png`:`giveaway-${width}-420.png`)
   const action=page.locator(route==='/me/profile'?'.me-profile-form button[type=submit]':'.giveaway-authoring button[type=submit]').first()
   c.primary={rect:await hit(action),disabled:await action.isDisabled(),label:await action.innerText()}
   await page.setViewportSize({width,height:844})
   await expect.poll(()=>page.evaluate(()=>parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height')))).toBe(844)
   await field.scrollIntoViewIfNeeded();await expect(field).toHaveValue('Viewport draft')
   c.restored=await metrics(page);assert(!c.restored.overflow)
   c.restoredScreenshot=await shot(page,route==='/me/profile'?`profile-${width}-844.png`:`giveaway-${width}-844.png`)
   c.sameCanvas=await page.evaluate(()=>window.__QA06_INITIAL_CANVAS__===document.querySelector('.shell-world-host canvas'));assert(c.sameCanvas)
   c.state=await state(page);assert(c.state.canvases===1&&c.state.socketPeak<=1)
   assert.equal(c.state.requests.filter(r=>r.method!=='GET').length,c.initialMutations,'No draft interaction issues server mutation')
   await page.getByRole('button',{name:'Back to previous panel',exact:true}).click()
   c.expectedParent=route==='/me/profile'?'/me':'/gifts'
   await expect.poll(async()=>(await state(page)).path).toBe(c.expectedParent)
   c.parentReturnFocus=await page.evaluate(()=>({tag:document.activeElement?.tagName,id:document.activeElement?.id}))
   await page.getByRole('button',{name:'Back to previous panel',exact:true}).click()
   await expect.poll(async()=>(await state(page)).path).toBe('/world')
   c.returnState=await state(page);assert(c.returnState.canvases===1&&c.returnState.socketPeak<=1)
   c.returnFocus=await page.evaluate(()=>({tag:document.activeElement?.tagName,id:document.activeElement?.id}))
   c.formQualification='DESKTOP_RESIZE_INPUT_REACHABILITY_ONLY';assert.deepEqual(c.errors,[]);c.status='PASS'
  }catch(e){c.status='FAIL';c.error=e.message;c.renderedState=await page.locator('.shell-panel-body').innerText().catch(()=>null);c.formQualification='NOT_RUN_OR_INCOMPLETE';report.errors.push({route,width,error:e.message})}
  finally{
   try{await page.evaluate(()=>window.__COMMUNITY_AUTHORITY_NATIVE__?.dispose());await expect.poll(()=>page.locator('canvas').count()).toBe(0);c.cleanup=await state(page);assert(c.cleanup.disposed&&c.cleanup.activeSockets===0)}catch(e){c.status='FAIL';c.cleanupError=e.message}
   await context.close();report.cases.push(c);await checkpoint()
  }
  if(c.status!=='PASS')break cases
 }
 for(const [p,sha]of Object.entries(report.sourceReadset))assert.equal(hash(await readFile(resolve(repo,p))),sha,'Unchanged loaded source '+p)
 report.status=report.cases.length===6&&report.cases.every(c=>c.status==='PASS')?'MOCK_READY':'INCOMPLETE_OR_FAILED'
} catch(e){report.status='FAIL';report.errors.push({error:e.message})}
finally{
 await browser?.close();await server?.close();assets.clear();report.finishedAt=new Date().toISOString();report.cleanup={browserClosed:!!browser,serverClosed:!!server,assetCacheCleared:assets.size===0}
 await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n')
}
console.log(JSON.stringify({status:report.status,cases:report.cases.length,errors:report.errors}));if(report.status!=='MOCK_READY')process.exitCode=1
