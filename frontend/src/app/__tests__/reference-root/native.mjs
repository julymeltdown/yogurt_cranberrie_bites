import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile, statfs } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const disk=await statfs('.');assert(disk.bavail*disk.bsize>=8*1024**3,'Disk floor: restore headroom before native capture')
const evidence=process.env.MCR_REFERENCE_ROOT_EVIDENCE??'/tmp/mcr-fe-reference-root-evidence';await mkdir(evidence,{recursive:true})
const server=await createServer({configLoader:'runner',cacheDir:'/tmp/mcr-fe-reference-root-cache',server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'});await server.listen()
let browser,stage='start';const cases=[],errors=[]
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 for(const [width,height] of [[360,800],[390,844],[430,932]]){
  const context=await browser.newContext({viewport:{width,height},hasTouch:true,reducedMotion:'reduce'}),page=await context.newPage();page.on('pageerror',e=>errors.push(e.message))
  const state=()=>page.evaluate(()=>window.__ILLUSTRATED_MAP__?.state())
  const navigate=path=>page.evaluate(p=>window.__ILLUSTRATED_MAP__.navigate(p),path)
  stage=`${width}/open`;await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/app/__tests__/reference-root/preview.html?social=1`)
  await expect.poll(async()=>(await state())?.published).toBe(true);await expect.poll(async()=>(await state())?.room).toBeTruthy();const before=await state()
  const stable=async()=>{const s=await state();assert.equal(s.mounts,before.mounts);assert.equal(s.games,1);assert.equal(s.peak,1);assert(s.activeSockets<=1);assert(s.socketPeak<=1);assert(s.sameCamera);if(s.outfit)assert.deepEqual(s.outfit,before.outfit);assert.equal(s.canvases,1)}
  const measure=()=>page.evaluate(()=>{
   const rect=s=>{const r=document.querySelector(s)?.getBoundingClientRect();return r?{x:r.x,y:r.y,width:r.width,height:r.height}:null}
   return {viewport:{width:innerWidth,height:innerHeight},shell:rect('.world-shell'),world:rect('.shell-world'),canvas:rect('.shell-world canvas'),panel:rect('.shell-panel'),body:rect('.shell-panel-body'),header:rect('.mcr-chrome-header'),era:rect('.shell-era-region'),nav:rect('.mcr-chrome-bottom'),overflow:document.documentElement.scrollWidth>innerWidth,mode:document.querySelector('.world-shell').dataset.panelSize,
    small:[...document.querySelectorAll('.mcr-chrome-header a,.mcr-chrome-eras button,.mcr-chrome-bottom a,.mcr-chrome-panel-header button')].filter(e=>e.checkVisibility()).map(e=>({name:e.getAttribute('aria-label')??e.textContent,w:e.getBoundingClientRect().width,h:e.getBoundingClientRect().height})).filter(r=>r.w<43.9||r.h<43.9)}
  })
  const shot=async name=>{await expect(page.getByText(/^Opening (this section|your account|your profile|your avatar|settings|map|wall|parties|gifts|account controls)…$/)).toHaveCount(0);const m=await measure();assert(!m.overflow);assert.deepEqual(m.small,[]);await page.screenshot({path:`${evidence}/${name}-${width}.png`});return m}
  const nav=page.getByRole('navigation',{name:'Main navigation'})
  await expect(nav).toBeVisible();const world=await shot('world');assert(world.world.height/height>=.70);assert.equal(world.mode,'closed')
  stage=`${width}/feed`;await nav.getByRole('link',{name:'Feed',exact:true}).focus();await page.keyboard.press('Enter')
  await expect(page.getByRole('heading',{name:'Feed',level:1,exact:true})).toBeFocused();await expect(nav).toBeHidden();const feed=await shot('feed');assert.equal(feed.mode,'full');assert(feed.panel.height/height>=.95);await stable()
  await page.getByRole('button',{name:'Reduce panel',exact:true}).tap();await expect(nav).toBeVisible();assert.equal((await measure()).mode,'half');await stable()
  await page.getByRole('button',{name:'Minimize panel',exact:true}).tap();await expect(page.locator('.shell-panel-body')).toBeHidden()
  await page.getByRole('button',{name:'Show panel content',exact:true}).tap();await expect(page.locator('.shell-panel-body')).toBeVisible()
  await page.getByRole('button',{name:'Expand panel',exact:true}).tap();await expect(nav).toBeHidden();await stable()
  await page.getByRole('button',{name:'Back to previous panel',exact:true}).tap();await expect(nav.getByRole('link',{name:'Feed',exact:true})).toBeFocused();await stable()
  const routes=[]
  for(const [path,name] of [['/world/map','map'],['/meet','meet'],['/gifts','gifts'],['/wall','wall'],['/me/avatar','customize'],['/me','profile']]){
   stage=`${width}/${name}`;await navigate(path);await expect(page.locator('.shell-panel')).toBeVisible();await expect(page.getByRole('button',{name:'Reduce panel',exact:true})).toBeVisible();await expect(nav).toBeHidden()
   const m=await shot(name);assert.equal(m.mode,'full');await stable();routes.push({path,name,...m})
   await page.getByRole('button',{name:'Back to previous panel',exact:true}).tap();if(name==='customize'){await expect.poll(async()=>(await state()).path).toBe('/me');await page.getByRole('button',{name:'Back to previous panel',exact:true}).tap()}await expect(nav).toBeVisible()
  }
  stage=`${width}/short`;await page.setViewportSize({width,height:430});await navigate('/me/settings');await expect(page.getByRole('button',{name:'Back to previous panel'})).toBeInViewport();await shot('settings-short');await stable()
  await page.getByRole('button',{name:'Back to previous panel'}).tap();await expect.poll(async()=>(await state()).path).toBe('/me');await page.getByRole('button',{name:'Back to previous panel'}).tap();await expect(nav).toBeVisible()
  await page.evaluate(()=>window.__ILLUSTRATED_MAP__.dispose());await expect.poll(async()=>(await state()).games).toBe(0);await expect.poll(async()=>(await state()).activeSockets).toBe(0)
  cases.push({width,height,world,feed,routes,sizing:true,keyboardBackFocus:true,singleGameSocketCamera:true,cleanup:true});await context.close()
 }
 assert.deepEqual(errors,[])
 const paths=['src/app/WorldShell.tsx','src/app/shell.css','src/app/shell-chrome/index.tsx','src/app/shell-chrome/identity.tsx','src/app/shell-chrome/parade-footer.tsx','src/app/shell-chrome/shell-chrome.css','src/shared/styles/base.css','src/shared/ui/concert-window/index.tsx','src/shared/ui/concert-window/concert-window.css','src/app/__tests__/reference-root/native.mjs','src/app/__tests__/reference-root/preview.tsx']
 const hashes=Object.fromEntries(await Promise.all(paths.map(async p=>[p,createHash('sha256').update(await readFile(p)).digest('hex')])))
 await writeFile(`${evidence}/result.json`,JSON.stringify({result:'PASS',referenceSHA256:'d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617',actualDefaultRoutes:true,actualPhaser:true,syntheticHTTPandSocket:true,approvedCharacters:false,liveServices:'NOT_RUN',fullReferenceAcceptance:'NOT_RUN',cases,errors,hashes},null,2)+'\n')
 console.log(JSON.stringify({result:'PASS',evidence,widths:cases.length,errors}))
}catch(error){console.error('FAILED',stage);throw error}finally{await browser?.close();await server.close()}
