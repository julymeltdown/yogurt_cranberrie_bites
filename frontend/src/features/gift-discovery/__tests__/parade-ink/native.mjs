// TEST_ONLY HTTP/clock counterparts; real default router, providers and feature controllers.
import assert from 'node:assert/strict'
import { readFile, writeFile, mkdir, readdir, statfs } from 'node:fs/promises'
import { existsSync, readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { resolve, dirname, relative, extname } from 'node:path'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import { chromium, expect } from '@playwright/test'
import { compareReferences } from './reference-comparison.mjs'
const frontend = resolve(fileURLToPath(new URL('../../../../../', import.meta.url)))
const output = process.env.MCR_PI04_OUTPUT, temporary = process.env.MCR_PI04_TEMP
const foundation = process.env.MCR_PI01_CHECKOUT
const borrowed = process.env.MCR_PI04_BORROWED
if (!output || !temporary || !foundation || !borrowed) throw Error('Explicit allocated output/temp and read-only source/dependency inputs required')
const port=Number(process.env.MCR_PI04_NATIVE_PORT)
assert.equal(port,5367,'This finite native phase owns only private port 5367')
await mkdir(output, { recursive: true })
const peer = resolve(foundation, 'frontend')
const frozenFoundation=await readFile(resolve(frontend,'../docs/mcr/evidence/parade-ink-giveaway/foundation-final-readset.json'))
assert.equal(createHash('sha256').update(frozenFoundation).digest('hex'),'4360966a5135644942bc4267bfe00477ccd83182510791f8c2d4acb591152c72')
const expected=JSON.parse(frozenFoundation),overlay=new Map(),cssReads=new Map()
// Read the same granted eighteen postimages from immutable Git, in memory only.
for(const item of expected.implementation){
 const bytes=execFileSync('git',['show',`80e6d082fdebcd237f64264e2eebb3eedbc5abdc:${item.path}`],{cwd:foundation})
 assert.equal(createHash('sha256').update(bytes).digest('hex'),item.sha256)
 overlay.set(resolve(peer,item.path.replace(/^frontend\//,'')),bytes)
}
function cssSource(path,stack=new Set()){
 assert.ok(!stack.has(path),'CSS import cycle')
 const next=new Set(stack).add(path),local=resolve(frontend,relative(peer,path))
 const raw=overlay.get(path)??readFileSync(local)
 cssReads.set(path,{path:relative(peer,path),sha256:createHash('sha256').update(raw).digest('hex'),pinned:overlay.has(path)})
 return raw.toString('utf8').replace(/url\(\s*(["']?)([^"')]+)\1\s*\)/g,(original,_quote,url)=>{
  if(!url.startsWith('.'))return original
  const asset=resolve(dirname(path),url)
  return overlay.has(asset)&&asset.endsWith('.ttf')?`url("/__pi04font/${asset.split('/').at(-1)}")`:`url("/src/${relative(resolve(frontend,'src'),resolve(frontend,relative(peer,asset)))}")`
 }).replace(/@import\s+["']([^"']+)["']\s*(?:layer\(([^)]+)\)\s*)?;/g,(original,source,layer)=>{
  if(!source.startsWith('.'))return original
  const expanded=cssSource(resolve(dirname(path),source),next)
  return layer?`@layer ${layer} {\n${expanded}\n}`:expanded
 })
}
const shared = ['src/app/reference-chrome-v2/WindowInk.tsx','src/app/reference-chrome-v2/chrome.css','src/app/shell-chrome/parade-footer.tsx','src/app/shell-chrome/shell-chrome.css','src/app/shell.css','src/shared/styles/base.css','src/shared/styles/themes/themes.css','src/shared/ui/concert-window/concert-window.css','src/shared/ui/concert-window/index.tsx']
async function pins() {
 const result = {}
 for (const path of shared) result[path]=createHash('sha256').update(overlay.get(resolve(peer,path))??await readFile(resolve(peer,path))).digest('hex')
 for(const [path,bytes] of overlay)result[relative(peer,path)]=createHash('sha256').update(bytes).digest('hex')
 return result
}
const before=await pins(), errors=[], cases=[], comparisonCaptures={}
const frozenAssets=new Map()
function resourcePins(value){
 if(Array.isArray(value)){value.forEach(resourcePins);return}
 if(!value||typeof value!=='object')return
 if(typeof value.path==='string'&&typeof value.sha256==='string'&&/^world\/[a-zA-Z0-9_./-]+$/.test(value.path)&&!value.path.includes('..')){
  const bytes=execFileSync('git',['show',`f025bdb6fb409f3b7706e475dfabb5f3d21fe168:frontend/public/${value.path}`],{cwd:frontend})
  assert.equal('sha256:'+createHash('sha256').update(bytes).digest('hex'),value.sha256)
  frozenAssets.set('/'+value.path,{bytes,sha256:value.sha256})
 }
 Object.values(value).forEach(resourcePins)
}
const staticRelease=execFileSync('git',['show','f025bdb6fb409f3b7706e475dfabb5f3d21fe168:frontend/public/world/avatars/derived-v1/static-release.json'],{cwd:frontend})
assert.equal(createHash('sha256').update(staticRelease).digest('hex'),'cc53c6852742e9cbb9858a2d0961411e1a9e131d1e30c0f2b73b265e03f91677')
frozenAssets.set('/world/avatars/derived-v1/static-release.json',{bytes:staticRelease,sha256:'sha256:cc53c6852742e9cbb9858a2d0961411e1a9e131d1e30c0f2b73b265e03f91677'})
resourcePins(JSON.parse(await readFile(resolve(frontend,'../contracts/fixtures/assets/selected-static/avatar-manifest.json'))))
for(const file of ['world/themes/binding-inputs.json','world/illustrated-runtime/bindings.json'])resourcePins(JSON.parse(await readFile(resolve(frontend,'public',file))))
const mapBinding=JSON.parse(await readFile(resolve(frontend,'public/world/maps/binding-inputs.json')))
resourcePins({path:mapBinding.tilemap_path,sha256:mapBinding.tilemap_sha256})
for(const item of expected.implementation)assert.equal(before[item.path.replace(/^frontend\//,'')],item.sha256,`PI01 source pin drift: ${item.path}`)
const report={classification:'TEST_ONLY',liveBackend:false,base:'f025bdb6fb409f3b7706e475dfabb5f3d21fe168',foundationStatus:'final18 immutable base-layer and panel-glyph correction; family native acceptance separate',originalFoundationCommit:'056a8c48ff667134dee8e5e99710185294e4b898',foundationCommit:'80e6d082fdebcd237f64264e2eebb3eedbc5abdc',foundationHashes:before,errors,cases,status:'NOT_RUN'}
report.staticAssetPins=[...frozenAssets].map(([path,asset])=>({path,sha256:asset.sha256,bytes:asset.bytes.length,source:'f025 immutable Git blob in memory; no renderer or authority override'}))
report.assetResponses=[];report.servedStaticAssets=[]
const servedAssets=new Set()
function observeAssets(page){
 page.on('response',response=>{const path=new URL(response.url()).pathname;if(path.startsWith('/world/')&&report.assetResponses.length<256)report.assetResponses.push({path,status:response.status(),type:response.headers()['content-type']??null})})
 page.on('requestfailed',request=>{const path=new URL(request.url()).pathname;if(path.startsWith('/world/')&&report.assetResponses.length<256)report.assetResponses.push({path,failure:request.failure()?.errorText??'failed'})})
}
const dev=await createServer({configFile:false,root:frontend,publicDir:false,cacheDir:resolve(temporary,'native-cache'),plugins:[{
 name:'pi04-pinned-read-composition',enforce:'pre',
 load(id){const path=id.split('?')[0],bytes=overlay.get(path);if(bytes)return path.endsWith('.css')?cssSource(path):path.endsWith('.ttf')?`export default "/__pi04font/${path.split('/').at(-1)}"`:bytes.toString('utf8')},
 resolveId(spec,importer){
  // Leave bare packages to Vite so CommonJS dependencies are optimized for ESM.
  if(!importer || !(spec.startsWith('.')||spec.startsWith('/')))return null
  const abs=spec.startsWith('/')?spec:resolve(dirname(importer.split('?')[0]),spec)
  const local=abs.startsWith(peer+'/')?resolve(frontend,relative(peer,abs)):abs
  if(!local.startsWith(frontend+'/'))return null
  for(const suffix of ['', '.ts','.tsx','/index.ts','/index.tsx']){
   const path=relative(frontend,local+suffix)
   if(shared.includes(path)||path.startsWith('src/shared/styles/parade-ink/')) {const selected=resolve(peer,path);if(existsSync(selected))return selected}
   if(abs.startsWith(peer+'/')&&existsSync(local+suffix))return local+suffix
  }
  return null
 },
 configureServer(server){server.middlewares.use((req,res,next)=>{
  const path=decodeURIComponent((req.url??'').split('?')[0])
  if(frozenAssets.has(path)){const asset=frozenAssets.get(path);if(!servedAssets.has(path)){servedAssets.add(path);report.servedStaticAssets.push({path,sha256:asset.sha256,bytes:asset.bytes.length})}res.setHeader('Content-Type',path.endsWith('.json')?'application/json':'image/png');res.end(asset.bytes);return}
  if(path.startsWith('/__pi04font/')){const font=[...overlay].find(([name])=>name.endsWith('/'+path.split('/').at(-1))&&name.endsWith('.ttf'));if(font){res.setHeader('Content-Type','font/ttf');res.end(font[1]);return}}
  if(path.startsWith('/src/')||path.startsWith('/@')||path.includes('..'))return next()
  for(const base of [resolve(frontend,'public'),resolve(borrowed,'public')]){
   const file=resolve(base,'.'+path)
   if(file.startsWith(base+'/')&&existsSync(file)&&extname(file)){
    const types={'.json':'application/json','.png':'image/png','.svg':'image/svg+xml','.webp':'image/webp','.jpg':'image/jpeg'}
    if(!types[extname(file)])continue
    res.setHeader('Content-Type',types[extname(file)]);res.end(readFileSync(file));return
   }
  }
  next()
 })}
},react()],optimizeDeps:{entries:['src/features/gift-discovery/__tests__/parade-ink/preview.html','src/features/gift-discovery/__tests__/parade-ink/state-preview.html'],include:['react','react-dom/client','react/jsx-runtime','react/jsx-dev-runtime','react-router-dom','@tanstack/react-query','react-hook-form','@hookform/resolvers/zod','zod','phaser']},resolve:{dedupe:['react','react-dom','react-router','react-router-dom','@tanstack/react-query']},server:{host:'127.0.0.1',port,strictPort:true,hmr:false,fs:{allow:[frontend,peer,borrowed]}}})
let browser
try{
 await dev.listen();report.port=dev.httpServer.address().port
 await dev.environments.client.depsOptimizer?.scanProcessing
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-breakpad']})
 report.browserVersion=browser.version();report.deviceScaleFactor=1;report.height=844
 const storage=await statfs(frontend);assert.ok(storage.bavail*storage.bsize>=9*1024**3,'After-ALL native floor failed')
 const origin=`http://127.0.0.1:${report.port}`
 const gift='00000000-0000-4000-8000-000000000005',reservation='00000000-0000-4000-8000-000000000006'
 const routes=[['browse','/gifts','.gifts-reference',false],['create','/gifts/new','.giveaway-authoring',false],['detail',`/gifts/${gift}`,'.gift-detail',false],['manage',`/gifts/${gift}/manage`,'.giveaway-authoring',true],['reserve',`/gifts/${gift}/reservation`,'.gift-pickup',false],['pickup',`/gifts/pickups/${reservation}`,'.gift-pickup',false],['activity','/gifts/mine','.gift-activity',false]]
 const selectedWidth=process.env.MCR_PI04_WIDTH?Number(process.env.MCR_PI04_WIDTH):null
 const affectedOnly=process.env.MCR_PI04_AFFECTED_ONLY==='1',worldOnly=process.env.MCR_PI04_WORLD_ONLY==='1'
 if(worldOnly)report.remainingCriteria='World lifecycle only; Native10 retains14 affected UI positives and21 PNGs, overallFAIL wrong covered-panel oracle preserved; two new detail portrait/fallback screenshots only.'
 if(affectedOnly)report.reusedEvidence={candidate:'cfd7c4f2a035e99007c46817bd8c963579099b2f',directory:'native06',overall:'FAIL retained',cases:'Unchanged authoring, activity and existing-reservation cases; affected browse/reserve/detail plus inventory/focus recaptured'}
 for(const width of worldOnly?[]:selectedWidth?[selectedWidth]:[390,360,430]){
  for(const [name,entry,selector,owner] of routes){
   if(affectedOnly&&(!['browse','reserve','detail','manage','create'].includes(name)||(['detail','create'].includes(name)&&width!==390)))continue
   const context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce'})
   const page=await context.newPage(),navigationRequests=[]
   observeAssets(page);page.on('pageerror',e=>errors.push({name,width,message:e.message}))
   page.on('request',request=>{if(request.isNavigationRequest())navigationRequests.push(request.url())})
   try{
    await page.goto(`${origin}/src/features/gift-discovery/__tests__/parade-ink/preview.html?entry=${encodeURIComponent(entry)}&owner=${owner?1:0}`)
    await expect(page.locator(selector)).toBeVisible({timeout:30000})
    await expect.poll(()=>page.locator(selector).evaluate(el=>getComputedStyle(el).getPropertyValue('--pi-contract').trim()),{timeout:10000}).toBe('"pi01-photo2-v1"')
    await page.evaluate(async()=>{await document.fonts.load('400 18px "Parade Ink Text"');await document.fonts.load('700 26px "Parade Ink Display"');await document.fonts.ready})
    if(width===390&&name==='browse'){
     report.loadedFonts=await page.evaluate(()=>[...document.fonts].filter(face=>face.status==='loaded').map(face=>face.family))
     assert.ok(report.loadedFonts.some(font=>font.includes('Parade Ink Display'))&&report.loadedFonts.some(font=>font.includes('Parade Ink Text')),'Actual pinned Oswald/Roboto font faces must load')
    }
    if(name==='browse')await expect(page.locator('.gift-reference-row')).toHaveCount(2)
    if(name==='detail')await expect(page.getByRole('heading',{name:'Fan badges',exact:true})).toBeVisible()
    if(name==='manage')await expect(page.getByLabel('Owner inventory')).toBeAttached()
    if(name==='reserve')await expect(page.getByRole('button',{name:'Confirm reservation',exact:true})).toBeVisible()
    if(name==='pickup')await expect(page.getByRole('heading',{name:'Reservation status',exact:true})).toBeVisible()
    const metrics=await page.locator(selector).evaluate(el=>{const s=getComputedStyle(el),r=el.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,font:s.fontFamily,fontSize:s.fontSize,lineHeight:s.lineHeight,color:s.color,viewport:innerWidth,overflow:document.documentElement.scrollWidth>innerWidth+1}})
    if(width===390&&name==='browse')report.giveawayRow=await page.locator('.gift-reference-row').first().evaluate(el=>{const rect=e=>{const r=e.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height}};return{row:rect(el),image:rect(el.querySelector('.gift-reference-image')),title:rect(el.querySelector('h3')),stock:rect(el.querySelector('.gift-reference-stock')),byline:rect(el.querySelector('.gift-reference-byline')),action:rect(el.querySelector('.gift-reference-action'))}})
    assert.equal(metrics.overflow,false,`${name}/${width} page overflow`)
    assert.equal(metrics.fontSize,'18px')
    const targets=await page.locator(`${selector} button,${selector} a,${selector} input:not([type=checkbox]),${selector} select,${selector} summary`).evaluateAll(nodes=>nodes.filter(e=>e.getClientRects().length).map(e=>{const r=e.getBoundingClientRect();return {label:e.getAttribute('aria-label')??e.textContent?.trim(),width:r.width,height:r.height}}))
    assert.ok(targets.every(t=>t.height>=43.5&&t.width>=43.5),`${name}/${width}: ${JSON.stringify(targets.filter(t=>t.height<43.5||t.width<43.5))}`)
    await page.screenshot({path:resolve(output,`${name}-${width}.png`)})
    if(width===390&&(name==='browse'||name==='reserve')){
     const component=page.locator(name==='browse'?'.gift-reference-row':'.gift-reserve-card').first(),bounds=await component.boundingBox()
     const bytes=await component.screenshot()
     comparisonCaptures[name==='browse'?'row':'reserve']={width:bounds.width,height:bounds.height,data:'data:image/png;base64,'+bytes.toString('base64')}
    }
    if(width===390&&name==='browse'){
     const tab=page.getByRole('tab',{name:'Browse',exact:true});await tab.focus();await page.keyboard.press('ArrowRight')
     await expect(page.getByRole('tab',{name:'My giving',exact:true})).toHaveAttribute('aria-selected','true')
     const selected=await page.getByRole('tab',{name:'My giving',exact:true}).evaluate(e=>({background:getComputedStyle(e).backgroundColor,color:getComputedStyle(e).color,outline:getComputedStyle(e).outlineStyle}))
     assert.equal(selected.background,'rgb(23, 23, 23)');assert.equal(selected.color,'rgb(255, 255, 255)');assert.notEqual(selected.outline,'none')
     report.selectedTab=selected
    }
    if(width===390&&name==='create'){
     await page.getByLabel('Item title',{exact:true}).fill('TEST_ONLY retained title')
     await page.getByRole('button',{name:'Create giveaway draft',exact:true}).click()
     await expect(page.getByRole('alert').first()).toBeVisible()
     await expect(page.getByLabel('Item title',{exact:true})).toHaveValue('TEST_ONLY retained title')
     await page.setViewportSize({width:390,height:480});await page.getByRole('button',{name:'Create giveaway draft',exact:true}).scrollIntoViewIfNeeded();await page.getByRole('button',{name:'Create giveaway draft',exact:true}).focus()
     await page.keyboard.press('Tab');await page.keyboard.press('Shift+Tab')
     assert.notEqual(await page.getByRole('button',{name:'Create giveaway draft',exact:true}).evaluate(e=>getComputedStyle(e).outlineStyle),'none','Short-height keyboard focus remains visible')
     await page.screenshot({path:resolve(output,'create-short-validation-focus-390.png')})
    }
    if(name==='manage'){
     const inventory=page.getByLabel('Owner inventory');await inventory.scrollIntoViewIfNeeded();await expect(inventory).toBeVisible()
     const table=page.getByRole('region',{name:'Reservation table'});await table.focus();await page.keyboard.press('ArrowRight');await expect.poll(()=>table.evaluate(el=>el.scrollLeft)).toBeGreaterThan(0)
     await page.screenshot({path:resolve(output,`owner-inventory-${width}.png`)})
    }
    const state=await page.evaluate(()=>window.__PI04_NATIVE__.state())
    assert.ok(state.canvases<=1,`${name}/${width}: no duplicate game while full panel covers World`)
    assert.equal(state.activeSockets,0,`${name}/${width}: fresh full panel does not start World presence`)
    assert.ok(state.socketPeak<=1,`${name}/${width}: duplicate socket`)
    cases.push({name,width,metrics,targets,state,status:'PASS'})
   }catch(error){
    await page.screenshot({path:resolve(output,'failure-screen.png')}).catch(()=>{})
    report.failureStyles=await page.locator(selector).evaluate(el=>({font:getComputedStyle(el).fontFamily,contract:getComputedStyle(el).getPropertyValue('--pi-contract'),faces:[...document.fonts].map(f=>({family:f.family,status:f.status})),styles:[...document.styleSheets].map(s=>({href:s.href,rules:(()=>{try{return s.cssRules.length}catch{return null}})()}))})).catch(()=>null)
    const body=(await page.locator('body').innerText({timeout:2000}).catch(()=>'' )).slice(0,24000).replace(/\b\d{6}\b/g,'[six digits]')
    await writeFile(resolve(output,'failure-dom.json'),JSON.stringify({name,width,url:page.url(),navigationRequests,body,hasFixtureApi:await page.evaluate(()=>!!window.__PI04_NATIVE__).catch(()=>false)},null,2)+'\n')
    throw error
   }finally{await context.close()}
  }
 }
 // Relevant terminal/unavailable presentation at the primary 390px width.
 if(!worldOnly&&(!selectedWidth||selectedWidth===390))for(const [name,entry,selector,visualCase,owner] of [
  ['sold-out',`/gifts/${gift}/reservation`,'.gift-pickup','sold-out',false],
  ['expired',`/gifts/pickups/${reservation}`,'.gift-pickup','expired',false],
  ['cancelled',`/gifts/pickups/${reservation}`,'.gift-pickup','cancelled',false],
  ['loading','/gifts','.gifts-reference','loading',false],
  ['error','/gifts','.gifts-reference','error',false],
  ['owner-claim',`/gifts/${gift}/manage`,'.giveaway-authoring','normal',true],
  ['claimed',`/gifts/${gift}/manage`,'.giveaway-authoring','normal',true],
  ['pending',`/gifts/${gift}/reservation`,'.gift-pickup','pending',false],
  ['uncertain',`/gifts/${gift}/reservation`,'.gift-pickup','uncertain',false],
  ['conflict',`/gifts/${gift}/manage`,'.giveaway-authoring','conflict',true]]){
  if(affectedOnly&&!['sold-out','pending','uncertain'].includes(name))continue
  const context=await browser.newContext({viewport:{width:390,height:844}}),page=await context.newPage()
  observeAssets(page);page.on('pageerror',e=>errors.push({name,width:390,message:e.message}))
  try{
   await page.goto(`${origin}/src/features/gift-discovery/__tests__/parade-ink/preview.html?entry=${encodeURIComponent(entry)}&case=${visualCase}&owner=${owner?1:0}`)
   await expect(page.locator(selector)).toBeVisible({timeout:30000})
   if(name==='owner-claim'||name==='claimed'){await page.getByRole('button',{name:'Open pickup 1',exact:true}).click();await expect(page.getByLabel('Recipient’s six-digit code')).toBeVisible();await page.locator('.gift-pickup').scrollIntoViewIfNeeded()}
   if(name==='sold-out')await expect(page.getByRole('button',{name:'Confirm reservation',exact:true})).toBeDisabled()
   if(name==='expired'||name==='cancelled'){await expect(page.getByRole('heading',{name:'Reservation status',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Request pickup code',exact:true})).toHaveCount(0)}
   if(name==='pending'||name==='uncertain'){await page.getByRole('button',{name:'Confirm reservation',exact:true}).click();await expect(page.getByRole('button',{name:'Check original command',exact:true})).toBeVisible();await expect(page.getByRole('heading',{name:'Collection receipt',exact:true})).toHaveCount(0)}
   if(name==='claimed'){await page.getByLabel('Recipient’s six-digit code').fill('123456');await page.getByRole('button',{name:'Confirm one-time collection',exact:true}).click();await expect(page.getByRole('heading',{name:'Collection receipt',exact:true})).toBeVisible()}
   if(name==='conflict'){await expect(page.getByRole('button',{name:'Save giveaway changes',exact:true})).toBeEnabled();await page.getByLabel('Item title',{exact:true}).fill('TEST_ONLY preserved conflict draft');await page.getByRole('button',{name:'Save giveaway changes',exact:true}).click();await expect(page.getByRole('status').filter({hasText:'Stock or settings changed'})).toBeVisible();await expect(page.getByLabel('Item title',{exact:true})).toHaveValue('TEST_ONLY preserved conflict draft')}
   if(name==='owner-claim')await expect(page.getByLabel('Recipient’s six-digit code')).toBeVisible()
   if(name==='loading')await expect(page.getByRole('status').filter({hasText:'Loading giveaways'})).toBeVisible()
   if(name==='error')await expect(page.getByRole('alert').filter({hasText:'Gifts could not be loaded'})).toBeVisible()
   await page.evaluate(()=>document.fonts.ready);await page.screenshot({path:resolve(output,`${name}-390.png`)})
   cases.push({name,width:390,status:'PASS'})
  }finally{await context.close()}
 }
 if(!worldOnly&&!affectedOnly&&(!selectedWidth||selectedWidth===390)){
  const context=await browser.newContext({viewport:{width:390,height:844}}),page=await context.newPage()
  observeAssets(page);page.on('pageerror',e=>errors.push({name:'qualified-code-component',width:390,message:e.message}))
  try{
   await page.goto(`${origin}/src/features/gift-discovery/__tests__/parade-ink/state-preview.html`)
   await expect(page.getByRole('button',{name:'Read private pickup details',exact:true})).toBeVisible()
   await page.getByRole('button',{name:'Read private pickup details',exact:true}).click()
   await expect(page.getByLabel('Authorized pickup details')).toBeVisible()
   await page.getByRole('button',{name:'Request pickup code',exact:true}).click()
   await expect(page.getByLabel('Pickup confirmation code')).toHaveText('654321')
   await page.screenshot({path:resolve(output,'code-qualified-component-390.png'),mask:[page.getByLabel('Pickup confirmation code')]})
   cases.push({name:'qualified-code-component',width:390,classification:'TEST_ONLY qualified feature fixture; NOT default-App physical authority',status:'PASS'})
  }finally{await context.close()}
 }
 if(!worldOnly)await compareReferences(browser,output,comparisonCaptures)
 if(worldOnly)for(const unavailable of [false,true]){
  const context=await browser.newContext({viewport:{width:390,height:844},reducedMotion:'reduce'}),page=await context.newPage()
  const name=unavailable?'detail-unavailable-portrait':'detail-approved-portrait'
  observeAssets(page);page.on('pageerror',e=>errors.push({name,width:390,message:e.message}))
  try{
   if(unavailable)await page.route('**/world/avatars/derived-v1/static-release.json',route=>route.abort('failed'))
   await page.goto(`${origin}/src/features/gift-discovery/__tests__/parade-ink/preview.html?entry=${encodeURIComponent(`/gifts/${gift}`)}`)
   await expect(page.locator('.gift-detail')).toBeVisible()
   await expect.poll(()=>page.locator('.gift-detail').evaluate(el=>getComputedStyle(el).getPropertyValue('--pi-contract').trim())).toBe('"pi01-photo2-v1"')
   await page.evaluate(async()=>{await document.fonts.load('400 18px "Parade Ink Text"');await document.fonts.load('700 26px "Parade Ink Display"');await document.fonts.ready})
   const portrait=page.locator('.gift-detail-portrait')
   if(unavailable)await expect(portrait.getByRole('status')).toBeVisible()
   else await expect(portrait.locator('[data-whole-character-state=ready] canvas')).toBeVisible({timeout:15000})
   const metrics=await portrait.evaluate(el=>({width:el.getBoundingClientRect().width,height:el.getBoundingClientRect().height,fontSize:getComputedStyle(el).fontSize,lineHeight:getComputedStyle(el).lineHeight,overflow:document.documentElement.scrollWidth>innerWidth+1,text:el.textContent}))
   assert.equal(metrics.overflow,false)
   if(unavailable){assert.ok(metrics.width>200);assert.equal(metrics.fontSize,'14px');assert.equal(metrics.lineHeight,'18px');assert.ok(metrics.height<=20,'Unavailable label must remain one readable text row')}
   else assert.equal(metrics.width,64)
   await page.screenshot({path:resolve(output,`${name}-390.png`)})
   cases.push({name,width:390,status:'PASS',metrics,classification:unavailable?'TEST_ONLY actual asset failure; no identity substitution':'Exact admitted selected-source portrait; immutable f025 release and atlas'})
  }finally{await context.close()}
 }
 // Opening World supplies its actual admission/engine boundary. A fresh hidden
 // full panel may correctly have no game yet; do not invent that initial state.
 {
  const context=await browser.newContext({viewport:{width:390,height:844}}),page=await context.newPage()
  observeAssets(page);page.on('pageerror',e=>errors.push({name:'world-lifecycle',width:390,message:e.message}))
  try{
   await page.goto(`${origin}/src/features/gift-discovery/__tests__/parade-ink/preview.html?entry=%2Fgifts`)
   await expect(page.locator('.gifts-reference')).toBeVisible()
   await page.evaluate(()=>window.__PI04_NATIVE__.navigate('/world'))
   await expect.poll(()=>page.evaluate(()=>window.__PI04_NATIVE__.state().activeSockets),{timeout:15000}).toBe(1)
   await expect.poll(()=>page.evaluate(()=>window.__PI04_NATIVE__.state().canvases),{timeout:15000}).toBe(1)
   const canvas=await page.locator('.shell-world-host canvas').elementHandle()
   report.worldResumed=await page.evaluate(()=>window.__PI04_NATIVE__.state())
   assert.equal(report.worldResumed.socketPeak,1,'Only one connection while World is visible')
   await page.evaluate(()=>window.__PI04_NATIVE__.navigate('/gifts'))
   await expect(page.locator('.gifts-reference')).toBeVisible()
   report.worldCovered={state:await page.evaluate(()=>window.__PI04_NATIVE__.state()),visibility:await page.locator('.shell-world').evaluate(el=>getComputedStyle(el).visibility),inert:await page.locator('.shell-world-host').evaluate(el=>el.inert)}
   assert.equal(report.worldCovered.state.activeSockets,1,'Committed policy keeps existing presence while a panel covers the engine')
   assert.equal(report.worldCovered.visibility,'hidden');assert.equal(report.worldCovered.inert,true)
   assert.equal(await canvas.evaluate(node=>node===document.querySelector('.shell-world-host canvas')),true,'Panel return retains the same game')
   await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,value:true});document.dispatchEvent(new Event('visibilitychange'))})
   await expect.poll(()=>page.evaluate(()=>window.__PI04_NATIVE__.state().activeSockets)).toBe(0)
   report.worldBackgrounded=await page.evaluate(()=>window.__PI04_NATIVE__.state())
   await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,value:false});document.dispatchEvent(new Event('visibilitychange'))})
   await expect.poll(()=>page.evaluate(()=>window.__PI04_NATIVE__.state().activeSockets)).toBe(1)
   report.worldForegrounded=await page.evaluate(()=>window.__PI04_NATIVE__.state())
   assert.equal(report.worldForegrounded.socketPeak,1)
   assert.equal(await canvas.evaluate(node=>node===document.querySelector('.shell-world-host canvas')),true,'Background/foreground retains the same game')
  }catch(error){report.worldFailure={url:page.url(),body:(await page.locator('body').innerText()).slice(0,16000),state:await page.evaluate(()=>window.__PI04_NATIVE__?.state())};throw error}
  finally{await context.close()}
 }
 const pngs=(await readdir(output)).filter(name=>name.endsWith('.png'))
 const bytes=await Promise.all(pngs.map(name=>readFile(resolve(output,name))))
 report.screenshotCount=pngs.length;report.screenshotBytes=bytes.reduce((total,b)=>total+b.length,0)
 assert.ok(report.screenshotCount<=(worldOnly?2:36)&&report.screenshotBytes<=8*1024**2,'Finite screenshot cap exceeded')
 if(affectedOnly){
  let familyCount=0,familyBytes=0
  for(const folder of await readdir(resolve(output,'..'),{withFileTypes:true}))if(folder.isDirectory()&&folder.name.startsWith('native'))for(const name of await readdir(resolve(output,'..',folder.name)))if(name.endsWith('.png')){familyCount++;familyBytes+=(await readFile(resolve(output,'..',folder.name,name))).length}
  report.familyScreenshots={count:familyCount,bytes:familyBytes};assert.ok(familyCount<=(worldOnly?66:64)&&familyBytes<=24*1024**2,'Inclusive family screenshot cap exceeded')
 }
 report.foundationUnchanged=JSON.stringify(before)===JSON.stringify(await pins())
 report.cssReads=[...cssReads.values()]
 assert.equal(report.foundationUnchanged,true,'PI01 changed during the comparison; refresh source pin and rerun affected comparisons')
 assert.deepEqual(errors,[]);report.status='PASS'
}catch(error){report.status='FAIL';report.failure=String(error);throw error}
finally{await browser?.close();await dev.close();await writeFile(resolve(output,'native.json'),JSON.stringify(report,null,2)+'\n')}
