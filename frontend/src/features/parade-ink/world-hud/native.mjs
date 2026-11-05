// Execute only under a distinct finite native offer. Actual main.tsx with controlled HTTP/C05.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createRequire } from 'node:module'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { foundationOverlay } from './foundation-overlay.mjs'
const frontend=resolve(import.meta.dirname,'../../../..'),deps=process.env.MCR_FRONTEND_DEPENDENCIES
const scratch=process.env.PI08_SCRATCH,out=process.env.PI08_OUTPUT
assert(deps&&scratch&&out,'Fresh finite native allocation required')
for(const entry of ['native.mjs','native-fixture.mjs','native-observer.mjs'])execFileSync(process.execPath,['--check',resolve(import.meta.dirname,entry)],{stdio:'inherit'})
const require=createRequire(resolve(deps,'package.json'))
const { createServer }=await import(pathToFileURL(require.resolve('vite')))
const { chromium,expect }=require('@playwright/test')
const { default:react }=await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
const { default:tailwind }=await import(pathToFileURL(require.resolve('@tailwindcss/vite')))
const { aliases }=await import('../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs')
const foundation=foundationOverlay(frontend)
const publicReadset=JSON.parse(await readFile(resolve(frontend,'../docs/mcr/evidence/parade-ink-world-hud/native-public-readset.json'),'utf8'))
const publicFiles=new Map(publicReadset.files.map(row=>[row.url,row])),usedAssets=new Map()
let assetPending=Promise.resolve()
const operator=JSON.parse(await readFile(resolve(frontend,'src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'),'utf8'))
const fixture=resolve(frontend,'src/features/parade-ink/world-hud/native-fixture.html')
const photo=await readFile(resolve(process.env.PI08_FOUNDATION,'docs/mcr/design/parade-ink-2026-11-05/parade-ink.jpg'))
assert.equal(createHash('sha256').update(photo).digest('hex'),'9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27')
const input=JSON.parse(await readFile(resolve(frontend,'src/app/world-entry/__tests__/map-presentation-v1/source-inputs/local-input.json'),'utf8'))
await mkdir(out,{recursive:true})
const retainedRoot=resolve(frontend,'../docs/mcr/evidence/parade-ink-world-hud/native02/output')
const retainedBytes=await readFile(resolve(retainedRoot,'native.json')),retained=JSON.parse(retainedBytes)
assert.equal(retained.failure.stage,'six-destinations');assert.deepEqual(retained.errors,[])
assert.equal(Object.keys(retained.captures).length,4);assert.equal(retained.era.length,3)
assert(retained.keyboard.captionReversible&&retained.keyboard.menuEscapeFocus&&retained.contextAction.returnedByIncumbentPanelControl)
const navigationBytes=await readFile(resolve(frontend,'../docs/mcr/evidence/parade-ink-world-hud/native03/output/native.json'))
const retainedNavigation=JSON.parse(navigationBytes)
assert.equal(retainedNavigation.failure.stage,'safe-area');assert.equal(retainedNavigation.navigation.length,6)
assert(retainedNavigation.eraSurvivesNavigation);assert.deepEqual(retainedNavigation.errors,[])
const report={result:'FAIL',qualification:'ACTUAL_MAIN_DEFAULT_APP_CONTROLLED_HTTP_C05_NOT_LIVE',foundation:'b5b44e47531e3b75febc5ee5cdd6a11b1648779c',foundationQualification:'ONE explicit shell.css correction; no full PI01 native acceptance inferred',reference:{sha256:createHash('sha256').update(photo).digest('hex'),original:[1280,853],crop:[436,60,832,847],normalization:390/396},DPR:1,errors:[],captures:{},era:[],navigation:[]}
report.retainedNative02={sha256:createHash('sha256').update(retainedBytes).digest('hex'),qualified:['390reference/default360430 geometry/fonts/44px/ink/overflow/incumbent continuity','keyboard caption/menu Escape focus','Nearby route and incumbent Back','REVENGE default and all3 independent eras'],captures:retained.captures,era:retained.era,keyboard:retained.keyboard,contextAction:retained.contextAction,assets:'Native02 asset-body handler ran; its per-asset list was not retained before failure. Native03 separately verifies used assets and does not claim a retained per-asset list.'}
report.retainedNative03={sha256:createHash('sha256').update(navigationBytes).digest('hex'),qualified:['all6destinationroutes/selectedDOMstate/incumbentcontinuity','DangereraSurvivesNavigation'],routes:retainedNavigation.navigation.map(item=>({label:item.label,path:new URL(item.url).pathname}))}
const server=await createServer({root:frontend,configFile:false,plugins:[foundation.plugin,react(),tailwind(),{
 name:'pi08-actual-main-fixture',configureServer(server){server.middlewares.use(async(request,response,next)=>{
  if(!request.headers.accept?.includes('text/html'))return next()
  response.setHeader('Content-Type','text/html')
  response.end(await server.transformIndexHtml(request.url,await readFile(fixture,'utf8')))
 })}
}],cacheDir:resolve(scratch,'cache'),logLevel:'warn',
 resolve:{alias:Object.entries({...aliases,'@hookform/resolvers/zod':require.resolve('@hookform/resolvers/zod')}).map(([name,replacement])=>({find:new RegExp('^'+name.replace(/[.*+?^${}()|[\]\\]/g,'\\$&')+'$'),replacement}))},
 define:{'import.meta.env.VITE_MCR_EVENT_ID':JSON.stringify(operator.event.event_id),'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG':JSON.stringify(JSON.stringify(input))},
 server:{host:'127.0.0.1',port:0,watch:null,fs:{allow:[frontend,deps,process.env.PI08_FOUNDATION]}}})
let browser,page,stage='startup'
const state=()=>page.evaluate(()=>window.__PI08_RUNTIME__.state())
const proof=()=>page.evaluate(()=>window.__PI08_RUNTIME__.worldProof())
const layout=()=>page.evaluate(()=>{
 const read=selector=>[...document.querySelectorAll(selector)].filter(el=>el.getClientRects().length).map(el=>{
  const r=el.getBoundingClientRect(),s=getComputedStyle(el)
  return {tag:el.tagName,text:el.textContent.trim(),x:r.x,y:r.y,width:r.width,height:r.height,font:s.fontFamily,size:s.fontSize,line:s.lineHeight,weight:s.fontWeight,color:s.color,background:s.backgroundColor,borders:[s.borderTopWidth,s.borderRightWidth,s.borderBottomWidth,s.borderLeftWidth],pressed:el.getAttribute('aria-pressed'),current:el.getAttribute('aria-current'),outline:s.outlineStyle}
 })
 return {viewport:[innerWidth,innerHeight],DPR:devicePixelRatio,overflow:document.documentElement.scrollWidth>innerWidth,
  root:read('.world-shell'),title:read('.pi08-world-title'),eras:read('.pi08-era-controls button'),context:read('.pi08-world-context'),action:read('.pi08-context-action'),nav:read('.pi08-navigation a'),headerControls:read('.pi08-world-title button,.pi08-world-title a'),fonts:{body:document.fonts.check('18px "Parade Ink Text"'),display:document.fonts.check('26px "Parade Ink Display"'),faces:[...document.fonts].map(face=>({family:face.family,status:face.status}))},safeArea:getComputedStyle(document.querySelector('.world-shell')).paddingBottom}
})
const settle=async()=>{
 await page.evaluate(()=>document.fonts.ready)
 await expect.poll(()=>page.evaluate(async()=>{
  const observed=(await import('/src/features/parade-ink/world-hud/native-observer.mjs')).observe()
  const box=document.querySelector('.shell-world-host canvas')?.parentElement?.getBoundingClientRect()
  return !!box&&observed.camera.width===Math.max(1,Math.floor(box.width))&&observed.camera.height===Math.max(1,Math.floor(box.height))
 }),{timeout:5000}).toBe(true)
 await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))))
}
const continuity=async(label)=>{
 const s=await state();assert.equal(s.active,1,label+' active sockets');assert.equal(s.canvases,1,label+' World canvases');assert.equal(s.peak,1,label+' peak sockets');assert.equal(s.documentId,report.initial.documentId,label+' document')
 assert.deepEqual(s.avatar,report.initial.avatar,label+' approved avatar');assert.deepEqual(s.manifestPin,report.initial.manifestPin,label+' manifest')
 assert(await page.evaluate(()=>document.querySelector('.shell-world-host canvas')===window.__PI08_CANVAS__),label+' canvas identity')
 const observed=await page.evaluate(async()=> (await import('/src/features/parade-ink/world-hud/native-observer.mjs')).observe())
 assert(observed.sameGame&&observed.sameCamera,label+' incumbent game/camera identity');report.lastObservation=observed
 return s
}
const capture=async(name,width,height)=>{
 await page.setViewportSize({width,height});await settle()
 const metrics=await layout();assert.equal(metrics.DPR,1);
 assert.equal(metrics.title[0].height,48,name+' fixed title height')
 for(const era of metrics.eras){assert.equal(era.size,'18px',name+' fixed era type');assert.equal(era.line,'22px',name+' fixed era leading')}
 for(const tab of metrics.nav){assert.equal(tab.size,'14px',name+' fixed tab label type')}
 for(const selected of [...metrics.eras.filter(x=>x.pressed==='true'),...metrics.nav.filter(x=>x.current==='page')]){assert.equal(selected.background,'rgb(23, 23, 23)',name+' selected ink');assert.equal(selected.color,'rgb(255, 255, 255)',name+' selected white')}
 assert.equal(metrics.overflow,false,name+' overflow')
 assert(metrics.fonts.body&&metrics.fonts.display,name+' pinned fonts')
 for(const family of ['Parade Ink Text','Parade Ink Display'])assert(metrics.fonts.faces.some(face=>face.family.replaceAll('"','')===family&&face.status==='loaded'),name+' loaded font '+family)
 for(const c of [...metrics.eras,...metrics.action,...metrics.nav,...metrics.headerControls])assert(c.width>=43.9&&c.height>=43.9,name+' 44px hit '+c.text)
 await page.screenshot({path:resolve(out,name+'.png')});report.captures[name]=metrics
 await continuity(name)
}
try{
 await server.listen();report.port=server.httpServer.address().port
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 page=await browser.newPage({viewport:{width:390,height:775},deviceScaleFactor:1});page.setDefaultTimeout(8000);page.setDefaultNavigationTimeout(20000)
 page.on('pageerror',error=>report.errors.push(error.message))
 page.on('response',response=>{
  const path=new URL(response.url()).pathname,row=publicFiles.get(path)
  if(!row||!response.ok())return
  assetPending=assetPending.then(async()=>{
   const bytes=await response.body(),blob=createHash('sha1').update('blob '+bytes.length+'\0').update(bytes).digest('hex')
   assert.equal(blob,row.gitBlob,'Served public asset matches frozen Git blob '+path)
   usedAssets.set(path,{path:row.path,gitBlob:blob,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')})
  }).catch(error=>{report.errors.push(String(error))})
 })
 await page.goto(`http://127.0.0.1:${report.port}/login?caller=main&era=REVENGE&scenario=eligible-attendee-contract-fixture`)
 await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password');await page.getByRole('button',{name:'Sign in with email'}).click()
 stage='default-world'
 await expect.poll(async()=>{try{return(await proof()).actors.length}catch{return 0}},{timeout:20000}).toBeGreaterThan(0)
 await expect(page.locator('.pi08-world-context')).toBeVisible();report.initial=await state();report.world=await proof()
 await page.evaluate(()=>{window.__PI08_CANVAS__=document.querySelector('.shell-world-host canvas')})
 report.initialObservation=await page.evaluate(async()=> (await import('/src/features/parade-ink/world-hud/native-observer.mjs')).observe())
 assert.equal(report.initialObservation.fixture.map.revision,4);assert.equal(report.initialObservation.fixture.map.contentHash,'sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8')
 const eras=page.getByRole('group',{name:'Choose an era'})
 if(!retained){
 await capture('world-390-reference-height',390,775)
 await capture('world-390-default',390,844)
 await capture('world-360',360,800)
 await capture('world-430',430,932)
 stage='details-keyboard-menu'
 await page.setViewportSize({width:390,height:844});await settle()
 const minimize=page.getByRole('button',{name:'Minimize World details'});await minimize.focus();await page.keyboard.press('Enter');await expect(page.getByText('Virtual presence, not physical arrival.')).toHaveCount(0)
 await page.getByRole('button',{name:'Show World details'}).click();await expect(page.getByText('Virtual presence, not physical arrival.')).toBeVisible()
 const menu=page.getByRole('button',{name:'World destinations'});await menu.click();await expect(page.getByRole('navigation',{name:'Account shortcuts'})).toBeVisible();await page.getByRole('link',{name:'Settings',exact:true}).focus();await page.keyboard.press('Escape');await expect(menu).toBeFocused();await expect(menu).toHaveAttribute('aria-expanded','false')
 report.keyboard={captionReversible:true,menuEscapeFocus:true,focusedOutline:await menu.evaluate(el=>{const s=getComputedStyle(el);return{style:s.outlineStyle,width:s.outlineWidth,color:s.outlineColor}})}
 await page.screenshot({path:resolve(out,'world-keyboard-focus.png')})
 stage='context-action'
 await page.getByRole('link',{name:'Nearby',exact:true}).click();await expect(page).toHaveURL(/\/world\/nearby$/)
 await continuity('context Nearby');await page.getByRole('button',{name:'Back to previous panel',exact:true}).click()
 await expect(page.locator('.pi08-world-context')).toBeVisible()
 report.contextAction={route:'/world/nearby',returnedByIncumbentPanelControl:true}
 stage='era-persistence'
 report.defaultEra=await eras.locator('[aria-pressed="true"]').innerText()
 assert.equal(report.defaultEra.trim(),'REVENGE','Parade Ink appearance must retain original REVENGE default')
 for(const era of ['REVENGE','PARADE','DANGER']){
  await eras.getByRole('button',{name:era[0]+era.slice(1).toLowerCase(),exact:true}).click();await expect(eras.getByRole('button',{name:era[0]+era.slice(1).toLowerCase(),exact:true})).toHaveAttribute('aria-pressed','true')
  await settle();const s=await continuity(era);const l=await layout()
  assert.equal(l.title[0].background,'rgb(23, 23, 23)');assert.equal(l.title[0].height,48)
  assert.equal(l.title[0].font,report.captures['world-390-default'].title[0].font)
  for(const tab of l.eras){assert.equal(tab.size,'18px');assert.equal(tab.line,'22px')}
  report.era.push({era,state:s,layout:l,localPreference:await page.evaluate(()=>Object.entries(localStorage).filter(([key])=>key.startsWith('mcr:theme:')))})
  await page.screenshot({path:resolve(out,'world-era-'+era.toLowerCase()+'.png')})
 }
 }else{
  // Restore the last qualified era as setup; do not replay the passed width/era matrix.
  await eras.getByRole('button',{name:'Danger',exact:true}).click()
  await expect(eras.getByRole('button',{name:'Danger',exact:true})).toHaveAttribute('aria-pressed','true')
  await settle();await continuity('remaining-round setup')
 }
 stage='six-destinations'
 if(!retainedNavigation){
 const nav=page.getByRole('navigation',{name:'Main navigation'})
 for(const label of ['Feed','Meet','Gifts','Wall','Me','World']){
  // The incumbent full-panel control returns the nav; do not change routing/panel authority.
  const reduce=page.getByRole('button',{name:'Reduce panel',exact:true});if(await reduce.isVisible())await reduce.click()
  await nav.getByRole('link',{name:label,exact:true}).click()
  await expect(page).toHaveURL(new RegExp('/'+label.toLowerCase()+'$'))
  // Full panels intentionally hide accessible navigation; inspect its existing DOM state.
  await expect(page.locator('#tab-'+label.toLowerCase())).toHaveAttribute('aria-current','page')
  report.navigation.push({label,url:page.url(),state:await continuity(label)})
 }
 }
 await expect(page.locator('.pi08-world-context')).toBeVisible();await expect(eras.getByRole('button',{name:'Danger',exact:true})).toHaveAttribute('aria-pressed','true')
 report.eraSurvivesNavigation=true
 const retainedFinalBytes=await readFile(resolve(frontend,'../docs/mcr/evidence/parade-ink-world-hud/native06/output/native.json'))
 const retainedFinal=JSON.parse(retainedFinalBytes)
 assert.equal(retainedFinal.failure.stage,'reload-local-era');assert.equal(retainedFinal.safeArea.safeArea,'22px');assert.deepEqual(retainedFinal.reducedMotion,{transition:'0s',animation:'none'})
 report.retainedNative06={sha256:createHash('sha256').update(retainedFinalBytes).digest('hex'),qualified:['safe-area additive22px','short-height/reduced-motion','unmasked reference comparison'],source:'native06/output/native.json'}
 report.safeArea=retainedFinal.safeArea;report.reducedMotion=retainedFinal.reducedMotion;report.reference=retainedFinal.reference
 if(!retainedFinal){
 stage='safe-area'
 const session=await page.context().newCDPSession(page)
 await session.send('Emulation.setSafeAreaInsetsOverride',{insets:{top:12,right:0,bottom:20,left:0}})
 report.safeArea=await layout();assert.equal(report.safeArea.safeArea,'22px','nonzero safe-area bottom remains additive')
 await page.screenshot({path:resolve(out,'world-safe-area.png')})
 await session.send('Emulation.setSafeAreaInsetsOverride',{insets:{top:0,right:0,bottom:0,left:0}});await session.detach()
 stage='short-height-reduced-motion'
 await page.emulateMedia({reducedMotion:'reduce'});await capture('world-short-reduced-motion',390,500)
 report.reducedMotion=await page.locator('.pi08-world-title').evaluate(el=>({transition:getComputedStyle(el).transitionDuration,animation:getComputedStyle(el).animationName}))
 stage='reference-comparison'
 await page.setViewportSize({width:390,height:775});await settle()
 // Generate an unmasked normalized reference and side-by-side full World comparison in-browser.
 const actual=await readFile(resolve(retainedRoot,'world-390-reference-height.png'))
 const artifacts=await page.evaluate(async({reference,actual})=>{
  const load=async(src)=>{const i=new Image();i.src=src;await i.decode();return i}
  const ref=await load(reference),app=await load(actual),crop=document.createElement('canvas');crop.width=390;crop.height=775
  crop.getContext('2d').drawImage(ref,436,60,396,787,0,0,390,775)
  const pair=document.createElement('canvas');pair.width=780;pair.height=775;const c=pair.getContext('2d');c.drawImage(crop,0,0);c.drawImage(app,390,0)
  const overlay=document.createElement('canvas');overlay.width=390;overlay.height=775;const o=overlay.getContext('2d');o.drawImage(crop,0,0);o.globalAlpha=.5;o.drawImage(app,0,0)
  return{reference:crop.toDataURL(),comparison:pair.toDataURL(),overlay:overlay.toDataURL()}
 },{reference:'data:image/jpeg;base64,'+photo.toString('base64'),actual:'data:image/png;base64,'+actual.toString('base64')})
 for(const [name,data] of Object.entries(artifacts))await writeFile(resolve(out,'photo2-'+name+'.png'),Buffer.from(data.split(',')[1],'base64'))
 report.reference.fixedRegions={title:[440,65,827,112],eras:[446,120,820,160],context:[447,687,820,761],navigation:[441,774,827,835]}
 const rect=values=>({x:(values[0]-436)*390/396,y:(values[1]-60)*390/396,width:(values[2]-values[0])*390/396,height:(values[3]-values[1])*390/396})
 report.reference.geometryDeltas={}
 const actualMetrics=retained.captures['world-390-reference-height']
 for(const [key,actual] of [['title',actualMetrics.title[0]],['context',actualMetrics.context[0]]]){
  const expected=rect(report.reference.fixedRegions[key]);report.reference.geometryDeltas[key]={reference:expected,actual,delta:Object.fromEntries(['x','y','width','height'].map(axis=>[axis,actual[axis]-expected[axis]]))}
 }
 report.reference.controlType={title:'26/32 Oswald',era:'18/22 Oswald',context:'18/22 Roboto',caption:'14/18 Roboto',navigation:'14/18 Oswald;24px icons',hitArea:'>=44x44; image glyphs24px',inset:'6px',frame:'2px symmetric border+2px keyline;safe area additive'}
 report.reference.literalDeviations=['Approved current Map4 and approved static C06v1 replace the sample illustrated scene/avatars; no art repaint or masking.','World / N in view and Nearby replace sample Plaza / 8 people here and JOIN. Count is authorized replication including self, not room occupancy.','Existing connection/renderer notices and World controls stay visible; caption/control wrapping can increase context/scene height.','REVENGE remains the fixture default; reference image selects PARADE. Each era remains independent.','Window glyph semantics: caption minimize/restore, existing destinations, real Feed navigation.']
 report.reference.comparison='Unmasked full World: normalized reference left, initial actual App REVENGE390x775 right; 50% overlay separate. Metrics and image use the same initial capture. Map/actors and truthful context differ literally; no whole-World/card masks.'
 }
 stage='reload-local-era'
 const beforeReload=report.initial.documentId
 // Keep the explicit positive fixture selection on reload; it is not a production capability.
 await page.evaluate(()=>history.replaceState(history.state,'',location.pathname+'?caller=main&era=REVENGE&scenario=eligible-attendee-contract-fixture'))
 report.reloadFixture='Same explicit controlled positive fixture selectors retained in URL before browser reload; no authority/controller override.'
 await page.reload()
 await expect.poll(async()=>await page.locator('.pi08-world-context').isVisible()||await page.getByRole('button',{name:'Sign in with email'}).isVisible()||await page.getByRole('link',{name:'Sign in',exact:true}).isVisible()).toBe(true)
 if(await page.getByRole('link',{name:'Sign in',exact:true}).isVisible()){
  await page.getByRole('link',{name:'Sign in',exact:true}).click()
  await expect(page.getByRole('button',{name:'Sign in with email'})).toBeVisible()
 }
 // Canonical Auth may require the existing login flow in a new document.
 if(await page.getByRole('button',{name:'Sign in with email'}).isVisible()){
  await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password');await page.getByRole('button',{name:'Sign in with email'}).click()
 }
 await expect.poll(async()=>{try{return(await proof()).actors.length}catch{return 0}},{timeout:20000}).toBeGreaterThan(0)
 await expect(page.getByRole('group',{name:'Choose an era'}).getByRole('button',{name:'Danger',exact:true})).toHaveAttribute('aria-pressed','true')
 report.afterReload=await state();assert.notEqual(report.afterReload.documentId,beforeReload);assert.equal(report.afterReload.active,1);assert.equal(report.afterReload.peak,1);assert.equal(report.afterReload.canvases,1)
 report.localEraSurvivesNewDocument=true
 stage='presence-withdrawal'
 await expect(page.locator('.pi08-world-context')).toContainText(/\d+ in view/)
 await page.evaluate(()=>{window.__PI08_RUNTIME__.hold(true);window.__PI08_RUNTIME__.offline(true);window.__PI08_RUNTIME__.close()})
 await expect(page.locator('.pi08-world-context')).not.toContainText(/\d+ in view/)
 report.withdrawnContext=await page.locator('.pi08-world-context').innerText();await page.screenshot({path:resolve(out,'world-presence-unavailable.png')})
 await assetPending;report.usedPublicAssets={source:publicReadset.source,tree:publicReadset.tree,files:Object.fromEntries([...usedAssets].sort(([a],[b])=>a.localeCompare(b)))}
 assert(usedAssets.size>0,'Actual World public assets observed and verified')
 foundation.verify();assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(error){report.failure={stage,message:String(error),stack:error.stack};if(page)await page.screenshot({path:resolve(out,'failure.png')}).catch(()=>{})}
finally{await browser?.close();await server.close();report.resourcesClosed=true;await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2)+'\n')}
if(report.result!=='PASS')process.exitCode=1
