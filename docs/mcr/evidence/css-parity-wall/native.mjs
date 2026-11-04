// CSS-PARITY-WALL: adapted read-only qualification from reference-target/browser.mjs at 54320d71.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, rm, stat, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createRequire } from 'node:module'
import { pathToFileURL } from 'node:url'
const require = createRequire(resolve(import.meta.dirname, '../../../../frontend/package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')))
const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
const { chromium, expect } = require('@playwright/test')
const root = resolve(import.meta.dirname, '../../../..'), out = resolve(import.meta.dirname, process.env.WALL_PARITY_PHASE ?? 'candidate'), cache = resolve(process.env.TMPDIR ?? resolve(import.meta.dirname, '.native-cache'), 'vite')
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
async function bytes(path) { let total=0; for(const entry of await readdir(path,{withFileTypes:true}).catch(()=>[])){if(entry.isSymbolicLink())continue;const child=resolve(path,entry.name);total+=entry.isDirectory()?await bytes(child):(await stat(child).catch(()=>({size:0}))).size}return total }
async function disk() { const f = await statfs(root); assert(f.bavail * f.bsize >= 8 * 1024 ** 3, '8GiB disk floor: pause captures/builds');assert(await bytes(resolve(import.meta.dirname))<=4*1024**2,'Durable evidence cap 4MiB');if(process.env.TMPDIR)assert(await bytes(process.env.TMPDIR)<=300*1024**2,'Private temporary cap 300MiB') }
await disk(); await mkdir(out, { recursive: true })
const reference = process.env.WALL_REFERENCE_PATH ?? '/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/AB983BDC-49A0-4F5E-A56F-9F45A5560B6D/1-Photo-1.jpg'
assert.equal(hash(await readFile(reference)), '2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23')
const report = { result: 'FAIL', base: execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),
  reference: { sha256: hash(await readFile(reference)), crop: 'bottom row fourth WALL; new JPG 1280x853: Wall panel x773 y450 w237 h397; canvas x779 y510 w222 h253', canvasAspect: 253/222 },
  externalInput: {rootCaption:'1dbca746018bdd6d3470cacd9112fb7e7d7aca88',localCaptionCherryPick:'87c95968605b8da3a8c35b170f7533c89787ca3e',baselineCss:'54320d71c4ae0bff3524d1b500cea772e0ba42e0'},
  scope: 'Production MuralSyncPanel/MuralEditor actual raster and canonical HTTP, frozen deterministic fixture; no production-root, liveBE, physical-device or IN04 claim.', sourceHashes: {}, widths: [], checks: [] }
for (const path of ['editor/MuralEditor.tsx','editor/editor.css','editor/controller.ts','editor/input.ts','editor/contract.ts','sync/MuralSyncPanel.tsx','sync/controller.ts','sync/http.ts','sync/contract.ts','sync/__tests__/fixture.ts','editor/__tests__/fixture.ts','editor/__tests__/reference-target/presentation.test.tsx','editor/__tests__/reference-target/preview.tsx','editor/__tests__/reference-target/preview.css','editor/__tests__/reference-target/preview.html','editor/__tests__/reference-target/browser.mjs'].map(s=>'frontend/src/features/mural/'+s).concat(['frontend/src/shared/ui/concert-window/index.tsx','frontend/src/shared/ui/concert-window/concert-window.css','contracts/domain/v1/mural.schema.json'])) report.sourceHashes[path] = hash(path.endsWith('/editor/editor.css') && process.env.WALL_PARITY_PHASE === 'baseline' ? execFileSync('git',['show','54320d71:'+path],{cwd:root}) : await readFile(resolve(root,path)))
const baselineCss = process.env.WALL_PARITY_PHASE === 'baseline' ? execFileSync('git', ['show', '54320d71:frontend/src/features/mural/editor/editor.css'], {cwd:root, encoding:'utf8'}) : null
const server = await createServer({ configFile:false, plugins:[{name:'wall-parity-baseline',enforce:'pre',load(id){if(baselineCss && id.split('?')[0] === resolve(root,'frontend/src/features/mural/editor/editor.css')) return baselineCss}},react()], root: resolve(root,'frontend'), cacheDir: cache, logLevel:'error', server:{host:'127.0.0.1',port:5299,strictPort:true,hmr:false,watch:null} })
let browser
try {
  await server.listen(); browser = await chromium.launch({ executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true }); report.browser = browser.version()
  for (const width of (process.env.WALL_PARITY_PHASE === 'baseline' ? [390] : [360,390,430])) {
    await disk()
    const page = await browser.newPage({viewport:{width,height:844},hasTouch:true,isMobile:true,deviceScaleFactor:1,reducedMotion:'reduce'}), errors=[]
    page.setDefaultTimeout(7000); page.on('pageerror',e=>errors.push(e.message))
    const surface=page.getByRole('group',{name:'Mural pixel preview'}), status=page.getByRole('status',{name:'Wall command status'}), palette=page.getByRole('group',{name:'Wall palette'}), syncStatus=page.getByRole('status',{name:'Wall synchronization status'})
    const counts=()=>page.evaluate(()=>window.__WALL_REFERENCE__.counts())
    async function open() { await page.goto('http://127.0.0.1:5299/src/features/mural/editor/__tests__/reference-target/preview.html'); await expect(syncStatus).toContainText('revision 10'); await expect(surface).toBeVisible(); await expect(page.getByRole('heading',{name:'THE BLACK PARADE',exact:true})).toBeVisible() }
    async function layout() {
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'page overflow')
      assert(await page.locator('button,input,summary').evaluateAll(es=>es.filter(e=>e.getClientRects().length).every(e=>{const r=e.getBoundingClientRect();return r.width>=44&&r.height>=44})),'control below44px')
    }
    async function capture(name) { await disk(); await layout(); if (width === 390 || name === 'loaded') await page.screenshot({path:resolve(out,`${width}-${name}.png`),fullPage:true}) }
    await open(); await layout()
    report.sourceHashes['docs/mcr/evidence/css-parity-wall/native.mjs']=hash(await readFile(resolve(import.meta.dirname,'native.mjs')))
    const geometry=await page.evaluate(()=>{const p=document.querySelector('.mcr-mural').getBoundingClientRect(),c=document.querySelector('canvas').getBoundingClientRect(),t=document.querySelector('.mcr-mural__toolbar').getBoundingClientRect(),a=document.querySelector('.mcr-mural__palette-frame').getBoundingClientRect();return {panel:{width:p.width,height:p.height},canvas:{top:c.top-p.top,width:c.width,height:c.height},tools:{right:p.right-t.right,top:t.top-p.top,width:t.width},palette:{top:a.top-p.top,height:a.height},ratio:c.height/p.height}})
    assert(geometry.canvas.top<=90,'canvas pushed below photo-derived heading allowance'); assert(geometry.ratio>=.55,'canvas not dominant'); assert(geometry.canvas.height/geometry.canvas.width>1.06,'portrait viewport missing')
    assert(geometry.tools.width===44 && geometry.tools.right<=14,'right tool stack'); assert(geometry.palette.height<=75,'palette expanded into rows')
    assert.equal(await page.getByRole('button',{name:/^Color \d+ #/}).count(),16)
    assert(await palette.evaluate(e=>e.scrollWidth>e.clientWidth),'16 full-size colors must remain scrollable')
    await palette.focus(); await page.keyboard.press('End'); assert(await palette.evaluate(e=>e.scrollLeft>0)); await page.keyboard.press('Home'); assert.equal(await palette.evaluate(e=>e.scrollLeft),0)
    await page.getByRole('button',{name:'Color 15 #FFFFFF',exact:true}).focus(); if(width===390) await capture('palette-focus'); await page.keyboard.press('Enter'); await expect(page.getByRole('button',{name:'Color 15 #FFFFFF',exact:true})).toHaveAttribute('aria-pressed','true')
    await palette.focus(); await page.keyboard.press('Home'); await page.getByRole('button',{name:'Color 2 #A22635',exact:true}).click()
    // Check all16384 cell centers against the current snapshot; portrait margins
    // and every draw/hit-test share a uniform width-based scale.
    assert(await page.evaluate(()=>{const c=document.querySelector('canvas'),ctx=c.getContext('2d'),s=window.__WALL_REFERENCE__.read().editor.snapshot,offset=(c.height-512)/2;for(let y=0;y<128;y++)for(let x=0;x<128;x++){const a=ctx.getImageData(x*4+2,Math.floor(offset+y*4+2),1,1).data,h=s.palette[s.pixels[y*128+x]];if(a[0]!==parseInt(h.slice(1,3),16)||a[1]!==parseInt(h.slice(3,5),16)||a[2]!==parseInt(h.slice(5,7),16)||a[3]!==255)return false}return true}),'raster differs from snapshot')
    await capture('loaded'); await page.keyboard.press('Tab'); await surface.focus(); await expect(surface).toBeFocused(); assert.equal(await surface.evaluate(e=>getComputedStyle(e).outlineStyle),'solid'); await capture('focus'); await page.locator('.mcr-mural__heading').click(); if(width===390){await page.emulateMedia({forcedColors:'active'});await page.keyboard.press('Tab');await surface.focus();await capture('forced-focus');assert.equal(await surface.evaluate(e=>matchMedia('(forced-colors: active)').matches),true);await page.emulateMedia({forcedColors:'none'});await page.locator('.mcr-mural__heading').click()}
    const bounds=await surface.boundingBox(), gridY=(bounds.height-bounds.width)/2
    await surface.click({position:{x:bounds.width*(127.5/128),y:gridY+bounds.width*(127.5/128)}})
    await expect(page.getByLabel('Selection preview')).toContainText('(127, 127)'); assert.equal((await counts()).posts,0)
    await page.getByRole('button',{name:'Cancel preview'}).click(); await expect(page.getByLabel('Selection preview')).toHaveCount(0)
    await page.getByRole('button',{name:'Zoom in',exact:true}).click(); await expect(page.getByLabel('Zoom level')).toHaveText('2×')
    await surface.click({position:{x:bounds.width/2+bounds.width/128,y:bounds.height/2+bounds.width/128}}); await expect(page.getByLabel('Selection preview')).toContainText('(64, 64)')
    await page.getByRole('button',{name:'Cancel preview'}).click()
    await page.getByRole('button',{name:'Pan wall',exact:true}).click(); await surface.focus(); await page.keyboard.press('ArrowRight'); assert.equal((await counts()).posts,0)
    await page.getByRole('button',{name:'Fit wall'}).click(); await expect(page.getByLabel('Zoom level')).toHaveText('1×')
    await page.getByRole('button',{name:'Pencil preview'}).click(); await surface.focus(); await page.keyboard.press('ArrowRight'); await page.keyboard.press('Enter'); assert.equal((await counts()).posts,0)
    await page.keyboard.press('Escape'); await expect(page.getByLabel('Selection preview')).toHaveCount(0)
    // Real pointer drag pans; never turns into a placement or a preview.
    await page.mouse.move(bounds.x+bounds.width/2,bounds.y+bounds.height/2); await page.mouse.down(); await page.mouse.move(bounds.x+bounds.width/2+30,bounds.y+bounds.height/2+20,{steps:5}); await page.mouse.up()
    assert.equal((await counts()).posts,0); await expect(page.getByLabel('Selection preview')).toHaveCount(0)
    await page.getByText('Coordinates & help',{exact:true}).click(); await expect(page.getByRole('spinbutton',{name:'X',exact:true})).toBeVisible(); await expect(page.locator('.mcr-mural__details')).toContainText('This wall is 128 × 128 pixels.')
    await page.getByRole('spinbutton',{name:'X',exact:true}).fill('127'); await page.getByRole('spinbutton',{name:'Y',exact:true}).fill('0'); await page.getByRole('button',{name:'Preview pixel',exact:true}).click()
    await expect(page.getByLabel('Selection preview')).toContainText('(127, 0)'); assert.equal((await counts()).posts,0)
    await page.getByText('Coordinates & help',{exact:true}).click(); await capture('preview')
    await page.getByRole('button',{name:'Confirm pixel',exact:true}).focus(); await page.keyboard.press('Enter'); await expect(status).toContainText('Server confirmed'); assert.equal((await counts()).posts,1)
    await expect(page.getByLabel('Selection preview')).toHaveCount(0); assert.equal(await page.evaluate(()=>window.__WALL_REFERENCE__.read().editor.snapshot.pixels[127]),2)
    await page.evaluate(()=>window.__WALL_REFERENCE__.cooldown()); await expect(page.locator('.mcr-mural__meta')).toContainText('Cooldown: 60s'); await capture('cooldown')

    await open(); await page.evaluate(()=>window.__WALL_REFERENCE__.mode('lost')); await surface.focus(); await page.keyboard.press('ArrowRight'); await page.getByRole('button',{name:'Confirm pixel',exact:true}).click()
    await expect(status).toContainText('unknown'); const original=await page.evaluate(()=>window.__WALL_REFERENCE__.read().editor.command.key)
    await page.getByRole('button',{name:'Refresh confirmed wall'}).click(); await expect(syncStatus).toContainText('revision 11')
    assert.equal(await page.evaluate(()=>window.__WALL_REFERENCE__.read().editor.command.key),original); await expect(page.getByRole('button',{name:'Confirm pixel',exact:true})).toBeDisabled()
    await capture('unknown'); await page.getByRole('button',{name:'Check original command'}).click(); await expect(status).toContainText('Server confirmed'); assert.equal((await counts()).posts,1)
    await page.evaluate(()=>window.__WALL_REFERENCE__.restore('FROZEN')); await expect(status).toContainText('frozen'); await expect(page.getByRole('button',{name:'Pencil preview'})).toBeDisabled(); await expect(page.getByRole('button',{name:'Pan wall'})).toBeEnabled(); await capture('frozen')
    await open(); await page.evaluate(()=>window.__WALL_REFERENCE__.mode('pending')); await surface.focus(); await page.keyboard.press('ArrowRight'); await page.getByRole('button',{name:'Confirm pixel',exact:true}).click()
    await expect(status).toContainText('pending'); await page.getByRole('button',{name:'Check original command'}).click(); await expect(status).toContainText('pending'); assert.equal((await counts()).posts,1)
    await page.evaluate(()=>window.__WALL_REFERENCE__.mode('ack')); await page.getByRole('button',{name:'Check original command'}).click(); await expect(status).toContainText('Server confirmed'); assert.equal((await counts()).posts,1)
    await open(); await page.evaluate(()=>window.__WALL_REFERENCE__.restore('PAUSED')); await expect(status).toContainText('paused'); await expect(page.getByRole('button',{name:'Pencil preview'})).toBeDisabled()
    await page.evaluate(()=>window.__WALL_REFERENCE__.readonly()); await expect(status).toContainText('read-only'); await expect(page.getByRole('button',{name:'Pencil preview'})).toBeDisabled()
    await page.setViewportSize({width,height:420}); await page.evaluate(()=>document.documentElement.style.setProperty('--mcr-wall-viewport-height','330px'))
    const small=await surface.boundingBox(); assert(small.height>=small.width,'short viewport must not crop fit-grid')
    await page.getByText('Coordinates & help',{exact:true}).click(); await page.getByRole('button',{name:'Refresh confirmed wall'}).scrollIntoViewIfNeeded(); await layout()
    await page.setViewportSize({width,height:844}); await page.evaluate(()=>document.documentElement.style.removeProperty('--mcr-wall-viewport-height'))
    await page.evaluate(()=>window.__WALL_REFERENCE__.logout()); await expect(page.locator('canvas')).toHaveCount(0); await expect(page.getByRole('button',{name:/^Color \d+ #/})).toHaveCount(0)
    await layout(); assert.deepEqual(errors,[]); report.widths.push({width,geometry,errors}); await page.evaluate(()=>window.__WALL_REFERENCE__.dispose()); await page.close()
  }
  report.result='PASS'; report.capturePolicy='390px loaded/focus/forced-focus/palette-focus/preview/cooldown/unknown/frozen; 360/430 loaded; same interaction checks all widths'; report.checks=['portrait actualcanvas dominant >=55% of idle panel; top<=90px (new photo scaled top approximately99px at390); right44pxtools; one framed palette row', 'all16384 RGBA cells match frozen server snapshot; uniform squarepixel scale, fit corner and zoomcenter pointer mapping', 'all16 server colors reachable by horizontal scroll/keyboard; selected state retained; no page overflow; >=44px controls', 'pencil/pan/zoom/fit and pointerdrag never place; keyboard Escape cancels; coordinates expandable; explicit confirmation onePOST', 'real servercooldown60s; unknown command retained across snapshotGET; status/resultGET recovery no replay; pending202 remains blocked then recovers original', 'frozen/paused/read-only keep viewingtools; privacy clears raster/palette/selection; 420px height and rootheightvariable retain fit-grid and reachable recovery; no page errors']
} catch(error) { report.failure=String(error.stack ?? error); throw error } finally { await browser?.close(); await server.close(); await rm(cache,{recursive:true,force:true}); await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n') }
console.log(JSON.stringify({result:report.result,browser:report.browser,widths:report.widths,checks:report.checks},null,2))
