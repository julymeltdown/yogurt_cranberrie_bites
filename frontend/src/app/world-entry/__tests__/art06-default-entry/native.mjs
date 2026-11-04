// ART06 changed-candidate actual-main validation. Fresh accepted resource lease required.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {execFileSync} from 'node:child_process'
import {readFile,writeFile,mkdir,statfs} from 'node:fs/promises'
import {resolve,extname} from 'node:path'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../../../..'),frontend=resolve(root,'frontend')
const output=process.env.MCR_ART06_OUTPUT,temporary=process.env.MCR_ART06_TMP,lease=process.env.MCR_ART06_LEASE
const deadline=Date.parse(process.env.MCR_ART06_DEADLINE??''),port=Number(process.env.MCR_ART06_PORT)
assert(output?.startsWith(root+'/docs/mcr/evidence/map-art06-default-app/')&&temporary?.startsWith('/private/tmp/mcr-art06-')&&lease&&Number.isFinite(deadline)&&port>1023)
const hash=bytes=>'sha256:'+createHash('sha256').update(bytes).digest('hex')
const candidate=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim()
const registration=await readFile(resolve(frontend,'src/world/reference-scene/map4-service-v2/registration.ts'),'utf8')
const expectedPresentationHash=registration.match(/presentationHash:\s*'(sha256:[a-f0-9]{64})'/)?.[1]
assert(expectedPresentationHash&&expectedPresentationHash!=='sha256:f7a89dc6d176f2414f845139de39607ba75830a54e5e623f2de2cd6fc76bed17','Fresh ART06 registration required')
const state=page=>page.evaluate(()=>window.__ART06_DEFAULT__.state())
const report={status:'IN_PROGRESS',qualification:'ACTUAL_MAIN_CANONICAL_UI_LOGIN_MAP4_HTTP_C05_FIXTURE_NOT_LIVE',candidate,lease,expectedPresentationHash,checks:[],assetPins:{},limits:['Only explicit eligible-attendee HTTP/C05 fixtures; no real operator/admission/live/device claim.','Changed ART06 only, no ART04 replay/build; original Map4/geometry/theme/avatar pins.','World invariance compares the same actual App before/after Map/control/route navigation; no historical framebuffer equivalence claim.','Full ten-screen fidelity remains open.']}
const pinDoc=JSON.parse(await readFile(resolve(root,'docs/mcr/evidence/map-art06-default-app/readset.json'),'utf8'))
for(const row of pinDoc.pins)assert.equal(hash(await readFile(resolve(root,row.path))),'sha256:'+row.sha256,row.path)
report.readsetHash=hash(await readFile(resolve(root,'docs/mcr/evidence/map-art06-default-app/readset.json')))
const publicPaths=new Set(execFileSync('git',['ls-tree','-r','--name-only',candidate,'frontend/public'],{cwd:root,encoding:'utf8'}).trim().split('\n'))
const input=JSON.parse(await readFile(resolve(frontend,'src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'),'utf8'))
const localInput=JSON.parse(await readFile(resolve(frontend,'src/app/world-entry/__tests__/map-presentation-v1/source-inputs/local-input.json'),'utf8'))
async function guard(){const disk=await statfs(root);assert(disk.bavail*disk.bsize>=8*1024**3);assert(Date.now()<deadline-60000)}
const worldCamera=page=>page.evaluate(()=>window.__ART06_DEFAULT__.camera())
async function settled(page){await expect.poll(async()=>{const c=await worldCamera(page),b=await page.locator('.shell-world-host canvas').evaluate(e=>{const r=e.parentElement.getBoundingClientRect();return {w:Math.floor(r.width),h:Math.floor(r.height)}});return !!c&&c.width===b.w&&c.height===b.h}).toBe(true);await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))}
let server,browser,context
await mkdir(output,{recursive:true})
try{
 await guard()
 server=await createServer({root:frontend,configLoader:'runner',cacheDir:resolve(temporary,'vite'),logLevel:'error',define:{'import.meta.env.VITE_MCR_EVENT_ID':JSON.stringify(input.event.event_id),'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG':JSON.stringify(JSON.stringify(localInput))},plugins:[{name:'art06-actual-main-frozen-public',configureServer(server){server.middlewares.use(async(req,res,next)=>{
 const path='frontend/public'+decodeURIComponent(new URL(req.url,'http://localhost').pathname)
 if(publicPaths.has(path)){const bytes=execFileSync('git',['show',`${candidate}:${path}`],{cwd:root,maxBuffer:64*1024**2});report.assetPins[path]={sha256:hash(bytes),bytes:bytes.length};const mime={'.png':'image/png','.svg':'image/svg+xml','.ttf':'font/ttf','.json':'application/json'}[extname(path)];if(mime)res.setHeader('Content-Type',mime);res.end(bytes);return}
 if(!req.headers.accept?.includes('text/html'))return next()
 res.setHeader('Content-Type','text/html');res.end(await server.transformIndexHtml(req.url,await readFile(resolve(import.meta.dirname,'preview.html'),'utf8')))
 })}}],server:{host:'127.0.0.1',port,strictPort:true,watch:null}})
 await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const era of ['REVENGE','PARADE','DANGER'])for(const {width,height} of [...[360,390,430].map(width=>({width,height:844})),...(era==='PARADE'?[{width:390,height:420}]:[])]){
 await guard();context=await browser.newContext({viewport:{width,height},reducedMotion:'reduce',serviceWorkers:'block'})
 const page=await context.newPage();page.setDefaultTimeout(10000);const errors=[],requests=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>requests.push(new URL(r.url()).pathname))
 report.current={era,width,height};const result={era,width,height,errors,requests,virtualActions:[],controls:{},status:'IN_PROGRESS'};report.checks.push(result)
 await page.goto(`http://127.0.0.1:${port}/login?`+new URLSearchParams({era,scenario:'eligible-attendee-contract-fixture'}))
 await page.getByLabel('Email',{exact:true}).fill('map-fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password');await page.getByRole('button',{name:'Sign in with email'}).click()
 await expect.poll(async()=>(await state(page)).activeSockets).toBe(1);await expect(page.locator('.shell-world-host canvas')).toHaveCount(1);await settled(page)
 const host=page.locator('[data-world-host]');await host.evaluate(e=>Reflect.set(e,'__art06Incumbent','same'))
 const worldBefore=await page.evaluate(()=>window.__ART06_DEFAULT__.worldProof());result.worldBefore=worldBefore
 await page.getByRole('button',{name:'World tools',exact:true}).click();await page.locator('a[href="/world/map"]').click()
 const image=page.getByRole('img',{name:`${era} illustrated virtual map`,exact:true});await expect(image).toBeVisible();await expect(page.getByRole('button',{name:'Fit map',exact:true})).toBeEnabled();await settled(page)
 const initial=await state(page);result.initial=initial;assert.equal(initial.canvases,1);assert.equal(initial.sockets,1);assert.equal(initial.map.revision,4);assert.equal(initial.map.contentHash,'sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8');assert.equal(initial.theme,era)
 result.raster=await image.evaluate(async(image)=>{
 const {currentComposition}=await import('/src/world/reference-scene/map4-service-v2/current.ts');const model=currentComposition();if(!model)throw Error('No ART06 model')
 const asset=model.registration.assets.find(a=>image.alt.startsWith(a.themeId));if(!asset)throw Error('No selected ART06 asset')
 const scene=await createImageBitmap(await(await fetch('/'+asset.path)).blob())
 const a=document.createElement('canvas'),b=document.createElement('canvas');a.width=b.width=768;a.height=b.height=1296;a.getContext('2d').drawImage(image,0,0);b.getContext('2d').drawImage(scene,0,0)
 const actual=a.getContext('2d').getImageData(0,0,768,1296).data,source=b.getContext('2d').getImageData(0,0,768,1296).data
 let compared=0,mismatches=0;for(let y=0;y<1296;y++)for(let x=0;x<768;x++){if(model.labels.some(l=>x>=l.exclusionRect.x&&x<l.exclusionRect.x+l.exclusionRect.width&&y>=l.exclusionRect.y&&y<l.exclusionRect.y+l.exclusionRect.height))continue;compared++;const i=(y*768+x)*4;if(actual[i]!==source[i]||actual[i+1]!==source[i+1]||actual[i+2]!==source[i+2]||actual[i+3]!==source[i+3])mismatches++}
 scene.close();a.width=a.height=b.width=b.height=0;return {presentationHash:model.registration.presentationHash,compared,mismatches}
 });assert.equal(result.raster.presentationHash,expectedPresentationHash);assert(result.raster.compared>700000);assert.equal(result.raster.mismatches,0,'Actual raster outside runtime labels equals exact ART06 scene')
 result.headerImages=await page.locator('.mcr-reference-window-ink:visible').evaluateAll(elements=>elements.map(image=>({src:image.getAttribute('src'),complete:image.complete,width:image.naturalWidth,height:image.naturalHeight})))
 assert.equal(result.headerImages.length,3);for(const image of result.headerImages)assert(image.complete&&image.width>0&&image.height>0,'Current canonical window SVGs load naturally in this changed-candidate capture')
 const screenshot=`art06-${era.toLowerCase()}-${width}${height===420?'-short':''}.png`;result.captureStartedAt=new Date().toISOString();await page.screenshot({path:resolve(output,screenshot),fullPage:false});result.screenshot=screenshot;result.screenshotHash=hash(await readFile(resolve(output,screenshot)));result.captureFinishedAt=new Date().toISOString()
        result.foreground = await page.evaluate(() => {
          const toolbar = document.querySelector('.places-map-tools button'), css = getComputedStyle(toolbar)
          const image = document.querySelector('.places-map-plane > img'), box = image.getBoundingClientRect()
          return { family: css.fontFamily, weight: css.fontWeight, fontSize: css.fontSize,
            loadedFaces: [...document.fonts].map(face => ({family:face.family,status:face.status,weight:face.weight})),
            labelFontCssPx: 40 * box.width / 768, image:{x:box.x,y:box.y,width:box.width,height:box.height},
            viewport: (() => {const el=document.querySelector('.places-map-viewport'); return {clientHeight:el.clientHeight,scrollHeight:el.scrollHeight,clientWidth:el.clientWidth,scrollWidth:el.scrollWidth}})() }
        })
        assert(result.foreground.family.startsWith('"Arial Narrow"'))
        assert.equal(result.foreground.weight,'800')
        result.foreground.runtimeFontDisposedAfterRaster = !result.foreground.loadedFaces.some(face => face.family.replaceAll('\"','') === 'MCR Map4 Composition Labels')
        assert(result.foreground.runtimeFontDisposedAfterRaster, 'Raster encoding releases its temporary FontFace; toolbar uses explicit bold fallback')
        // Exact original 40-unit text size, including the local-scroll short mode.
        assert(result.foreground.labelFontCssPx >= 13.8)
        result.foreground.actualLabelInk = await page.evaluate(async () => {
          const {currentComposition}=await import('/src/world/reference-scene/map4-service-v2/current.ts')
          const {compositionLabelPaintRect}=await import('/src/world/reference-scene/map4-service-v2/labels.ts')
          const model=currentComposition();if(!model)throw Error('Frozen composition unavailable')
          // Independent bounded metric probe over the exact retained original font.
          // Production rasterization has already retired its own temporary face.
          const response=await fetch('/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf'),bytes=await response.arrayBuffer()
          const digest=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(b=>b.toString(16).padStart(2,'0')).join('')
          if(digest!=='f44e493f61e2d91c2156dd5abc720eeb6274a9c02b11d9097d1ea38d50011e8b')throw Error('Original font bytes changed')
          const face=await new FontFace('MCR Map4 Composition Labels',bytes).load();document.fonts.add(face)
          try {const canvas=document.createElement('canvas'),context=canvas.getContext('2d');context.font='40px "MCR Map4 Composition Labels"'
            return model.labels.map(label=>{const metrics=context.measureText(label.label),paint=compositionLabelPaintRect(label,metrics);return {label:label.label,font:context.font,advance:metrics.width,inkLeft:metrics.actualBoundingBoxLeft,inkRight:metrics.actualBoundingBoxRight,ascent:metrics.actualBoundingBoxAscent,descent:metrics.actualBoundingBoxDescent,paint,exclusion:label.rect,fontProbeQualification:'Independent original-byte metrics; not a persistent production font owner'}})
          } finally {document.fonts.delete(face)}
        })
        assert.equal(result.foreground.actualLabelInk.length,6)
        for(const row of result.foreground.actualLabelInk) { assert(row.advance>0);assert(row.paint.width<=row.exclusion.width);assert(row.paint.height<=row.exclusion.height);assert(row.paint.textX+row.inkRight<=row.paint.x+row.paint.width) }
        for (const intent of ['wall', 'feed', 'meet', 'gifts']) {
          const link = page.locator(`#map-feature-${intent}`)
          await link.scrollIntoViewIfNeeded(); const box = await link.boundingBox()
          assert(box && box.width >= 44 && box.height >= 44)
          assert(box.x >= 0 && box.y >= 0 && box.x + box.width <= width && box.y + box.height <= height, 'Full target visible after local scroll')
          const unobstructed = await link.evaluate(element => {
            const r = element.getBoundingClientRect()
            return document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.closest('a') === element
          })
          assert(unobstructed, 'Feature target must not be blocked by controls/canonical markers')
          const controlOverlaps = await link.evaluate(element => {
            const a=element.getBoundingClientRect()
            return [...document.querySelectorAll('.places-map-zoom button,.places-map-tools button')].filter(control => {
              const b=control.getBoundingClientRect(); return Math.min(a.right,b.right)-Math.max(a.left,b.left)>.5 && Math.min(a.bottom,b.bottom)-Math.max(a.top,b.top)>.5
            }).map(control => control.getAttribute('aria-label') || control.textContent)
          })
          assert.deepEqual(controlOverlaps,[], 'Full virtual hit rectangle must avoid fixed zoom/toolbar controls')
          await expect(link).toHaveAttribute('href', '/' + intent)
          await link.focus(); await page.keyboard.press('Enter')
          await expect(page).toHaveURL(new RegExp('/'+intent+'(?:\\?|$)'))
          const routeWorld = await state(page)
          assert.equal(routeWorld.canvases, 1, 'Feature navigation retains the incumbent World canvas')
          assert.equal(routeWorld.sockets, 1, 'Feature navigation creates no second socket')
          assert(routeWorld.activeSockets <= 1)
          assert.equal(routeWorld.canvases === 1, true)
          await page.goBack()
          await expect(link).toBeFocused()
          result.virtualActions.push({ intent, path: '/' + intent, width: box.width, height: box.height, keyboardAndBackFocus: true, routeWorld: { canvases: routeWorld.canvases, socketsCreated: routeWorld.sockets, activeSockets: routeWorld.activeSockets, singleCanvas: routeWorld.canvases === 1 } })
        }
      await settled(page);const cameraBefore = await worldCamera(page)
      await page.getByRole('button', { name: 'Zoom in map view', exact: true }).click()
      await page.getByRole('button', { name: 'Fit map', exact: true }).click()
      await page.getByRole('button', { name: 'Reset view', exact: true }).click()
      result.controls = { localZoomFitReset: true, cameraUnchanged: JSON.stringify(await worldCamera(page)) === JSON.stringify(cameraBefore) }
      assert(result.controls.cameraUnchanged)
      {
        const controls=page.locator('.places-map-tools button,.places-map-zoom button')
        result.controls.bounds=await controls.evaluateAll(elements=>elements.map(element=>{const r=element.getBoundingClientRect();return {name:element.getAttribute('aria-label')||element.textContent,width:r.width,height:r.height}}))
        for(const box of result.controls.bounds) assert(box.width>=44 && box.height>=44)
        const fit=page.getByRole('button',{name:'Fit map',exact:true})
        await page.emulateMedia({forcedColors:'active'});await fit.focus();await page.keyboard.press('Tab');await page.keyboard.press('Shift+Tab');await expect(fit).toBeFocused()
        result.controls.forcedFocus=await fit.evaluate(element=>{const css=getComputedStyle(element);return {style:css.outlineStyle,width:css.outlineWidth,offset:css.outlineOffset,color:css.outlineColor}})
        assert.equal(result.controls.forcedFocus.style,'solid');assert(parseFloat(result.controls.forcedFocus.width)>=3)
        if(era==='PARADE' && width===390) { const file=`forced-focus-parade-390${height===420?'-short':''}.png`;await page.screenshot({path:resolve(output,file),fullPage:false});result.controls.forcedFocus.screenshot=file }
        await page.emulateMedia({forcedColors:'none'});await fit.evaluate(element=>element.blur())
      }

 const stable=await state(page);assert.equal(stable.sockets,1);assert.equal(stable.canvases,1);assert.equal(await host.evaluate(e=>Reflect.get(e,'__art06Incumbent')),'same')
 await expect(page.getByText('No physical arrival is implied.',{exact:true})).toBeVisible()
 await page.getByRole('button',{name:'Back to previous panel',exact:true}).click();await expect(page).toHaveURL(/\/world(?:\?|$)/);await settled(page)
 result.worldAfter=await page.evaluate(()=>window.__ART06_DEFAULT__.worldProof());assert.deepEqual(result.worldAfter,worldBefore,'Current App World RGBA/objects/camera invariant after Map and feature routes')
 assert.deepEqual(errors,[])
 await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Me',exact:true}).click();await page.getByText('Profile options',{exact:true}).click();await page.getByRole('link',{name:'Account',exact:true}).click();await page.getByRole('button',{name:'Sign out',exact:true}).click();await page.getByRole('button',{name:'Confirm sign out',exact:true}).click()
 await expect.poll(async()=>{const s=await state(page);return s.activeSockets===0&&s.canvases===0&&s.urls.created.every(u=>s.urls.revoked.includes(u))}).toBe(true)
 result.cleanup=await state(page);result.status='PASS';await context.close();context=null;await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n')
 }
 report.status='PASS'
}catch(error){report.status='FAIL';report.error=String(error.stack??error);report.failureState=await context?.pages()[0]?.evaluate(()=>({state:window.__ART06_DEFAULT__?.state(),text:document.body.innerText})).catch(()=>null);process.exitCode=1}
finally{await context?.close();await browser?.close();await server?.close();report.finishedAt=new Date().toISOString();await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n')}
