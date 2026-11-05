import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import { chromium, expect } from '@playwright/test'
assert.equal(process.env.MCR_PI_GUARDED, '1')
const frontend=resolve(import.meta.dirname,'../../../..'), root=resolve(frontend,'..')
const evidence=process.env.MCR_PI_OUTPUT, temporary=process.env.MCR_PI_TMP, port=Number(process.env.MCR_PI_PORT)
assert(evidence.startsWith(resolve(root,'docs/mcr/evidence/parade-ink-foundation')+'/'))
assert(temporary.startsWith('/private/tmp/mcr-parade-ink-'))
assert(Number.isInteger(port)&&port>1024)
await mkdir(evidence,{recursive:true})
const feedPin='805dd5491dc2288ed296825367bf5d38f11c0da8'
const headerOnly=process.env.MCR_PI_FOCUS==='header'
const borrowed=['src/features/event-feed/EventFeed.tsx','src/features/event-feed/feed.css','src/features/event-feed/parade-ink/AuthorHeading.tsx','src/features/feed-thread/FeedThread.tsx']
const sources=new Map(borrowed.map(path=>[resolve(frontend,path),execFileSync('git',['show',`${feedPin}:frontend/${path}`],{cwd:root,encoding:'utf8'})]))
const overlay={name:'pi01-readonly-qualified-feed',enforce:'pre',resolveId(source,importer){const path=source.split('?')[0];if(sources.has(path))return source;if(importer&&source.startsWith('.')){const target=resolve(dirname(importer.split('?')[0]),path);for(const candidate of [target,`${target}.tsx`,`${target}.css`])if(sources.has(candidate))return candidate}},load(id){return sources.get(id.split('?')[0])??null}}
const report={result:'FAIL',foundation:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),borrowed:borrowed.map(path=>({path:`frontend/${path}`,commit:feedPin,sha256:createHash('sha256').update(sources.get(resolve(frontend,path))).digest('hex')})),qualification:'Actual incumbent App router/WorldShell/production Auth and Phaser composition, deterministic HTTP/C05/geometric test terrain; exact read-only qualified Feed/Thread overlay. No live backend, approved World-art comparison, physical device or multiuser/load acceptance. Supplemental states are separately labeled.',screenshots:[],cases:[],errors:[]}
const server=await createServer({root:frontend,configFile:false,cacheDir:resolve(temporary,'vite-cache'),plugins:[overlay,react()],server:{host:'127.0.0.1',port,strictPort:true,fs:{allow:[root]}},logLevel:'warn'})
let browser,page,stage='start'
const image=async(name)=>{await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));const file=`${name}.png`;await page.screenshot({path:resolve(evidence,file)});const bytes=await readFile(resolve(evidence,file));report.screenshots.push({file,sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length,capturedAt:new Date().toISOString()})}
try{
 await server.listen()
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 const base=`http://127.0.0.1:${port}/src/app/__tests__/parade-ink-foundation`
 for(const width of [360,390,430]){
  const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,reducedMotion:'reduce'})
  page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  const state=()=>page.evaluate(()=>window.__DEFAULT_QUALIFIED__?.state())
  const nav=path=>page.evaluate(p=>window.__DEFAULT_QUALIFIED__.navigate(p),path)
  stage=`${width}/feed`;await page.goto(`${base}/preview.html?social=1&path=/feed`)
  await expect.poll(async()=>(await state())?.published).toBe(true)
  await expect(page.locator('.feed-post')).toHaveCount(1)
  await page.evaluate(()=>document.fonts.ready)
  const initial=await state()
  const metrics=await page.evaluate(()=>{
   const rect=s=>{const e=document.querySelector(s),r=e.getBoundingClientRect(),c=getComputedStyle(e);return {x:r.x,y:r.y,width:r.width,height:r.height,font:c.font,color:c.color,background:c.backgroundColor,border:c.borderWidth}}
   return {frame:rect('.world-shell'),header:rect('.mcr-chrome-panel-header'),body:rect('.shell-panel-body'),tabs:rect('.mcr-tablist'),selected:rect('.mcr-tab[aria-selected="true"]'),footer:rect('.feed-footer'),postBackground:getComputedStyle(document.querySelector('.feed-post')).backgroundImage,footerBackground:getComputedStyle(document.querySelector('.feed-footer')).backgroundImage,rootOverflow:document.documentElement.scrollWidth>innerWidth,fontLoaded:document.fonts.check('26px "Parade Ink Display"')&&document.fonts.check('18px "Parade Ink Text"')}
  })
  assert.equal(metrics.rootOverflow,false);assert.equal(metrics.fontLoaded,true)
  assert.equal(metrics.header.height,48);assert.equal(metrics.selected.background,'rgb(23, 23, 23)');assert.equal(metrics.selected.color,'rgb(255, 255, 255)')
  assert(metrics.footer.y>700,`Full-height footer misplaced: ${JSON.stringify(metrics.footer)}`)
  assert.equal(metrics.postBackground,'none');assert.equal(metrics.footerBackground,'none')
  const controls=await page.locator('.mcr-chrome-panel-header button,.mcr-tab,.feed-footer button,.feed-footer summary').evaluateAll(es=>es.map(e=>{const r=e.getBoundingClientRect();return {label:e.getAttribute('aria-label')??e.textContent,width:r.width,height:r.height,x:r.x,y:r.y}}))
  for(const control of controls){assert(control.width>=44&&control.height>=44,JSON.stringify(control))}
  await image(`app-feed-${width}`)
  if(headerOnly){
   const toggle=page.getByRole('button',{name:'Minimize panel',exact:true})
   assert.equal(await toggle.locator('svg').evaluate(e=>getComputedStyle(e).transform),'matrix(0, 1, -1, 0, 0, 0)')
   await expect(toggle.locator('path')).toHaveAttribute('fill','currentColor')
   await toggle.click()
   const restore=page.getByRole('button',{name:'Show panel content',exact:true})
   await expect(restore).toBeVisible()
   assert.equal(await restore.locator('svg').evaluate(e=>getComputedStyle(e).transform),'matrix(0, -1, 1, 0, 0, 0)')
   if(width===390)await image('app-header-minimized-390')
   await restore.click();await expect(page.locator('.feed-post')).toBeVisible()
   if(width===390)await image('app-header-restored-390')
   report.cases.push({width,metrics,controls,headerToggle:true,qualification:'Header glyph and actual minimize/restore only; prior full native06 remains separate immutable evidence.'})
   await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.dispose());await context.close();continue
  }
  await page.getByRole('tab',{name:'MEETUP',exact:true}).focus();await page.keyboard.press('Home');await expect(page.getByRole('tab',{name:'ALL',exact:true})).toBeFocused()
  await page.getByRole('button',{name:'Write a post',exact:true}).click();await expect(page.getByLabel('Post text',{exact:true})).toBeVisible()
  await page.getByLabel('Post text',{exact:true}).fill('Retained Parade Ink fixture draft')
  await image(`app-composer-${width}`)
  await nav('/world');await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeVisible()
  await expect.poll(async()=>(await state()).room).toBeTruthy()
  await page.getByRole('button',{name:'Parade',exact:true}).click();await expect.poll(async()=>(await state()).theme?.phase).toBe('ready');await expect.poll(async()=>(await state()).theme?.selected).toBe('PARADE')
  const uiPalette=await page.locator('.mcr-chrome-bottom a[aria-current="page"]').evaluate(e=>({bg:getComputedStyle(e).backgroundColor,color:getComputedStyle(e).color}))
  assert.deepEqual(uiPalette,{bg:'rgb(23, 23, 23)',color:'rgb(255, 255, 255)'})
  await image(`app-world-parade-${width}`)
  const end=await state();assert.equal(end.mounts,1);assert.equal(end.games,1);assert.equal(end.peak,1);assert.equal(end.activeSockets,1);assert.equal(end.sameCamera,true);assert.equal(end.geometry,initial.geometry)
  await nav('/feed/new');await expect(page.getByLabel('Post text',{exact:true})).toHaveValue('Retained Parade Ink fixture draft')
  await page.setViewportSize({width,height:430});await page.getByRole('button',{name:'Publish post',exact:true}).scrollIntoViewIfNeeded();await expect(page.getByRole('button',{name:'Publish post',exact:true})).toBeInViewport();assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false)
  await image(`app-short-composer-${width}`)
  if(width===390){
   const cdp=await context.newCDPSession(page)
   await cdp.send('Emulation.setSafeAreaInsetsOverride',{insets:{top:20,bottom:34,left:0,right:0}})
   const safe=await page.locator('.world-shell').evaluate(e=>({top:parseFloat(getComputedStyle(e).paddingTop),bottom:parseFloat(getComputedStyle(e).paddingBottom)}))
   assert.equal(safe.top,22);assert.equal(safe.bottom,36)
   await page.getByRole('button',{name:'Publish post',exact:true}).scrollIntoViewIfNeeded();await expect(page.getByRole('button',{name:'Publish post',exact:true})).toBeInViewport()
   await image('app-short-safe-area-composer-390')
   report.safeArea={emulation:'Chrome CDP insets, not physical device',top:20,bottom:34,computed:safe}
   await cdp.send('Emulation.setSafeAreaInsetsOverride',{insets:{top:0,bottom:0,left:0,right:0}});await cdp.detach()
  }
  await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.dispose());assert.equal((await state()).activeSockets,0);assert.equal((await state()).games,0)
  report.cases.push({width,metrics,controls,oneWorld:true,oneSocket:true,independentEra:true,retainedDraft:true,shortActionsReachable:true})
  await context.close()
 }
 if(!headerOnly){
 stage='shared-states';page=await browser.newPage({viewport:{width:390,height:844},reducedMotion:'reduce'})
 await page.goto(`${base}/states.html`);await page.evaluate(()=>document.fonts.ready)
 await expect(page.getByRole('button',{name:'Saving · pending'})).toBeDisabled();await expect(page.getByRole('button',{name:'Unavailable action'})).toBeDisabled()
 await expect(page.getByRole('textbox',{name:'Draft'})).toHaveAttribute('aria-invalid','true')
 await page.getByRole('tab',{name:'ALL',exact:true}).focus();await page.keyboard.press('ArrowRight');await expect(page.getByRole('tab',{name:'NEXT',exact:true})).toBeFocused()
 const focusStyle=await page.getByRole('tab',{name:'NEXT',exact:true}).evaluate(e=>({width:getComputedStyle(e).outlineWidth,style:getComputedStyle(e).outlineStyle}))
 assert.notEqual(focusStyle.width,'0px');assert.equal(focusStyle.style,'solid')
 await image('supplemental-shared-focus-error-disabled')
 const retry=page.getByRole('button',{name:'Try again',exact:true}),box=await retry.boundingBox();assert(box)
 await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down()
 assert.equal(await retry.evaluate(e=>e.matches(':active')),true)
 await image('supplemental-shared-pressed');await page.mouse.up()
 await page.getByRole('button',{name:'Open sheet'}).click();await expect(page.getByRole('dialog',{name:'Details'})).toBeVisible();await expect(page.getByRole('button',{name:'Close dialog'})).toBeFocused()
 await page.setViewportSize({width:390,height:430});await page.getByRole('button',{name:'Save draft'}).scrollIntoViewIfNeeded();await expect(page.getByRole('button',{name:'Save draft'})).toBeInViewport();await image('supplemental-short-sheet')
 await page.keyboard.press('Escape');await expect(page.getByRole('button',{name:'Open sheet'})).toBeFocused()
 }
 assert.deepEqual(report.errors,[]);report.focus=headerOnly?'HEADER_CORRECTION_ONLY':'FULL_FOUNDATION';report.result='PASS'
}catch(error){report.failure={stage,error:String(error)};if(page)await image('failure');throw error}
finally{await browser?.close();await server.close();await writeFile(resolve(evidence,'native.json'),JSON.stringify(report,null,2)+'\n')}
