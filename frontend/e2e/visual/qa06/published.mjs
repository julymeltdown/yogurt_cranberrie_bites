// Frozen published blobs served read-only; no checkout, application replacement or peer writes.
import assert from 'node:assert/strict'
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {readFile,writeFile,mkdir,readdir,stat} from 'node:fs/promises'
import {resolve,posix,extname} from 'node:path'
import {createRequire} from 'node:module'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
import {requireCaptureSpace,reservePort,temporaryCache,clearTemporaryCache} from './resources.mjs'
const repo=resolve(import.meta.dirname,'../../../..'),pin=process.env.QA06_PUBLISHED_PIN,out=process.env.QA06_PUBLISHED_OUTPUT,scope=process.env.QA06_PUBLISHED_SCOPE??'repairs'
const startAt=Number(process.env.QA06_PUBLISHED_START??0)
assert(/^[a-f0-9]{40}$/.test(pin??'')&&out?.startsWith(resolve(repo,'docs/mcr/evidence/visual/qa06/')+'/'))
assert(['repairs','scene'].includes(scope))
assert(Number.isInteger(startAt)&&startAt>=0&&startAt<27)
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:64*1024*1024}),hash=b=>createHash('sha256').update(b).digest('hex')
git('merge-base','--is-ancestor',pin,'origin/master')
await requireCaptureSpace(repo);await mkdir(out,{mode:0o700})
const tracked=new Set(git('ls-tree','-r','--name-only',pin).toString().trim().split('\n')),cache=new Map(),prefix='/__qa06_frozen__/',fixture='frontend/e2e/visual/qa06/preview.tsx'
const ownFixture=await readFile(resolve(repo,fixture),'utf8'),sourceHashes={}
const blob=p=>{assert(tracked.has(p),p);if(!cache.has(p))cache.set(p,git('show',pin+':'+p));sourceHashes[p]=hash(cache.get(p));return cache.get(p)}
const require=createRequire('/Users/lhs/Desktop/mcr/frontend/package.json'),packages=require('./package.json')
const aliases=Object.keys(packages.dependencies).filter(n=>!n.startsWith('@types/')).map(find=>({find,replacement:find==='phaser'?resolve(require.resolve('phaser/package.json'),'../dist/phaser.js'):require.resolve(find)}))
for(const find of ['react/jsx-runtime','react/jsx-dev-runtime','react-dom/client'])aliases.push({find,replacement:require.resolve(find)})
aliases.sort((a,b)=>b.find.length-a.find.length)
const cacheDir=temporaryCache('published'),port=await reservePort(),errors=[]
const report={task:'QA06',scope,startAt,status:'IN_PROGRESS',candidate:pin,originMaster:git('rev-parse','origin/master').toString().trim(),qualification:'ACTUAL_PUBLISHED_SOURCE_WITH_CORRECTED_SYNTHETIC_HTTP_C05_DESKTOP_ONLY',harnessSha256:hash(await readFile(new URL(import.meta.url))),fixtureSha256:hash(ownFixture),sourceHashes,cases:[],errors,physicalIOS:'NOT_RUN',physicalAndroid:'NOT_RUN'}
// Four full-viewport PNGs plus two JSON receipts fit within this conservative reserve.
async function beforeCase(){await requireCaptureSpace(repo);const files=await readdir(out,{withFileTypes:true});const sizes=await Promise.all(files.filter(f=>f.isFile()).map(async f=>(await stat(resolve(out,f.name))).size));assert(sizes.reduce((a,b)=>a+b,0)+16*1024**2<=.2*1024**3,'0.2GiB output ceiling with next-case reserve')}
const server=await createServer({configFile:false,root:resolve(repo,'frontend'),cacheDir,logLevel:'error',esbuild:{jsx:'automatic'},resolve:{alias:aliases},server:{host:'127.0.0.1',port,strictPort:true,hmr:false,fs:{allow:[repo,'/Users/lhs/Desktop/mcr/frontend']}},plugins:[{name:'qa06-frozen-published-source',enforce:'pre',resolveId(source,importer){
 if(source.startsWith(prefix))return source
 if(source.startsWith('.')&&importer?.startsWith(prefix)){
  const p=posix.normalize(posix.join(posix.dirname(importer.slice(prefix.length)),source))
  for(const suffix of ['','.ts','.tsx','.mjs','.js','.json','/index.ts','/index.tsx'])if(tracked.has(p+suffix))return prefix+p+suffix
  throw Error('Unresolved frozen relative import '+p)
 }
},load(id){if(id.startsWith(prefix)){const p=id.slice(prefix.length).split('?')[0];return p===fixture?ownFixture:blob(p).toString()}},configureServer(s){s.middlewares.use((req,res,next)=>{
 const path=new URL(req.url,'http://localhost').pathname
 if(['/__qa06/start.html','/__qa06/scene.html'].includes(path)){const entry=path.endsWith('scene.html')?'frontend/src/app/world-entry/__tests__/reference-scene-v5/preview.tsx':fixture;res.setHeader('Content-Type','text/html');res.end('<!doctype html><html lang="en"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover"><div id="root"></div><script type="module" src="'+prefix+entry+'"></script></html>');return}
 const p='frontend/public'+decodeURIComponent(path)
 if(!tracked.has(p))return next()
 res.setHeader('Content-Type',({'.png':'image/png','.ttf':'font/ttf','.json':'application/json','.svg':'image/svg+xml'})[extname(p)]??'application/octet-stream');res.end(blob(p))
})}}]})
let browser
try{await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 if(scope==='scene'){
  const labels=JSON.parse(blob('frontend/src/world/reference-scene/spatial-v5/map-labels-source.json')).labels
  sceneCases: for(const width of [360,390,430])for(const era of ['REVENGE','PARADE','DANGER']){
   await beforeCase();const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,isMobile:true,reducedMotion:'reduce'}),page=await context.newPage(),pageErrors=[];page.on('pageerror',e=>pageErrors.push(e.message));page.setDefaultTimeout(20000)
   const c={width,era,status:'IN_PROGRESS',errors:pageErrors}
   try{await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'));await page.goto(`http://127.0.0.1:${port}/__qa06/scene.html?era=${era}`);await expect.poll(()=>page.evaluate(()=>window.__SCENE_V5_ROOT__?.state().overview?.themeId)).toBe(era);await page.waitForLoadState('networkidle')
    c.state=await page.evaluate(()=>window.__SCENE_V5_ROOT__.state());c.world=await page.evaluate(()=>window.__SCENE_V5_ROOT__.inspectWorld());assert.equal(c.world.sprites,6);assert.equal(c.world.games,1);assert.equal(c.state.map.revision,7005);assert.equal(c.state.originalMapProvenance.review_state,'UNREVIEWED');assert.equal(c.state.productionAdmitted,false)
    c.worldScreenshot=`world-${era.toLowerCase()}-${width}.png`;await page.screenshot({path:resolve(out,c.worldScreenshot)})
    if(width===390){await page.setViewportSize({width,height:554});await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));c.worldReferenceScreenshot=`world-${era.toLowerCase()}-${width}-h554.png`;await page.screenshot({path:resolve(out,c.worldReferenceScreenshot)});await page.setViewportSize({width,height:844})}
    await page.evaluate(()=>window.__SCENE_V5_ROOT__.navigate('/world/map'));await expect(page.getByAltText(era+' illustrated virtual map',{exact:true})).toBeVisible()
    const mapCheck=async()=>{
     const result=await page.evaluate(labels=>{
      const rect=n=>{const r=n.getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,width:r.width,height:r.height}},image=rect(document.querySelector('.places-map-plane > img')),viewport=rect(document.querySelector('.places-map-viewport')),controls=[...document.querySelectorAll('.places-map-caption,.places-map-zoom')].map(rect)
      const ink=labels.map(l=>({label:l.label,left:image.left+l.rect.x/768*image.width,right:image.left+(l.rect.x+l.rect.width)/768*image.width,top:image.top+l.rect.y/1296*image.height,bottom:image.top+(l.rect.y+l.rect.height)/1296*image.height}))
      return {image,viewport,controls,ink,covered:ink.filter(l=>controls.some(c=>l.left<c.right&&l.right>c.left&&l.top<c.bottom&&l.bottom>c.top)).map(l=>l.label),overflow:document.documentElement.scrollWidth>innerWidth}
     },labels)
     assert(result.image.left>=result.viewport.left-1&&result.image.right<=result.viewport.right+1&&result.image.top>=result.viewport.top-1&&result.image.bottom<=result.viewport.bottom+1);assert.deepEqual(result.covered,[]);assert.equal(result.overflow,false);return result
    }
    c.initialMap=await mapCheck();await page.getByRole('button',{name:'Zoom in map view',exact:true}).click();await page.getByRole('button',{name:'Reset view',exact:true}).click();c.resetMap=await mapCheck()
    c.mapScreenshot=`map-${era.toLowerCase()}-${width}.png`;await page.screenshot({path:resolve(out,c.mapScreenshot)})
    if(width===390){await page.setViewportSize({width,height:754});await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));c.referenceMap=await mapCheck();c.mapReferenceScreenshot=`map-${era.toLowerCase()}-${width}-h754.png`;await page.screenshot({path:resolve(out,c.mapReferenceScreenshot)})}
    assert.deepEqual(pageErrors,[]);c.status='PASS'
   }catch(e){c.status='FAIL';c.error=e.message}
   finally{try{await page.evaluate(()=>window.__SCENE_V5_ROOT__?.dispose());if(await page.evaluate(()=>!!window.__SCENE_V5_ROOT__)){await expect.poll(()=>page.evaluate(()=>window.__SCENE_V5_ROOT__.state().activeSockets)).toBe(0);await expect.poll(()=>page.evaluate(()=>window.__SCENE_V5_ROOT__.state().canvases)).toBe(0);c.cleanup=await page.evaluate(()=>window.__SCENE_V5_ROOT__.state());assert(c.cleanup.disposed);assert(c.cleanup.sourceCaches.every(v=>v.disposed&&v.entries===0&&v.residentBytes===0));assert(c.cleanup.urls.created.every(v=>c.cleanup.urls.revoked.includes(v)))}}catch(e){c.status='FAIL';c.cleanupError=e.message}await context.close()}
   report.cases.push(c);console.log(JSON.stringify({scope,width,era,status:c.status,error:c.error}));await writeFile(resolve(out,'report.partial.json'),JSON.stringify(report,null,2)+'\n');if(c.status==='FAIL')break sceneCases
  }
  report.status=report.cases.length===9&&report.cases.every(c=>c.status==='PASS')?'BOUNDED_PUBLISHED_SCENE_PASS':'FAIL'
 }else{
 let ordinal=0
 captures: for(const width of [360,390,430])for(const era of ['REVENGE','PARADE','DANGER'])for(const kind of ['feed','me','meet']){
  if(ordinal++<startAt)continue
  await beforeCase();const context=await browser.newContext({viewport:{width,height:844},hasTouch:true,isMobile:true,reducedMotion:'reduce'}),page=await context.newPage(),pageErrors=[];page.on('pageerror',e=>pageErrors.push(e.message));page.setDefaultTimeout(20000)
  const c={width,era,kind,status:'IN_PROGRESS',errors:pageErrors}
  try{await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'));await page.goto(`http://127.0.0.1:${port}/__qa06/start.html?`+new URLSearchParams({entry:'/'+kind,era}));await expect.poll(()=>page.evaluate(()=>window.__QA06__?.state().published)).toBe(true);await page.waitForLoadState('networkidle');await page.evaluate(()=>document.fonts.ready)
   c.state=await page.evaluate(()=>window.__QA06__.state());assert.equal(c.state.theme.selected,era);assert.equal(c.state.canvases,1);assert(c.state.socketPeak<=1);assert.equal(c.state.sockets,0)
   const check=async height=>{
    const metrics=await page.evaluate(()=>({overflow:document.documentElement.scrollWidth>innerWidth,brokenImages:[...document.images].filter(i=>!i.complete||i.naturalWidth===0).map(i=>i.alt)}));assert.equal(metrics.overflow,false);assert.deepEqual(metrics.brokenImages,[])
    if(kind==='feed'){const compose=page.getByRole('button',{name:'Write a post',exact:true});await expect(compose).toHaveCount(1);const b=await compose.boundingBox();assert(b.height>=44);assert(b.y+b.height<=height);if(height===844)assert(b.y>height-120);await expect(page.getByRole('navigation',{name:'Feed sections',exact:true})).toHaveCount(0);metrics.compose=b}
    if(kind==='me'){await expect(page.getByRole('navigation',{name:'Me sections',exact:true})).toHaveCount(0);assert(!c.state.requests.some(r=>r.path.endsWith('/feed')));await expect(page.getByText('Recent activity is unavailable in this connection.',{exact:true})).toBeVisible()}
    if(kind==='meet'){const action=page.locator('.meet-create__action'),copy=page.locator('.meet-create__unavailable');for(const e of [action,copy]){const r=await e.boundingBox();assert(r.y>=0&&r.y+r.height<=height-70,JSON.stringify({height,rect:r}))}metrics.create=await action.boundingBox()}
    return metrics
   }
   c.metrics=await check(844);c.screenshot=`${kind}-${era.toLowerCase()}-${width}.png`;await page.screenshot({path:resolve(out,c.screenshot)})
   if(kind==='me'){await page.getByText('Profile options',{exact:true}).click();const options=page.getByRole('navigation',{name:'More account options',exact:true});for(const a of await options.getByRole('link').all()){await a.scrollIntoViewIfNeeded();const r=await a.boundingBox();assert(r.height>=44);await a.focus();await expect(a).toBeFocused()}await page.getByText('Profile options',{exact:true}).click()}
   const referenceHeight={feed:631,me:595,meet:637}[kind]
   for(const height of (width===390?[referenceHeight,420]:[420])){await page.setViewportSize({width,height});await expect.poll(()=>page.evaluate(()=>parseFloat(document.querySelector('.world-shell').style.getPropertyValue('--shell-viewport-height')))).toBe(height);await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));c['height'+height]=await check(height);const shot=`${kind}-${era.toLowerCase()}-${width}-h${height}.png`;await page.screenshot({path:resolve(out,shot)});c['screenshot'+height]=shot}
   assert.deepEqual(pageErrors,[]);c.status='PASS'
  }catch(e){c.status='FAIL';c.error=e.message}
  finally{try{await page.evaluate(()=>window.__QA06__?.dispose());if(await page.evaluate(()=>!!window.__QA06__)){await expect.poll(()=>page.evaluate(()=>window.__QA06__.state().activeSockets)).toBe(0);await expect.poll(()=>page.evaluate(()=>document.querySelectorAll('canvas').length)).toBe(0);c.cleanup=await page.evaluate(()=>window.__QA06__.state());assert(c.cleanup.disposed)}}catch(e){c.status='FAIL';c.cleanupError=e.message}await context.close()}
  report.cases.push(c);console.log(JSON.stringify({width,era,kind,status:c.status,error:c.error}));await writeFile(resolve(out,'report.partial.json'),JSON.stringify(report,null,2)+'\n');if(c.status==='FAIL')break captures
 }
 report.status=report.cases.length===27-startAt&&report.cases.every(c=>c.status==='PASS')?'BOUNDED_PUBLISHED_REPAIRS_PASS':'FAIL'
 }
}catch(e){report.status='FAIL';report.error=e.message}
finally{await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await clearTemporaryCache(cacheDir)}
if(report.status==='FAIL')process.exitCode=1
