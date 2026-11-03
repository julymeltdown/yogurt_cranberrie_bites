// Real Chrome/canvas. Only explicitly synthetic geometric fixtures are used; no product art admission.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, mkdir, writeFile, mkdtemp, rm, statfs } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { syntheticAvatarPng } from '../../../../world/avatar/__tests__/fixtures/png.mjs'
const root = fileURLToPath(new URL('../../../../../', import.meta.url)), out = fileURLToPath(new URL('./evidence/native/', import.meta.url))
async function diskGuard() { const disk = await statfs(root); assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, 'Disk at/below8GiB floor; stop output growth') }
await diskGuard(); await mkdir(out, { recursive: true })
const temp = await mkdtemp('/private/tmp/mcr-ref-customize-native-')
const reference = await readFile('/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/codex-clipboard-1c0bbe99-a4bc-4721-a1d3-478a34ef9e76.png')
const hash = bytes => createHash('sha256').update(bytes).digest('hex'), png = syntheticAvatarPng(), pngHash = hash(png)
const participant = JSON.parse(await readFile(root+'../contracts/fixtures/core/private-participant.json'))
const conflict = JSON.parse(await readFile(root+'../contracts/fixtures/core/error-version-conflict.json'))
const writes = [], assets = [], errors = [], checks = [], hashes = {}
for (const path of ['src/features/avatar/AvatarEditor.tsx', 'src/features/avatar/OptionStrip.tsx', 'src/features/avatar/controller.ts', 'src/features/avatar/avatar-editor.css', 'src/features/avatar/preview.ts', 'src/features/avatar/contract.ts', 'src/entities/avatar/runtime/compositor.ts']) hashes[path] = hash(await readFile(root+path))
let saved = structuredClone(participant.profile.avatar), mode = 'ok', browser
const reply = (res, status, body) => { res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(JSON.stringify(body)) }
const dev = await createServer({ root, configLoader: 'runner', cacheDir: temp+'/vite', logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'test-only-avatar-options', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (req.url === '/__reference.png') { res.writeHead(200, { 'Content-Type': 'image/png' }); res.end(reference); return }
    if (req.url?.startsWith('/__compact/control?')) {
      const selected = new URL(req.url, 'http://fixture.test').searchParams.get('mode')
      if (selected === 'rebase') { saved = { revision: 4, config: structuredClone(participant.profile.avatar.config) }; mode = 'ok' } else mode = selected
      return reply(res, 200, {})
    }
    if (req.url === '/__fe13_options/metadata') return reply(res, 200, { sha256: `sha256:${pngHash}` })
    if (req.url === '/world/fe13-options-test/avatar.png') { assets.push({ authorization: req.headers.authorization ?? null, cookie: req.headers.cookie ?? null, referer: req.headers.referer ?? null }); res.writeHead(200, { 'Content-Type': 'image/png', 'Cache-Control': 'no-store' }); res.end(png); return }
    if (!req.url?.startsWith('/api/')) return next()
    try {
      assert.equal(req.headers.authorization, 'Bearer option-fixture-token')
      if (req.method === 'GET' && req.url.endsWith('/me')) return reply(res, 200, { ...participant, profile: { ...participant.profile, avatar: saved } })
      if (req.method === 'PUT' && req.url.endsWith('/me/avatar')) {
        const chunks = []; for await (const chunk of req) chunks.push(chunk)
        const body = JSON.parse(Buffer.concat(chunks).toString()); writes.push(body)
        assert.equal(body.expected_revision, saved.revision)
        if (mode === 'denied') return reply(res, 403, { code: 'GATEWAY_403', message: 'Test access denied', details: { reason: 'POLICY_DENIED' } })
        if (mode === 'conflict') return reply(res, 409, conflict)
        saved = { revision: saved.revision + 1, config: body.config }
        if (mode === 'uncertain') return reply(res, 503, { code: 'GATEWAY', message: 'Fixture upstream unavailable after commit', details: { reason: 'DEPENDENCY_UNAVAILABLE' } })
        return reply(res, 200, saved)
      }
      reply(res, 404, {})
    } catch (error) { errors.push(String(error)); if (!res.destroyed) reply(res, 500, {}) }
  })
} }] })
const state = page => page.evaluate(() => window.__FE_REF_CUSTOMIZE__.state())
async function geometry(page) {
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
  for (const control of await page.locator('.mcr-avatar-editor button,.mcr-avatar-editor select,.mcr-avatar-editor summary').all()) {
    if (!await control.isVisible()) continue
    const box = await control.boundingBox(); assert(box.width >= 44 && box.height >= 44, `44px target: ${await control.getAttribute('aria-label')}`)
  }
  return page.evaluate(() => {
    const box = selector => { const r = document.querySelector(selector).getBoundingClientRect(); return { x:r.x, y:r.y, width:r.width, height:r.height } }
    return { editor:box('.mcr-avatar-editor'), preview:box('.mcr-avatar-preview'), rail:box('.mcr-avatar-category-rail'), choices:box('.mcr-avatar-option-strip'), actions:box('.mcr-avatar-actions') }
  })
}
const ready = page => expect(page.locator('.mcr-avatar-preview canvas')).toHaveCount(1)
try {
  await diskGuard(); await dev.listen(); browser = await chromium.launch({ executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless:true })
  const base = dev.resolvedUrls.local[0]+'src/features/avatar/__tests__/reference-target/preview.html'
  for (const width of [360,390,430]) {
    await diskGuard(); saved = structuredClone(participant.profile.avatar); mode='ok'
    const beforeWrites=writes.length, beforeAssets=assets.length
    const height=Math.round(width*530/298)+20, context=await browser.newContext({ viewport:{width,height}, hasTouch:true, isMobile:true, deviceScaleFactor:1 }), page=await context.newPage()
    page.on('pageerror', error=>errors.push(error.message)); await page.goto(base); await ready(page)
    await expect(page.getByRole('heading',{name:'CUSTOMIZE',exact:true})).toBeVisible()
    assert.equal((await state(page)).owners,1); assert.equal((await state(page)).reads,1); assert((await state(page)).aliveCanvases<=4)
    assert(await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.mainPixels()))
    await page.getByRole('tab',{name:'Hair',exact:true}).focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('button',{name:'Try hair fixture · full outfit',exact:true})).toBeEnabled()
    assert.equal((await state(page)).aliveCanvases,2)
    const measured=await geometry(page); assert.equal(measured.rail.width,46); assert(measured.preview.width>measured.rail.width*4)
    assert(measured.actions.y+measured.actions.height<=height,'actions below matching-aspect viewport')
    assert((await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.optionPixels())).every(p=>p.equal&&!p.mainCanvas))
    await page.screenshot({path:out+`target-${width}.png`})
    await page.getByRole('tab',{name:'Presets',exact:true}).focus(); await page.keyboard.press('Enter'); await ready(page)
    const main=await page.locator('.mcr-avatar-preview canvas').elementHandle()
    const next=page.getByRole('button',{name:'Next outfit choices',exact:true}); await next.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('button',{name:'Try look 6 · full outfit',exact:true})).toBeEnabled(); await expect(next).toBeFocused()
    assert(await main.evaluate(canvas=>canvas.isConnected&&canvas.width>0)); assert.equal(writes.length,beforeWrites)
    await page.getByRole('button',{name:'Previous outfit choices',exact:true}).tap(); await ready(page)
    await page.getByRole('button',{name:'Try look 2 · full outfit',exact:true}).focus(); await page.keyboard.press('Enter'); await ready(page)
    assert.equal((await state(page)).editor.draft.item_ids.accessory,null); assert.equal(writes.length,beforeWrites)
    await page.getByRole('button',{name:'Reset draft',exact:true}).tap(); await ready(page)
    assert.deepEqual((await state(page)).editor.draft,participant.profile.avatar.config); assert.equal(writes.length,beforeWrites)
    await page.getByRole('button',{name:'Random outfit',exact:true}).tap(); await ready(page)
    assert.equal((await state(page)).editor.draft.item_ids.accessory,null); assert.equal(writes.length,beforeWrites)
    await page.getByRole('button',{name:'Skin tone tone unsupported',exact:true}).isDisabled().then(disabled=>assert(disabled))
    await page.getByRole('button',{name:'Save outfit',exact:true}).tap(); await expect(page.getByRole('status')).toContainText('Outfit saved.')
    assert.equal(writes.length,beforeWrites+1)
    if(width===390){
      await page.getByRole('button',{name:'Random outfit',exact:true}).tap(); await ready(page)
      await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.mode('uncertain'))
      await page.getByRole('button',{name:'Save outfit',exact:true}).tap(); await expect(page.getByRole('status')).toContainText('save result is unknown')
      await page.getByRole('button',{name:'Reset draft',exact:true}).tap(); assert((await state(page)).editor.needsReconciliation)
      await expect(page.getByRole('button',{name:'Save outfit',exact:true})).toBeDisabled(); assert.equal(writes.length,beforeWrites+2)
      await geometry(page); await page.screenshot({path:out+'uncertain-reset-390.png'})
      await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.mode('rebase'))
      await page.getByRole('button',{name:'Check latest saved outfit',exact:true}).tap(); await expect(page.getByRole('button',{name:'Reset draft',exact:true})).toBeDisabled()
      assert((await state(page)).editor.candidate); assert((await state(page)).editor.needsReconciliation)
      await page.getByRole('button',{name:'Keep my draft',exact:true}).tap()
      assert.equal(writes.length,beforeWrites+2)
    }
    await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.hold(true)); await page.getByRole('tab',{name:'Hair',exact:true}).tap()
    await expect.poll(async()=>(await state(page)).pending).toBe(1)
    for(let n=0;n<12;n++) await page.getByRole('tab',{name:n%2?'Eyes':'Body',exact:true}).tap()
    assert.equal((await state(page)).pending,1); assert.equal((await state(page)).active,1)
    await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.release()); await expect.poll(async()=>(await state(page)).active).toBe(0)
    assert((await state(page)).aliveCanvases<=4); assert.equal(assets.length,beforeAssets+1)
    await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.suspend()); await expect(page.locator('.mcr-avatar-editor canvas')).toHaveCount(0)
    assert.equal((await state(page)).aliveCanvases,0); assert.equal((await state(page)).atlasDisposed,1)
    await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.dispose())
    if(width===390){
      await page.goto(base+'?state=unavailable'); await expect(page.getByRole('status')).toContainText('Reviewed avatar artwork is unavailable')
      assert.equal((await state(page)).reads,0); assert.equal(assets.length,beforeAssets+1)
      await expect(page.locator('.mcr-avatar-editor canvas')).toHaveCount(0); await geometryUnavailable(page)
      await page.screenshot({path:out+'production-unavailable-390.png'}); await page.evaluate(()=>window.__FE_REF_CUSTOMIZE__.dispose())
    }
    await context.close(); checks.push({width,height,geometry:measured,keyboardAndTouch:true,randomResetDraftOnly:true,explicitSave:true,currentPaletteOnly:true,oneOwnerOneRead:true,boundedChurn:true,scopeCleanup:true})
  }
  // Render source screen5 and actual production UI in equal-aspect frames. No image pixels are derived or used as product artwork.
  await diskGuard(); const context=await browser.newContext({viewport:{width:816,height:748},deviceScaleFactor:1}), comparison=await context.newPage()
  const frameHeight=390*530/298
  await comparison.goto(base); await ready(comparison); await comparison.evaluate(()=>window.__FE_REF_CUSTOMIZE__.dispose()); await comparison.evaluate(({base,frameHeight})=>{
    document.body.innerHTML=`<div style="display:flex;gap:12px;padding:8px;font:12px system-ui"><section><p>Exact target · screen5</p><div style="position:relative;width:390px;height:${frameHeight}px;overflow:hidden"><img src="/__reference.png" style="position:absolute;max-width:none;width:${1536*390/298}px;left:${-1238*390/298}px;top:0"></div></section><section><p>Production UI · TEST ONLY geometry / artwork gap</p><iframe title="Production Customize fixture" src="${base}" style="width:390px;height:${frameHeight}px;border:0"></iframe></section></div>`
  },{base,frameHeight})
  const frame=comparison.frameLocator('iframe'); await ready(frame); await frame.getByRole('tab',{name:'Hair',exact:true}).click()
  await expect(frame.getByRole('button',{name:'Try hair fixture · full outfit',exact:true})).toBeEnabled()
  await comparison.screenshot({path:out+'comparison-390.png'}); await context.close()
  assert.deepEqual(errors,[]); assert(assets.every(a=>a.authorization===null&&a.cookie===null&&a.referer===null))
  await writeFile(out+'report.json',JSON.stringify({result:'PASS',base:'f837bef676cd26869c26c172a699a91b7842d832',referenceSha256:hash(reference),referenceScreen:{x:1238,y:0,width:298,height:530},browser:browser.version(),syntheticOnly:true,approvedAvatarArt:'UNAVAILABLE',hashes,checks,errors,assetRequests:assets,explicitWrites:writes,temporaryDirectory:temp},null,2)+'\n')
} catch(error){ await writeFile(out+'report.json',JSON.stringify({result:'FAIL',hashes,checks,errors,failure:String(error)},null,2)+'\n'); throw error }
finally{ await browser?.close(); await dev.close(); await rm(temp,{recursive:true,force:true}) }
async function geometryUnavailable(page){
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth))
  for(const name of ['Random outfit','Reset draft','Save outfit']) await expect(page.getByRole('button',{name,exact:true})).toBeDisabled()
}
