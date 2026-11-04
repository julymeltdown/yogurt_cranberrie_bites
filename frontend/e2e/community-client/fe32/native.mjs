// FE32 owner runner adapted from retained reference-target/browser.mjs; preserves old evidence.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, rm, stat, statfs, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../..'), out = resolve(root, 'docs/mcr/evidence/fe32-client/native'), cache = `/tmp/mcr-fe32-native-${process.pid}`
const slot=process.env.MCR_NATIVE_SLOT; assert(slot?.startsWith('FE32-NATIVE-'),'Explicit named coordinator slot required')
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
async function bytes(path) { let total=0; for(const e of await readdir(path,{withFileTypes:true}).catch(()=>[])){const p=resolve(path,e.name);total+=e.isDirectory()?await bytes(p):(await stat(p)).size} return total }
async function disk(start=false) { const f = await statfs(root); assert(f.bavail * f.bsize >= (start?16:8) * 1024 ** 3, '8GiB disk floor: pause captures/builds') }
await disk(true); await mkdir(out, { recursive: true })
const startingDisk=await statfs(root)
const reference = process.env.WALL_REFERENCE_PATH ?? '/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/327C1E99-C917-4D55-96F7-C83C59A70F90/1-Photo-1.jpg'
assert.equal(hash(await readFile(reference)), '2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23')
const report = { result: 'FAIL', slot, diskFreeGiB: startingDisk.bavail*startingDisk.bsize/1024**3, base: execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),
  reference: { sha256: hash(await readFile(reference)), crop: 'bottom row fourth WALL; approx canvas274x304, frame293x474', canvasAspect: 304/274 },
  scope: 'Production MuralSyncPanel/MuralEditor actual raster and canonical HTTP, frozen deterministic fixture; no production-root, liveBE, physical-device or IN04 claim.', sourceHashes: {}, widths: [], checks: [] }
for (const path of ['editor/MuralEditor.tsx','editor/editor.css','editor/controller.ts','editor/input.ts','editor/contract.ts','sync/MuralSyncPanel.tsx','sync/controller.ts','sync/http.ts','sync/contract.ts','sync/__tests__/fixture.ts','editor/__tests__/fixture.ts','editor/__tests__/reference-target/preview.tsx','editor/__tests__/reference-target/preview.css'].map(s=>'frontend/src/features/mural/'+s).concat(['frontend/src/shared/ui/concert-window/index.tsx','frontend/src/shared/ui/concert-window/concert-window.css','contracts/domain/v1/mural.schema.json','contracts/domain/v1/operations.json','frontend/e2e/community-client/fe32/preview.tsx','frontend/e2e/community-client/fe32/preview.css','frontend/e2e/community-client/fe32/preview.html','frontend/e2e/community-client/fe32/native.mjs'])) report.sourceHashes[path] = hash(await readFile(resolve(root,path)))
const server = await createServer({ root: resolve(root,'frontend'), cacheDir: cache, logLevel:'error', server:{host:'127.0.0.1',port:5302,strictPort:true} })
let browser
try {
  await server.listen(); browser = await chromium.launch({ executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true }); report.browser = browser.version()
  for (const width of [360,390,430]) {
    await disk()
    const page = await browser.newPage({viewport:{width,height:844},hasTouch:true,isMobile:true,deviceScaleFactor:1,reducedMotion:'reduce'}), errors=[]
    page.setDefaultTimeout(7000); page.on('pageerror',e=>errors.push(e.message))
    const surface=page.getByRole('group',{name:'Mural pixel preview'}), status=page.getByRole('status',{name:'Wall command status'}), palette=page.getByRole('group',{name:'Wall palette'}), syncStatus=page.getByRole('status',{name:'Wall synchronization status'})
    const counts=()=>page.evaluate(()=>window.__FE32_CLIENT__.counts())
    async function open() { await page.goto('http://127.0.0.1:5302/e2e/community-client/fe32/preview.html'); await expect(syncStatus).toContainText('revision 10'); await expect(surface).toBeVisible() }
    async function layout() {
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'page overflow')
      assert(await page.locator('button,input,summary').evaluateAll(es=>es.filter(e=>e.getClientRects().length).every(e=>{const r=e.getBoundingClientRect();return r.width>=44&&r.height>=44})),'control below44px')
    }
    async function capture(name) { await disk(); assert(await bytes(out)+await bytes(cache)<0.2*1024**3,'0.2GiB hard new-output ceiling'); await layout(); await page.screenshot({path:resolve(out,`${width}-${name}.png`),fullPage:true}) }
    await open(); await layout()
    const geometry=await page.evaluate(()=>{const p=document.querySelector('.mcr-mural').getBoundingClientRect(),c=document.querySelector('canvas').getBoundingClientRect(),t=document.querySelector('.mcr-mural__toolbar').getBoundingClientRect(),a=document.querySelector('.mcr-mural__palette-frame').getBoundingClientRect();return {panel:{width:p.width,height:p.height},canvas:{top:c.top-p.top,width:c.width,height:c.height},tools:{right:p.right-t.right,top:t.top-p.top,width:t.width},palette:{top:a.top-p.top,height:a.height},ratio:c.height/p.height}})
    assert(geometry.canvas.top<=75,'canvas pushed down'); assert(geometry.ratio>=.55,'canvas not dominant'); assert(geometry.canvas.height/geometry.canvas.width>1.06,'portrait viewport missing')
    assert(geometry.tools.width===44 && geometry.tools.right<=14,'right tool stack'); assert(geometry.palette.height<=75,'palette expanded into rows')
    assert.equal(await page.getByRole('button',{name:/^Color \d+ #/}).count(),16)
    assert(await palette.evaluate(e=>e.scrollWidth>e.clientWidth),'16 full-size colors must remain scrollable')
    await palette.focus(); await page.keyboard.press('End'); assert(await palette.evaluate(e=>e.scrollLeft>0)); await page.keyboard.press('Home'); assert.equal(await palette.evaluate(e=>e.scrollLeft),0)
    await page.getByRole('button',{name:'Color 15 #FFFFFF',exact:true}).focus(); await page.keyboard.press('Enter'); await expect(page.getByRole('button',{name:'Color 15 #FFFFFF',exact:true})).toHaveAttribute('aria-pressed','true')
    await palette.focus(); await page.keyboard.press('Home'); await page.getByRole('button',{name:'Color 2 #A22635',exact:true}).click()
    // Check all16384 cell centers against the current snapshot; portrait margins
    // and every draw/hit-test share a uniform width-based scale.
    assert(await page.evaluate(()=>{const c=document.querySelector('canvas'),ctx=c.getContext('2d'),s=window.__FE32_CLIENT__.read().editor.snapshot,offset=(c.height-512)/2;for(let y=0;y<128;y++)for(let x=0;x<128;x++){const a=ctx.getImageData(x*4+2,Math.floor(offset+y*4+2),1,1).data,h=s.palette[s.pixels[y*128+x]];if(a[0]!==parseInt(h.slice(1,3),16)||a[1]!==parseInt(h.slice(3,5),16)||a[2]!==parseInt(h.slice(5,7),16)||a[3]!==255)return false}return true}),'raster differs from snapshot')
    await capture('loaded')
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
    const cdp=await page.context().newCDPSession(page), contacts=[1,2,3,4].map((id,i)=>({id,x:bounds.x+60+i*24,y:bounds.y+bounds.height/2,radiusX:2,radiusY:2,force:1}))
    const touch=async(type,ids)=>cdp.send('Input.dispatchTouchEvent',{type,touchPoints:ids.map(i=>contacts[i-1])})
    await touch('touchStart',[1]);await touch('touchStart',[1,2]);await touch('touchStart',[1,2,3]);await touch('touchEnd',[2,3]);await touch('touchEnd',[3]);await touch('touchStart',[3,4]);await touch('touchEnd',[3]);await touch('touchEnd',[])
    await expect(page.getByLabel('Selection preview')).toHaveCount(0);assert.equal((await counts()).posts,0)
    await touch('touchStart',[1]); contacts[0].x+=36; await touch('touchMove',[1]); contacts[0].x-=36; await touch('touchMove',[1]); await touch('touchEnd',[]); await expect(page.getByLabel('Selection preview')).toHaveCount(0); assert.equal((await counts()).posts,0)
    await touch('touchStart',[1]); await touch('touchCancel',[]); await expect(page.getByLabel('Selection preview')).toHaveCount(0)
    await cdp.detach()
    // A clean touch tap after all contacts end is a preview, never a POST.
    await page.touchscreen.tap(bounds.x+70,bounds.y+bounds.height/2)
    await expect(page.getByLabel('Selection preview')).toBeVisible();assert.equal((await counts()).posts,0)
    await page.getByRole('button',{name:'Cancel preview'}).tap()
    await page.getByText('Coordinates & help',{exact:true}).click(); await expect(page.getByRole('spinbutton',{name:'X',exact:true})).toBeVisible()
    await page.getByRole('spinbutton',{name:'X',exact:true}).fill('128'); await page.getByRole('button',{name:'Preview pixel',exact:true}).click(); assert.equal(await page.getByRole('spinbutton',{name:'X',exact:true}).evaluate(e=>e.validity.rangeOverflow),true); assert.equal((await counts()).posts,0); await expect(page.getByLabel('Selection preview')).toHaveCount(0);
    await page.getByRole('spinbutton',{name:'X',exact:true}).fill('127'); await page.getByRole('spinbutton',{name:'Y',exact:true}).fill('0'); await page.getByRole('button',{name:'Preview pixel',exact:true}).click()
    await expect(page.getByLabel('Selection preview')).toContainText('(127, 0)'); assert.equal((await counts()).posts,0)
    await page.getByText('Coordinates & help',{exact:true}).click(); await capture('preview')
    await page.getByRole('button',{name:'Confirm pixel',exact:true}).focus(); await page.keyboard.press('Enter'); await expect(status).toContainText('Server confirmed'); assert.equal((await counts()).posts,1)
    await expect(page.getByLabel('Selection preview')).toHaveCount(0); assert.equal(await page.evaluate(()=>window.__FE32_CLIENT__.read().editor.snapshot.pixels[127]),2)
    await page.evaluate(()=>window.__FE32_CLIENT__.cooldown()); await expect(page.locator('.mcr-mural__meta')).toContainText('Cooldown: 60s'); await surface.focus(); await page.keyboard.press('ArrowRight'); await expect(page.getByRole('button',{name:'Confirm pixel',exact:true})).toBeDisabled(); await capture('cooldown'); await page.evaluate(()=>window.__FE32_CLIENT__.advance(60000)); await expect(page.getByRole('button',{name:'Confirm pixel',exact:true})).toBeEnabled()

    await open(); await page.evaluate(()=>window.__FE32_CLIENT__.mode('lost')); await surface.focus(); await page.keyboard.press('ArrowRight'); await page.getByRole('button',{name:'Confirm pixel',exact:true}).click()
    await expect(status).toContainText('unknown'); const original=await page.evaluate(()=>window.__FE32_CLIENT__.read().editor.command.key)
    await page.getByRole('button',{name:'Refresh confirmed wall'}).click(); await expect(syncStatus).toContainText('revision 11')
    assert.equal(await page.evaluate(()=>window.__FE32_CLIENT__.read().editor.command.key),original); await expect(page.getByRole('button',{name:'Confirm pixel',exact:true})).toBeDisabled()
    await capture('unknown'); await page.getByRole('button',{name:'Check original command'}).click(); await expect(status).toContainText('Server confirmed'); assert.equal((await counts()).posts,1)
    await page.evaluate(()=>window.__FE32_CLIENT__.restore('FROZEN')); await expect(status).toContainText('frozen'); await expect(page.getByRole('button',{name:'Pencil preview'})).toBeDisabled(); await expect(page.getByRole('button',{name:'Pan wall'})).toBeEnabled(); await capture('frozen')
    await open(); await page.evaluate(()=>window.__FE32_CLIENT__.mode('pending')); await surface.focus(); await page.keyboard.press('ArrowRight'); await page.getByRole('button',{name:'Confirm pixel',exact:true}).click()
    await expect(status).toContainText('pending'); await page.getByRole('button',{name:'Check original command'}).click(); await expect(status).toContainText('pending'); assert.equal((await counts()).posts,1)
    await page.evaluate(()=>window.__FE32_CLIENT__.mode('ack')); await page.getByRole('button',{name:'Check original command'}).click(); await expect(status).toContainText('Server confirmed'); assert.equal((await counts()).posts,1)
    await open(); await page.evaluate(()=>window.__FE32_CLIENT__.mode('rejected')); await surface.focus(); await page.keyboard.press('ArrowRight'); await page.getByRole('button',{name:'Confirm pixel',exact:true}).click(); await expect(status).toContainText('rejected'); await expect(page.getByLabel('Selection preview')).toHaveCount(0); assert.equal((await counts()).posts,1); assert.equal(await page.evaluate(()=>window.__FE32_CLIENT__.read().editor.snapshot.revision),10);
    await open(); await page.evaluate(()=>window.__FE32_CLIENT__.restore('PAUSED')); await expect(status).toContainText('paused'); await expect(page.getByRole('button',{name:'Pencil preview'})).toBeDisabled()
    await page.evaluate(()=>window.__FE32_CLIENT__.readonly()); await expect(status).toContainText('read-only'); await expect(page.getByRole('button',{name:'Pencil preview'})).toBeDisabled()
    await page.setViewportSize({width,height:420}); await page.evaluate(()=>document.documentElement.style.setProperty('--mcr-wall-viewport-height','330px'))
    const small=await surface.boundingBox(); assert(small.height>=small.width,'short viewport must not crop fit-grid')
    await page.getByText('Coordinates & help',{exact:true}).click(); await page.getByRole('button',{name:'Refresh confirmed wall'}).scrollIntoViewIfNeeded(); await layout()
    await page.setViewportSize({width,height:844}); await page.evaluate(()=>document.documentElement.style.removeProperty('--mcr-wall-viewport-height'))
    await page.evaluate(()=>window.__FE32_CLIENT__.logout()); await expect(page.locator('canvas')).toHaveCount(0); await expect(page.getByRole('button',{name:/^Color \d+ #/})).toHaveCount(0)
    await layout(); assert.deepEqual(errors,[]); report.widths.push({width,geometry,errors}); await page.evaluate(()=>window.__FE32_CLIENT__.dispose()); await page.close()
  }
  report.result='PASS'; report.checks=['portrait actualcanvas dominant >=55% of idle panel; top<=75px; right44pxtools; one framed palette row', 'all16384 RGBA cells match frozen server snapshot; uniform squarepixel scale, fit corner and zoomcenter pointer mapping', 'all16 server colors reachable by horizontal scroll/keyboard; selected state retained; no page overflow; >=44px controls', 'three-plus-contact ambiguity remains quarantined until all touches end; clean touch tap only previews; pencil/pan/zoom/fit and pointerdrag never place; keyboard Escape cancels; coordinates expandable; explicit confirmation onePOST', 'synthetic servercooldown60s blocks confirmation then expires against monotonic clock; terminal rejection removes preview without painting; invalid coordinate128 never previews;  unknown command retained across snapshotGET; status/resultGET recovery no replay; pending202 remains blocked then recovers original', 'frozen/paused/read-only keep viewingtools; privacy clears raster/palette/selection; 420px height and rootheightvariable retain fit-grid and reachable recovery; no page errors']
} finally { await browser?.close(); await server.close(); await rm(cache,{recursive:true,force:true}); await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n') }
console.log(JSON.stringify({result:report.result,browser:report.browser,widths:report.widths,checks:report.checks},null,2))
