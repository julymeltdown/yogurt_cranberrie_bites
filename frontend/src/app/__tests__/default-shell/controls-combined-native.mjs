import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
const frontend=resolve(import.meta.dirname,'../../../..'),evidence=resolve(import.meta.dirname,'evidence/controls-combined')
await mkdir(evidence,{recursive:true})
const paths=['src/app/WorldShell.tsx','src/app/shell.css','src/app/world-panels/index.tsx','src/app/world-panels/presentation.css','src/world/composition/index.ts','src/world/input/controller.ts','src/world/input/controls.css','src/world/runtime.ts','src/world/scenes/MapScene.ts']
const pins=Object.fromEntries(await Promise.all(paths.map(async p=>[p,createHash('sha256').update(await readFile(resolve(frontend,p))).digest('hex')])))
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:'/tmp/mcr-default-controls-combined-cache',server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'})
await server.listen()
const report={result:'FAIL',revision:execFileSync('git',['rev-parse','HEAD'],{cwd:frontend,encoding:'utf8'}).trim(),pins,qualification:'Actual default canonical Auth/HTTP/production resolver/Phaser, deterministic C04/C05 MockSocket/geometric resources. Combined clean root+compact controls. Visible intersection measured against every clipping ancestor; native scroll may move scene out while viewing help, which remains reversible. No live service/approved art/two-user/device/load acceptance.',cases:[],screenshots:[],errors:[]}
let browser,currentPage,stage='start'
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 const base=`http://127.0.0.1:${server.httpServer.address().port}`
 for(const [width,height]of [[360,800],[390,844],[430,932]]){
  const page=await browser.newPage({viewport:{width,height},hasTouch:true,reducedMotion:'reduce'});currentPage=page;page.on('pageerror',e=>report.errors.push(e.message))
  const state=()=>page.evaluate(()=>window.__DEFAULT_QUALIFIED__?.state())
  const measure=()=>page.locator('.shell-world-host canvas').evaluate(async canvas=>{
   await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))
   const r=canvas.getBoundingClientRect();let top=Math.max(0,r.top),bottom=Math.min(innerHeight,r.bottom),left=Math.max(0,r.left),right=Math.min(innerWidth,r.right)
   for(let parent=canvas.parentElement;parent;parent=parent.parentElement){const style=getComputedStyle(parent),box=parent.getBoundingClientRect();if(/hidden|clip|auto|scroll/.test(style.overflowY)){top=Math.max(top,box.top);bottom=Math.min(bottom,box.bottom)}if(/hidden|clip|auto|scroll/.test(style.overflowX)){left=Math.max(left,box.left);right=Math.min(right,box.right)}}
   const scroll=document.querySelector('.world-composition')
   const active=document.activeElement,focusBox=active.getBoundingClientRect()
   return {intrinsicHeight:canvas.height,visibleHeight:Math.max(0,bottom-top),visibleWidth:Math.max(0,right-left),scrollTop:scroll.scrollTop,clientHeight:scroll.clientHeight,scrollHeight:scroll.scrollHeight,focus:{tag:active.tagName,name:active.getAttribute('aria-label')??active.textContent?.slice(0,80),top:focusBox.top,bottom:focusBox.bottom}}
  })
  const top=()=>page.locator('.world-composition').evaluate(el=>{el.scrollTop=0})
  const shot=async name=>{await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));const file=`${name}-${width}.png`;await page.screenshot({path:resolve(evidence,file),fullPage:true});report.screenshots.push(file)}
  stage=`${width}/normal`;await page.goto(`${base}/src/app/__tests__/default-shell/qualified.html`);await expect.poll(async()=>(await state())?.published).toBe(true);await expect.poll(async()=>(await state()).room).toBeTruthy()
  const initial=await state(),normal=await measure();assert(normal.visibleHeight>=120);await shot('normal')
  const controls=page.locator('.world-controls'),details=controls.locator('details'),summary=details.locator('summary');await expect(summary).toHaveText(/Controls.*help/i)
  stage=`${width}/short-closed`;await page.setViewportSize({width,height:430});await top();const closed=await measure();assert(closed.visibleHeight>=100);await shot('short-closed')
  stage=`${width}/short-expanded`;await summary.focus();await expect(summary).toBeInViewport();await page.keyboard.press('Enter');await expect(details).toHaveAttribute('open','');await top();const expanded=await measure();assert(expanded.visibleHeight>=100);await shot('short-expanded-scene')
  const zoom=controls.getByRole('button',{name:'Zoom in',exact:true});await zoom.focus();await expect(zoom).toBeFocused();await expect(zoom).toBeInViewport({ratio:1});await page.keyboard.press('Enter');await expect.poll(async()=>(await state()).camera.camera.zoom).toBe(2);await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport({ratio:1});const helpFocus=await measure();await shot('short-expanded-focus')
  await summary.focus();await page.keyboard.press('Enter');await expect(details).not.toHaveAttribute('open','');await top();const restored=await measure();assert(restored.visibleHeight>=100)
  stage=`${width}/typing`;await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.navigate('/world/map'));const input=page.getByRole('searchbox',{name:'Find a place',exact:true});await input.focus();await input.scrollIntoViewIfNeeded();await expect(input).toBeInViewport();await page.keyboard.press('ArrowLeft');await page.keyboard.press('Escape');assert.equal((await state()).path,'/world/map');await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport();await shot('short-keyboard-map')
  const end=await state();assert.equal(end.mounts,1);assert.equal(end.peak,1);assert.equal(end.sockets,1);assert.equal(end.activeSockets,1);assert.equal(end.room,initial.room);assert.equal(end.geometry,initial.geometry);assert.equal(end.sameCamera,true)
  await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.dispose());const disposed=await state();assert.equal(disposed.games,0);assert.equal(disposed.activeSockets,0)
  report.cases.push({width,height,normal,closed,expanded,helpFocus,restored,oneMount:end.mounts,disposed:disposed.disposals});await page.close()
 }
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(error){report.failure={stage,error:String(error),state:await currentPage?.evaluate(()=>window.__DEFAULT_QUALIFIED__?.state()),text:await currentPage?.locator('body').innerText()};await currentPage?.screenshot({path:resolve(evidence,'failure.png')});throw error}finally{await writeFile(resolve(evidence,'report.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close()}
