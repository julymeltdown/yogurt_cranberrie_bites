// Execute only inside a fresh finite browser offer. Actual canonical main/Auth/providers/Phaser.
import assert from 'node:assert/strict'
import { readFile,writeFile,mkdir } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import tailwind from '@tailwindcss/vite'
import { chromium,expect } from '@playwright/test'
import { gitSource,baseline } from './git-source.mjs'
const root=resolve(import.meta.dirname,'../../../../../../..'),frontend=resolve(root,'frontend')
const out=process.env.PI08_OUTPUT,scratch=process.env.PI08_SCRATCH,port=Number(process.env.PI08_PORT)
assert(out&&scratch&&port===5367,'Fresh finite PI08 allocation/strict5367 required')
const source=gitSource(root),sha=raw=>createHash('sha256').update(raw).digest('hex')
// Literal Root153 transfer: only recognized-reference display fallback; exact postimage frozen below.
source.owned.add('frontend/src/world/scenes/MapScene.ts')
const inputBytes=await readFile((process.env.PI08_INPUT_FILE??resolve(root,'docs/mcr/evidence/pi08-finish-01/native01-inputs.json'))),inputPins=JSON.parse(inputBytes)
const operator=JSON.parse(source.bytes('frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'))
const input=JSON.parse(source.bytes('frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/local-input.json'))
// Exact TEST_ONLY pair from immutable pre-world-policy/__tests__/fixtures.ts; no production recognition.
const policyRelease={version:1,applicationVersion:'0.1.0',buildId:'a'.repeat(64),sourceRevision:'b'.repeat(40)}
source.bytes('frontend/src/app/pre-world-policy/__tests__/fixtures.ts')
const photo=await readFile(process.env.PI08_PHOTO);assert.equal(sha(photo),'9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27')
const report={status:'FAIL',classification:'ACTUAL_DEFAULT_APP_CONTROLLED_HTTP_C05_NOT_LIVE',baseline,exercisedSource:execFileSync('git',['rev-parse','HEAD'],{cwd:root}).toString().trim(),inputReadsetSha256:sha(inputBytes),reference:{sha256:sha(photo),crop:[436,60,832,847],normalization:390/396},captures:{},errors:[],served:[],source:[]}
const server=await createServer({root:frontend,configFile:false,publicDir:false,plugins:[source.plugin,react(),tailwind()],cacheDir:resolve(scratch,'vite'),logLevel:'warn',
 define:{'import.meta.env.VITE_MCR_APPLICATION_RELEASE':JSON.stringify(policyRelease),'import.meta.env.VITE_MCR_EVENT_ID':JSON.stringify(operator.event.event_id),'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG':JSON.stringify(JSON.stringify(input))},
 optimizeDeps:{entries:[],noDiscovery:true,include:['react','react-dom','react-dom/client','react/jsx-runtime','react/jsx-dev-runtime','react-router-dom','@tanstack/react-query','react-hook-form','@hookform/resolvers/zod','zod','phaser','ky','zustand','browser-image-compression']},
 resolve:{dedupe:['react','react-dom','react-router','react-router-dom','@tanstack/react-query']},server:{host:'127.0.0.1',port,strictPort:true,hmr:false,watch:null,fs:{allow:[root,process.env.MCR_FRONTEND_DEPENDENCIES]}}})
let browser,page,stage='startup'
const state=()=>page.evaluate(()=>window.__PI08_RUNTIME__.state())
const observe=()=>page.evaluate(async()=>(await import('/src/features/parade-ink/world-hud/native-observer.mjs')).observe())
const layout=()=>page.evaluate(()=>{
 const read=selector=>[...document.querySelectorAll(selector)].filter(el=>el.getClientRects().length).map(el=>{const r=el.getBoundingClientRect(),s=getComputedStyle(el);return{x:r.x,y:r.y,width:r.width,height:r.height,text:el.textContent.trim(),font:s.fontFamily,size:s.fontSize,line:s.lineHeight,weight:s.fontWeight,color:s.color,background:s.backgroundColor,outline:s.outlineStyle,pressed:el.getAttribute('aria-pressed'),current:el.getAttribute('aria-current')}})
 return{viewport:[innerWidth,innerHeight],DPR:devicePixelRatio,overflow:document.documentElement.scrollWidth>innerWidth,title:read('.pi08-world-title'),eras:read('.pi08-era-controls button'),context:read('.pi08-world-context'),copy:read('.pi08-context-copy'),action:read('.pi08-context-action'),navigation:read('.pi08-navigation a'),headerControls:read('.pi08-world-title button,.pi08-world-title a'),canvas:read('.shell-world-host canvas'),bottomPadding:getComputedStyle(document.querySelector('.world-shell')).paddingBottom,faces:[...document.fonts].map(f=>({family:f.family,status:f.status}))}
})
async function settle(){
 await page.evaluate(async()=>{await document.fonts.load('400 18px "Parade Ink Text"');await document.fonts.load('700 26px "Parade Ink Display"');await document.fonts.ready})
 await expect.poll(async()=>{const v=await observe();return v.camera?.width>0&&v.sameGame&&v.sameCamera},{timeout:10000}).toBe(true)
 await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))
}
async function continuity(label){const s=await state(),v=await observe();assert.equal(s.active,1,label);assert.equal(s.canvases,1,label);assert.equal(s.peak,1,label);assert(v.sameGame&&v.sameCamera,label);assert(await page.evaluate(()=>document.querySelector('.shell-world-host canvas')===window.__PI08_CANVAS__),label);return s}
async function capture(name,width,height){await page.setViewportSize({width,height});await settle();const m=await layout();assert.equal(m.overflow,false,name+' page overflow');assert.equal(m.DPR,1);for(const control of [...m.eras,...m.action,...m.navigation,...m.headerControls])assert(control.width>=43.9&&control.height>=43.9,name+' 44px hit '+control.text);for(const family of ['Parade Ink Text','Parade Ink Display'])assert(m.faces.some(f=>f.family.replaceAll('"','')===family&&f.status==='loaded'),name+' font '+family);const camera=(await observe()).camera;assert(camera.width/camera.zoom<=768+1e-6&&camera.height/camera.zoom<=1296+1e-6,name+' camera view stays covered by unchanged world');await continuity(name);await page.screenshot({path:resolve(out,name+'.png')});report.captures[name]=m;return m}
async function compare(actual){
 const artifacts=await page.evaluate(async({photo,actual,canvasRect})=>{
  const image=src=>new Promise((resolve,reject)=>{const i=new Image();i.onload=()=>resolve(i);i.onerror=reject;i.src=src})
  const [ref,app]=await Promise.all([image(photo),image(actual)]),scale=390/396,h=Math.round(787*scale)
  const r=document.createElement('canvas');r.width=390;r.height=h;const rc=r.getContext('2d');rc.drawImage(ref,436,60,396,787,0,0,390,h)
  const pair=document.createElement('canvas');pair.width=780;pair.height=h;pair.getContext('2d').drawImage(r,0,0);pair.getContext('2d').drawImage(app,390,0)
  const overlay=document.createElement('canvas');overlay.width=390;overlay.height=h;const oc=overlay.getContext('2d');oc.drawImage(r,0,0);oc.globalAlpha=.5;oc.drawImage(app,0,0)
  const diff=document.createElement('canvas');diff.width=390;diff.height=h;const dc=diff.getContext('2d');dc.drawImage(r,0,0);const first=dc.getImageData(0,0,390,h);dc.clearRect(0,0,390,h);dc.drawImage(app,0,0);const second=dc.getImageData(0,0,390,h);for(let i=0;i<first.data.length;i+=4){for(let c=0;c<3;c++)first.data[i+c]=Math.abs(first.data[i+c]-second.data[i+c]);first.data[i+3]=255}dc.putImageData(first,0,0)
  const scene=document.createElement('canvas');scene.width=780;scene.height=500;const sc=scene.getContext('2d');sc.fillStyle='#fff';sc.fillRect(0,0,780,500);sc.drawImage(ref,441,173,384,504,0,0,390,500);sc.drawImage(app,canvasRect.x,canvasRect.y,canvasRect.width,canvasRect.height,390,0,390,500)
  return{reference:r.toDataURL(),pair:pair.toDataURL(),overlay:overlay.toDataURL(),diff:diff.toDataURL(),scenery:scene.toDataURL()}
 },{photo:'data:image/jpeg;base64,'+photo.toString('base64'),actual:'data:image/png;base64,'+actual.toString('base64'),canvasRect:report.captures['world-parade-390-reference'].canvas[0]})
 for(const[name,data]of Object.entries(artifacts))await writeFile(resolve(out,'photo2-'+name+'.png'),Buffer.from(data.split(',')[1],'base64'))
 report.reference.transforms={full:'crop[436,60,832,847], scale390/396, no repaint/masks; pair reference-left/App-right; 50% overlay; absolute RGB difference',scenery:'reference crop[441,173,825,677] and exact actual canvas rect each normalized to390x500 for composition inspection; full pair retains original aspect ratios'}
 const rect=([x,y,x2,y2])=>({x:(x-436)*390/396,y:(y-60)*390/396,width:(x2-x)*390/396,height:(y2-y)*390/396})
 report.reference.regions={title:rect([440,65,827,112]),context:rect([447,687,820,761]),navigation:rect([441,774,827,835])};report.reference.deltas={}
 const m=report.captures['world-parade-390-reference'];for(const[key,value]of Object.entries(report.reference.regions)){const a=key==='navigation'?{...m.navigation[0],width:m.navigation.reduce((v,x)=>v+x.width,0)}:m[key][0];report.reference.deltas[key]=Object.fromEntries(['x','y','width','height'].map(k=>[k,a[k]-value[k]]))}
 report.geometryQualified=['x','y','width','height'].every(k=>Math.abs(report.reference.deltas.context[k])<=1)
}
try{
 await mkdir(out,{recursive:true});await server.listen();await server.environments.client.depsOptimizer?.scanProcessing
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-breakpad']});report.browser=browser.version()
 page=await browser.newPage({viewport:{width:390,height:775},deviceScaleFactor:1,reducedMotion:'reduce'});page.on('pageerror',e=>report.errors.push(e.message))
 await page.goto(`http://127.0.0.1:${port}/login?caller=main&era=PARADE&scenario=eligible-attendee-contract-fixture`)
 stage='login';await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password');await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
 stage='world';await expect(page.locator('.pi08-world-context')).toBeVisible({timeout:30000});await expect.poll(async()=>{try{return(await observe()).textures.length}catch{return 0}},{timeout:20000}).toBeGreaterThan(0)
 await page.evaluate(()=>{window.__PI08_CANVAS__=document.querySelector('.shell-world-host canvas')});report.initial=await state();assert.equal(report.initial.theme,'PARADE');report.preWorldPolicy=await page.evaluate(()=>window.__PI08_POLICY__);assert(report.preWorldPolicy.reads.length>0&&report.preWorldPolicy.reads.every(read=>read.recognized&&read.authorized&&read.method==='GET'),'Actual protected service-policy fixture read')
 await capture('world-parade-390-reference',390,775);report.initialObservation=await observe();assert(Math.abs(report.initialObservation.camera.zoom-Math.min(1,report.initialObservation.camera.width/584,report.initialObservation.camera.height/584))<1e-9,'Actual incumbent recognized-reference display scale');report.sceneProof=await page.evaluate(()=>window.__PI08_RUNTIME__.worldProof());await compare(await readFile(resolve(out,'world-parade-390-reference.png')))
 if(process.env.PI08_DISPLAY_ONLY==='1'){
  stage='changed-display-widths';await capture('world-parade-360',360,800);await capture('world-parade-430',430,932)
  stage='short-display-regression';await capture('world-short',390,500)
  report.shortObservation=await observe();report.shortSceneProof=await page.evaluate(()=>window.__PI08_RUNTIME__.worldProof())
  const props=proof=>proof.objects.filter(item=>item.texture?.startsWith('mcr-reviewed-map-')).map(({x,y,depth,scaleX,scaleY,frame})=>({x,y,depth,scaleX,scaleY,frame}))
  assert.deepEqual(props(report.shortSceneProof),props(report.sceneProof),'Resizing changes display camera only; authored scenery anchors remain exact')
  assert.deepEqual((await state()).map,report.initial.map,'Authoritative map identity remains exact')
  assert.equal(report.errors.length,0);assert(report.geometryQualified)
  report.retainedMatrix={path:'../native08/native.json',qualification:'Original App matrix PASS at prior exact postimage; current expression fixes cover/look-ahead only, unchanged UI/fixture checks retained; not replayed'}
  report.status='PASS_CHANGED_DISPLAY_AND_SHORT_VIEWPORT_RETAINED_NATIVE08_MATRIX_SCENERY_REQUIRES_DEVELOPER_INSPECTION'
 }else{
 await capture('world-parade-390',390,844);await capture('world-parade-360',360,800);await capture('world-parade-430',430,932)
 stage='focus';await page.setViewportSize({width:390,height:844});const minimize=page.getByRole('button',{name:'Minimize World details',exact:true});await minimize.focus();await page.keyboard.press('Enter');await expect(page.getByText('Virtual presence, not physical arrival.',{exact:true})).toHaveCount(0);await page.getByRole('button',{name:'Show World details',exact:true}).click();await expect(page.getByText('Virtual presence, not physical arrival.',{exact:true})).toBeVisible()
 const menu=page.getByRole('button',{name:'World destinations',exact:true});await menu.focus();await page.keyboard.press('Enter');await expect(menu).toHaveAttribute('aria-expanded','true');await page.getByRole('link',{name:'Inbox',exact:true}).focus();await page.keyboard.press('Escape');await expect(menu).toBeFocused();report.focus={captionReversible:true,menuEscapeFocus:true};await page.screenshot({path:resolve(out,'world-focus.png')})
 stage='independent-era';const eras=page.getByRole('group',{name:'Choose an era'});await eras.getByRole('button',{name:'Revenge',exact:true}).click();await expect(eras.getByRole('button',{name:'Revenge',exact:true})).toHaveAttribute('aria-pressed','true');await continuity('Revenge selection');await eras.getByRole('button',{name:'Parade',exact:true}).click();await expect(eras.getByRole('button',{name:'Parade',exact:true})).toHaveAttribute('aria-pressed','true');await continuity('Parade selection');report.independentEra=true
 stage='short';await capture('world-short',390,500)
 stage='safe-area';await page.setViewportSize({width:390,height:844});const cdp=await page.context().newCDPSession(page);await cdp.send('Emulation.setSafeAreaInsetsOverride',{insets:{top:0,left:0,bottom:20,right:0}});await capture('world-safe-area',390,844);assert.equal((await layout()).bottomPadding,'30px');await cdp.send('Emulation.setSafeAreaInsetsOverride',{insets:{top:0,left:0,bottom:0,right:0}})
 stage='withdrawal';await page.evaluate(()=>{window.__PI08_RUNTIME__.hold(true);window.__PI08_RUNTIME__.close()});await expect(page.locator('.pi08-world-context')).toContainText('Presence unavailable');await expect(page.locator('.pi08-context-action')).toHaveText('Rooms');report.withdrawal=true;await page.screenshot({path:resolve(out,'world-presence-unavailable.png')})
 assert.equal(report.errors.length,0,'Page errors');assert(report.geometryQualified,'Context bounds outside CSS-pixel rounding');report.status='PASS_GEOMETRY_AND_BEHAVIOR_SCENERY_REQUIRES_DEVELOPER_INSPECTION'
 }
}catch(error){report.failure={stage,message:String(error),stack:error.stack};throw error}
finally{if(page){report.finalPolicy=await page.evaluate(()=>window.__PI08_POLICY__??null).catch(()=>null);report.finalRuntime=await state().catch(()=>null)}report.postimageChecks=[];for(const row of inputPins.implementationAndDrivers){const raw=await readFile(resolve(root,row.path));const match=raw.length===row.bytes&&sha(raw)===row.sha256;report.postimageChecks.push({path:row.path,match});if(!match)report.errors.push('Input drift '+row.path)}if(page&&report.status==='FAIL')await page.screenshot({path:resolve(out,'failure.png')}).catch(()=>{});if(browser)await browser.close();await server.close();report.served=[...source.served.values()];report.source=[...source.used.values()];await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2)+'\n')}
