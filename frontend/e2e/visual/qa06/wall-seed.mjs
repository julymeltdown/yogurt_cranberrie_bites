// Original QA06: only the uncovered approved Wall motif state.
// Existing actual-main/Auth/C05 fixture stays unchanged; a synthetic C04 read snapshot
// supplies the exact approved pixels/palette. No app/fixture/renderer/source overlay.
import assert from 'node:assert/strict'
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {readFile,writeFile} from 'node:fs/promises'
import {resolve,extname} from 'node:path'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const repo=resolve(import.meta.dirname,'../../../..'),frontend=resolve(repo,'frontend')
const out=process.env.QA06_WALL_SEED_OUTPUT,temp=process.env.QA06_WALL_SEED_TEMP
const pin=process.env.QA06_WALL_SEED_CANDIDATE,job=process.env.QA06_WALL_SEED_JOB,port=Number(process.env.QA06_WALL_SEED_PORT)
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:32*1024**2})
const hash=b=>createHash('sha256').update(b).digest('hex')
assert.equal(process.version,'v24.22.2')
assert(job&&/^[a-f0-9]{40}$/.test(pin??'')&&out?.startsWith(repo+'/docs/mcr/evidence/visual/qa06/wall-approved-motif/'))
assert(temp?.startsWith('/private/tmp/mcr-qa06-wall-seed-')&&port>1024)
assert.equal(git('rev-parse','HEAD').toString().trim(),pin)
git('diff','--quiet','HEAD','--','frontend/src','frontend/public','contracts','frontend/package.json','frontend/pnpm-lock.yaml')
const origin='http://127.0.0.1:'+port,prefix='/src/app/world-entry/mural-production/__tests__/default-entry/'
const art='frontend/public/wall/initial-motif-v1/',manifestBytes=git('show',pin+':'+art+'manifest.json')
const manifest=JSON.parse(manifestBytes),paletteBytes=git('show',pin+':'+art+manifest.files.palette.path),pixelsBytes=git('show',pin+':'+art+manifest.files.pixels.path)
assert.equal(hash(manifestBytes),'3f1aa318f0e4b7d8fa7f079b1101f7386619291a8dbb3bb04ef70820952f34ce')
assert.equal(hash(paletteBytes),manifest.files.palette.sha256);assert.equal(hash(pixelsBytes),manifest.files.pixels.sha256)
const palette=JSON.parse(paletteBytes),pixels=Array.from(pixelsBytes)
assert.equal(manifest.width,128);assert.equal(manifest.height,128);assert.equal(palette.length,16);assert.equal(pixels.length,16384)
assert(palette.every(c=>/^#[0-9A-F]{6}$/.test(c))&&pixels.every(c=>c>=0&&c<16))
const expectedRGBA=Buffer.from(pixels.flatMap(c=>[parseInt(palette[c].slice(1,3),16),parseInt(palette[c].slice(3,5),16),parseInt(palette[c].slice(5,7),16),255]))
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n')),assetCache=new Map()
const report={task:'QA06',case:'QA06-WALL-APPROVED-MOTIF',job,candidate:pin,status:'IN_PROGRESS',startedAt:new Date().toISOString(),
 qualification:'ACTUAL_MAIN_CANONICAL_UI_LOGIN_EXISTING_HTTP_C05_FIXTURE_APPROVED_ART_AS_SYNTHETIC_C04_SNAPSHOT_NOT_LIVE',
 physicalIOS:'NOT_RUN',physicalAndroid:'NOT_RUN',softwareKeyboard:'NOT_RUN',hardwareSafeArea:'NOT_RUN',otherEras:'NOT_RUN',
 runnerSha256:hash(await readFile(new URL(import.meta.url))),sourceReadset:{},assetReadset:{},cases:[],errors:[],
 approvedPack:{path:art,manifestSha256:hash(manifestBytes),pixelsSha256:hash(pixelsBytes),paletteSha256:hash(paletteBytes),expected128RGBA:hash(expectedRGBA)},
 unchangedFixture:{path:prefix+'preview.ts',sha256:hash(await readFile(resolve(frontend,'.'+prefix+'preview.ts')))},
 limits:['Exact published art as explicitly synthetic server read data, not backend provisioning/real authority/live snapshot.',
 'Existing actual-main/Auth/C05/placement fixture unchanged; no Auth/provider/admission override or application/renderer source overlay.',
 'Only new approved-motif state at360/390/430 REVENGE; no unchanged all-screen matrix or other-era claim.',
 'No paint/fill/erase/undo/submit/save/upload. Toolbar/palette/fit and DOM focus/scroll only.',
 'Reduced420 viewport is not a physical mobile keyboard or hardware safe-area check.',
 'Whole original QA06, full61IA/live/full-reference/device acceptance remain open.']}
let server,browser
const state=p=>p.evaluate(()=>window.__MURAL_ROOT_ENTRY__.state())
async function reachable(locator){
 await locator.scrollIntoViewIfNeeded();const r=await locator.boundingBox();assert(r&&r.width>=43.5&&r.height>=43.5,'44px target')
 assert(await locator.evaluate(e=>{const r=e.getBoundingClientRect(),hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return hit===e||e.contains(hit)}),'Target center reachable')
 return r
}
async function sampleEditor(page){
 const bytes=await page.locator('.mcr-mural__surface canvas').evaluate(canvas=>{
  const ctx=canvas.getContext('2d'),rgba=ctx.getImageData(0,0,canvas.width,canvas.height).data,result=[]
  const offset=(canvas.height-512)/2
  for(let y=0;y<128;y++)for(let x=0;x<128;x++){
   const index=(Math.floor(offset+y*4+2)*canvas.width+x*4+2)*4
   result.push(...rgba.slice(index,index+4))
  }
  return result
 })
 return hash(Buffer.from(bytes))
}
async function screenshot(page,name){const bytes=await page.screenshot({path:resolve(out,name),fullPage:false});return {path:name,sha256:hash(bytes),bytes:bytes.length}}
async function geometry(page){return page.evaluate(()=>{
 const panel=document.querySelector('.shell-panel').getBoundingClientRect()
 return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth+1,panel:panel.toJSON(),viewportVariable:parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height'))}
})}
try{
 const input={revision:'TEST_ONLY_MURAL_DEFAULT_APP',reviewedBy:'Synthetic map admission',movementPolicy:{speedPxPerSecond:96,footWidthPx:12,footHeightPx:8,interactionRadiusPx:96},zoom:1}
 server=await createServer({root:frontend,configFile:resolve(frontend,'vite.config.ts'),configLoader:'runner',cacheDir:resolve(temp,'vite'),logLevel:'error',
 define:{'import.meta.env.VITE_MCR_EVENT_ID':JSON.stringify('00000000-0000-4000-8000-000000000201'),'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG':JSON.stringify(JSON.stringify(input))},
 plugins:[{name:'qa06-wall-approved-readonly',async transform(_code,id){const path=id.split('?')[0];if(path.startsWith(frontend+'/src/')&&['.ts','.tsx','.css','.json'].includes(extname(path)))report.sourceReadset[path.slice(repo.length+1)]=hash(await readFile(path))},
 configureServer(s){s.middlewares.use(async(req,res,next)=>{
  const p='frontend/public'+decodeURIComponent(new URL(req.url,origin).pathname)
  if(tracked.has(p)){
   try{if(!assetCache.has(p))assetCache.set(p,git('show',pin+':'+p));const bytes=assetCache.get(p);report.assetReadset[p]={sha256:hash(bytes),bytes:bytes.length}
    const mime={'.png':'image/png','.jpg':'image/jpeg','.jpeg':'image/jpeg','.svg':'image/svg+xml','.json':'application/json','.woff2':'font/woff2','.ttf':'font/ttf','.bin':'application/octet-stream'}
    res.setHeader('Content-Type',mime[extname(p)]??'application/octet-stream');res.end(bytes);return
   }catch(e){next(e);return}
  }
  if(!req.headers.accept?.includes('text/html'))return next()
  res.setHeader('Content-Type','text/html');res.end(await s.transformIndexHtml(req.url,await readFile(resolve(frontend,'.'+prefix+'preview.html'),'utf8')))
 })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false,watch:null}})
 // Reuse the exact placement for this unchanged incumbent fixture; avoid a second
 // SSR module runner solely to recompute its already-qualified map admission tuple.
 const placementReceiptPath='docs/mcr/evidence/mural-production-binding/backoff-native-01/run/capture/report.json'
 const placementReceiptBytes=git('show',pin+':'+placementReceiptPath)
 const fixturePlacement=JSON.parse(placementReceiptBytes).fixturePlacement
 assert(fixturePlacement?.map?.contentHash&&fixturePlacement?.map?.geometryHash)
 report.existingPlacementReceipt={path:placementReceiptPath,sha256:hash(placementReceiptBytes)}
 server.config.define['import.meta.env.VITE_MCR_MURAL_CONFIG']=JSON.stringify(JSON.stringify(fixturePlacement))
 report.fixturePlacement=fixturePlacement;await server.listen()
 browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const width of [360,390,430]){
  const c={width,era:'REVENGE',height:844,status:'IN_PROGRESS',errors:[]},context=await browser.newContext({viewport:{width,height:844},hasTouch:true,locale:'en-GB',timezoneId:'Asia/Seoul',serviceWorkers:'block'}),page=await context.newPage()
  page.setDefaultTimeout(15000);page.on('pageerror',e=>c.errors.push(e.message))
  // Install a boundary around the existing fixture fetch assignment. Its captured
  // native fallback remains native; all Auth/C05/tools/deadlines pass through unchanged.
  await page.addInitScript(({palette,pixels})=>{
   let current=window.fetch,installed=false
   const audit={snapshotReads:0,classification:'TEST_ONLY_SYNTHETIC_C04_APPROVED_ART_READ_ONLY',mutatedSnapshotFields:['palette','pixels']}
   Object.defineProperty(window,'fetch',{configurable:true,get:()=>current,set(value){
    installed=true;current=async(input,init)=>{
     const response=await value(input,init),request=input instanceof Request?input:new Request(new URL(String(input),location.origin),init)
     if(installed&&request.method==='GET'&&new URL(request.url).pathname.endsWith('/mural/snapshot')&&response.ok){
      const body=await response.json();if(body.width!==128||body.height!==128||body.pixels?.length!==16384)throw Error('Unexpected existing C04 snapshot shape')
      audit.snapshotReads++;return Response.json({...body,palette:[...palette],pixels:[...pixels]},{status:response.status})
     }
     return response
    }
   }})
   Object.assign(window,{__QA06_APPROVED_MOTIF__:audit})
  },{palette,pixels})
  try{
   await page.goto(origin+'/login')
   await page.getByLabel('Email',{exact:true}).fill('wall-fixture@example.test')
   await page.getByLabel('Password',{exact:true}).fill('fixture-password')
   await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
   await expect.poll(async()=>(await state(page)).active,{timeout:30000}).toBe(1)
   await expect(page.getByRole('button',{name:'Revenge',exact:true})).toHaveAttribute('aria-pressed','true')
   await expect.poll(()=>page.evaluate(async()=>(await window.__MURAL_ROOT_ENTRY__.worldProof()).mural?.objects??0),{timeout:25000}).toBe(1)
   await page.evaluate(()=>window.__QA06_WORLD_CANVAS__=document.querySelector('.shell-world-host canvas'))
   c.initialMutations=(await state(page)).requests.filter(r=>r.method!=='GET').length
   await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Wall',exact:true}).click()
   const editor=page.getByRole('region',{name:'Wall editor'});await expect(editor).toBeVisible()
   await expect(editor.getByRole('button',{name:'Pencil preview',exact:true})).toBeEnabled()
   await expect(page.locator('.shell-stage')).toHaveAttribute('data-panel-size','full')
   const icons=page.locator('img.mcr-reference-window-ink');await expect(icons).toHaveCount(3)
   await expect.poll(()=>icons.evaluateAll(nodes=>nodes.every(n=>n.complete&&n.naturalWidth>0))).toBe(true)
   await page.evaluate(()=>document.fonts.ready)
   await editor.getByRole('button',{name:'Fit wall',exact:true}).click()
   await expect(page.getByLabel('Zoom level',{exact:true})).toHaveText('1×')
   await expect.poll(()=>sampleEditor(page)).toBe(report.approvedPack.expected128RGBA)
   c.editor128RGBA=await sampleEditor(page);c.audit=await page.evaluate(()=>window.__QA06_APPROVED_MOTIF__);assert(c.audit.snapshotReads>0)
   c.geometry=await geometry(page);assert(!c.geometry.overflow&&c.geometry.panel.y>=-1&&c.geometry.panel.bottom<=845)
   await page.locator('.shell-panel-body').evaluate(e=>e.scrollTop=0)
   c.screenshot=await screenshot(page,'wall-approved-motif-'+width+'-844.png')
   c.controls=[]
   for(const label of ['Pencil preview','Fill preview','Zoom in','Pan wall','Fit wall'])c.controls.push({label,rect:await reachable(editor.getByRole('button',{name:label,exact:true}))})
   const colors=editor.getByRole('button',{name:/^Color [0-9]+ #/});await expect(colors).toHaveCount(16)
   c.paletteFirst=await reachable(colors.first());c.paletteLast=await reachable(colors.last())
   await colors.last().focus();await page.keyboard.press('Home')
   c.focusDiagnostic=await page.evaluate(()=>({tag:document.activeElement?.tagName,label:document.activeElement?.getAttribute('aria-label')}))
   if(width===390){
    await page.setViewportSize({width,height:420});await expect.poll(()=>page.evaluate(()=>parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height')))).toBe(420)
    c.reduced=await geometry(page);assert(!c.reduced.overflow&&c.reduced.panel.bottom<=421)
    c.reducedFitTarget=await reachable(editor.getByRole('button',{name:'Fit wall',exact:true}))
    c.reducedScreenshot=await screenshot(page,'wall-approved-motif-390-420.png')
    await page.setViewportSize({width,height:844});await expect.poll(()=>page.evaluate(()=>parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height')))).toBe(844)
   }
   c.sameCanvas=await page.evaluate(()=>window.__QA06_WORLD_CANVAS__===document.querySelector('.shell-world-host canvas'));assert(c.sameCanvas)
   c.state=await state(page);assert.equal(c.state.peak,1);assert.equal(c.state.requests.filter(r=>r.method!=='GET').length,c.initialMutations)
   c.worldProof=await page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.worldProof());assert(c.worldProof.mural.objects===1&&c.worldProof.games===1&&c.worldProof.canvases===1)
   await page.getByRole('button',{name:'Back to previous panel',exact:true}).click()
   await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Me',exact:true}).click()
   await page.getByText('Profile options',{exact:true}).click();await page.getByRole('link',{name:'Account',exact:true}).click()
   await page.getByRole('button',{name:'Sign out',exact:true}).click();await page.getByRole('button',{name:'Confirm sign out',exact:true}).click()
   await expect.poll(async()=>(await state(page)).active).toBe(0);await expect.poll(async()=>(await state(page)).blobURLs).toBe(0)
   assert.equal(await page.locator('.shell-world-host canvas').count(),0);c.cleanup=await state(page);assert.deepEqual(c.errors,[]);c.status='PASS'
  }catch(e){c.status='FAIL';c.error=e.message;report.errors.push({width,error:e.message});c.renderedText=await page.locator('body').innerText().catch(()=>null)}
  finally{await context.close();report.cases.push(c);await writeFile(resolve(out,'report.partial.json'),JSON.stringify(report,null,2)+'\n')}
  if(c.status!=='PASS')break
 }
 for(const [p,sha]of Object.entries(report.sourceReadset))assert.equal(hash(await readFile(resolve(repo,p))),sha,'Unchanged source '+p)
 report.status=report.cases.length===3&&report.cases.every(c=>c.status==='PASS')?'MOCK_READY':'INCOMPLETE_OR_FAILED'
}catch(e){report.status='FAIL';report.errors.push({error:e.message})}
finally{
 await browser?.close();server?.httpServer?.closeAllConnections()
 const closeStart=Date.now();let closeTimer
 const closed=await Promise.race([server?.close().then(()=>true),new Promise(resolve=>{closeTimer=setTimeout(()=>resolve(false),5000)})]);clearTimeout(closeTimer)
 report.cleanup={browserDisconnected:!browser?.isConnected(),serverCloseResolved:closed,serverCloseMs:Date.now()-closeStart}
 if(!closed){report.status='RUNTIME_PASSED_SHUTDOWN_INCOMPLETE';report.errors.push({error:'Owned Vite shutdown did not resolve in5s'})}
 assetCache.clear();report.finishedAt=new Date().toISOString();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n')
 if(!closed)process.exit(2)
}
console.log(JSON.stringify({status:report.status,cases:report.cases.length,errors:report.errors}));if(report.status!=='MOCK_READY')process.exitCode=1
