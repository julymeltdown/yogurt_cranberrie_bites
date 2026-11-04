// Evidence only. Reuses existing production-component fixtures without changing product TSX.
import assert from 'node:assert/strict'
import {createRequire} from 'node:module'
import {pathToFileURL} from 'node:url'
import {readFile,writeFile,mkdir,mkdtemp,rm,statfs} from 'node:fs/promises'
import {resolve} from 'node:path'
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
const repo=process.env.MCR_FEED_REPO, phase=process.env.MCR_FEED_PHASE??'after'
assert(repo);assert(process.env.MCR_FEED_SLOT,'named accepted slot required')
const frontend=resolve(repo,'frontend'),req=createRequire(resolve(frontend,'package.json'))
const {createServer}=await import(pathToFileURL(req.resolve('vite')))
const {chromium,expect}=req('@playwright/test')
const {default:react}=await import(pathToFileURL(req.resolve('@vitejs/plugin-react')))
const evidence=resolve(repo,'docs/mcr/evidence/css-parity-feed',phase)
const hash=b=>createHash('sha256').update(b).digest('hex')
const git=(...a)=>execFileSync('git',a,{cwd:repo,encoding:'utf8',maxBuffer:64*1024**2})
const disk=await statfs(repo);assert(disk.bavail*disk.bsize>=8*1024**3)
await mkdir(evidence,{recursive:true});const tmp=await mkdtemp('/tmp/mcr-css-feed-')
const report={phase,slot:process.env.MCR_FEED_SLOT,base:git('rev-parse','HEAD').trim(),cssSha256:hash(await readFile(resolve(frontend,'src/features/event-feed/feed.css'))),qualification:'Existing actual EventFeed/reference and default Auth/router/Phaser fixture, synthetic HTTP/media. Not live/device/QA06 acceptance.',cases:[],errors:[],status:'FAIL'}
report.renderedCssSha256=phase==='before'?hash(await readFile(resolve(repo,'docs/mcr/evidence/css-parity-feed/baseline-feed.css.txt'))):report.cssSha256
const server=await createServer({root:frontend,configFile:false,plugins:[react()],cacheDir:resolve(tmp,'cache'),logLevel:'error',server:{host:'127.0.0.1',port:5292,strictPort:true,hmr:false}})
let browser,page,stage='boot'
const sizes=async()=>page.locator('.feed-browse').evaluate(el=>{
 const rect=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom}}
 const css=getComputedStyle(el.querySelector('.feed-post')??el)
 return {viewport:innerWidth,documentWidth:document.documentElement.scrollWidth,feed:rect(el),rows:[...el.querySelectorAll('.feed-post')].map(rect),portraits:[...el.querySelectorAll('.feed-portrait')].map(rect),media:[...el.querySelectorAll('.feed-media')].map(rect),footer:el.querySelector('.feed-footer')?rect(el.querySelector('.feed-footer')):null,controls:[...el.querySelectorAll('button,summary')].filter(e=>e.checkVisibility()).map(e=>({name:e.getAttribute('aria-label')||e.textContent,...rect(e)})),background:{color:css.backgroundColor,image:css.backgroundImage},copyFont:el.querySelector('.feed-post-copy')?getComputedStyle(el.querySelector('.feed-post-copy')).font:null}
})
const shot=async name=>{const path=resolve(evidence,name+'.png');await page.screenshot({path,fullPage:true});return {path:path.slice(repo.length+1),sha256:hash(await readFile(path))}}
try {
 await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const width of phase==='before'?[390]:[360,390,430]){
  stage='reference/'+width
  const context=await browser.newContext({viewport:{width,height:844},deviceScaleFactor:1,isMobile:true,hasTouch:true,reducedMotion:'reduce',locale:'en-GB',timezoneId:'Asia/Seoul'})
  page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  if(phase==='before')await page.route('**/src/features/event-feed/feed.css*',async route=>{const css=await readFile(resolve(repo,'docs/mcr/evidence/css-parity-feed/baseline-feed.css.txt'),'utf8');await route.fulfill({contentType:'text/javascript',body:'import { updateStyle } from "/@vite/client"; updateStyle("feed-parity-baseline", '+JSON.stringify(css)+');'})})
  await page.route('**/api/concert/events/*/media/*/content',route=>route.fulfill({contentType:'image/png',path:resolve(frontend,'public/world/illustrated',route.request().url().includes('000000000075')?'parade-scene.png':'revenge-scene.png')}))
  const base='http://127.0.0.1:5292/src/features/event-feed/__tests__/reference/preview.html'
  await page.goto(base);await expect(page.locator('.feed-post')).toHaveCount(2);await expect(page.locator('.feed-media img')).toHaveCount(2)
  await page.locator('.feed-media img').evaluateAll(es=>Promise.all(es.map(e=>e.decode())));await page.evaluate(()=>document.fonts.ready)
  const geometry=await sizes();assert.equal(geometry.documentWidth,width);assert(geometry.controls.every(c=>c.width>=44&&c.height>=44),JSON.stringify(geometry.controls))
  const row={width,geometry,capture:await shot('feed-'+width)};report.cases.push(row)
  if(phase==='before'){await context.close();continue}
  await page.getByRole('tab',{name:'ALL',exact:true}).focus();await page.keyboard.press('ArrowRight');await expect(page.getByRole('tab',{name:'MEETUP'})).toBeFocused();await expect(page.getByText('No posts yet.',{exact:true})).toBeVisible()
  await page.keyboard.press('End');await expect(page.getByRole('tab',{name:'NOTICE'})).toBeFocused();await page.getByRole('button',{name:'Fixture notice'}).click();await expect(page.getByRole('link',{name:'Source',exact:true})).toBeVisible()
  await page.getByRole('tab',{name:'ALL',exact:true}).click();await expect(page.locator('.feed-post')).toHaveCount(2)
  for(const name of ['View 42 reactions in thread','Open thread · 7 comments','View post sharing in thread']){await page.getByRole('button',{name,exact:true}).first().click();await expect(page.locator('#destination')).toContainText('Fixture thread')}
  await page.getByRole('button',{name:'Write a post'}).focus();await page.keyboard.press('Enter');await expect(page.locator('#destination')).toHaveText('Fixture compose destination')
  row.focus=await page.getByRole('button',{name:'Write a post'}).evaluate(e=>({outline:getComputedStyle(e).outline,rect:{width:e.clientWidth,height:e.clientHeight}}));assert.match(row.focus.outline,/3px/);row.focusCapture=await shot('focus-'+width)
  if(width===390){await page.emulateMedia({forcedColors:'active'});await page.getByRole('button',{name:'Write a post'}).focus();row.forcedFocus=await page.getByRole('button',{name:'Write a post'}).evaluate(e=>getComputedStyle(e).outline);assert.match(row.forcedFocus,/3px/);row.forcedFocusCapture=await shot('forced-focus-390');await page.emulateMedia({forcedColors:'none'})}
  await page.locator('.feed-tools summary').click();await expect(page.getByRole('checkbox',{name:'Images only'})).toBeVisible();await page.getByRole('checkbox',{name:'Images only'}).check();await expect(page.locator('.feed-post')).toHaveCount(2);await page.locator('.feed-tools summary').click()
  await page.getByRole('button',{name:'Refresh feed',exact:true}).click();await expect(page.locator('.feed-post')).toHaveCount(2)
  await page.evaluate(()=>window.__REFERENCE_FEED__.invalidate());await expect(page.locator('.feed-media img')).toHaveCount(0);assert.equal(await page.evaluate(()=>window.__REFERENCE_FEED__.resources()),0)
  for(const mode of ['empty','readonly','unavailable','pending','error']){
   await page.goto(base+'?mode='+mode)
   if(mode==='readonly'){await expect(page.locator('.feed-post')).toHaveCount(2);await expect(page.getByRole('button',{name:'Write a post'})).toHaveCount(0)}
   else if(mode==='error')await expect(page.getByRole('alert')).toBeVisible()
   else await expect(page.locator('.feed-browse [role=status]').first()).toBeVisible()
   assert.equal((await sizes()).documentWidth,width);if(width===390)await shot(mode+'-390')
  }
  row.checks=['tabs/keyboard','notices/source','all thread callbacks','compose/focus','options/media filter','refresh','privacy media revocation','empty/read-only/unavailable/pending/error','44px targets/no horizontal overflow']
  await page.evaluate(()=>window.__REFERENCE_FEED__.dispose());await context.close()
 }
 if(phase!=='before'){
  stage='default-root/390';const context=await browser.newContext({viewport:{width:390,height:844},deviceScaleFactor:1,hasTouch:true});page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  await page.goto('http://127.0.0.1:5292/src/app/__tests__/cozy-backgrounds-v1/preview.html?entry=%2Fworld&era=PARADE');await expect.poll(()=>page.evaluate(()=>Boolean(window.__COZY_PANEL__)),{timeout:25000}).toBe(true)
  await expect.poll(()=>page.evaluate(()=>window.__COZY_PANEL__.state().phase),{timeout:20000}).toBe('live');await page.evaluate(()=>window.__COZY_PANEL__.navigate('/feed'));await expect(page.locator('.feed-post')).toHaveCount(1)
  await expect(page.locator('.feed-portrait canvas')).toBeVisible();report.root={geometry:await sizes(),capture:await shot('default-root-390'),state:await page.evaluate(()=>window.__COZY_PANEL__.state())};assert.equal(report.root.state.canvases,1);assert.equal(report.root.state.socketPeak,1)
  await page.getByRole('button',{name:'Write a post'}).click();await expect(page.getByRole('textbox',{name:'Post text',exact:true})).toBeVisible();assert.equal(await page.locator('.feed-browse').count(),0)
  await page.getByRole('textbox',{name:'Post text',exact:true}).fill('Unsubmitted CSS qualification draft');await page.getByRole('textbox',{name:'Post text',exact:true}).focus();report.root.composerCapture=await shot('composer-390')
  await page.evaluate(()=>window.__COZY_PANEL__.dispose());report.root.cleanup=await page.evaluate(()=>window.__COZY_PANEL__.state());assert.equal(report.root.cleanup.activeSockets,0);assert.equal(report.root.cleanup.canvases,0);await context.close()
 }
 assert.deepEqual(report.errors,[]);report.status='PASS'
}catch(e){report.failure={stage,error:String(e.stack)};await page?.screenshot({path:resolve(evidence,'failure.png')}).catch(()=>{});throw e}
finally{await browser?.close();await server.close();await rm(tmp,{recursive:true,force:true});report.cleanup={browserClosed:true,viteClosed:true,privateCacheRemoved:true};await writeFile(resolve(evidence,'report.json'),JSON.stringify(report,null,2)+'\n')}
console.log(JSON.stringify({status:report.status,phase,widths:report.cases.map(c=>c.width)}))
