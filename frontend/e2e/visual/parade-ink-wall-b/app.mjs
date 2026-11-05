// PI09 actual default-App comparison, reusing the unchanged qualified QA06 motif fixture.
// Exact PI01 checkpoint presentation is a declared composite, never canonical release proof.
import assert from 'node:assert/strict'
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {readFile,writeFile} from 'node:fs/promises'
import {resolve,extname,dirname} from 'node:path'
import {createRequire} from 'node:module'
import {pathToFileURL} from 'node:url'
import {createServer} from 'vite'
import {sparseSource} from './sparse-source.mjs'
import {transformAdmission} from './admission-fixture/transform.mjs'
import {chromium,expect} from '@playwright/test'
const repo=resolve(import.meta.dirname,'../../../..'),frontend=resolve(repo,'frontend')
const out=process.env.PI09_NATIVE_OUTPUT,temp=process.env.PI09_NATIVE_TEMP
const pin=process.env.PI09_NATIVE_CANDIDATE,job=process.env.PI09_NATIVE_JOB,port=Number(process.env.PI09_NATIVE_PORT)
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:32*1024**2})
const hash=b=>createHash('sha256').update(b).digest('hex')
assert.equal(process.version,'v24.22.2')
assert(job&&/^[a-f0-9]{40}$/.test(pin??'')&&out?.startsWith(repo+'/docs/mcr/evidence/parade-ink-wall-b/'))
assert(temp?.startsWith('/private/tmp/mcr-pi09-native-')&&port>1024)
assert.equal(git('rev-parse','HEAD').toString().trim(),pin)
git('diff','--quiet','HEAD','--','frontend/src','frontend/public','contracts','frontend/package.json','frontend/pnpm-lock.yaml')
const sparseInputs=sparseSource(repo,pin)
const tool=createRequire(resolve(process.env.PI09_TOOLING_ROOT,'package.json'))
const {default:react}=await import(pathToFileURL(tool.resolve('@vitejs/plugin-react')))
const {default:tailwindcss}=await import(pathToFileURL(tool.resolve('@tailwindcss/vite')))
const foundation='056a8c48ff667134dee8e5e99710185294e4b898'
const shellCorrection={pin:'b5b44e47531e3b75febc5ee5cdd6a11b1648779c',path:'frontend/src/app/shell.css',sha256:'b12f4bbddb55d144fb6da89ab2686e0225455ed3bedaff70ad8a34ef6a0161dc'}
const correctionReadset=git('show',shellCorrection.pin+':docs/mcr/evidence/parade-ink-foundation/qualification-readset.json');assert.equal(hash(correctionReadset),'9630cc0b8b487b6a6348612dbadc27be868729f5e5189f61c5a559b31558c2c9')
const foundationReadset=JSON.parse(git('show',foundation+':docs/mcr/evidence/parade-ink-foundation/readset.json'))
const composite=new Map(foundationReadset.implementation.map(row=>{const corrected=row.path===shellCorrection.path,bytes=git('show',(corrected?shellCorrection.pin:foundation)+':'+row.path);assert.equal(hash(bytes),corrected?shellCorrection.sha256:row.sha256);return [row.path,bytes]}))
// Vite's CSS import reader bypasses plugin load hooks; expand exact local
// checkpoint imports in memory, preserving order and all original declarations.
async function pinnedCSS(path,seen=[]){
 assert(!seen.includes(path),'Cyclic checkpoint CSS import')
 const rel=path.slice(repo.length+1)
 let code=(composite.get(rel)??sparseInputs.read(path)??await readFile(path)).toString()
 if(rel.endsWith('/tokens.css'))code=code.replaceAll('./fonts/','/__pi01/fonts/')
 const pattern=/@import\s+(?:url\(\s*)?["']([^"']+)["']\s*\)?\s*;/g
 let result='',cursor=0
 for(const match of code.matchAll(pattern)){
  result+=code.slice(cursor,match.index)
  result+=match[1].startsWith('.')?await pinnedCSS(resolve(dirname(path),match[1]),[...seen,path]):match[0]
  cursor=match.index+match[0].length
 }
 return result+code.slice(cursor)
}
const admissionPin='5ac4bc6e69ee47e6782c2d0b25c72c5544a540d6'
const admissionPins=[["frontend/src/features/auth/model/enrollment-contract.ts", "407dfaa9f512ca35d474b5f4470a864b082bc24e8b8e6b9a968ceefb9cb7b37b"], ["frontend/src/features/auth/__tests__/enrollment-consent/fixtures.ts", "6b6a5b8f757d626d565fd71a36a891bb4b6a15aaaa23d206a69dc89726a9fbd4"], ["frontend/src/features/auth/__tests__/fixtures/data.ts", "00577a6ebb610df3279996d36a88d512d0c4df838a6c11e7877238b1e079986f"], ["frontend/src/app/world-entry/__tests__/map-roof-street-uptake/preview.ts", "cde383b4571b2f22e9f5701288e33f9b6047d326767ce297668406087449103b"]]
for(const [path,sha]of admissionPins){assert.equal(hash(git('show',admissionPin+':'+path)),sha);assert.equal(hash(await readFile(resolve(repo,path))),sha)}
const admissionPath='frontend/src/app/world-entry/mural-production/__tests__/default-entry/preview.ts'
const originalAdmissionFixture=git('show',admissionPin+':'+admissionPath);assert.equal(hash(await readFile(resolve(repo,admissionPath))),hash(originalAdmissionFixture))
const admittedFixture=transformAdmission(originalAdmissionFixture)
const mode=process.env.PI09_NATIVE_MODE??'motif';assert(['motif','commands'].includes(mode))
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
 'Existing Auth/session/provider/decoder/C05/placement/Wall fixture bytes unchanged; ONLY Root-granted exact protected GET admission response added virtually to the existing HTTP fixture, TEST_ONLY not live. No Auth state/direct ADMITTED flag or application/renderer source overlay.',
 'Only new approved-motif state at360/390/430 REVENGE; no unchanged all-screen matrix or other-era claim.',
 'Read-only approved-motif comparison; changed-command cases run separately against the unchanged original fixture, never this snapshot overlay.',
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
async function screenshot(page,name){if(mode==='commands'&&process.env.PI09_REUSE_COMMAND_CAPTURES){const reused=await (await import('./capture-reuse.mjs')).reuseCapture(name,out,process.env.PI09_REUSE_COMMAND_CAPTURES);if(reused)return reused}const bytes=await page.screenshot({path:resolve(out,name),fullPage:false});return {path:name,sha256:hash(bytes),bytes:bytes.length}}
async function geometry(page){return page.evaluate(()=>{
 const panel=document.querySelector('.shell-panel').getBoundingClientRect()
 return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth+1,panel:panel.toJSON(),viewportVariable:parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height'))}
})}
try{
 const input={revision:'TEST_ONLY_MURAL_DEFAULT_APP',reviewedBy:'Synthetic map admission',movementPolicy:{speedPxPerSecond:96,footWidthPx:12,footHeightPx:8,interactionRadiusPx:96},zoom:1}
 server=await createServer({root:frontend,configFile:false,publicDir:false,cacheDir:process.env.PI09_NATIVE_CACHE??resolve(temp,'vite'),logLevel:'error',
 resolve:{alias:Object.keys(JSON.parse(execFileSync('git',['show',pin+':frontend/package.json'],{cwd:repo})).dependencies).map(name=>({find:new RegExp('^'+name+'(?=/|$)'),replacement:resolve(process.env.PI09_TOOLING_ROOT,'node_modules',name)})),dedupe:['react','react-dom']},
 optimizeDeps:{include:['react','react/jsx-runtime','react/jsx-dev-runtime','react-dom/client','phaser']},
 define:{'import.meta.env.VITE_MCR_EVENT_ID':JSON.stringify('00000000-0000-4000-8000-000000000201'),'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG':JSON.stringify(JSON.stringify(input))},
 plugins:[react(),tailwindcss(),{name:'pi09-explicit-pi01-composite',enforce:'pre',
 resolveId(id,importer){
  const abs=id.startsWith('.')&&importer?resolve(dirname(importer.split('?')[0]),id):id.startsWith('/src/')?frontend+id:id.startsWith(frontend+'/')?id:null
  if(abs){for(const candidate of [abs,abs+'.ts',abs+'.tsx',abs+'.css',abs+'/index.tsx'])if(composite.has(candidate.slice(repo.length+1))||sparseInputs.missing(candidate))return candidate}
 },
 async load(id){const rel=id.split('?')[0].slice(repo.length+1);if(rel===admissionPath)return admittedFixture.code;if(composite.has(rel)&&/\.(css|tsx?|json)$/.test(rel)){if(rel.endsWith('.css'))return pinnedCSS(id.split('?')[0]);return composite.get(rel).toString()}if(sparseInputs.missing(id.split('?')[0]))return sparseInputs.read(id.split('?')[0])},
},{name:'qa06-wall-approved-readonly',async transform(_code,id){const path=id.split('?')[0];if(path.startsWith(frontend+'/src/')&&['.ts','.tsx','.css','.json'].includes(extname(path))){const rel=path.slice(repo.length+1);report.sourceReadset[rel]=hash(composite.get(rel)??sparseInputs.read(path)??await readFile(path))}},
 configureServer(s){s.middlewares.use(async(req,res,next)=>{
  const pathname=decodeURIComponent(new URL(req.url,origin).pathname)
  if(pathname.startsWith('/__pi01/fonts/')){const p='frontend/src/shared/styles/parade-ink/fonts/'+pathname.split('/').at(-1),bytes=composite.get(p);assert(bytes);res.setHeader('Content-Type','font/ttf');res.end(bytes);return}
  const p='frontend/public'+pathname
  if(tracked.has(p)){
   try{if(!assetCache.has(p))assetCache.set(p,git('show',pin+':'+p));const bytes=assetCache.get(p);report.assetReadset[p]={sha256:hash(bytes),bytes:bytes.length}
    const mime={'.png':'image/png','.jpg':'image/jpeg','.jpeg':'image/jpeg','.svg':'image/svg+xml','.json':'application/json','.woff2':'font/woff2','.ttf':'font/ttf','.bin':'application/octet-stream'}
    res.setHeader('Content-Type',mime[extname(p)]??'application/octet-stream');res.end(bytes);return
   }catch(e){next(e);return}
  }
  if(!req.headers.accept?.includes('text/html'))return next()
  res.setHeader('Content-Type','text/html');res.end(await s.transformIndexHtml(req.url,await readFile(resolve(frontend,'.'+prefix+'preview.html'),'utf8')))
 })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false,watch:null,fs:{allow:[frontend,process.env.PI09_TOOLING_ROOT]}}})
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
 report.mode=mode;report.renderContent=mode==='motif'?'APPROVED_MOTIF_READ_ONLY_SYNTHETIC_SNAPSHOT':'UNCHANGED_EXISTING_DEFAULT_APP_ISLAND_FIXTURE'
 for(const width of (mode==='motif'?[360,390,430]:[390])){
  const c={width,era:'REVENGE',height:844,status:'IN_PROGRESS',errors:[]},context=await browser.newContext({viewport:{width,height:844},hasTouch:true,locale:'en-GB',timezoneId:'Asia/Seoul',serviceWorkers:'block'}),page=await context.newPage()
  page.setDefaultTimeout(15000);page.on('pageerror',e=>c.errors.push(e.message))
  // Install a boundary around the existing fixture fetch assignment. Its captured
  // native fallback remains native; all Auth/C05/tools/deadlines pass through unchanged.
  if(mode==='motif')await page.addInitScript(({palette,pixels})=>{
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
   await expect(page.getByLabel('Email',{exact:true})).toBeVisible()
   assert.equal((await state(page)).active,0,'Unadmitted fixture has no World connection')
   c.admissionNegative=await page.evaluate(async()=>{
    const cases=[{name:'missing',method:'GET',headers:{}},{name:'wrong',method:'GET',headers:{Authorization:'Bearer TEST_ONLY_WRONG'}},{name:'method',method:'POST',headers:{}}]
    const results=[];for(const test of cases){const response=await fetch('/api/auth/enrollment/admission',{method:test.method,headers:test.headers});results.push({name:test.name,status:response.status,body:await response.text()})}return results
   })
   assert.deepEqual(c.admissionNegative.map(row=>row.status),[401,401,405]);assert(c.admissionNegative.every(row=>!row.body.includes('ADMITTED')))
   assert.equal((await state(page)).active,0,'Negative reads cannot admit the real App')
   await page.getByLabel('Email',{exact:true}).fill('wall-fixture@example.test')
   await page.getByLabel('Password',{exact:true}).fill('fixture-password')
   await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
   await expect.poll(async()=>(await state(page)).active,{timeout:30000}).toBe(1)
   await expect(page.getByRole('button',{name:'Revenge',exact:true})).toHaveAttribute('aria-pressed','true')
   await expect.poll(()=>page.evaluate(async()=>(await window.__MURAL_ROOT_ENTRY__.worldProof()).mural?.objects??0),{timeout:25000}).toBe(1)
   await page.evaluate(()=>window.__QA06_WORLD_CANVAS__=document.querySelector('.shell-world-host canvas'))
   c.admissionAudit=await page.evaluate(()=>structuredClone(window.__PI09_ADMISSION_AUDIT__))
   const admittedReads=c.admissionAudit.filter(row=>row.status===200)
   assert(admittedReads.length>0&&admittedReads.every(row=>row.method==='GET'&&row.protected&&row.response.decision==='ADMITTED'&&row.response.policyRevision===row.response.acceptedRevision))
   const originalRequests=(await state(page)).requests,loginIndex=originalRequests.findIndex(row=>row.path==='/api/auth/login'),bootstrapIndex=originalRequests.findIndex(row=>row.path.endsWith('/bootstrap'))
   assert(loginIndex>=0&&admittedReads.every(row=>row.atOriginalRequestIndex>loginIndex)&&bootstrapIndex>=admittedReads[0].atOriginalRequestIndex,'Canonical login then protected admission before bootstrap')
   c.initialMutations=(await state(page)).requests.filter(r=>r.method!=='GET').length
   await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Wall',exact:true}).click()
   const editor=page.getByRole('region',{name:'Wall editor'});await expect(editor).toBeVisible()
   await expect(editor.getByRole('button',{name:'Pencil preview',exact:true})).toBeEnabled()
   await expect(page.locator('.shell-stage')).toHaveAttribute('data-panel-size','full')
   await page.evaluate(()=>document.fonts.ready)
   await editor.getByRole('button',{name:'Fit wall',exact:true}).click()
   await expect(page.getByLabel('Zoom level',{exact:true})).toHaveText('1×')
   if(mode==='motif'){await expect.poll(()=>sampleEditor(page)).toBe(report.approvedPack.expected128RGBA);c.editor128RGBA=await sampleEditor(page);c.audit=await page.evaluate(()=>window.__QA06_APPROVED_MOTIF__);assert(c.audit.snapshotReads>0)}
   c.geometry=await geometry(page);assert(!c.geometry.overflow&&c.geometry.panel.y>=-1&&c.geometry.panel.bottom<=845)
   await page.locator('.shell-panel-body').evaluate(e=>e.scrollTop=0)
   c.screenshot=await screenshot(page,'wall-parade-ink-'+width+'-844.png')
   c.workbench=await editor.locator('.mcr-mural__workbench').evaluate(e=>{const r=e.getBoundingClientRect(),surface=e.querySelector('.mcr-mural__surface').getBoundingClientRect(),palette=e.querySelector('.mcr-mural__palette-frame').getBoundingClientRect();return {bounds:r.toJSON(),surface:surface.toJSON(),palette:palette.toJSON()}})
   assert(Math.abs(c.workbench.surface.width-c.workbench.surface.height)<1,'Square canvas')
   assert(c.workbench.palette.x>c.workbench.surface.right,'Palette to right of canvas')
   c.controls=[]
   for(const label of ['Pencil preview','Fill preview','Zoom in','Pan wall','Fit wall'])c.controls.push({label,rect:await reachable(editor.getByRole('button',{name:label,exact:true}))})
   const colors=editor.getByRole('button',{name:/^Color [0-9]+ #/});await expect(colors).toHaveCount(16)
   c.paletteFirst=await reachable(colors.first());c.paletteLast=await reachable(colors.last())
   const paletteGroup=editor.getByRole('group',{name:'Wall palette',exact:true});await paletteGroup.focus();await page.keyboard.press('End')
   await expect.poll(()=>paletteGroup.evaluate(e=>Math.abs(e.scrollWidth-e.clientWidth-e.scrollLeft))).toBeLessThanOrEqual(1)
   await page.keyboard.press('Home');await expect.poll(()=>paletteGroup.evaluate(e=>e.scrollLeft)).toBe(0)
   await colors.last().focus();await expect(colors.last()).toBeFocused()
   c.focusDiagnostic=await page.evaluate(()=>({tag:document.activeElement?.tagName,label:document.activeElement?.getAttribute('aria-label')}))
   if(width===390){
    await page.setViewportSize({width,height:420});await expect.poll(()=>page.evaluate(()=>parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height')))).toBe(420)
    c.reduced=await geometry(page);assert(!c.reduced.overflow&&c.reduced.panel.bottom<=421)
    c.reducedFitTarget=await reachable(editor.getByRole('button',{name:'Fit wall',exact:true}))
    c.reducedScreenshot=await screenshot(page,'wall-approved-motif-390-420.png')
    await page.setViewportSize({width,height:844});await expect.poll(()=>page.evaluate(()=>parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height')))).toBe(844)
   }
   c.sameCanvas=await page.evaluate(()=>window.__QA06_WORLD_CANVAS__===document.querySelector('.shell-world-host canvas'));assert(c.sameCanvas)
   if(mode==='motif'&&width===390){
    await colors.nth(2).click();await expect(colors.nth(2)).toHaveAttribute('aria-pressed','true');const surface=editor.getByRole('group',{name:'Mural pixel preview'}),rect=await surface.boundingBox();await surface.click({position:{x:rect.width*20.5/128,y:rect.height*20.5/128}});await expect(editor.getByLabel('Selection preview')).toContainText('Pixel (20, 20)');c.pointerProof={x:20,y:20,rect};await page.keyboard.press('Escape');await surface.focus();await page.keyboard.press('ArrowRight')
    await expect(editor.getByRole('button',{name:'Confirm pixel',exact:true})).toBeEnabled()
    c.previewScreenshot=await screenshot(page,'wall-parade-ink-390-preview.png');await page.keyboard.press('Escape')
   }
   if(mode==='commands'){
    const api=()=>page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.state())
    const coordinates=async()=>{const details=editor.locator('.mcr-mural__details');if(!await details.evaluate(e=>e.open))await editor.getByText('Coordinates & help',{exact:true}).click();await editor.getByLabel('X',{exact:true}).fill('10');await editor.getByLabel('Y',{exact:true}).fill('10');await editor.getByRole('button',{name:'Preview pixel',exact:true}).click()}
    await colors.nth(2).click();await editor.getByRole('button',{name:'Fill preview',exact:true}).click();await coordinates()
    await expect(editor.getByLabel('Selection preview')).toContainText('16 changed cells')
    c.commandPreview=await screenshot(page,'wall-parade-ink-390-fill-preview.png')
    const before=(await api()).revision
    await editor.getByRole('button',{name:'Confirm fill',exact:true}).click();await expect.poll(async()=>(await api()).revision).toBe(before+1)
    await expect(editor.getByRole('button',{name:'Preview undo of my last action',exact:true})).toBeEnabled()
    await editor.getByRole('button',{name:'Preview undo of my last action',exact:true}).click();await editor.getByRole('button',{name:'Confirm undo',exact:true}).click();await expect.poll(async()=>(await api()).revision).toBe(before+2)
    await editor.getByRole('button',{name:'Erase preview',exact:true}).click();await coordinates();await editor.getByRole('button',{name:'Confirm erase',exact:true}).click();await expect.poll(async()=>(await api()).revision).toBe(before+3)
    await page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.loseNext());await editor.getByRole('button',{name:'Pencil preview',exact:true}).click();await coordinates()
    const beforeUnknown=(await api()).requests.filter(r=>r.method==='POST').length
    await editor.getByRole('button',{name:'Confirm pixel',exact:true}).click();const check=editor.getByRole('button',{name:'Check original command',exact:true});await expect(check).toBeVisible()
    c.unknownScreenshot=await screenshot(page,'wall-parade-ink-390-unknown.png');await check.click()
    await expect(check).toHaveCount(0);assert.equal((await api()).requests.filter(r=>r.method==='POST').length,beforeUnknown+1)
    await (await import('./sync-ready.mjs')).denyToolsAndRefresh(page,editor,expect)
    await expect(page.getByText('Wall unavailable. Reconnect to a qualified world scene before opening the current wall.',{exact:true})).toBeVisible()
    await expect(editor).toHaveCount(0);assert.equal((await page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.worldProof())).mural,null)
    c.unavailableScreenshot=await screenshot(page,'wall-parade-ink-390-unavailable.png')
    await (await import('./sync-ready.mjs')).restoreTools(page,editor,expect)
    c.commands={status:'PASS_EXISTING_SYNTHETIC_FIXTURE_NOT_LIVE',final:await api(),explicitFillUndoErase:true,originalUnknownCheckedWithoutReplay:true,unavailableFailClosed:true}
   }
   c.state=await state(page);assert.equal(c.state.peak,1);if(mode==='motif')assert.equal(c.state.requests.filter(r=>r.method!=='GET').length,c.initialMutations)
   c.worldProof=await page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.worldProof());assert(c.worldProof.mural.objects===1&&c.worldProof.games===1&&c.worldProof.canvases===1)
   if(mode==='commands'){
    const writesBefore=(await state(page)).requests.filter(row=>row.method==='POST').length
    await page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.close());await expect.poll(async()=>(await state(page)).active).toBe(0)
    c.syncDegraded=await page.evaluate(()=>({phase:document.querySelector('.mcr-mural__sync-state')?.getAttribute('data-phase')??null,editorPresent:!!document.querySelector('.mcr-mural'),writeButtons:[...document.querySelectorAll('.mcr-mural button')].filter(e=>['Pencil preview','Erase preview','Fill preview'].includes(e.getAttribute('aria-label'))).map(e=>({label:e.getAttribute('aria-label'),disabled:e.disabled}))}))
    assert(c.syncDegraded.phase===null||c.syncDegraded.phase!=='ready');assert(!c.syncDegraded.editorPresent||(c.syncDegraded.writeButtons.length===3&&c.syncDegraded.writeButtons.every(button=>button.disabled)))
    assert.equal((await state(page)).requests.filter(row=>row.method==='POST').length,writesBefore)
    c.syncDegradedScreenshot=await screenshot(page,'wall-parade-ink-390-sync-degraded.png')
    await (await import('./sync-recover.mjs')).reconnectAfterDegraded(page,editor,expect)
    c.syncRecovered={state:await state(page),world:await page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.worldProof())};assert.equal(c.syncRecovered.state.peak,1);assert.equal(c.syncRecovered.state.requests.filter(row=>row.method==='POST'&&row.path.includes('/mural/')).length,c.state.requests.filter(row=>row.method==='POST'&&row.path.includes('/mural/')).length);assert.equal(c.syncRecovered.world.mural?.objects,1)
   }
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
 for(const [p,sha]of Object.entries(report.sourceReadset))assert.equal(hash(composite.get(p)??sparseInputs.read(resolve(repo,p))??await readFile(resolve(repo,p))),sha,'Pinned source '+p)
 report.admissionFixture={source:admissionPin,pins:admissionPins,originalPath:admissionPath,...admittedFixture.readset,wrapperSha256:hash(await readFile(new URL('./admission-fixture/transform.mjs',import.meta.url)))}
 report.sparseInputs=sparseInputs.readset
 report.foundation={pin:foundation,shellCorrection,correctionReadsetSha256:hash(correctionReadset),correction:'ONLY exact shell.css full-height correction from b5b44; other17postimages read from original056, no wholebranch import',status:'EXPLICIT_UNQUALIFIED_PI01_COMPOSITE_NOT_CANONICAL_RELEASE',implementation:foundationReadset.implementation.map(row=>row.path===shellCorrection.path?{...row,sha256:shellCorrection.sha256,source:shellCorrection.pin}:{...row,source:foundation}),cssImportAdaptation:'Exact local checkpoint CSS imports expanded in memory in original order because Vite CSS reader bypasses load hooks; no source write',fontURLAdaptation:'tokens.css ./fonts/ -> /__pi01/fonts/; exact font bytes served read-only from checkpoint'}
 report.status=report.cases.length===(mode==='motif'?3:1)&&report.cases.every(c=>c.status==='PASS')?'MOCK_READY':'INCOMPLETE_OR_FAILED'
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
