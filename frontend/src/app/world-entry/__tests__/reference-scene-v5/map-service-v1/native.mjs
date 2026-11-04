// Actual root/Places/Phaser with explicit source resolver and synthetic C03/C05. No admission bypass.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, readdir, rm, stat, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'
import { writeEditable } from './editable.mjs'

const root = resolve(import.meta.dirname, '../../../../../../..'), frontend = resolve(root, 'frontend')
const output = process.env.MCR_MAP_SERVICE_OUTPUT
assert(output?.startsWith('/'), 'Set MCR_MAP_SERVICE_OUTPUT to a new absolute output directory')
assert(process.env.MCR_CAPTURE_ALLOCATION, 'A fresh named BackendSL capture allocation is required')
const disk = await statfs(root); assert(disk.bavail * disk.bsize >= 16 * 1024 ** 3, 'Fresh start requires16GiB free')
await mkdir(output, { recursive: false, mode: 0o700 }) // A retry must use a new directory and retain its predecessor.
const browserTemp=resolve(output,'browser-tmp'),priorTemp=process.env.TMPDIR
await mkdir(browserTemp,{mode:0o700});process.env.TMPDIR=browserTemp // Include this process's Playwright profile/artifact storage in the cap.
const labelSource = JSON.parse(await readFile(resolve(frontend, 'public/world/reference-scene-v2/map-service-labels-v1/label-presentation.json'), 'utf8'))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const report = { status: 'IN_PROGRESS', qualification: 'MAP_SERVICE_V1_ACTUAL_DEFAULT_ROOT_MAP_ACTIONS_AND_NEW_EDITABLE_FIXTURES',
  startedAt: new Date().toISOString(),
  resourceAllocation: process.env.MCR_CAPTURE_ALLOCATION, resourceLimit: { minimumFreeGiB: 8, maximumNewGiB: 0.2 },
  candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourcePins: {}, checks: [],
  limits: ['Synthetic map7005 is not an allocated backend release', 'No invented REVIEWED provenance; optional skins remain guarded',
    'Desktop Chrome viewports and reduced height, not iOS Safari/Android device or actual software keyboard/notch acceptance',
    'Destination smoke checks actual route and mounted production panel or its honest guard; no invented service content/editor availability',
    'No live Auth/server/multiuser/physical/fidelity or release admission'] }
for (const path of ['frontend/src/app/world-entry/reference-scene-overview.ts', 'frontend/src/app/WorldShell.tsx',
  'frontend/src/app/world-entry/production.ts', 'frontend/src/app/world-entry/resolver.ts', 'frontend/src/app/world-panels/index.tsx',
  'frontend/src/world/scenes/MapScene.ts', 'frontend/src/world/composition/index.ts', 'frontend/src/world/composition/release.ts',
  'frontend/src/world/theme/compatibility.ts', 'frontend/src/features/places/ui/IllustratedMap.tsx', 'frontend/src/features/places/ui/reference-scene/spatial-v5-label-obstacles.ts',
  ...['presentation.json', 'handoff.json', 'map-labels-bold.ttf', 'OFL.txt'].map(p => 'frontend/public/world/reference-scene-v2/map-labels-v1/' + p),
  ...['model.ts', 'metadata.ts', 'index.ts', 'loader.ts', 'phaser.ts', 'overview.ts'].map(p => 'frontend/src/world/reference-scene/' + p),
  ...['model.ts', 'source-inputs.json', 'tile-pin.json', 'build-source-inputs.py', 'map-labels.ts', 'map-labels-source.json', 'map-labels-pins.json','map-landscape-source.json','map-landscape-pin.json','build-map-landscape-inputs.py','backend-allocation.json','camera.ts'].map(p => 'frontend/src/world/reference-scene/spatial-v5/' + p),
  ...['data.ts', 'preview.tsx', 'preview.html', 'native.mjs', 'contracts.test.ts'].map(p => 'frontend/src/app/world-entry/__tests__/reference-scene-v5/' + p),
  ...['canonical-map.json', 'source-binding.json'].map(p => 'frontend/public/world/reference-scene-v2/runtime/spatial-v5/' + p)]) {
  report.sourcePins[path] = hash(await readFile(resolve(root, path)))
}
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(output, 'vite-cache'), logLevel: 'error',
  server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false } })
let browser
const resourceSamples=[]
async function resourceGuard() {
 const disk=await statfs(root),freeBytes=disk.bavail*disk.bsize
 const size=async path=>{let total=0;for(const entry of await readdir(path,{withFileTypes:true})){const p=resolve(path,entry.name);if(entry.isDirectory())total+=await size(p);else if(entry.isFile())total+=(await stat(p)).size}return total}
 const outputBytes=await size(resolve(output,'..'))
 const mains=execFileSync('ps',['-axo','command='],{encoding:'utf8'}).split('\n').filter(line=>line.trimStart().startsWith('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome ')&&line.includes('--headless'))
 resourceSamples.push({freeBytes,outputBytes,headlessChromeMains:mains.length})
 assert(freeBytes>=8*1024**3,'Fresh8GiBfloor');assert(outputBytes<=0.2*1024**3,'Newoutput/cachehardcap');assert(mains.length<=(browser?1:0),'Another headless capture is active')
}
const state = page => page.evaluate(() => window.__MAP_SERVICE_ROOT__?.state())
const world = page => page.evaluate(() => window.__MAP_SERVICE_ROOT__.inspectWorld())
const navigate = (page, path) => page.evaluate(path => window.__MAP_SERVICE_ROOT__.navigate(path), path)
const mapBounds = page => page.evaluate(() => {
  const rect = node => { const r = node.getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height } }
  return { image: rect(document.querySelector('.places-map-plane > img')), viewport: rect(document.querySelector('.places-map-viewport')) }
})
function assertFit({ image, viewport }) {
  assert(image.width > 0 && image.height > 0)
  assert(image.left >= viewport.left - 1 && image.right <= viewport.right + 1)
  assert(image.top >= viewport.top - 1 && image.bottom <= viewport.bottom + 1)
}
async function open(width, era, suffix = '') {
  await resourceGuard()
  const space = await statfs(root); assert(space.bavail * space.bsize >= 8 * 1024 ** 3, 'Disk floor before next browser context')
  const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce', serviceWorkers: 'block', hasTouch: true })
  const page = await context.newPage(), errors = [], requests = []
  page.on('pageerror', error => errors.push(error.message)); page.on('request', r => requests.push(new URL(r.url()).pathname))
  if (suffix.includes('font=corrupt')) await page.route('**/map-labels-v1/map-labels-bold.ttf', route => route.fulfill({ status: 200, body: Buffer.alloc(145012), contentType: 'font/ttf' }))
  await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
  await page.goto(server.resolvedUrls.local[0] + `src/app/world-entry/__tests__/reference-scene-v5/map-service-v1/preview.html?era=${era}${suffix}`)
  return { context, page, errors, requests }
}
async function close({ context, page }) {
  await page.evaluate(() => window.__MAP_SERVICE_ROOT__?.dispose())
  await expect.poll(async () => { const s = await state(page); return s.urls.created.every(url => s.urls.revoked.includes(url)) }).toBe(true)
  const s = await state(page), w = await world(page)
  assert.equal(w.games, 0); assert.equal(s.activeSockets, 0); assert.equal(s.canvases, 0)
  assert.deepEqual(new Set(s.urls.created), new Set(s.urls.revoked))
  assert.equal(await page.evaluate(() => [...document.fonts].filter(f => f.family === 'MCR Map Labels').length), 0)
  assert(s.sourceCaches.every(cache => cache.disposed && cache.entries === 0 && cache.residentBytes === 0))
  await context.close(); return { state: s, world: w }
}

for (const path of [
 ...['map-layout-v2.mjs','map-layout-v2-probe.ts','map-layout-v2-editable.mjs'].map(p => 'frontend/src/app/world-entry/__tests__/reference-scene-v5/' + p),
 ...['handoff.json','label-presentation.json','landscape-presentation.json','target-measurements.json','verification.json'].map(p => 'frontend/public/world/reference-scene-v2/map-layout-v2/' + p),
 'frontend/src/world/reference-scene/__tests__/spatial-v5/map-layout-v2.test.ts',
]) report.sourcePins[path] = hash(await readFile(resolve(root, path)))
const plants = JSON.parse(await readFile(resolve(frontend, 'src/world/reference-scene/spatial-v5/map-landscape-source.json'), 'utf8'))
const measurements = JSON.parse(await readFile(resolve(frontend, 'public/world/reference-scene-v2/map-layout-v2/target-measurements.json'), 'utf8'))
async function settled(page) { await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))) }
async function inspectLayout(page) {
 return page.evaluate(({ labels, plants }) => {
  const r = document.querySelector('.places-map-plane > img').getBoundingClientRect()
  const project = box => ({ left:r.left+box.x/768*r.width, right:r.left+(box.x+box.width)/768*r.width, top:r.top+box.y/1296*r.height, bottom:r.top+(box.y+box.height)/1296*r.height })
  const ink = labels.map(l => ({ label:l.label, ...project(l.rect) }))
  const overlaps = (a,b) => a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
  const controls = [...document.querySelectorAll('.places-map-zoom,.places-map-caption,.places-map-plaque:not([data-map-painted-label]):not([data-map-feature])')].map(n => ({ name:n.getAttribute('aria-label')??n.className, rect:n.getBoundingClientRect() }))
  const coveredLabels = ink.flatMap(l => controls.filter(c=>overlaps(l,c.rect)).map(c=>[l.label,c.name]))
  const movedBeds = plants.filter(p=>['map_garden_plaza_south_west','map_garden_plaza_south_east'].includes(p.id)).map(p=>({ id:p.id, ...project(p) }))
  const coveredMovedBeds = movedBeds.flatMap(p=>controls.filter(c=>overlaps(p,c.rect)).map(c=>[p.id,c.name]))
  const painted = [...document.querySelectorAll('[data-map-feature]')].map(n => {
   const id=n.dataset.mapFeature, label=labels.find(l=>l.navigation_intent===id), box=n.getBoundingClientRect(), expected=project(label.rect)
   return {id, width:box.width, expectedWidth:expected.right-expected.left, centerX:(box.left+box.right)/2, expectedX:(expected.left+expected.right)/2, centerY:(box.top+box.bottom)/2, expectedY:(expected.top+expected.bottom)/2, height:box.height}
  })
  return {ink,movedBeds,coveredLabels,coveredMovedBeds,painted,overflow:document.documentElement.scrollWidth>innerWidth}
 }, {labels:labelSource.labels, plants:plants.decorations})
}
function assertLayout(layout) {
 assert.deepEqual(layout.coveredLabels,[]); assert.deepEqual(layout.coveredMovedBeds,[]); assert.equal(layout.overflow,false)
 assert.equal(layout.painted.length,4)
 for(const p of layout.painted){assert(Math.abs(p.width-p.expectedWidth)<.1);assert(Math.abs(p.centerX-p.expectedX)<.1);assert(Math.abs(p.centerY-p.expectedY)<.1);assert(p.height>=44)}
}
for (const path of [...['build-architecture-inputs.py','architecture-source.json','architecture-pin.json','architecture.ts'].map(p=>'frontend/src/world/reference-scene/spatial-v5/'+p), ...['native.mjs','renderer.ts','renderer.html','editable.mjs','probe.ts'].map(p=>'frontend/src/app/world-entry/__tests__/reference-scene-v5/architecture-v2/'+p),'frontend/src/world/reference-scene/__tests__/spatial-v5/architecture.test.ts','frontend/public/world/reference-scene-v2/runtime/architecture-v2/source-binding.json','frontend/public/world/reference-scene-v2/architecture-v2/atlas-handoff.json']) report.sourcePins[path] = hash(await readFile(resolve(root,path)))
for(const path of [...['build-map-service-inputs.py','map-service-source.json','map-service-pins.json','map-service.ts'].map(p=>'frontend/src/world/reference-scene/spatial-v5/'+p),...['native.mjs','preview.tsx','preview.html','editable.mjs','probe.ts','label-reference.js'].map(p=>'frontend/src/app/world-entry/__tests__/reference-scene-v5/map-service-v1/'+p),'frontend/src/features/places/ui/reference-scene/map-service-links.tsx','frontend/src/app/shell.css','frontend/src/shared/styles/base.css'])report.sourcePins[path]=hash(await readFile(resolve(root,path)))
for(const path of ['frontend/src/app/router.tsx','frontend/src/app/routes.ts','frontend/src/app/social-composition/index.ts','frontend/src/app/social-composition/panels.tsx','frontend/src/app/social-composition/provider.tsx','frontend/src/app/community-entry/routes.tsx','frontend/src/app/CommunityFeatures.tsx','frontend/src/app/mural-entry/provider.tsx','frontend/src/features/places/ui/IllustratedMap.tsx','frontend/src/features/places/ui/reference-scene/spatial-v5-label-obstacles.ts','frontend/src/app/shell-chrome/index.tsx','frontend/src/app/shell-chrome/parade-footer.tsx','frontend/src/app/shell-chrome/shell-chrome.css','frontend/src/shared/ui/concert-window/concert-window.css'])report.sourcePins[path]=hash(await readFile(resolve(root,path)))
const servicePins=JSON.parse(await readFile(resolve(frontend,'src/world/reference-scene/spatial-v5/map-service-pins.json'),'utf8'))
for(const pin of servicePins.outputs)report.sourcePins[pin.path]=hash(await readFile(resolve(root,pin.path)))
const architecturePins=JSON.parse(await readFile(resolve(frontend,'src/world/reference-scene/spatial-v5/architecture-pin.json'),'utf8'))
for(const pin of architecturePins.outputs) report.sourcePins[pin.path]=hash(await readFile(resolve(root,pin.path)))
for(const era of ['revenge','parade','danger']) {
 const path='docs/mcr/evidence/reference-scene-v5/map-layout-v2/qualified-v2/editable-'+era+'.svg'
 report.sourcePins[path]=hash(await readFile(resolve(root,path)))
}

try {
 await resourceGuard()
 await server.listen(); browser=await chromium.launch({channel:'chrome',headless:true}); report.browser=browser.version()
 for(const width of [390,360,430]) for(const era of ['PARADE','REVENGE','DANGER']) {
  const fixture=await open(width,era),{page}=fixture
  try {
   await expect.poll(async()=> (await state(page))?.overview?.themeId,{timeout:20000}).toBe(era)
   const before=await page.evaluate(()=>window.__MAP_SERVICE_ROOT__.cameraProjection().self)
   const rootWorld=await world(page);assert.equal(rootWorld.referenceObjects,35);await expect.poll(async()=>(await world(page)).sprites).toBe(6)
   await navigate(page,'/world/map')
   const image=page.getByAltText(`${era} illustrated virtual map`,{exact:true}); await expect(image).toBeVisible();await settled(page)
   const pixels=await page.evaluate(async()=>{const module=await import('/src/app/world-entry/__tests__/reference-scene-v5/map-service-v1/probe.ts');const pins=window.__MAP_SERVICE_ROOT__.state().overview;return module.probeServiceMap(pins.imageUrl,pins.themeId)}); assert.equal(pixels.differentChannels,0); assert.deepEqual(pixels.invalidPairsRejected,[true,true]);assert.equal(pixels.objects,51)
   assertFit(await mapBounds(page)); const original=await mapBounds(page)
   const layout=await inspectLayout(page);assertLayout(layout);assert.equal(await page.locator('.places-map-plaque').count(),8)
   await page.screenshot({path:resolve(output,`map-${era.toLowerCase()}-${width}.png`)})
   const navigation=[]
   const names=await page.locator('.places-map-plaque:not([data-map-feature])').evaluateAll(nodes=>nodes.map(n=>n.getAttribute('aria-label')))
   for(const name of names){const link=page.getByRole('link',{name,exact:true});await link.focus();await expect(link).toBeFocused();await page.keyboard.press('Enter');const back=page.getByRole('link',{name:'All places',exact:true});await expect(back).toBeVisible();await back.click();await expect(image).toBeVisible();navigation.push(name)}
   const featureActions=[]
   for(const intent of ['feed','wall','meet','gifts']) {
    const link=page.locator(`[data-map-feature="${intent}"]`)
    await expect(link).toBeVisible()
    const box=await link.boundingBox();assert(box.width>=44&&box.height>=44)
    if(intent==='feed'||intent==='wall') await link.tap()
    else {await link.focus();await page.keyboard.press('Enter')}
    await expect.poll(async()=>(await state(page)).path).toBe('/'+intent)
    const heading=page.locator('.mcr-chrome-panel-header h1');await expect(heading).toHaveText(new RegExp('^'+intent+'$','i'))
    if(width===390&&era==='PARADE')await page.screenshot({path:resolve(output,`action-${intent}-390.png`)})
    const panel=page.locator('.shell-panel');await expect(panel).toBeVisible();await expect(page.getByText('This page is unavailable',{exact:true})).toHaveCount(0)
    const destination={renderedText:(await panel.innerText()).slice(0,1800),requests:(await state(page)).requests.map(r=>({path:r.path,method:r.method})),intent,path:(await state(page)).path,heading:await heading.textContent(),activation:intent==='feed'||intent==='wall'?'touch':'keyboard'}
    await page.evaluate(()=>window.__MAP_SERVICE_ROOT__.back());await expect(image).toBeVisible();await expect(link).toBeFocused()
    featureActions.push({...destination,backFocus:true})
   }
   if(width===390&&era==='PARADE')console.log(JSON.stringify({event:'FIRST_MAP_SLICE_READY',candidate:report.candidate,capturedAt:new Date().toISOString(),file:resolve(output,'map-parade-390.png'),featureActions,limits:report.limits}))
   const controlsCamera=(await state(page)).camera
   await page.getByRole('button',{name:'Zoom in map view',exact:true}).click();await page.getByRole('button',{name:'Reset view',exact:true}).click();await settled(page);assertFit(await mapBounds(page));assert.deepEqual((await state(page)).camera,controlsCamera)
   let short=null
   if(width===390){await page.setViewportSize({width,height:754});await expect.poll(async()=>(await mapBounds(page)).viewport.height).not.toBe(original.viewport.height);await settled(page);assertFit(await mapBounds(page));short=await inspectLayout(page);assertLayout(short);await page.screenshot({path:resolve(output,`map-${era.toLowerCase()}-${width}-h754.png`)})}
   const after=await state(page),w=await world(page);assert.equal(after.factoryCalls,1);assert.equal(after.socketPeak,1);assert.equal(w.totalGames,1);assert.deepEqual(await page.evaluate(()=>window.__MAP_SERVICE_ROOT__.cameraProjection().self),before);assert.deepEqual(fixture.errors,[])
   report.checks.push({width,height:844,era,capturedAt:new Date().toISOString(),pixels,layout,short,navigation,featureActions,singleton:w,cleanup:await close(fixture)})
  }catch(error){await page.screenshot({path:resolve(output,'failure.png')}).catch(()=>{});await close(fixture).catch(()=>{});throw error}
 }
 for(const status of ['closed','unavailable']){
  const fixture=await open(390,'PARADE','&markerStatus='+status),{page}=fixture
  await expect.poll(async()=> (await state(page))?.overview?.themeId,{timeout:20000}).toBe('PARADE');await navigate(page,'/world/map')
  const marker=page.locator('[data-map-compact-marker*="park"]');await expect(marker).toBeVisible();if(status==='unavailable'){await marker.click();await expect(page.getByText('This place is unavailable',{exact:true})).toBeVisible();await page.getByRole('link',{name:'All places',exact:true}).click()}
  await marker.focus();await expect(marker).toBeFocused();assert((await marker.getAttribute('aria-label')).includes(status));assert.equal(await page.locator('.places-map-plaque').count(),8)
  const bounds=await marker.evaluate(n=>{const a=n.getBoundingClientRect(),b=n.querySelector('small').getBoundingClientRect();return{width:a.width,height:a.height,badgeInside:b.left>=a.left&&b.right<=a.right&&b.top>=a.top&&b.bottom<=a.bottom}});assert.deepEqual(bounds,{width:44,height:44,badgeInside:true})
  await page.screenshot({path:resolve(output,`marker-${status}-390.png`)});assert.deepEqual(fixture.errors,[]);report.checks.push({status,capturedAt:new Date().toISOString(),bounds,cleanup:await close(fixture)})
 }
 report.editable=[]
 for(const era of ['REVENGE','PARADE','DANGER']){
  await resourceGuard()
  const disk=await statfs(root);assert(disk.bavail*disk.bsize>=8*1024**3,'Disk floor before editable render')
  const {svg,metadata}=await writeEditable(frontend,output,era),page=await browser.newPage({viewport:{width:768,height:1296}})
  await page.setContent(`<html><body style="margin:0">${svg}</body></html>`);await page.evaluate(()=>document.fonts.ready);await settled(page)
  const geometry=await page.evaluate(()=>({objects:document.querySelectorAll('[data-object]').length,labels:[...document.querySelectorAll('[data-label]')].map(n=>({label:n.dataset.label,anchor:n.dataset.anchor,rect:['x','y','width','height'].map(k=>Number(n.querySelector('[data-paper]').getAttribute(k)))})),font:[...document.fonts].some(f=>f.family.includes('MCR Map Labels')&&f.status==='loaded')}))
  assert.equal(geometry.objects,51);assert.equal(geometry.font,true);assert.deepEqual(geometry.labels.map(l=>l.rect),labelSource.labels.map(l=>[l.rect.x,l.rect.y,l.rect.width,l.rect.height]))
  await page.screenshot({path:resolve(output,`editable-${era.toLowerCase()}.png`)});await page.close();report.editable.push({capturedAt:new Date().toISOString(),metadata,geometry})
 }
 report.targetRegistration=measurements.labels.map(l=>({label:l.label,error:l.proposed_center_error_source_px}));assert(report.targetRegistration.every(l=>l.error.every(e=>Math.abs(e)<=.5)))
 report.status='PASS'
}catch(error){report.status='FAIL';report.failure=String(error);throw error}
finally{
 await resourceGuard().catch(error=>{report.resourceFailure=String(error);report.status='FAIL'})
 await browser?.close();await server.close();await rm(resolve(output,'vite-cache'),{recursive:true,force:true});await rm(browserTemp,{recursive:true,force:true});if(priorTemp===undefined)delete process.env.TMPDIR;else process.env.TMPDIR=priorTemp
 report.cleanup={browserClosed:!browser?.isConnected(),serverClosed:!server.httpServer?.listening,ownedCacheAndBrowserTempRemoved:true}
 report.resources={samples:resourceSamples,minimumFreeBytes:Math.min(...resourceSamples.map(s=>s.freeBytes)),maximumNewBytes:Math.max(...resourceSamples.map(s=>s.outputBytes))}
 report.finishedAt=new Date().toISOString()
 await writeFile(resolve(output,'native.json'),JSON.stringify(report,null,2)+'\n')
}
console.log(JSON.stringify({status:report.status,mapCases:9,markerCases:2,editable:report.editable?.length}))
assert.equal(report.status,'PASS','Incomplete/resource-failed native evidence cannot exit successfully')
