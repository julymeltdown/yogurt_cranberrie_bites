// Actual application fixture; synthetic HTTP/C05, not a live/backend/device gate.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {execFileSync} from 'node:child_process'
import {readFile,writeFile,mkdir,mkdtemp,rm,statfs} from 'node:fs/promises'
import {resolve,extname} from 'node:path'
import {tmpdir} from 'node:os'
import {createServer} from 'vite'
import {createServer as createPortProbe} from 'node:net'
import {chromium,expect} from '@playwright/test'

const frontend=resolve(import.meta.dirname,'../../../../..'),repo=resolve(frontend,'..')
const phase=process.env.MCR_FIDELITY_PHASE??'after',out=resolve(import.meta.dirname,'evidence',phase)
const hash=b=>createHash('sha256').update(b).digest('hex'),git=(...a)=>execFileSync('git',a,{cwd:repo,maxBuffer:64*1024**2})
const disk=await statfs(repo);assert(disk.bavail*disk.bsize>=8*1024**3,'8GiB disk floor')
await mkdir(out,{recursive:true});const cache=await mkdtemp(resolve(tmpdir(),'mcr-qa06-fidelity-'))
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n')),assetReads=new Map()
const report={status:'FAIL',phase,candidate:git('rev-parse','HEAD').toString().trim(),qualification:'Actual Auth/browser-router/default world loader/Phaser/Feed/Me; synthetic HTTP/C05 and selected source fixtures. No live/device/whole QA06/FE34/IN acceptance.',referenceSha256:hash(await readFile(resolve(repo,'docs/mcr/design/reference-target-2026-11-04.png'))),sourcePins:{},cases:[],errors:[],assets:[]}
for(const p of git('ls-files','frontend/src','contracts').toString().trim().split('\n').filter(p=>/\.(tsx?|css|json)$/.test(p)&&!p.includes('/evidence/')))report.sourcePins[p]=hash(await readFile(resolve(repo,p)))
for(const p of ['preview.tsx','preview.html','native.mjs'])report.sourcePins['frontend/src/features/event-feed/__tests__/qa06-fidelity/'+p]=hash(await readFile(resolve(import.meta.dirname,p)))
const probe=createPortProbe();await new Promise(r=>probe.listen(0,'127.0.0.1',r));const port=probe.address().port;await new Promise(r=>probe.close(r))
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:cache,logLevel:'error',plugins:[{name:'read-only-tracked-assets',configureServer(s){s.middlewares.use((req,res,next)=>{
 const u=new URL(req.url,'http://localhost'),p='frontend/public'+decodeURIComponent(u.pathname)
 if(!tracked.has(p)||!['.png','.jpg','.webp','.ttf'].includes(extname(p)))return next()
 try{const b=git('show','HEAD:'+p);assetReads.set(p,{path:p,sha256:hash(b),bytes:b.length});res.setHeader('Content-Type',extname(p)==='.ttf'?'font/ttf':'image/'+extname(p).slice(1));res.end(b)}catch(e){next(e)}
})}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false}})
let browser,page,stage='boot'
const state=()=>page.evaluate(()=>window.__QA06_FIDELITY__.state())
async function geometry(){return page.evaluate(()=>{
 const box=s=>{const e=document.querySelector(s),r=e?.getBoundingClientRect();return r?{x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom}:null}
 return {body:box('.shell-panel-body'),feed:box('.feed-browse'),footer:box('.feed-footer'),profile:box('.me-profile-header'),activity:box('.reference-activity'),options:box('.me-profile-options'),duplicateCompose:document.querySelectorAll('.shell-section-links a[href="/feed/new"]').length,genericMe:document.querySelectorAll('.shell-section-links').length,documentWidth:document.documentElement.scrollWidth,viewport:innerWidth}
})}
try{
 await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const width of(phase==='before'?[390]:[360,390,430])){
  const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,isMobile:true,reducedMotion:'reduce',serviceWorkers:'block'});page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  const row={width};report.cases.push(row)
  const open=async(entry,extra='')=>{await page.goto(server.resolvedUrls.local[0]+'src/features/event-feed/__tests__/qa06-fidelity/preview.html?'+new URLSearchParams({entry:'/world'})+extra);await expect.poll(()=>page.evaluate(()=>Boolean(window.__QA06_FIDELITY__)),{timeout:20000}).toBe(true);await expect.poll(async()=>(await state()).published,{timeout:20000}).toBe(true);await expect.poll(async()=>(await state()).phase,{timeout:15000}).toBe('live');await page.evaluate(path=>window.__QA06_FIDELITY__.navigate(path),entry);await page.evaluate(()=>document.fonts.ready);await page.waitForTimeout(150)}
  const shot=async name=>{const p=resolve(out,name+'-'+width+'.png');await page.screenshot({path:p});return {file:p.slice(repo.length+1),sha256:hash(await readFile(p))}}
  stage=width+'/feed';await open('/feed');await expect(page.locator('.feed-post')).toHaveCount(1);row.feed={geometry:await geometry(),capture:await shot('feed-short'),state:await state()}
  if(phase==='before'){assert.equal(row.feed.geometry.duplicateCompose,1);assert(row.feed.geometry.body.bottom-row.feed.geometry.footer.bottom>200)}
  else{
   assert.equal(row.feed.geometry.duplicateCompose,0);assert(row.feed.geometry.body.bottom-row.feed.geometry.footer.bottom<=10,JSON.stringify(row.feed.geometry));await expect(page.getByRole('button',{name:'Write a post'})).toHaveCount(1)
   await page.getByRole('button',{name:'Write a post'}).click();await expect.poll(async()=>(await state()).path).toBe('/feed/new');await page.goBack();await expect(page.locator('.feed-post')).toHaveCount(1)
   await page.getByRole('tab',{name:'ALL',exact:true}).focus();await page.keyboard.press('End');await expect(page.getByRole('tab',{name:'NOTICE',exact:true})).toBeFocused();await page.keyboard.press('Home');await expect(page.getByRole('tab',{name:'ALL',exact:true})).toBeFocused()
   await open('/feed','&posts=long');await expect(page.locator('.feed-post')).toHaveCount(16);await page.locator('.shell-panel-body').evaluate(e=>{e.scrollTop=e.scrollHeight});await expect(page.getByRole('button',{name:'Write a post'})).toBeInViewport();row.longFeed={geometry:await geometry(),capture:await shot('feed-long')};const lastPost=await page.locator('.feed-post').last().boundingBox(),composeFooter=await page.locator('.feed-footer').boundingBox();assert(lastPost.y+lastPost.height<=composeFooter.y,'last post remains above sticky compose footer')
  }
  stage=width+'/me';await open('/me');await expect(page.locator('.me-identity')).toBeVisible();await expect(page.getByText('Recent activity is unavailable in this connection.',{exact:true})).toBeVisible();row.me={geometry:await geometry(),capture:await shot('me-cold'),state:await state()}
  assert(!row.me.state.requests.some(r=>r.path.endsWith('/feed')),'Me opening must not create a feed read')
  if(phase!=='before'){
   assert.equal(row.me.geometry.genericMe,0);assert(row.me.geometry.activity.height<110,'unavailable activity remains compact')
   await page.getByRole('link',{name:'Open feed',exact:true}).click();await expect.poll(async()=>(await state()).path).toBe('/feed')
   await open('/feed','&posts=own');await expect(page.locator('.feed-post')).toHaveCount(1);await page.evaluate(()=>window.__QA06_FIDELITY__.navigate('/me'));await expect(page.getByRole('link',{name:/Posted to the feed/})).toHaveCount(1);row.meWarm={geometry:await geometry(),capture:await shot('me-loaded-activity'),state:await state()}
   await page.locator('.me-profile-options summary').click()
   const destinations=['/me/profile','/me/avatar','/me/social','/me/requests','/me/settings','/me/age','/me/blocks','/me/account']
   for(const path of destinations)await expect(page.locator(`.me-hub a[href="${path}"]`)).toHaveCount(1)
   row.accountDestinations=destinations
   const controls=await page.locator('.me-hub a:visible,.me-hub summary').evaluateAll(es=>es.map(e=>({name:e.getAttribute('aria-label')||e.textContent,width:e.getBoundingClientRect().width,height:e.getBoundingClientRect().height})));assert(controls.every(c=>c.width>=44&&c.height>=44),JSON.stringify(controls));row.controls=controls
   assert(!await page.locator('.me-hub').innerText().then(t=>/Lv\.|points|Reserved MCR|Joined “/.test(t)));await page.getByRole('link',{name:'Edit profile',exact:true}).click();await expect(page.getByRole('textbox',{name:'Display name',exact:true})).toBeVisible();await page.goBack();await expect(page.locator('.me-identity')).toBeVisible();row.profileRoute='PASS'
  }
  assert.equal((await geometry()).documentWidth,width);const final=await state();assert.equal(final.canvases,1);assert.equal(final.socketPeak,1);await page.evaluate(()=>window.__QA06_FIDELITY__.dispose());row.cleanup=await state();assert.equal(row.cleanup.canvases,0);assert.equal(row.cleanup.activeSockets,0);row.status='PASS';await context.close()
 }
 assert.deepEqual(report.errors,[]);report.status='PASS'
}catch(e){report.failure={stage,error:String(e.stack)};await page?.screenshot({path:resolve(out,'failure.png')}).catch(()=>{});throw e}
finally{report.assets=[...assetReads.values()];await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');await page?.evaluate(()=>window.__QA06_FIDELITY__?.dispose()).catch(()=>{});await browser?.close();await server.close();await rm(cache,{recursive:true,force:true})}
console.log(JSON.stringify({status:report.status,phase,widths:report.cases.map(c=>c.width)}))
