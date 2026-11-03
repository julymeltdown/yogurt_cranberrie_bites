import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../..'),out=resolve(import.meta.dirname,'evidence')
await mkdir(out,{recursive:true})
const cache=await mkdtemp(resolve(tmpdir(),'mcr-reference-world-')),hash=data=>createHash('sha256').update(data).digest('hex')
const sources=['src/world/scenes/MapScene.ts','src/world/illustrated/phaser.ts','src/world/illustrated/model.ts','src/world/camera/port.ts','src/world/runtime.ts','src/world/input/controller.ts','src/world/input/controls.css','src/world/input/model.ts','src/world/input/simulation.ts','src/world/composition/index.ts','src/world/map/resources.ts','src/world/theme/compatibility.ts','src/world/__tests__/reference-world/native.ts','src/world/__tests__/reference-world/native.html','src/world/__tests__/reference-world/native.mjs','src/world/__tests__/illustrated/fixtures.ts','src/world/__tests__/fixtures/movement-data.ts','public/world/illustrated-runtime/bindings.json','public/world/maps/geometry.json','../contracts/assets/v1/theme-manifest.schema.json','../contracts/realtime/v1/policy.json']
const report={result:'FAIL',qualification:'Production Phaser/loadMap/scenery/input/runtime with original reviewed scenery and deterministic local intent fixture. Explicit test-only host approximates assigned root seam; no canonical Auth, socket, live self/entity/avatar, device or integrated route acceptance.',base:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),sources:Object.fromEntries(await Promise.all(sources.map(async p=>[p,hash(await readFile(resolve(root,p)))]))),cases:[],artifacts:{},errors:[]}
report.referenceHash='d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617'
const server=await createServer({root,configLoader:'runner',cacheDir:cache,logLevel:'warn',server:{host:'127.0.0.1',port:0,strictPort:false}});await server.listen()
let browser,page,stage='start'
const frame=()=>page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))
const state=()=>page.evaluate(()=>window.referenceWorld.state())
const invoke=(method,...args)=>page.evaluate(async({method,args})=>window.referenceWorld[method](...args),{method,args})
const shot=async name=>{await frame();const file=name+'.jpg';await page.screenshot({path:resolve(out,file),type:'jpeg',quality:82});report.artifacts[file]=hash(await readFile(resolve(out,file)))}
const measure=()=>page.evaluate(()=>{const canvas=document.querySelector('#host canvas'),r=canvas.getBoundingClientRect(),host=document.querySelector('#host').getBoundingClientRect();let top=Math.max(0,r.top),bottom=Math.min(innerHeight,r.bottom);for(let e=canvas.parentElement;e;e=e.parentElement){if(/hidden|clip|auto|scroll/.test(getComputedStyle(e).overflowY)){const q=e.getBoundingClientRect();top=Math.max(top,q.top);bottom=Math.min(bottom,q.bottom)}}const targets=[...document.querySelectorAll('.world-controls button,.world-controls summary')].filter(e=>e.getClientRects().length).map(e=>{const q=e.getBoundingClientRect();return{name:e.getAttribute('aria-label')||e.textContent,x:q.x,y:q.y,width:q.width,height:q.height}});return{scene:{x:r.x,y:r.y,width:r.width,height:r.height,visibleHeight:Math.max(0,bottom-top)},host:{width:host.width,height:host.height},targets,overflow:document.documentElement.scrollWidth>innerWidth}})
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 for(const [width,height]of[[360,800],[390,844],[430,932]]){
  stage=`${width}/startup`;page=await browser.newPage({viewport:{width,height},hasTouch:true,reducedMotion:'reduce'});page.on('pageerror',e=>report.errors.push(e.message));page.setDefaultTimeout(8000)
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/world/__tests__/reference-world/native.html`)
  await expect.poll(()=>page.evaluate(()=>window.referenceWorld?.state().runtime.phase)).toBe('ready');await frame()
  const initial=await state(),normal=await measure();assert.equal(initial.mounts,1);assert.equal(initial.canvas,1);assert.equal(initial.camera.zoom,1);assert.equal(initial.scenery.textures,3);assert.equal(initial.scenery.objects,30)
  assert(!normal.overflow);assert(normal.scene.visibleHeight>=height-132);assert(normal.targets.every(t=>t.width>=44&&t.height>=44));assert(normal.targets.every(t=>t.y>=normal.scene.y&&t.y+t.height<=normal.scene.y+normal.scene.height))
  const stageTop=await invoke('project',{x:96,y:329}),stageBottom=await invoke('project',{x:320,y:480})
  assert(stageTop.x>=0&&stageBottom.x<=width);assert(stageTop.y>=normal.scene.y&&stageBottom.y<=normal.scene.y+normal.scene.height)
  await shot(`startup-${width}`)
  const help=page.locator('.world-controls summary'),canvas=page.locator('#host canvas');await expect(help).toHaveAttribute('aria-label','Controls & help')
  stage=`${width}/help`;await help.focus();await page.keyboard.press('Enter');const select=page.getByRole('combobox',{name:'Nearby interaction'});await expect(select).toBeVisible()
  const zoom=page.getByRole('button',{name:'Zoom in',exact:true});await zoom.focus();await page.keyboard.press('Enter');await expect.poll(async()=>(await state()).camera.zoom).toBe(1) // No authority: input may request zoom but cannot move real camera.
  await invoke('self',{x:208,y:528});await zoom.press('Enter');await expect.poll(async()=>(await state()).camera.zoom).toBe(2)
  await page.getByRole('button',{name:'Zoom out',exact:true}).click();await expect.poll(async()=>(await state()).camera.zoom).toBe(1)
  await select.focus();await page.keyboard.press('Escape');await expect(zoom).not.toBeVisible();await expect(help).toBeFocused()
  stage=`${width}/movement`;await canvas.focus();const before=(await state()).movement.foot;await page.keyboard.down('ArrowLeft');await expect.poll(async()=>(await state()).movement.foot.x).toBeLessThan(before.x);await page.keyboard.up('ArrowLeft')
  await expect.poll(async()=>(await state()).intents.at(-1)).toEqual({axisX:0,axisY:0,buttons:[]})
  const stable=(await state()).movement.foot;await frame();assert.deepEqual((await state()).movement.foot,stable)
  const stick=page.locator('.world-joystick'),box=await stick.boundingBox();await page.mouse.move(box.x+box.width*.12,box.y+box.height*.5);await page.mouse.down();await expect.poll(async()=>(await state()).intents.at(-1)?.axisX).toBeLessThan(0)
  await page.evaluate(()=>window.dispatchEvent(new PointerEvent('pointercancel',{pointerId:1,bubbles:true})));await page.mouse.up();await expect.poll(async()=>(await state()).intents.at(-1)).toEqual({axisX:0,axisY:0,buttons:[]})
  await canvas.focus();await page.keyboard.down('ArrowLeft');await expect.poll(async()=>(await state()).intents.at(-1)?.axisX).toBeLessThan(0);await page.evaluate(()=>window.dispatchEvent(new Event('blur')));await page.keyboard.up('ArrowLeft');assert.deepEqual((await state()).intents.at(-1),{axisX:0,axisY:0,buttons:[]});await page.evaluate(()=>window.dispatchEvent(new Event('focus')))
  await canvas.focus();await page.keyboard.down('ArrowLeft');await expect.poll(async()=>(await state()).intents.at(-1)?.axisX).toBeLessThan(0);await page.getByRole('button',{name:'Open panel',exact:true}).click();await page.keyboard.up('ArrowLeft');await expect(stick).toBeDisabled();const paused=(await state()).movement.foot;await frame();assert.deepEqual((await state()).movement.foot,paused);await page.getByRole('button',{name:'Back to world',exact:true}).click();await expect(stick).toBeEnabled()
  stage=`${width}/camera`;await invoke('look',{x:920,y:400});const away=(await state()).camera;await page.getByRole('button',{name:'Recenter',exact:true}).click();const recentered=(await state()).camera;assert.notEqual(away.scrollX,recentered.scrollX)
  const point={x:208,y:528},client=await invoke('project',point),roundtrip=await invoke('roundtrip',client);assert(Math.abs(roundtrip.x-point.x)<1e-6&&Math.abs(roundtrip.y-point.y)<1e-6)
  stage=`${width}/eras`;const identity=(await state()).geometryHash,eraStates=[]
  for(const era of[0,2,1]){await invoke('select',era);await frame();const next=await state();assert.equal(next.geometryHash,identity);assert.equal(next.mounts,1);assert.equal(next.canvas,1);assert.equal(next.scenery.textures,3);assert.deepEqual(next.camera,recentered);eraStates.push({era,themeHash:next.themeHash,scenery:next.scenery});if(width===390&&era!==1)await shot(`era-${era}-390`)}
  stage=`${width}/short`;await page.setViewportSize({width,height:430});await frame();const short=await measure();assert(!short.overflow);assert(short.scene.visibleHeight>=298);assert(short.targets.every(t=>t.width>=44&&t.height>=44));await help.click();await zoom.focus();await expect(zoom).toBeInViewport({ratio:1});await select.focus();await expect(select).toBeInViewport({ratio:1});if(width===390)await shot('short-help-390');await page.keyboard.press('Escape');await expect(help).toBeFocused()
  if(width===390){await page.setViewportSize({width,height});await help.click();await shot('expanded-help-390');await help.click();await invoke('fail');await canvas.focus();await page.keyboard.down('ArrowLeft');await expect(page.getByText('Movement unavailable: input consumer failed.',{exact:true})).toBeVisible();await page.keyboard.up('ArrowLeft');await shot('error-390')}
  const cleanup=await invoke('dispose');assert.equal(cleanup.canvas,0);assert.equal(cleanup.controls,0);assert.equal(cleanup.scenery.textures,0);assert.equal(cleanup.destroys,1)
  report.cases.push({width,height,initial,normal,stageFrame:{top:stageTop,bottom:stageBottom},short,eraStates,cleanup});await page.close()
 }
 stage='positive-selection';page=await browser.newPage({viewport:{width:390,height:844}});page.on('pageerror',e=>report.errors.push(e.message));page.setDefaultTimeout(8000)
 await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/world/__tests__/reference-world/native.html?input=1`)
 await expect.poll(()=>page.evaluate(()=>window.referenceWorld?.state().runtime.phase)).toBe('ready')
 const canvas=page.locator('#host canvas');await canvas.focus();await page.keyboard.down('ArrowRight');await expect.poll(async()=>(await state()).movement.foot.x).toBeGreaterThan(104);await page.keyboard.up('ArrowRight')
 await page.locator('.world-controls summary').click();const selected=page.getByRole('combobox',{name:'Nearby interaction'});await expect(selected).toBeEnabled()
 const id=await selected.locator('option').first().getAttribute('value');assert(id);await selected.selectOption(id);await selected.focus();await expect(selected).toBeFocused()
 const beforeCount=(await state()).intents.filter(i=>i.buttons.length).length;await page.getByRole('button',{name:/^Interact:/}).click();await expect.poll(async()=>(await state()).intents.filter(i=>i.buttons.length).length).toBe(beforeCount+1)
 await frame();assert.equal((await state()).intents.filter(i=>i.buttons.length).length,beforeCount+1);report.positiveSelection={kind:'TEST ONLY synthetic terrain with production input/runtime',selectedId:id,oneShot:true,cleanup:await invoke('dispose')};await page.close()
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(error){report.failure={stage,error:String(error)};if(page&&!page.isClosed())await shot('failure');throw error}
finally{await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await rm(cache,{recursive:true,force:true})}
console.log(JSON.stringify({result:report.result,cases:report.cases.map(x=>({width:x.width,scene:x.normal.scene,short:x.short.scene})),screenshots:Object.keys(report.artifacts)}))
