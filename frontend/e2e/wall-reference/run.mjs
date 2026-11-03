import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { statfsSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import { chromium, expect } from '@playwright/test'

const root = fileURLToPath(new URL('../../../', import.meta.url)), owned = resolve(root, 'frontend/e2e/wall-reference')
const phase = process.argv.includes('--baseline') ? 'baseline' : 'candidate'
const out = resolve(owned, 'evidence', phase), hash = data => createHash('sha256').update(data).digest('hex')
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim()
const featurePaths=['frontend/src/features/mural/editor/MuralEditor.tsx','frontend/src/features/mural/editor/editor.css','frontend/src/features/mural/sync/MuralSyncPanel.tsx']
const pinnedSources=new Map(), consumedPins=new Set()
for(const [pin,paths] of [[process.env.WALL_REFERENCE_AUTHOR,featurePaths]]){
  if(!pin)continue
  assert(/^[0-9a-f]{40}$/.test(pin),'A full committed source revision is required')
  git('cat-file','-e',`${pin}^{commit}`)
  for(const path of paths)pinnedSources.set(path,execFileSync('git',['show',`${pin}:${path}`],{cwd:root,encoding:'utf8'}))
}
if(phase==='candidate')assert(process.env.WALL_REFERENCE_AUTHOR,'Candidate run requires final author handoff pin; no uncommitted source consumption')
function disk() { const fs = statfsSync(root); assert(fs.bavail * fs.bsize >= 8 * 1024 ** 3, '8GiB disk floor required') }
disk(); await mkdir(out, { recursive: true })
const report = { task: 'FE-REF-WALL-QA', reportVersion:2, phase, status: 'FAIL', fixtureOnly: true,
  scope: 'Native desktop Chrome mobile/touch emulation of actual MuralEditor/MuralSyncPanel with canonical memory session/privacy/HTTP/controllers and frozen C04 deterministic fixtures. No physical device, live login/backend authority, real two-client proof or IN04 acceptance.',
  base: 'f837bef676cd26869c26c172a699a91b7842d832', sourceRevision: git('rev-parse', 'HEAD'), referenceSha256: hash(await readFile(resolve(owned, 'reference.png'))),
  referenceRegion: { imageWidth: 1536, imageHeight: 1024, wall: { x: 925, y: 541, width: 290, height: 480 }, method: 'manual region annotation of supplied 10-screen image, bottom row fourth' },
  sourceHashes: {}, browser: '', node: process.version, widths: [], errors: [], developmentSockets: 0, developmentSocketEndpoints: [], captures: [], sourceDependencies: { mural: process.env.WALL_REFERENCE_AUTHOR ?? null, sharedPresentationBase: 'f837bef676cd26869c26c172a699a91b7842d832', rootOverride: false }, pinnedPaths: [...pinnedSources.keys()] }
assert.equal(report.referenceSha256, 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617')
for (const path of ['frontend/src/features/mural/editor/MuralEditor.tsx','frontend/src/features/mural/editor/editor.css','frontend/src/features/mural/editor/controller.ts','frontend/src/features/mural/editor/input.ts','frontend/src/features/mural/editor/contract.ts','frontend/src/features/mural/editor/__tests__/fixture.ts','frontend/src/features/mural/sync/MuralSyncPanel.tsx','frontend/src/features/mural/sync/controller.ts','frontend/src/features/mural/sync/contract.ts','frontend/src/features/mural/sync/projection.ts','frontend/src/features/mural/sync/http.ts','frontend/src/features/mural/sync/__tests__/fixture.ts','frontend/src/shared/api/http.ts','frontend/src/shared/query/privacy.ts','frontend/src/shared/ui/concert-window/index.tsx','frontend/src/shared/ui/concert-window/concert-window.css','frontend/src/shared/styles/base.css','contracts/domain/v1/mural.schema.json','contracts/generated/mcr-domain.openapi.json','frontend/e2e/wall-reference/index.html','frontend/e2e/wall-reference/fixture.tsx','frontend/e2e/wall-reference/host.css','frontend/e2e/wall-reference/run.mjs','frontend/e2e/wall-reference/check-types.mjs']) report.sourceHashes[path] = hash(pinnedSources.get(path) ?? await readFile(resolve(root,path)))
const server = await createServer({ configFile: false, root: resolve(root, 'frontend'), plugins: [{name:'qa-read-only-committed-source',enforce:'pre',load(id){const path=id.split('?')[0],relative=path.startsWith(root)?path.slice(root.length):null;if(relative && pinnedSources.has(relative)){consumedPins.add(relative);return pinnedSources.get(relative)}return null}},react()], cacheDir: resolve(owned,'.cache/vite'), logLevel: 'error', server: { host:'127.0.0.1', port:0, hmr:false, watch:null } })
let browser
async function state(page) { return page.evaluate(() => window.__WALL_REFERENCE__.read()) }
async function posts(page) { return page.evaluate(() => window.__WALL_REFERENCE__.counts().placements) }
async function coordinates(page, x, y) {
  const input = page.getByRole('spinbutton', { name:'X', exact:true, includeHidden:true })
  if (!await input.isVisible()) { await page.locator('.mcr-mural details > summary').click({timeout:5000}) }
  await input.fill(String(x)); await page.getByRole('spinbutton',{name:'Y',exact:true}).fill(String(y))
  await page.getByRole('button',{name:'Preview pixel',exact:true}).click()
}
async function metrics(page) {
  return page.evaluate(() => {
    const rect = e => { if (!e) return null; const r=e.getBoundingClientRect(); return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right} }
    const panel=document.querySelector('.mcr-mural'), surface=panel.querySelector('.mcr-mural__surface'), palette=panel.querySelector('.mcr-mural__palette')
    const buttons=[...panel.querySelectorAll('button,input,summary')].filter(e=>e.getClientRects().length).map(e=>({name:e.getAttribute('aria-label')||e.textContent.trim(),...rect(e)}))
    const zoom=[...panel.querySelectorAll('button')].find(e=>e.getAttribute('aria-label')==='Zoom in'||e.textContent.trim()==='Zoom in')
    const colors=[...panel.querySelectorAll('[aria-label^="Color "]')].map(rect)
    const status=panel.querySelector('[aria-label="Wall command status"]')
    return {panel:rect(panel),surface:rect(surface),palette:rect(palette),zoomIn:rect(zoom),status:rect(status),paletteRows:new Set(colors.map(r=>Math.round(r.y))).size,controls:buttons,documentWidth:document.documentElement.scrollWidth,viewportWidth:innerWidth}
  })
}
function visual(m) {
  return {canvasNearHeader:m.surface.y-m.panel.y<=100,canvasDominatesWidth:m.surface.width/m.panel.width>=0.80,
    toolsOnCanvasRight:m.zoomIn.x>=m.surface.right-60 && m.zoomIn.y>=m.surface.y && m.zoomIn.bottom<=m.surface.bottom+2,
    singlePaletteRow:m.paletteRows===1,paletteImmediatelyBelow:m.palette.y>=m.surface.bottom-2 && m.palette.y-m.surface.bottom<=20,
    statusBelowPalette:m.status.y>=m.palette.bottom-2,compactInitialPanel:m.panel.height<=760,portraitSurface:m.surface.height/m.surface.width>=1.07 && m.surface.height/m.surface.width<=1.14}
}
async function raster(page) {
  return page.locator('.mcr-mural__surface canvas').evaluate(canvas=>{
    const width=canvas.width,height=canvas.height,rect=canvas.getBoundingClientRect()
    const bytes=canvas.getContext('2d').getImageData(0,0,width,height).data
    // At Fit the 128-square raster uses a uniform width scale, centered in a portrait viewport.
    // Derive CSS and backing coordinates independently instead of forcing a 512-square bitmap.
    const top=(rect.height-rect.width)/2
    return Array.from({length:16384},(_,i)=>{
      const cssX=(i%128+0.5)*rect.width/128,cssY=top+(Math.floor(i/128)+0.5)*rect.width/128
      const x=Math.floor(cssX*width/rect.width),y=Math.floor(cssY*height/rect.height)
      const offset=(y*width+x)*4
      return Array.from(bytes.slice(offset,offset+4))
    })
  })
}
async function confirmedRaster(page) {
  await page.getByRole('button',{name:'Fit wall',exact:true}).click()
  await expect.poll(async()=>{
    const s=(await state(page)).editor.snapshot, pixels=await raster(page)
    return pixels.every((p,i)=>p[3]===255 && '#'+p.slice(0,3).map(v=>v.toString(16).padStart(2,'0')).join('').toUpperCase()===s.palette[s.pixels[i]])
  }).toBe(true)
}
async function capture(page,width,scenario) {
  disk(); const name=`${width}-${scenario}.png`, path=resolve(out,name)
  await page.locator('.mcr-mural').screenshot({path,animations:'disabled'})
  report.captures.push({width,scenario,path:name,sha256:hash(await readFile(path))})
}
try {
  await server.listen(); const origin=`http://127.0.0.1:${server.httpServer.address().port}`
  browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
  for (const width of [360,390,430]) {
    disk()
    const row={width,geometry:null,visual:null,checks:[],flows:[],status:'FAIL'}, context=await browser.newContext({viewport:{width,height:844},hasTouch:true,isMobile:true,deviceScaleFactor:1,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage()
    page.on('pageerror',e=>report.errors.push(e.message));page.on('websocket',socket=>{
      const url=new URL(socket.url()), expected=new URL(origin);expected.protocol='ws:'
      if(url.origin===expected.origin && url.pathname==='/'){report.developmentSockets++;if(!report.developmentSocketEndpoints.some(e=>e.origin===url.origin && e.pathname===url.pathname))report.developmentSocketEndpoints.push({origin:url.origin,pathname:url.pathname,kind:'same-origin Vite development client; query intentionally omitted'})}
      else report.errors.push('Unexpected non-development fixture WebSocket')
    })
    await page.route('**/*',route=>new URL(route.request().url()).origin===origin?route.continue():route.abort())
    const open=async scenario=>{
      await page.goto(`${origin}/e2e/wall-reference/index.html?scenario=${scenario}`)
      await expect(page.getByRole('status',{name:'Wall synchronization status'})).toContainText('revision 10')
    }
    try {
      await open('loaded');await confirmedRaster(page)
      row.confirmedColors=new Set((await raster(page)).map(p=>p.join(','))).size
      assert(row.confirmedColors>=3,'Actual fixture canvas must contain multiple authoritative colors, not an empty bitmap')
      row.geometry=await metrics(page);row.visual=visual(row.geometry);await capture(page,width,'loaded')
      assert.equal(row.geometry.documentWidth,width)
      for(const c of row.geometry.controls)assert(c.width>=43.9 && c.height>=43.9,`undersized target ${JSON.stringify(c)}`)
      row.checks.push('initial actual 128x128 canvas matches every confirmed RGBA pixel','all visible controls>=44px and no horizontal page overflow')
      await page.getByRole('button',{name:/^Color 2 #/}).click();await expect(page.getByRole('button',{name:/^Color 2 #/})).toHaveAttribute('aria-pressed','true')
      const lastColor=page.getByRole('button',{name:/^Color 15 #/})
      await lastColor.scrollIntoViewIfNeeded();await lastColor.focus();await page.keyboard.press('Enter')
      await expect(lastColor).toHaveAttribute('aria-pressed','true');assert.equal(await posts(page),0)
      await page.getByRole('button',{name:/^Color 2 #/}).click()
      row.checks.push('all16 palette colors retained; last horizontal swatch reachable by keyboard without POST')
      const pencil=page.getByRole('button',{name:/^Pencil$|^Pencil preview$|^Select pixel$|^Pixel preview$|^Pencil tool$/})
      if(await pencil.count())await pencil.click()
      const surface=page.getByRole('group',{name:'Mural pixel preview',exact:true}),confirm=page.getByRole('button',{name:'Confirm pixel',exact:true})
      await surface.focus();await surface.press('ArrowRight');await surface.press('ArrowDown');await surface.press('Enter')
      assert((await state(page)).editor.selection);assert.equal(await posts(page),0)
      assert(await surface.evaluate(e=>getComputedStyle(e).outlineStyle!=='none' && parseFloat(getComputedStyle(e).outlineWidth)>=2))
      await surface.press('Escape');assert.equal((await state(page)).editor.selection,null)
      row.checks.push('palette selects real color','arrow/Enter canvas preview does not POST; Escape cancels; keyboard focus visible')
      const viewport=await surface.boundingBox(),letterbox=(viewport.height-viewport.width)/2
      const geometry=await surface.locator('canvas').evaluate(c=>{const r=c.getBoundingClientRect();return {width:c.width,height:c.height,cssWidth:r.width,cssHeight:r.height}})
      assert(Math.abs(geometry.width/geometry.cssWidth-geometry.height/geometry.cssHeight)<=0.02,'No CSS stretching of square pixels')
      if(letterbox>3){
        await page.mouse.click(viewport.x+viewport.width/2,viewport.y+letterbox/2)
        assert.equal((await state(page)).editor.selection,null,'Blank letterbox must not select a pixel')
      }
      for(const [x,y] of [[0,0],[127,0],[0,127],[127,127]]){
        await page.mouse.click(viewport.x+(x+0.5)*viewport.width/128,viewport.y+letterbox+(y+0.5)*viewport.width/128)
        const selected=(await state(page)).editor.selection
        assert.equal(selected?.x,x);assert.equal(selected?.y,y);assert.equal(await posts(page),0)
        await surface.press('Escape')
      }
      row.checks.push('native physical corner clicks map to128-grid bounds; portrait letterbox selects nothing; square pixels are not CSS-stretched')
      await page.getByRole('button',{name:'Zoom in',exact:true}).click();await expect(page.getByLabel('Zoom level')).toHaveText('2×')
      const before=hash(JSON.stringify(await raster(page))), box=await surface.boundingBox()
      const pan=page.getByRole('button',{name:/^Pan wall$|^Pan$/})
      if(await pan.count())await pan.click()
      await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2+40,box.y+box.height/2-25,{steps:8});await page.mouse.up()
      assert.notEqual(hash(JSON.stringify(await raster(page))),before)
      await page.getByRole('button',{name:'Zoom out',exact:true}).click();await expect(page.getByLabel('Zoom level')).toHaveText('1×')
      await confirmedRaster(page);assert.equal(await posts(page),0)
      row.checks.push('actual raster changes on pan/zoom and fit restores authoritative pixels without POST')
      if(await pencil.count())await pencil.click()
      await coordinates(page,3,4);assert.equal(await posts(page),0);await capture(page,width,'preview')
      await confirm.focus();await page.keyboard.press('Enter')
      await expect(page.getByRole('status',{name:'Wall command status'})).toContainText('Server confirmed')
      assert.equal(await posts(page),1);assert.equal((await state(page)).editor.snapshot.pixels[4*128+3],2)
      assert.equal((await state(page)).remaining,5000);if(await confirm.count())await expect(confirm).toBeDisabled();else await expect(confirm).toHaveCount(0)
      row.checks.push('one explicit keyboard confirm makes exactly one canonical fixture HTTP POST','truthful 5s fixture cooldown disables new confirmation')
      row.flows.push({scenario:'explicitConfirm',placements:await posts(page),revision:(await state(page)).editor.snapshot.revision,remainingMs:(await state(page)).remaining})
      await capture(page,width,'cooldown')
      await page.setViewportSize({width,height:420});await coordinates(page,127,127)
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth))
      await page.setViewportSize({width,height:844})
      await page.evaluate(()=>window.__WALL_REFERENCE__.dispose())
      for(const scenario of ['pending','unknown']) {
        await open(scenario);await coordinates(page,7,8);await page.getByRole('button',{name:'Confirm pixel',exact:true}).click()
        await expect(page.getByRole('status',{name:'Original placement status'})).toContainText('unresolved')
        const original=(await state(page)).editor.command.key;assert.equal(await posts(page),1)
        await page.getByRole('button',{name:'Refresh confirmed wall',exact:true}).click();await expect(page.getByRole('status',{name:'Wall synchronization status'})).toContainText('revision 11')
        await page.getByRole('button',{name:'Check original command',exact:true}).click()
        assert.equal((await state(page)).editor.command.key,original);assert.equal(await posts(page),1)
        await capture(page,width,scenario)
        await page.evaluate(()=>window.__WALL_REFERENCE__.allowOriginal());await page.getByRole('button',{name:'Check original command',exact:true}).click()
        await expect(page.getByRole('status',{name:'Wall command status'})).toContainText('Server confirmed')
        assert.equal((await state(page)).editor.command,null);assert.equal(await posts(page),1)
        const requests=await page.evaluate(()=>window.__WALL_REFERENCE__.requests())
        assert(requests.some(r=>r.method==='GET' && r.path.endsWith(`/commands/${original}/result`)))
        row.checks.push(`${scenario}: snapshot equality cannot ACK; explicit status/original-result GET resolves captured key with no POST replay`)
        row.flows.push({scenario,originalKey:original,counts:await page.evaluate(()=>window.__WALL_REFERENCE__.counts()),requests})
        await page.evaluate(()=>window.__WALL_REFERENCE__.dispose())
      }
      for(const scenario of ['paused','frozen','read-only']) {
        await open(scenario);await capture(page,width,scenario)
        assert.equal((await state(page)).editor.selection,null);assert.equal(await posts(page),0)
        const button=page.getByRole('button',{name:'Confirm pixel',exact:true});if(await button.count())await expect(button).toBeDisabled()
        await expect(page.getByRole('button',{name:/^Color 2 #/})).toBeDisabled()
        assert((await page.getByRole('status',{name:'Wall command status'}).innerText()).toLowerCase().includes(scenario==='read-only'?'read-only':scenario))
        await page.evaluate(()=>window.__WALL_REFERENCE__.dispose())
      }
      row.checks.push('PAUSED/FROZEN/read-only fixture state forbids editing and retains truthful status')
      row.status='PASS'
    } catch(error) { row.error=String(error);report.errors.push(`${width}: ${String(error)}`) }
    finally {report.widths.push(row);await context.close()}
  }
  report.loadedPinnedPaths=[...consumedPins]
  for(const path of pinnedSources.keys())assert(consumedPins.has(path),`Pinned source was not actually loaded: ${path}`)
  const behaviorPass=report.widths.every(r=>r.status==='PASS') && report.errors.length===0
  const visualPass=report.widths.every(r=>Object.values(r.visual??{}).length===8 && Object.values(r.visual).every(Boolean))
  report.status=behaviorPass && (phase==='baseline'||visualPass)?'PASS':'FAIL'
  report.presentation=visualPass?'MEETS_STRUCTURAL_GEOMETRY_CHECKS':'REFERENCE_GEOMETRY_MISMATCH'
} finally {
  await browser?.close();await server.close()
  await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n')
}
console.log(JSON.stringify({phase,status:report.status,presentation:report.presentation,browser:report.browser,widths:report.widths.map(({width,status,visual,error})=>({width,status,visual,error})),errors:report.errors}))
if(report.status!=='PASS')process.exitCode=1
