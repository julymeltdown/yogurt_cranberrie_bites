import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {mkdir,readFile,statfs,writeFile} from 'node:fs/promises'
import {resolve} from 'node:path'
import {execFileSync} from 'node:child_process'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../..'),repo=resolve(root,'..'),out=resolve(import.meta.dirname,'evidence')
const disk=await statfs(root);assert(disk.bavail*disk.bsize>8*1024**3,'8GiB disk floor')
await mkdir(out,{recursive:true})
const server=await createServer({root,configLoader:'runner',cacheDir:'/tmp/mcr-fe07-journey-vite',logLevel:'warn',server:{host:'127.0.0.1',port:0,strictPort:false}});await server.listen()
const base=`http://127.0.0.1:${server.httpServer.address().port}/src/app/__tests__/fe07-shell-journey/preview.html`
const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
const report={startedAt:new Date().toISOString(),toolchain:{node:process.version,chrome:await browser.version()},candidate:execFileSync('git',['rev-parse','HEAD'],{cwd:repo,encoding:'utf8'}).trim(),result:'FAIL',qualification:'Actual createAppRouter/browser history, canonical Auth, default production composition/real Phaser. Synthetic HTTP/C05, isolated map/place admission, immutable selected-character data. No production admission, live/backend, device, movement/physical-arrival or IN02 proof.',cases:[],errors:[],findings:[],sources:{}}
let stage='startup',activePage=null
const seams=['src/app/router.tsx','src/app/routes.ts','src/app/navigation.tsx','src/app/WorldShell.tsx','src/app/ports.ts','src/app/WorldHost.tsx','src/world/composition/index.ts','src/features/auth/model/session.ts','src/features/auth/ui/SignIn.tsx','src/app/social-composition/owner.ts','src/app/social-composition/navigation.tsx','src/app/__tests__/fe07-shell-journey/preview.tsx','src/app/__tests__/fe07-shell-journey/native.mjs','e2e/reference-integrated/whole/data.ts','../contracts/fixtures/domain/giveaway-management.json','../contracts/domain/v1/giveaway.schema.json']
for(const file of seams)report.sources[file]=createHash('sha256').update(await readFile(resolve(root,file))).digest('hex')
async function open(query,width){const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,isMobile:true});const page=await context.newPage();activePage=page;page.setDefaultTimeout(10000);page.on('pageerror',error=>report.errors.push(error.message));await page.goto(base+'?'+new URLSearchParams(query));await page.waitForFunction(()=>window.__FE07_JOURNEY__);return {context,page}}
const state=page=>page.evaluate(()=>window.__FE07_JOURNEY__.state())
const path=page=>page.evaluate(()=>location.pathname)
const cameraCenter=view=>({following:view.following,zoom:view.camera.zoom,x:view.camera.scrollX+view.camera.width/2,y:view.camera.scrollY+view.camera.height/2})
const navigate=(page,to)=>page.evaluate(to=>window.__FE07_JOURNEY__.navigate(to),to)
async function world(page){await expect.poll(async()=>(await state(page)).games).toBe(1);await expect.poll(async()=>(await state(page)).phase).toBe('live');await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeVisible();await expect.poll(async()=>{const s=await state(page),rect=await page.locator('.shell-world-host canvas').boundingBox();return rect&&s.camera?.camera?Math.abs(s.camera.camera.height-rect.height):999}).toBeLessThan(2)}
async function bounded(page){const s=await state(page);assert.equal(s.games,1);assert.equal(s.canvases,1);assert.equal(s.peakGames,1);assert.equal(s.peakSockets,1);assert.equal(s.activeSockets,1);assert.equal(s.sameGame,true);assert.equal(s.sameCamera,true);return s}
async function back(page,to){await page.getByRole('button',{name:'Back to previous panel',exact:true}).click();await expect.poll(()=>path(page)).toBe(to);if(to==='/world')await expect(page.getByRole('navigation',{name:'World tools',exact:true})).toBeVisible();else await expect(page.locator('#panel-title')).toHaveText({'/meet':'Meet','/gifts':'Gifts'}[to]??'')}
async function shot(page,name){await page.screenshot({path:resolve(out,name+'.png'),fullPage:true})}
try{
 stage='direct beacon parent fallback'
 {
  const {context,page}=await open({entry:'/meet/beacons/00000000-0000-4000-8000-000000000015',foreign:'1'},390)
  await expect.poll(async()=>(await state(page)).games).toBe(1);const initial=await state(page)
  await back(page,'/meet');await back(page,'/world');await world(page);const final=await bounded(page)
  assert.equal(final.sockets,1);assert.equal(final.historyLength,initial.historyLength);assert.equal(final.action,'REPLACE')
  await page.evaluate(()=>window.__FE07_JOURNEY__.dispose());await context.close()
  report.cases.push({result:'PASS',directBeacon:'parent Meet then World; same game/camera/connection; foreign history stays bounded',initial,final})
 }
 const widths=(process.env.FE07_WIDTHS??'360,390,430').split(',').map(Number)
 for(const width of widths){
  stage=`${width}/cold gift`
  const gift='/gifts/00000000-0000-4000-8000-000000000005'
  const {context,page}=await open({entry:gift,anonymous:'1','hold-bootstrap':'1',foreign:'1'},width)
  let s=await state(page);assert.equal(s.games,0);assert.equal(s.sockets,0);assert.equal(s.path,gift);const cold={path:s.path,games:s.games,sockets:s.sockets,auth:s.auth}
  assert.equal((await page.evaluate(()=>performance.getEntriesByType('resource').map(item=>item.name))).some(url=>/\/(?:deps\/)?phaser(?:\.js|\?)/.test(url)),false,'anonymous entry must not load Phaser')
  await page.getByRole('link',{name:'Sign in',exact:true}).click();await expect.poll(()=>path(page)).toBe('/login')
  assert.equal(await page.evaluate(()=>new URLSearchParams(location.search).get('returnTo')),gift)
  await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password')
  await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
  await expect.poll(()=>path(page)).toBe(gift);await expect.poll(async()=>(await state(page)).bootstrapHeld).toBe(true)
  const held=await state(page);assert.equal(held.games,0);assert.equal(held.sockets,0)
  await page.evaluate(()=>window.__FE07_JOURNEY__.releaseBootstrap())
  await expect(page.getByRole('heading',{name:'Synthetic shell gift 1',exact:true})).toBeVisible()
  await expect.poll(async()=>(await state(page)).games).toBe(1)
  const direct=await state(page);assert.equal(direct.knownGames,1);assert.equal(direct.peakGames,1)
  await shot(page,`direct-gift-${width}`)
  // The shell Back button replaces a direct/foreign entry with known parents.
  const length=direct.historyLength;await back(page,'/gifts');await expect(page.getByRole('link',{name:'Synthetic shell gift 18',exact:true})).toBeVisible()
  assert.equal((await state(page)).action,'REPLACE');await back(page,'/world');await world(page)
  assert.equal((await state(page)).historyLength,length);await bounded(page)
  const nav=page.getByRole('navigation',{name:'Main navigation'});await expect(nav.getByRole('link')).toHaveCount(6)
  for(const link of await nav.getByRole('link').all()){const box=await link.boundingBox();assert(box&&box.width>=44&&box.height>=44,'six touch targets must be at least 44px')}
  await expect(nav.getByRole('link',{name:'World',exact:true})).toHaveAttribute('aria-current','page')
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'world shell must fit the mobile width')
  const start=await state(page);await page.evaluate(()=>window.__FE07_JOURNEY__.camera());const camera=(await state(page)).camera
  stage=`${width}/native history and scroll`
  await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Gifts',exact:true}).click()
  await expect.poll(()=>path(page)).toBe('/gifts')
  const opener=page.getByRole('link',{name:'Synthetic shell gift 18',exact:true});await opener.scrollIntoViewIfNeeded()
  const scroll=await page.locator('.shell-panel-body').evaluate(element=>element.scrollTop);assert(scroll>100,'real gift list must scroll')
  await opener.click();await expect(page.getByRole('heading',{name:'Synthetic shell gift 18',exact:true})).toBeVisible();const detail=await path(page)
  await back(page,'/gifts');assert.equal((await state(page)).action,'POP');await expect(opener).toBeFocused()
  report.scrollObservation={before:scroll,after:await page.locator('.shell-panel-body').evaluate(element=>element.scrollTop)};if(Math.abs(report.scrollObservation.after-scroll)>=3)report.findings.push({criterion:'list scroll restoration',width,...report.scrollObservation})
  await page.goForward();await expect.poll(()=>path(page)).toBe(detail)
  await page.goBack();await expect.poll(()=>path(page)).toBe('/gifts');await bounded(page)
  await back(page,'/world');await world(page);assert.deepEqual(cameraCenter((await state(page)).camera),cameraCenter(camera))
  assert.deepEqual((await state(page)).position,start.position);assert.equal((await state(page)).room,start.room)
  stage=`${width}/six tabs and nested routes`
  for(const [tab,to] of [['Feed','/feed'],['Meet','/meet'],['Gifts','/gifts'],['Wall','/wall'],['Me','/me']]){
   await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:tab,exact:true}).click();await expect.poll(()=>path(page)).toBe(to)
   await expect(page.locator('.shell-panel')).toHaveCount(1);await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeHidden()
   await bounded(page);await expect(page.locator('.world-joystick')).toBeDisabled();await back(page,'/world');await world(page)
  }
  stage=`${width}/safe draft and panel sizing`
  await navigate(page,'/me/profile');const bio=page.getByLabel('Bio (optional)');await expect(bio).toBeEnabled();const initialBio=await bio.inputValue();await bio.fill('Unsaved shell draft — synthetic')
  await page.getByRole('button',{name:'Reduce panel',exact:true}).click();assert.equal(await page.locator('.world-shell').getAttribute('data-panel-size'),'half')
  await page.getByRole('button',{name:'Minimize panel',exact:true}).click();assert.equal(await page.locator('.world-shell').getAttribute('data-panel-size'),'peek')
  await page.getByRole('button',{name:'Show panel content',exact:true}).click();await bounded(page)
  await navigate(page,'/world');await world(page);await navigate(page,'/me/profile');await expect(bio).toHaveValue('Unsaved shell draft — synthetic')
  // No save click and no fabricated successful server mutation.
  assert.equal((await state(page)).requests.filter(r=>r.method!=='GET'&&!r.path.endsWith('/login')&&!r.path.endsWith('/world-tickets')).length,0)
  await navigate(page,'/world');await world(page)
  if(await page.locator('.world-recenter').isDisabled())report.findings.push({criterion:'world controls resume after closing draft panel',width})
  stage=`${width}/denied detail and cleanup`
  await page.evaluate(()=>window.__FE07_JOURNEY__.giftMode('denied'));await navigate(page,'/gifts/00000000-0000-4000-8000-000000000fff')
  await expect(page.getByRole('heading',{name:'These gifts are not available to this account',exact:true})).toBeVisible()
  assert(!(await page.locator('body').innerText()).includes('PRIVATE_DIAGNOSTIC'))
  await back(page,'/gifts');await back(page,'/world');await world(page)
  const final=await bounded(page);assert(final.requests.filter(request=>request.path.startsWith('/api/concert/')).every(request=>request.protected),'concert requests must use canonical Auth');if(final.sockets!==1)report.findings.push({criterion:'retain one world connection through panels/history',width,sockets:final.sockets})
  await shot(page,`world-after-history-${width}`)
  // Revert the tested draft through the real editor before independently qualifying shell exit.
  await navigate(page,'/me/profile');await expect(bio).toHaveValue('Unsaved shell draft — synthetic');await bio.fill(initialBio);await navigate(page,'/world');await world(page)
  await page.evaluate(()=>window.__FE07_JOURNEY__.dispose());await expect.poll(async()=>(await state(page)).games).toBe(0);const disposed=await state(page);assert.equal(disposed.games,0);assert.equal(disposed.activeSockets,0)
  report.cases.push({width,result:report.findings.some(item=>item.width===width)?'OPEN_FINDINGS':'PASS',cold,held,directGift:direct,history:'native POP/forward + parent REPLACE + foreign fallback',scroll:report.scrollObservation,retainedDraft:true,final,disposed})
  await context.close()
 }
 stage='safe return targets'
 for(const target of ['https://example.invalid/','//example.invalid/','/admin/event','/world/../admin']){
  const {context,page}=await open({entry:'/login?returnTo='+encodeURIComponent(target),anonymous:'1'},390)
  assert.equal((await state(page)).games,0)
  await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password');await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
  await expect.poll(()=>path(page)).toBe('/world');await world(page);await page.evaluate(()=>window.__FE07_JOURNEY__.dispose());await context.close()
  report.cases.push({target,result:'PASS',returnTo:'/world'})
 }
 for(const width of widths){
  stage=`${width}/clean public and admin boundaries`
  const {context,page}=await open({entry:'/world'},width);await world(page);await bounded(page)
  await navigate(page,'/legal/privacy');await expect.poll(()=>path(page)).toBe('/legal/privacy')
  await expect.poll(async()=>(await state(page)).games).toBe(0);await expect.poll(async()=>(await state(page)).activeSockets).toBe(0)
  const publicExit=await state(page)
  await page.goBack();await world(page);const returned=await state(page);assert.equal(returned.knownGames,2);assert.equal(returned.peakGames,1);assert.equal(returned.peakSockets,1)
  await navigate(page,'/admin/reports');await expect(page.getByRole('heading',{name:'Access denied',exact:true})).toBeVisible()
  await expect.poll(async()=>(await state(page)).games).toBe(0);await expect.poll(async()=>(await state(page)).activeSockets).toBe(0)
  const adminExit=await state(page);await page.evaluate(()=>window.__FE07_JOURNEY__.dispose());await context.close()
  report.cases.push({width,result:'PASS',boundaries:'clean public exit, native Back remount, denied admin exit',publicExit,returned,adminExit})
 }
 assert.deepEqual(report.errors,[])
 stage='required shell findings';assert.deepEqual(report.findings,[])
 report.result='PASS'
}catch(error){report.failure={stage,message:error.message};if(activePage&&!activePage.isClosed()){report.failure.state=await state(activePage);report.failure.text=await activePage.locator('body').innerText();await shot(activePage,'failure');}console.error(stage,error.message);process.exitCode=1}
finally{
 for(const file of seams){const hash=createHash('sha256').update(await readFile(resolve(root,file))).digest('hex');if(hash!==report.sources[file]){report.result='FAIL';report.errors.push(`Source changed during capture: ${file}`);process.exitCode=1}}
 report.endedAt=new Date().toISOString()
 await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2)+'\n');await browser.close();await server.close()
}
console.log(JSON.stringify({result:report.result,cases:report.cases.length,errors:report.errors,failure:report.failure}))
