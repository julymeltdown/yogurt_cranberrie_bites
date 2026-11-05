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
source.owned.add('frontend/src/world/camera/port.ts')
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
async function settle(){
 await page.evaluate(async()=>{await document.fonts.load('400 18px "Parade Ink Text"');await document.fonts.load('700 26px "Parade Ink Display"');await document.fonts.ready})
 await expect.poll(async()=>{const v=await observe();return v.camera?.width>0&&v.sameGame&&v.sameCamera},{timeout:10000}).toBe(true)
 await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))
}
async function continuity(label){const s=await state(),v=await observe();assert.equal(s.active,1,label);assert.equal(s.canvases,1,label);assert.equal(s.peak,1,label);assert(v.sameGame&&v.sameCamera,label);assert(await page.evaluate(()=>document.querySelector('.shell-world-host canvas')===window.__PI08_CANVAS__),label);return s}
try{
 await mkdir(out,{recursive:true});await server.listen();await server.environments.client.depsOptimizer?.scanProcessing
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-breakpad']});report.browser=browser.version()
 page=await browser.newPage({viewport:{width:390,height:775},deviceScaleFactor:1,reducedMotion:'reduce'});page.on('pageerror',e=>report.errors.push(e.message))
 await page.goto(`http://127.0.0.1:${port}/login?caller=main&era=PARADE&scenario=eligible-attendee-contract-fixture`)
 stage='login';await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password');await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
 stage='world';await expect(page.locator('.pi08-world-context')).toBeVisible({timeout:30000});await expect.poll(async()=>{try{return(await observe()).textures.length}catch{return 0}},{timeout:20000}).toBeGreaterThan(0)
 await page.evaluate(()=>{window.__PI08_CANVAS__=document.querySelector('.shell-world-host canvas')});report.initial=await state();assert.equal(report.initial.theme,'PARADE');report.preWorldPolicy=await page.evaluate(()=>window.__PI08_POLICY__);assert(report.preWorldPolicy.reads.length>0&&report.preWorldPolicy.reads.every(read=>read.recognized&&read.authorized&&read.method==='GET'),'Actual protected service-policy fixture read')
 stage='zoom-controls';await settle()
 const initialProof=await page.evaluate(()=>window.__PI08_RUNTIME__.worldProof())
 const anchors=proof=>proof.objects.filter(item=>item.texture?.startsWith('mcr-avatar-')).map(({x,y,texture})=>({x,y,texture}))
 const help=page.getByText('Controls & help',{exact:true});await help.click()
 const before=await observe(),fit=before.camera.zoom
 assert(fit>0&&fit<1,'Actual reference camera is fitted below logical zoom1')
 report.fit=fit;report.zoomSteps=[]
 for(let logical=2;logical<=8;logical++){
  await page.getByRole('button',{name:'Zoom in',exact:true}).click()
  await expect.poll(async()=>Math.abs((await observe()).camera.zoom-logical*fit),{timeout:3000}).toBeLessThan(1e-8)
  await continuity('zoom+'+logical);report.zoomSteps.push({logical,actual:(await observe()).camera.zoom})
 }
 await page.screenshot({path:resolve(out,'world-zoom-8.png')})
 for(let logical=7;logical>=1;logical--){
  await page.getByRole('button',{name:'Zoom out',exact:true}).click()
  await expect.poll(async()=>Math.abs((await observe()).camera.zoom-logical*fit),{timeout:3000}).toBeLessThan(1e-8)
  await continuity('zoom-'+logical);report.zoomSteps.push({logical,actual:(await observe()).camera.zoom})
 }
 await page.getByRole('button',{name:'Zoom out',exact:true}).click()
 assert(Math.abs((await observe()).camera.zoom-fit)<1e-8,'Logical lower bound is idempotent')
 await help.click();await settle()
 const afterProof=await page.evaluate(()=>window.__PI08_RUNTIME__.worldProof())
 assert.deepEqual(anchors(afterProof),anchors(initialProof),'Zoom does not move authoritative actor feet')
 assert.deepEqual((await state()).map,report.initial.map,'Map identity remains exact')
 report.initialActorAnchors=anchors(initialProof);report.finalActorAnchors=anchors(afterProof)
 report.finalObservation=await observe()
 await page.screenshot({path:resolve(out,'world-zoom-restored.png')})
 assert.equal(report.errors.length,0)
 report.retainedEvidence=['../native08/native.json','../native09/native.json']
 report.status='PASS_ACTUAL_LOGICAL_ZOOM_1_TO_8_AND_BACK_ONLY_FULL_PHOTO2_OPEN'
}catch(error){report.failure={stage,message:String(error),stack:error.stack};throw error}
finally{if(page){report.finalPolicy=await page.evaluate(()=>window.__PI08_POLICY__??null).catch(()=>null);report.finalRuntime=await state().catch(()=>null)}report.postimageChecks=[];for(const row of inputPins.implementationAndDrivers){const raw=await readFile(resolve(root,row.path));const match=raw.length===row.bytes&&sha(raw)===row.sha256;report.postimageChecks.push({path:row.path,match});if(!match)report.errors.push('Input drift '+row.path)}if(page&&report.status==='FAIL')await page.screenshot({path:resolve(out,'failure.png')}).catch(()=>{});if(browser)await browser.close();await server.close();report.served=[...source.served.values()];report.source=[...source.used.values()];await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2)+'\n')}
