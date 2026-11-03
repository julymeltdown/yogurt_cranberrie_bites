import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, mkdtemp, readFile, writeFile, rm } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
const root=resolve(import.meta.dirname,'../../../..'),out=resolve(import.meta.dirname,'evidence'),before=process.argv.includes('--before'),name=before?'before':'after'
await mkdir(out,{recursive:true});const cache=await mkdtemp(resolve(tmpdir(),'mcr-controls-cache-'))
const hash=data=>createHash('sha256').update(data).digest('hex')
const sources=['src/world/input/controller.ts','src/world/input/controls.css','src/world/input/model.ts','src/world/input/simulation.ts','src/world/runtime.ts','src/world/scenes/MapScene.ts','src/world/composition/index.ts','src/world/composition/release.ts','src/app/WorldShell.tsx','src/app/shell.css','src/app/router.tsx','src/app/__tests__/default-shell/qualified.tsx','src/app/__tests__/default-shell/qualified-data.ts','src/world/__tests__/fixtures/movement-data.ts','src/world/__tests__/controls-layout/native.mjs']
const report={result:'FAIL',qualification:'Actual default shell/production composition/canonical Auth/Phaser with deterministic HTTP/MockSocket and TEST ONLY geometric resources. Secondary local-runtime fixture uses actual Phaser. No live/multiuser/device/art/prediction qualification.',phase:name,testedParent:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),sourceHashes:Object.fromEntries(await Promise.all(sources.map(async p=>[p,hash(await readFile(resolve(root,p)))]))),cases:[],artifacts:{},errors:[]}
report.referenceHash=hash(await readFile(resolve(root,'../docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg')))
const server=await createServer({root,configLoader:'runner',cacheDir:cache,server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'});await server.listen()
let browser,page,stage='start'
const frame=page=>page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))
const shot=async(file)=>{await frame(page);const p=resolve(out,file);await page.screenshot({path:p,fullPage:true});report.artifacts[file]=hash(await readFile(p))}
async function geometry(){return page.evaluate(()=>{const rect=s=>{const e=document.querySelector(s),r=e?.getBoundingClientRect();return r?{x:r.x,y:r.y,width:r.width,height:r.height}:null};return{controls:rect('.world-controls'),scene:rect('.shell-world-host canvas'),viewport:{width:innerWidth,height:innerHeight},overflow:document.documentElement.scrollWidth>innerWidth}})}
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 const origin=`http://127.0.0.1:${server.httpServer.address().port}`
 for(const [width,height]of[[360,800],[390,844],[430,932]]){
  const context=await browser.newContext({viewport:{width,height},reducedMotion:'reduce'});page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  stage=`${width}/ready`;await page.goto(`${origin}/src/app/__tests__/default-shell/qualified.html`)
  await expect.poll(()=>page.evaluate(()=>window.__DEFAULT_QUALIFIED__?.state().published)).toBe(true)
  await expect(page.locator('.world-controls')).toHaveCount(1);await frame(page)
  const initial=await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.state()),closed=await geometry();await shot(`${name}-world-${width}.png`)
  assert.equal(closed.overflow,false)
  if(!before){
   assert(closed.controls.height<=150,JSON.stringify(closed));assert(closed.scene.height>closed.controls.height,JSON.stringify(closed))
   assert(await page.locator('.world-controls button:visible,.world-controls summary:visible').evaluateAll(es=>es.every(e=>e.getBoundingClientRect().height>=44&&e.getBoundingClientRect().width>=44)))
   const help=page.locator('.world-controls summary');await help.focus();await page.keyboard.press('Enter');await expect(page.getByRole('combobox',{name:'Nearby interaction',exact:true})).toBeVisible()
   const zoom=page.locator('.world-controls').getByRole('button',{name:'Zoom in',exact:true});await zoom.focus();await expect(zoom).toBeFocused();await page.keyboard.press('Enter')
   await expect.poll(()=>page.evaluate(()=>window.__DEFAULT_QUALIFIED__.state().camera.camera.zoom)).toBe(2)
   await help.focus();await page.keyboard.press('Enter');await expect(zoom).not.toBeVisible();await expect(help).toBeFocused();await shot(`${name}-closed-help-${width}.png`)
   await help.press('Enter');await shot(`${name}-help-${width}.png`)
   await page.setViewportSize({width,height:430});await help.scrollIntoViewIfNeeded();const content=page.locator('.world-control-details')
   await expect(content).toBeVisible();await content.evaluate(e=>{e.scrollTop=e.scrollHeight});await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport();await shot(`${name}-short-help-${width}.png`)
   assert.equal((await geometry()).overflow,false);await help.press('Enter');await shot(`${name}-short-${width}.png`);report.cases.push({width,height,closed,short:await geometry()});await page.setViewportSize({width,height})
   await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Feed',exact:true}).click();await expect(page.getByRole('heading',{name:'Feed',exact:true,level:1})).toBeVisible()
   await expect(page.locator('.world-joystick')).toBeDisabled();await page.getByRole('link',{name:'World',exact:true}).click();await expect(page.locator('.world-joystick')).toBeEnabled()
   const end=await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.state());assert.equal(end.mounts,initial.mounts);assert.equal(end.sockets,initial.sockets);assert.equal(end.games,1)
  }else{await page.setViewportSize({width,height:430});await shot(`${name}-short-${width}.png`);report.cases.push({width,height,closed,short:await geometry()})}
  await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.dispose());await expect(page.locator('.world-controls')).toHaveCount(0);await expect(page.locator('canvas')).toHaveCount(0);await context.close()
 }
 if(!before){
  page=await browser.newPage({viewport:{width:390,height:844}});page.on('pageerror',e=>report.errors.push(e.message));stage='local/interactions'
  await page.goto(`${origin}/src/world/__tests__/controls-layout/preview.html`);await expect.poll(()=>page.evaluate(()=>window.__LAYOUT__?.state().runtime.phase)).toBe('ready')
  const canvas=page.locator('canvas');await canvas.focus();await page.keyboard.down('ArrowRight');await expect.poll(()=>page.evaluate(()=>window.__LAYOUT__.state().movement.foot.x)).toBeGreaterThan(104);await page.keyboard.up('ArrowRight')
  await page.locator('.world-controls summary').click();const select=page.getByRole('combobox',{name:'Nearby interaction'});await expect(select).toBeEnabled();await select.focus();await page.keyboard.press('ArrowDown');await expect(select).toBeFocused()
  await page.locator('.world-controls').getByRole('button',{name:/^Interact:/}).click();await expect.poll(()=>page.evaluate(()=>window.__LAYOUT__.state().intents.filter(i=>i.buttons.length).length)).toBe(1)
  await page.locator('.world-controls summary').click();const stick=page.locator('.world-joystick'),rect=await stick.boundingBox();await page.mouse.move(rect.x+rect.width*.9,rect.y+rect.height*.5);await page.mouse.down()
  await expect.poll(()=>page.evaluate(()=>window.__LAYOUT__.state().intents.at(-1)?.axisX)).toBeGreaterThan(0)
  await page.evaluate(()=>window.dispatchEvent(new PointerEvent('pointercancel',{pointerId:1,bubbles:true})));await page.mouse.up()
  assert.deepEqual(await page.evaluate(()=>window.__LAYOUT__.state().intents.at(-1)),{axisX:0,axisY:0,buttons:[]})
  await canvas.focus();await page.keyboard.down('ArrowRight');await expect.poll(()=>page.evaluate(()=>window.__LAYOUT__.state().intents.at(-1)?.axisX)).toBeGreaterThan(0)
  await page.evaluate(()=>window.__LAYOUT__.blocked(true));await page.keyboard.up('ArrowRight');const paused=await page.evaluate(()=>window.__LAYOUT__.state().movement.foot)
  await frame(page);assert.deepEqual(await page.evaluate(()=>window.__LAYOUT__.state().movement.foot),paused);await expect(stick).toBeDisabled();await page.evaluate(()=>window.__LAYOUT__.blocked(false))
  await page.evaluate(()=>window.__LAYOUT__.fail());await canvas.focus();await page.keyboard.down('ArrowRight');await expect(page.getByText('Movement unavailable: input consumer failed.',{exact:true})).toBeVisible();await page.keyboard.up('ArrowRight');await shot('after-failure-390.png')
  await page.evaluate(()=>window.__LAYOUT__.dispose());await expect(page.locator('.world-controls')).toHaveCount(0);await expect(canvas).toHaveCount(0);await page.close()
 }
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(e){report.failure={stage,error:String(e)};if(page&&!page.isClosed())await shot(`${name}-failure.png`);throw e}
finally{await writeFile(resolve(out,`${name}-report.json`),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await rm(cache,{recursive:true,force:true})}
console.log(JSON.stringify({result:report.result,cases:report.cases,sourceHashes:report.sourceHashes},null,2))
