// Native FE19/FE14/FE15 presentation proof over explicit synthetic fixtures.
import assert from 'node:assert/strict'
import { chromium, expect } from '@playwright/test'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { bundle, serve, fixturePath } from './build.mjs'
const root = resolve(import.meta.dirname, '../../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const paths = ['frontend/src/features/places/ui/Places.tsx','frontend/src/features/places/ui/places.css','frontend/src/features/places/model/controller.ts','frontend/src/features/places/model/contract.ts',
  'frontend/src/features/places/__tests__/fixtures/data.ts','frontend/src/features/places/__tests__/fixtures/preview.tsx','frontend/src/features/places/__tests__/places.test.ts','frontend/src/features/places/__tests__/places-ui.test.tsx',
  'frontend/src/features/places/__tests__/visual-adoption/preview.tsx','frontend/src/features/places/__tests__/visual-adoption/preview.html','frontend/src/features/places/__tests__/visual-adoption/build.mjs','frontend/src/features/places/__tests__/visual-adoption/browser.mjs',
  'frontend/src/shared/ui/concert-window/index.tsx','frontend/src/shared/ui/concert-window/concert-window.css','frontend/src/world/runtime.ts','frontend/src/world/input/controller.ts','frontend/src/world/camera/port.ts','frontend/src/world/map/resources.ts',
  'contracts/generated/mcr-core.openapi.json','contracts/assets/v1/map-manifest.schema.json','contracts/assets/v1/common.schema.json',
  'docs/mcr/design/ui-ux-reference-review.md','docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg']
const report = { result: 'FAIL', base: execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(), scope: 'UIV05 actual Places UI, existing shell, FE14 loader/Phaser and FE15 camera over synthetic C03/C06 fixtures. No live server/device/venue/approved map art/production world acceptance.',
  browser: '', sourceHashes: {}, widths: [], geometry: [], checks: [], screenshots: [], errors: [], cleanup: 'NOT_RUN' }
for (const path of paths) report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(root,path))).digest('hex')
await bundle(); const server = await serve(); let browser, stage = 'start'
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  const base = `http://127.0.0.1:${server.httpServer.address().port}`
  for (const [width,height] of [[360,800],[390,844],[430,932]]) {
    stage = `${width}/start`
    const context = await browser.newContext({ viewport:{width,height}, hasTouch:true, reducedMotion:'reduce', locale:'en-GB', timezoneId:'Asia/Seoul' }), page = await context.newPage()
    page.on('pageerror',error=>report.errors.push(error.message))
    const state = () => page.evaluate(()=>window.__UIV05_MAP__.state())
    const mode = value => page.evaluate(value=>window.__UIV05_MAP__.mode(value),value)
    const ready = async () => { await expect.poll(async()=>(await state()).runtime?.phase).toBe('ready'); await expect.poll(async()=>(await state()).places.list.phase).toBe('ready') }
    const top = () => page.locator('.shell-panel-body').evaluate(el=>{el.scrollTop=0})
    const shot = async name => { await top(); const file = `${name}-${width}.png`; await page.screenshot({path:resolve(evidence,file),fullPage:true}); report.screenshots.push(file) }
    const geometry = async name => {
      const result = await page.locator('.places-view').evaluate(element=>{
        const visible = [...element.querySelectorAll('button,a,input,summary')].filter(el=>el.checkVisibility())
        const diagram = element.querySelector('.places-diagram svg')?.getBoundingClientRect(), camera = element.querySelector('[aria-label="Virtual camera controls"]')?.getBoundingClientRect(), frame = element.getBoundingClientRect()
        return { pageWidth:document.documentElement.scrollWidth, innerWidth, titleBackground:getComputedStyle(element.querySelector('.concert-window__titlebar')).backgroundColor,
          diagramOffset:diagram ? diagram.top-frame.top : null, diagramHeight:diagram?.height, cameraGap:diagram&&camera ? camera.top-diagram.bottom : null,
          shortControls:visible.map(el=>({label:el.textContent,width:el.getBoundingClientRect().width,height:el.getBoundingClientRect().height})).filter(r=>r.width<43.9||r.height<43.9) }
      })
      report.geometry.push({width,state:name,...result}); assert(result.pageWidth<=width,JSON.stringify(result)); assert.deepEqual(result.shortControls,[])
      assert.equal(result.titleBackground,'rgb(27, 25, 23)'); return result
    }
    const recovered = async () => {
      const previous = (await state()).documentId
      await Promise.all([page.waitForEvent('load'),page.getByRole('button',{name:'Reload event',exact:true}).click()])
      await page.waitForFunction(()=>Boolean(window.__UIV05_MAP__)); await ready()
      assert.notEqual((await state()).documentId,previous); assert.equal((await state()).places.mapMismatch,false); assert.equal((await state()).canvases,1)
      await page.getByRole('button',{name:'Expand panel'}).click()
    }
    await page.goto(`${base}${fixturePath}`); await ready(); await page.getByRole('button',{name:'Expand panel'}).click()
    await expect(page.getByRole('heading',{name:'MAP',exact:true})).toBeVisible()
    const first = await geometry('loaded'); assert(first.diagramOffset<80); assert(first.diagramHeight>=150); assert(first.cameraGap>=0&&first.cameraGap<100)
    await expect(page.getByText(/Schema-derived virtual schematic/)).toBeVisible(); await shot('overview')
    assert.equal((await state()).canvases,1); assert.equal((await state()).mounts,1)
    const foot = (await state()).foot
    await page.getByRole('button',{name:'Zoom in',exact:true}).last().focus(); await page.keyboard.press('Enter')
    await expect.poll(async()=>(await state()).camera?.camera?.zoom).toBe(2)
    await expect(page.getByText('Camera updated in the virtual world. Your avatar has not moved.',{exact:true})).toBeVisible(); await shot('camera-status')
    await page.getByRole('button',{name:'Zoom out',exact:true}).last().click(); await expect.poll(async()=>(await state()).camera?.camera?.zoom).toBe(1)
    await page.getByRole('button',{name:'Show Synthetic reviewed landmark on virtual map'}).click(); await expect.poll(async()=>(await state()).camera?.following).toBe(false)
    await page.getByRole('button',{name:'Recenter avatar',exact:true}).click(); await expect.poll(async()=>(await state()).camera?.following).toBe(true)
    assert.deepEqual((await state()).foot,foot)
    const legend = page.locator('.places-legend-disclosure > summary'); await legend.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('region',{name:'Virtual and physical place legend'})).toBeVisible(); await page.keyboard.press('Enter')
    assert.equal(await legend.evaluate(el=>getComputedStyle(el).outlineWidth),'3px')
    const concept = page.getByRole('link',{name:'View Synthetic virtual stage'}); await concept.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('heading',{name:'Synthetic virtual stage',exact:true})).toBeVisible(); await expect(page.locator('#panel-title')).toBeFocused()
    await expect(page.getByText('This landmark exists in the virtual world only. Do not use it as a physical meeting destination.')).toBeVisible()
    await geometry('concept-detail'); await shot('concept-detail'); await page.goBack(); await expect(concept).toBeFocused()
    for (const [label,status] of [['Synthetic reviewed landmark','Verified physical reference — check current venue guidance'],['Synthetic unverified gate','Physical place — verification pending'],['Synthetic closed landmark','Closed — do not use as a meeting destination'],['Synthetic unavailable source','Physical reference — source unavailable']]) {
      stage = `${width}/${label}`
      await page.getByRole('link',{name:`View ${label}`}).click(); await expect(page.getByRole('heading',{name:label,exact:true})).toBeVisible()
      await expect(page.getByText(status,{exact:true})).toBeVisible(); await expect(page.getByRole('heading',{name:'Source and review'})).toBeVisible()
      const source = page.getByRole('link',{name:'Open source reference (new tab)'})
      if(label.includes('source')) { await expect(source).toHaveCount(0); await expect(page.getByText('Source link unavailable. No external destination is offered.')).toBeVisible() }
      else { await expect(source).toHaveAttribute('href','https://venue.example.invalid/reference'); await expect(source).toHaveAttribute('rel','noopener noreferrer'); await expect(source).toHaveAttribute('referrerpolicy','no-referrer') }
      if(label.includes('closed')) await expect(page.getByRole('button',{name:'Show on virtual map',exact:true})).toBeDisabled()
      await geometry(label); await shot(label.includes('unverified')?'unverified-detail':label.includes('closed')?'closed-detail':label.includes('source')?'source-unavailable-detail':'reviewed-detail')
      assert.equal(await page.locator('.places-view img').count(),0)
      await page.getByRole('link',{name:'All places',exact:true}).click()
    }
    await page.getByRole('button',{name:'Show Synthetic closed landmark on virtual map'}).isDisabled().then(value=>assert(value))
    await page.getByRole('searchbox',{name:'Find a place'}).fill('unverified'); await expect(page.getByText('1 place shown')).toBeVisible(); await shot('search')
    await page.getByRole('searchbox').fill('not present'); await expect(page.getByText('No places match. Try a different name.')).toBeVisible(); await shot('search-empty'); await page.getByRole('searchbox').fill('')
    for (const value of ['empty','error','denied','camera-unavailable','pending']) {
      stage = `${width}/${value}`; await mode(value)
      if(value==='empty') await expect(page.getByText('No places are published for this event yet.')).toBeVisible()
      if(value==='error') await expect(page.getByRole('heading',{name:'Places could not be loaded'})).toBeVisible()
      if(value==='denied') { await expect(page.getByRole('heading',{name:'Places are not available to this account'})).toBeVisible(); await expect(page.getByRole('button',{name:'Reload places',exact:true})).toHaveCount(0) }
      if(value==='camera-unavailable') {
        await expect(page.getByText('Live camera controls are unavailable. All listed places can still be read.')).toBeVisible()
        for(const name of ['Recenter avatar','Zoom out','Zoom in']) await expect(page.locator('.places-view').getByRole('button',{name,exact:true})).toBeDisabled()
        await expect(page.getByRole('button',{name:'Show Synthetic virtual stage on virtual map'})).toBeDisabled(); await expect(concept).toBeVisible()
      }
      if(value==='pending') await expect(page.getByText('Loading places…',{exact:true})).toBeVisible()
      await geometry(value); await shot(value); await mode('ok'); await ready()
    }
    stage = `${width}/mismatch`; await mode('mismatch')
    await expect(page.getByRole('heading',{name:'Your map and places do not match'})).toBeVisible()
    await expect(page.getByRole('button',{name:'Reload places',exact:true})).toHaveCount(0); await expect(page.getByRole('img',{name:/Virtual map overview/})).toHaveCount(0)
    await expect(page.getByRole('button',{name:'Show Synthetic virtual stage on virtual map'})).toBeDisabled(); await geometry('mismatch'); await shot('mismatch'); await recovered(); await shot('document-recovered')
    stage = `${width}/authoritative-latches`
    await page.getByRole('link',{name:'View Synthetic reviewed landmark'}).click(); await expect(page.getByRole('heading',{name:'Synthetic reviewed landmark',exact:true})).toBeVisible()
    await mode('removed'); await expect(page.getByRole('heading',{name:'This place is unavailable'})).toBeVisible(); await shot('removed-detail')
    await page.getByRole('link',{name:'All places',exact:true}).click(); await expect(page.getByRole('button',{name:'Show Synthetic reviewed landmark on virtual map'})).toBeDisabled()
    await expect(page.getByRole('button',{name:'Show Synthetic virtual stage on virtual map'})).toBeEnabled()
    await mode('ok'); await ready(); await expect(page.getByRole('button',{name:'Show Synthetic reviewed landmark on virtual map'})).toBeDisabled()
    await page.getByRole('link',{name:'View Synthetic reviewed landmark'}).click(); await expect(page.getByRole('button',{name:'Show on virtual map',exact:true})).toBeEnabled()
    await mode('detail-map-error'); await expect(page.getByRole('heading',{name:'Your map and places do not match'})).toBeVisible(); await page.getByRole('link',{name:'All places',exact:true}).click()
    await mode('ok'); await ready(); await expect(page.getByRole('button',{name:'Show Synthetic virtual stage on virtual map'})).toBeDisabled()
    await expect(page.locator('[aria-label="Virtual camera controls"]')).toHaveCount(0); await shot('latched-mismatch'); await recovered()
    await page.getByRole('link',{name:'Controls and meeting safety',exact:true}).click(); await expect(page.getByRole('heading',{name:'One world, at your pace'})).toBeVisible()
    await page.getByRole('link',{name:'Browse places',exact:true}).click(); await ready()
    assert.equal((await state()).mounts,1); assert.equal((await state()).canvases,1); assert.deepEqual((await state()).foot,foot)
    report.checks.push(`${width}: map-first frame, adjacent camera controls, 44px controls/no overflow, schema label/reference comparison; loaded/empty/error/denied/pending/search/closed/unverified/source-unavailable/camera-unavailable/mismatch; keyboard/back/focus; actual camera zoom/look/recenter with unchanged avatar; missing target latch + matching-detail reconciliation; release mismatch latch + full-document recovery; one canvas/owner through map/detail/help navigation`)
    await page.evaluate(()=>window.__UIV05_MAP__.dispose()); assert.equal(await page.locator('canvas').count(),0); assert.equal(await page.locator('.world-controls').count(),0)
    assert.equal((await state()).disposals,1); report.widths.push({width,height,result:'PASS',cleanup:'PASS'}); await context.close()
  }
  assert.deepEqual(report.errors,[]); report.result='PASS'
} catch(error) { report.failureStage=stage; report.failure={name:error.name,message:error.message}; process.exitCode=1 }
finally { await browser?.close(); await new Promise(done=>server.httpServer.close(done)); report.cleanup='PASS'; await writeFile(resolve(evidence,'report.json'),JSON.stringify(report,null,2)+'\n') }
console.log(JSON.stringify({result:report.result,widths:report.widths,screenshots:report.screenshots.length,failureStage:report.failureStage,errors:report.errors}))
if(report.result==='PASS') {
  const reference=relative(evidence,resolve(root,'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg'))
  await writeFile(resolve(evidence,'comparison.html'),`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>UIV05 MAP reference comparison</title><style>body{font:16px/1.5 Arial;background:#eee5d3;color:#1d1a17;margin:20px}section{display:flex;align-items:start;gap:16px;flex-wrap:wrap}figure{width:390px;margin:0}img{max-width:100%;height:auto}</style><h1>UIV05 map presentation — synthetic actual-component evidence</h1><p>The original ten-screen reference is unchanged; compare its upper-row MAP composition. Black window chrome, an ivory symmetric frame, map-first hierarchy and adjacent camera controls are adopted. The current schema-derived schematic is explicitly labeled and uses existing validated geometry only. No artwork, routes, landmarks, venue facts or photographic assets were invented. Improving presentation is not approved map-art or venue/world acceptance.</p><img width="1100" src="${reference}" alt="Unchanged user reference"><section><figure><h2>Before · 390px</h2><img src="before-overview-390.png" alt="Original Places UI in synthetic native shell fixture"></figure><figure><h2>After · 390px</h2><img src="overview-390.png" alt="Map-first adopted Places UI, synthetic native shell fixture"></figure><figure><h2>Detail · 390px</h2><img src="reviewed-detail-390.png" alt="Synthetic place source/status detail"></figure></section></html>`)
}
