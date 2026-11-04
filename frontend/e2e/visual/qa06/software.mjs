// Original QA06 software remainder. Existing production router/Auth/Phaser fixture,
// unchanged server facts. No product source, composition, admission or art overlay.
import assert from 'node:assert/strict'
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {readFile,writeFile} from 'node:fs/promises'
import {resolve,extname,relative,dirname} from 'node:path'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
import {settingsCase,giftsActivityCase} from './settings-activity.mjs'
const repo=resolve(import.meta.dirname,'../../../..'),frontend=resolve(repo,'frontend')
const out=process.env.QA06_SOFTWARE_OUTPUT,temp=process.env.QA06_SOFTWARE_TEMP
const job=process.env.QA06_SOFTWARE_JOB,pin=process.env.QA06_SOFTWARE_CANDIDATE,port=Number(process.env.QA06_SOFTWARE_PORT)
const cohort=JSON.parse(await readFile(resolve(import.meta.dirname,'cohorts',process.env.QA06_SOFTWARE_COHORT+'.json'),'utf8'))
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:32*1024**2})
const hash=b=>createHash('sha256').update(b).digest('hex')
assert.equal(process.version,'v24.22.2')
assert(job&&/^[a-f0-9]{40}$/.test(pin??'')&&out?.startsWith(repo+'/docs/mcr/evidence/visual/qa06/software-remainder/'))
assert(temp?.startsWith('/private/tmp/mcr-qa06-software-')&&port>1024)
assert.equal(git('rev-parse','HEAD').toString().trim(),pin)
const admission=JSON.parse(await readFile(process.env.QA06_SOFTWARE_ADMISSION,'utf8'))
assert(admission.job===job&&admission.candidate===pin&&admission.cohort===cohort.id&&Date.now()/1000<admission.workDeadline)
assert.equal(admission.runnerSha256,hash(await readFile(new URL(import.meta.url))))
assert.equal(admission.guardSha256,hash(await readFile(resolve(import.meta.dirname,'software-guard.py'))))
git('diff','--quiet','HEAD','--','frontend/src','frontend/public','contracts','frontend/package.json','frontend/pnpm-lock.yaml')
const fixture='/e2e/visual/qa06/software-preview.html',origin='http://127.0.0.1:'+port
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n')),assets=new Map()
const report={task:'QA06',job,candidate:pin,cohort:cohort.id,startedAt:new Date().toISOString(),status:'IN_PROGRESS',
 qualification:'EXISTING_DEFAULT_APP_CANONICAL_AUTH_SYNTHETIC_HTTP_C05_DESKTOP_SOFTWARE_ONLY',
 physicalIOS:'NOT_RUN',physicalAndroid:'NOT_RUN',softwareKeyboard:'NOT_RUN',hardwareSafeArea:'NOT_RUN',
 admission,runnerSha256:hash(await readFile(new URL(import.meta.url))),cohortSha256:hash(await readFile(resolve(import.meta.dirname,'cohorts',cohort.id+'.json'))),
 fixture:{path:fixture,sha256:hash(await readFile(resolve(frontend,fixture.slice(1)))),scriptPath:'frontend/e2e/visual/qa06/software-preview.tsx',scriptSha256:hash(await readFile(resolve(import.meta.dirname,'software-preview.tsx'))),authAdmissionFixtureSha256:hash(await readFile(resolve(frontend,'src/features/auth/__tests__/enrollment-consent/fixtures.ts')))},sourceReadset:{},assetReadset:{},cases:[],errors:[],
 limits:['Incumbent canonical Auth/router/providers/default production Phaser; own headless derivative adds exact existing Auth admission() test-server response. Explicit synthetic HTTP/C05/map/policy authority, not live server/admission/physical proof.',
 'Virtual scenario keeps incumbent virtual place facts; no physical verification/provider/age/bootstrap/renderer or source override.',
 'Only exact cohort states, not the ten-surface/Wall/reflow replay, whole61IA or full-reference acceptance.',
 'No domain write; era persistence is the actual account/event-scoped LOCAL preference, explicitly not server synchronization.']}
const reusedIds=new Set()
if(process.env.QA06_SOFTWARE_RESUME){
 const source=process.env.QA06_SOFTWARE_RESUME;assert(source.startsWith(repo+'/docs/mcr/evidence/visual/qa06/software-remainder/'+cohort.id+'/'))
 const bytes=await readFile(source),previous=JSON.parse(bytes);assert(previous.job===job&&previous.candidate===pin&&previous.cohort===cohort.id)
 for(const [path,sha]of Object.entries(previous.sourceReadset))assert.equal(hash(await readFile(resolve(repo,path))),sha)
 assert.equal(previous.fixture.scriptSha256,report.fixture.scriptSha256)
 report.reusedCases=previous.cases.filter(c=>c.status==='PASS').map(c=>{reusedIds.add(c.id);return {id:c.id,status:'PASS',sourceReport:relative(out,source),sourceReportSha256:hash(bytes),originalRunnerSha256:previous.runnerSha256,screenshot:c.screenshot?{...c.screenshot,path:relative(out,resolve(dirname(source),c.screenshot.path))}:null}})
}
let server,browser
const state=p=>p.evaluate(()=>window.__COMMUNITY_AUTHORITY_NATIVE__.state())
const checkpoint=()=>writeFile(resolve(out,'report.partial.json'),JSON.stringify(report,null,2)+'\n')
const url=(era,entry='/world')=>origin+fixture+'?'+new URLSearchParams({entry,era,scenario:'virtual'})
const selected=s=>s.theme?.selected
const view=s=>s.camera?.camera?{zoom:s.camera.camera.zoom,x:s.camera.camera.scrollX+s.camera.camera.width/2,y:s.camera.camera.scrollY+s.camera.camera.height/2}:null
async function hit(locator){
 await locator.scrollIntoViewIfNeeded();const r=await locator.boundingBox();assert(r&&r.width>=43.5&&r.height>=43.5,'44px target')
 assert(await locator.evaluate(e=>{const r=e.getBoundingClientRect(),h=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return h===e||e.contains(h)}),'Visible target center reachable')
 return r
}
async function geometry(page){return page.evaluate(()=>{
 const p=document.querySelector('.shell-panel'),b=document.querySelector('.shell-panel-body')
 return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth+1,panel:p?.getBoundingClientRect().toJSON(),body:b?.getBoundingClientRect().toJSON()}
})}
async function bounds(page){const g=await geometry(page);assert(!g.overflow&&g.panel&&g.panel.y>=-1&&g.panel.bottom<=845,'Panel bounds/no horizontal overflow');return g}
async function shot(page,name){const capturedAt=new Date().toISOString(),bytes=await page.screenshot({path:resolve(out,name),fullPage:false});return {path:name,capturedAt,sha256:hash(bytes),bytes:bytes.length}}
async function ready(page,era,panel=false){await page.waitForFunction(()=>Boolean(window.__COMMUNITY_AUTHORITY_NATIVE__));await expect.poll(async()=>(await state(page)).published,{timeout:25000}).toBe(true);await expect.poll(async()=>selected(await state(page))).toBe(era);if(panel){await expect.poll(async()=>['live','suspended'].includes((await state(page)).phase)).toBe(true);await expect(page.locator('.world-joystick')).toBeDisabled()}else await expect.poll(async()=>(await state(page)).phase).toBe('live');assert.equal((await state(page)).admission,'ADMITTED')}
async function navigate(page,path){await page.evaluate(path=>window.__COMMUNITY_AUTHORITY_NATIVE__.navigate(path),path);await expect.poll(async()=>(await state(page)).path).toBe(path);await expect(page.locator('.shell-panel')).toBeVisible()}
async function retained(page,c){
 const s=await state(page);assert(s.canvases===1&&s.socketPeak===1&&s.activeSockets===1)
 assert(await page.evaluate(()=>window.__QA06_SOFTWARE_CANVAS__===document.querySelector('.shell-world-host canvas')),'Same World canvas')
 assert.equal(s.requests.filter(r=>r.method!=='GET').length,c.initialMutations,'No extra domain write')
 return {canvases:s.canvases,socketPeak:s.socketPeak,activeSockets:s.activeSockets,phase:s.phase,entities:s.entities,theme:selected(s),camera:view(s)}
}
try{
 server=await createServer({root:frontend,configLoader:'runner',cacheDir:resolve(temp,'vite'),logLevel:'error',resolve:{dedupe:['react','react-dom']},
  plugins:[{name:'qa06-software-source-bound-readonly',async transform(_code,id){const path=id.split('?')[0];if(path.startsWith(frontend+'/src/')&&['.ts','.tsx','.css','.json'].includes(extname(path)))report.sourceReadset[path.slice(repo.length+1)]=hash(await readFile(path))},configureServer(s){s.middlewares.use((req,res,next)=>{
   const p='frontend/public'+decodeURIComponent(new URL(req.url,origin).pathname)
   if(!tracked.has(p))return next()
   try{if(!assets.has(p))assets.set(p,git('show',pin+':'+p));const b=assets.get(p);report.assetReadset[p]={sha256:hash(b),bytes:b.length}
    const mime={'.png':'image/png','.svg':'image/svg+xml','.json':'application/json','.ttf':'font/ttf','.woff2':'font/woff2','.jpg':'image/jpeg','.jpeg':'image/jpeg','.webp':'image/webp','.bin':'application/octet-stream'}
    res.setHeader('Content-Type',mime[extname(p)]??'application/octet-stream');res.end(b)
   }catch(e){next(e)}
  })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false,watch:null}})
 await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const spec of cohort.cases.filter(spec=>!reusedIds.has(spec.id))){
  const c={...spec,status:'IN_PROGRESS',errors:[]},context=await browser.newContext({viewport:{width:spec.width,height:844},hasTouch:true,isMobile:true,serviceWorkers:'block'}),page=await context.newPage()
  page.setDefaultTimeout(12000);page.on('pageerror',e=>c.errors.push(e.message))
  try{
   await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'));await page.goto(url(spec.era));await ready(page,spec.era)
   await page.evaluate(()=>window.__QA06_SOFTWARE_CANVAS__=document.querySelector('.shell-world-host canvas'))
   c.initialMutations=(await state(page)).requests.filter(r=>r.method!=='GET').length
   if(spec.kind==='nearby'){
    // Real World-tools opener gives the actual shell a restoration destination.
    await page.getByRole('button',{name:'World tools',exact:true}).click()
    const opener=page.getByRole('navigation',{name:'World tools',exact:true}).getByRole('link',{name:'Nearby',exact:true});c.opener=await hit(opener);await opener.click()
    await expect.poll(async()=>(await state(page)).path).toBe('/world/nearby')
    const list=page.getByRole('list',{name:'People authorized in your current virtual room',exact:true});await expect(list).toBeVisible()
    const people=list.locator('a[data-nearby-user-id]');await expect(people).toHaveCount((await state(page)).entities-1)
    await expect(page.getByText('People currently visible to you in this virtual room. This list does not show physical location or distance.',{exact:true})).toBeVisible()
    c.authorizedPeople=await people.evaluateAll(nodes=>nodes.map(n=>({id:n.dataset.nearbyUserId,name:n.textContent})));assert(c.authorizedPeople.length>0)
    c.targets=[];for(const person of await people.all())c.targets.push(await hit(person))
    await people.first().focus();await expect(people.first()).toBeFocused();await page.keyboard.press('Tab');await expect(people.nth(1)).toBeFocused()
    await people.first().scrollIntoViewIfNeeded();c.geometry=await bounds(page)
    if(spec.capture)c.screenshot=await shot(page,spec.id+'.png')
    c.runtime=await retained(page,c)
    await page.getByRole('button',{name:'Back to previous panel',exact:true}).click();await expect.poll(async()=>(await state(page)).path).toBe('/world')
    await expect(page.locator('#world-nearby')).toBeFocused();c.backFocus='world-nearby';c.returnState=await retained(page,c)
   }else if(spec.kind==='theme'){
    await navigate(page,'/world/theme');const panel=page.locator('.shell-panel-body'),group=panel.getByRole('group',{name:'Choose an era',exact:true})
    await expect(group.getByRole('button',{name:'Revenge',exact:true})).toHaveAttribute('aria-pressed','true');c.cameraBefore=view(await state(page));assert(c.cameraBefore)
    c.selections=[]
    for(const era of ['PARADE','DANGER']){
     const label=era[0]+era.slice(1).toLowerCase(),button=group.getByRole('button',{name:label,exact:true});const rect=await hit(button);await button.click();await ready(page,era,true)
     await expect(button).toHaveAttribute('aria-pressed','true');assert.deepEqual(view(await state(page)),c.cameraBefore,'Skin does not move camera')
     c.selections.push({era,rect,runtime:await retained(page,c)})
    }
    await expect(panel.getByText('Era preferences are saved on this device for this account and event. Server preference synchronization is unavailable.',{exact:true})).toBeVisible()
    c.localPreference=await page.evaluate(()=>Object.entries(localStorage).filter(([key])=>key.startsWith('mcr:theme:v1:')));assert.equal(c.localPreference.length,1);assert.equal(c.localPreference[0][1],'DANGER')
    c.geometry=await bounds(page);if(spec.capture)c.screenshot=await shot(page,spec.id+'.png')
    // New canonical login/bootstrap lifetime, same browser's actual local preference.
    await page.goto(url('REVENGE','/world/theme'));await ready(page,'DANGER',true);c.persistedAfterNewLifetime='DANGER';c.reloadState=await state(page)
    await expect(page.locator('.shell-panel-body').getByRole('button',{name:'Danger',exact:true})).toHaveAttribute('aria-pressed','true')
   }else if(spec.kind==='help'){
    await page.getByRole('button',{name:'World tools',exact:true}).click();await page.getByRole('navigation',{name:'World tools',exact:true}).getByRole('link',{name:'Help',exact:true}).click()
    const help=page.getByRole('article',{name:'Controls and meeting safety',exact:true});await expect(help).toBeVisible();await expect(help.getByRole('heading',{name:'One world, at your pace',exact:true})).toBeVisible()
    c.headings=[];for(const label of ['Move around','Look around','Virtual is not physical','Meet with care']){const heading=help.getByRole('heading',{name:label,exact:true});await heading.scrollIntoViewIfNeeded();await expect(heading).toBeInViewport({ratio:1});c.headings.push(label)}
    c.links=[];for(const label of ['Browse places','Return to world'])c.links.push({label,rect:await hit(help.getByRole('link',{name:label,exact:true}))})
    c.geometry=await bounds(page);if(spec.capture)c.screenshot=await shot(page,spec.id+'.png');c.runtime=await retained(page,c)
    await help.getByRole('link',{name:'Return to world',exact:true}).click();await expect.poll(async()=>(await state(page)).path).toBe('/world');c.returnState=await retained(page,c)
   }else if(spec.kind==='settings'||spec.kind==='gifts-activity'){
    await (spec.kind==='settings'?settingsCase:giftsActivityCase)({page,c,navigate,state,selected,hit,bounds,shot,retained})
   }else throw Error('Unknown original QA06 software case '+spec.kind)
   assert.deepEqual(c.errors,[]);c.status='PASS'
  }catch(e){c.status='FAIL';c.error=e.message;c.renderedText=await page.locator('body').innerText().catch(()=>null);report.errors.push({id:c.id,error:e.message})}
  finally{
   try{await page.evaluate(()=>window.__COMMUNITY_AUTHORITY_NATIVE__?.dispose());await expect.poll(()=>page.locator('.shell-world-host canvas').count()).toBe(0);const s=await state(page);assert(s.disposed&&s.activeSockets===0);c.cleanup={disposed:s.disposed,activeSockets:s.activeSockets,canvases:s.canvases}}catch(e){c.status='FAIL';c.cleanupError=e.message}
   await context.close();report.cases.push(c);await checkpoint()
  }
  if(c.status!=='PASS')break
 }
 for(const [p,sha]of Object.entries(report.sourceReadset))assert.equal(hash(await readFile(resolve(repo,p))),sha,'Unchanged loaded source '+p)
 report.status=report.cases.length+reusedIds.size===cohort.cases.length&&report.cases.every(c=>c.status==='PASS')?'MOCK_READY':'INCOMPLETE_OR_FAILED'
}catch(e){report.status='FAIL';report.errors.push({error:e.message})}
finally{
 await browser?.close();server?.httpServer?.closeAllConnections();let timer
 const closed=await Promise.race([server?.close().then(()=>true),new Promise(resolve=>{timer=setTimeout(()=>resolve(false),5000)})]);clearTimeout(timer)
 report.cleanup={browserDisconnected:!browser?.isConnected(),serverCloseResolved:closed};if(!closed){report.status='SHUTDOWN_INCOMPLETE';report.errors.push({error:'Owned Vite shutdown unresolved after5s'})}
 assets.clear();report.finishedAt=new Date().toISOString();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');if(!closed)process.exit(2)
}
console.log(JSON.stringify({status:report.status,cases:report.cases.length,errors:report.errors}));if(report.status!=='MOCK_READY')process.exitCode=1
