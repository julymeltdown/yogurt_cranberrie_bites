// Evidence only. Reuses existing production-component fixtures without changing product TSX.
import assert from 'node:assert/strict'
import {createRequire} from 'node:module'
import {pathToFileURL} from 'node:url'
import {readFile,writeFile,mkdir,mkdtemp,rm,statfs} from 'node:fs/promises'
import {resolve} from 'node:path'
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
const repo=process.env.MCR_FEED_REPO, phase=process.env.MCR_FEED_PHASE??'intrinsic'
assert(repo);assert(process.env.MCR_FEED_SLOT,'named accepted slot required')
const frontend=resolve(repo,'frontend'),req=createRequire(resolve(frontend,'package.json'))
const {createServer}=await import(pathToFileURL(req.resolve('vite')))
const {chromium,expect}=req('@playwright/test')
const {default:react}=await import(pathToFileURL(req.resolve('@vitejs/plugin-react')))
const evidence=resolve(repo,'docs/mcr/evidence/css-parity-feed',phase)
const hash=b=>createHash('sha256').update(b).digest('hex')
const git=(...a)=>execFileSync('git',a,{cwd:repo,encoding:'utf8',maxBuffer:64*1024**2})
const disk=await statfs(repo);assert(disk.bavail*disk.bsize>=8*1024**3)
await mkdir(evidence,{recursive:true});const tmp=await mkdtemp(resolve(process.env.TMPDIR??'/tmp','mcr-css-feed-intrinsic-'))
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
  const prior=JSON.parse(await readFile(resolve(repo,'docs/mcr/evidence/css-parity-feed/round2/report.json'),'utf8')).cases.find(c=>c.width===width)
  row.authorizedMedia=await page.locator('.feed-media img').evaluateAll(es=>es.map(e=>({naturalWidth:e.naturalWidth,naturalHeight:e.naturalHeight,width:e.getBoundingClientRect().width,height:e.getBoundingClientRect().height,maxHeight:getComputedStyle(e).maxHeight,blob:e.src.startsWith('blob:')})))
  assert(row.authorizedMedia.every(i=>i.naturalWidth===1152&&i.naturalHeight===896&&i.blob))
  assert(geometry.media.every((m,i)=>Math.abs(m.width-prior.geometry.media[i].width)<.1&&Math.abs(m.height-prior.geometry.media[i].height)<.1),'existing authorized thumbnails retain prior cap')
  row.priorThumbnailGeometryMatches=true

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
  // Separate, explicitly synthetic geometry probes; never posts or approved media.
  const probe=await context.newPage()
  const css=await readFile(resolve(frontend,'src/features/event-feed/feed.css'),'utf8')
  const mediaWidth=geometry.media[0].width
  const fixtureCases=[['wide',420,100],['landscape',1152,896],['portrait',300,600]]
  const figures=fixtureCases.map(([name,w,h])=>{
   const svg=`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><rect x="1" y="1" width="${w-2}" height="${h-2}" fill="#ddd" stroke="#222"/><path d="M0 0 L${w} ${h} M${w} 0 L0 ${h}" stroke="#555"/><text x="8" y="24" font-size="16">${name}: geometry only</text></svg>`
   return `<figure data-probe="${name}"><img width="${w}" height="${h}" src="data:image/svg+xml,${encodeURIComponent(svg)}" alt="${name} geometry probe, not post media"></figure>`
  }).join('')
  await probe.setContent(`<html lang="en"><meta name="viewport" content="width=device-width,initial-scale=1"><style>${css}body{margin:14px;font:14px Arial} .feed-browse{min-height:0;width:${mediaWidth}px}</style><p>CSS geometry probes only. Not posts, photographs, product art or live data.</p><section class="event-feed feed-browse"><div class="feed-media">${figures}<figure data-probe="loading"><p role="status">Loading image…</p></figure><figure data-probe="failure"><div><p>Image unavailable. Post text is still available.</p><button type="button">Retry image read</button></div></figure></div></section></html>`)
  await probe.locator('img').evaluateAll(es=>Promise.all(es.map(e=>e.decode())))
  row.intrinsic=await probe.locator('[data-probe]').evaluateAll(es=>es.map(e=>{
   const r=e.getBoundingClientRect(),img=e.querySelector('img'),button=e.querySelector('button'),i=img?.getBoundingClientRect(),b=button?.getBoundingClientRect()
   return {name:e.dataset.probe,width:r.width,height:r.height,overflow:getComputedStyle(e).overflow,image:i?{width:i.width,height:i.height,naturalWidth:img.naturalWidth,naturalHeight:img.naturalHeight,maxHeight:getComputedStyle(img).maxHeight}:null,button:b?{width:b.width,height:b.height,contained:b.top>=r.top&&b.bottom<=r.bottom}:null}
  }))
  assert(await probe.evaluate(()=>CSS.supports('container-type','inline-size')&&CSS.supports('max-height','calc(100cqi / 2.85 - 2px)')))
  const cap=mediaWidth/2.85-2
  for(const item of row.intrinsic.filter(i=>i.image)){
   const natural=(mediaWidth-2)*item.image.naturalHeight/item.image.naturalWidth
   assert(Math.abs(item.image.height-Math.min(natural,cap))<.1,JSON.stringify(item))
   if(item.name==='wide')assert(Math.abs(item.image.width/item.image.height-4.2)<.01,'wide image preserves full natural ratio')
  }
  const failure=row.intrinsic.find(i=>i.name==='failure');assert(failure.button.contained&&failure.button.width>=44&&failure.button.height>=44&&failure.overflow==='visible')
  await probe.getByRole('button',{name:'Retry image read'}).focus();assert.match(await probe.getByRole('button',{name:'Retry image read'}).evaluate(e=>getComputedStyle(e).outline),/3px/)
  const probePath=resolve(evidence,'intrinsic-geometry-'+width+'.png');await probe.screenshot({path:probePath,fullPage:true});row.intrinsicCapture={path:probePath.slice(repo.length+1),sha256:hash(await readFile(probePath))};await probe.close()
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
