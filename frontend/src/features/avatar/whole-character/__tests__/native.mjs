// Genuine published whole-character runtime and reviewed source art; deterministic HTTP only.
import assert from 'node:assert/strict'
import { readFile, writeFile, mkdir, mkdtemp, rm, statfs } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root=fileURLToPath(new URL('../../../../../',import.meta.url)), out=fileURLToPath(new URL('./evidence/native/',import.meta.url))
const hash=b=>createHash('sha256').update(b).digest('hex')
const sourceHashes={}
for(const path of ['src/features/avatar/whole-character/controller.ts','src/features/avatar/whole-character/model.ts','src/features/avatar/whole-character/WholeCharacterEditor.tsx','src/features/avatar/whole-character/whole-character.css','src/features/avatar/whole-character/__tests__/preview.tsx','src/features/avatar/whole-character/__tests__/native.mjs','src/entities/avatar/whole-character/contract.ts','src/entities/avatar/whole-character/runtime.ts','src/entities/avatar/whole-character/view.tsx'])sourceHashes[path]=hash(await readFile(root+path))
async function diskGuard(){const s=await statfs(root);assert(s.bavail*s.bsize>8*1024**3,'Disk floor: halt growth')}
await diskGuard(); await mkdir(out,{recursive:true}); const temp=await mkdtemp('/private/tmp/mcr-whole-native-')
const participant=JSON.parse(await readFile(root+'../contracts/fixtures/core/private-participant.json'))
const manifest=JSON.parse(await readFile(root+'../contracts/fixtures/assets/selected-static/avatar-manifest.json'))
const conflict=JSON.parse(await readFile(root+'../contracts/fixtures/core/error-version-conflict.json'))
const reference=await readFile(root+'../docs/mcr/design/reference-target-2026-11-04.png')
assert.equal(hash(reference),'d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617')
const initial={revision:1,config:manifest.presets[0].config}, assets=[], writes=[], errors=[], checks=[]
let saved=structuredClone(initial), mode='ok', browser
const reply=(res,status,value)=>{res.writeHead(status,{'Content-Type':'application/json','Cache-Control':'no-store'});res.end(JSON.stringify(value))}
const dev=await createServer({root,configLoader:'runner',cacheDir:temp+'/vite',logLevel:'error',server:{host:'127.0.0.1',port:0},plugins:[{name:'whole-editor-http-fixture',configureServer(server){
  server.middlewares.use(async(req,res,next)=>{
    if(req.url==='/__reference.png'){res.writeHead(200,{'Content-Type':'image/png'});res.end(reference);return}
    if(req.url?.startsWith('/__whole/control?')){mode=new URL(req.url,'http://fixture.test').searchParams.get('mode');if(mode==='rebase'){saved={revision:4,config:manifest.presets[2].config};mode='ok'}return reply(res,200,{})}
    if(req.url?.startsWith('/world/avatars/derived-v1/')){assets.push({path:req.url,authorization:req.headers.authorization??null,cookie:req.headers.cookie??null,referer:req.headers.referer??null});return next()}
    if(!req.url?.startsWith('/api/'))return next()
    try{
      assert.equal(req.headers.authorization,'Bearer whole-fixture-token')
      if(req.method==='GET'&&req.url.endsWith('/me'))return reply(res,200,{...participant,profile:{...participant.profile,avatar:saved}})
      if(req.method==='PUT'&&req.url.endsWith('/me/avatar')){
        assert.equal(req.headers['x-mcr-intent'],'browser');assert.equal(req.headers['idempotency-key'],undefined)
        assert.equal(req.headers.origin,'http://'+req.headers.host)
        const chunks=[];for await(const c of req)chunks.push(c);const body=JSON.parse(Buffer.concat(chunks).toString());writes.push(body)
        assert.deepEqual(Object.keys(body).sort(),['config','expected_revision']);assert.equal(body.expected_revision,saved.revision)
        const preset=manifest.presets.find(p=>p.preset_id===body.config?.preset_id);assert(preset);assert.deepEqual(body.config,preset.config)
        if(mode==='denied')return reply(res,403,{code:'DENIED',message:'Denied',details:{reason:'PERMISSION_DENIED'}})
        if(mode==='conflict')return reply(res,409,conflict)
        saved={revision:saved.revision+1,config:body.config}
        if(mode==='uncertain')return reply(res,503,{code:'UPSTREAM',message:'Unknown after commit',details:{reason:'DEPENDENCY_UNAVAILABLE'}})
        return reply(res,200,saved)
      }return reply(res,404,{})
    }catch(e){errors.push(String(e));return reply(res,500,{})}
  })
}}]})
const state=page=>page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.state())
const status=page=>page.locator('.mcr-whole-status > [role=status]')
async function ready(page){await expect(page.locator('.mcr-whole-editor [data-whole-character-state=ready]')).toHaveCount(4)}
async function geometry(page){
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'horizontal overflow')
  for(const control of await page.locator('.mcr-whole-editor button').all())if(await control.isVisible()){const b=await control.boundingBox();assert(b.width>=44&&b.height>=44,'44px control')}
  return page.evaluate(()=>Object.fromEntries(['preview','choices','actions'].map(name=>{const b=document.querySelector('.mcr-whole-'+name).getBoundingClientRect();return[name,{x:b.x,y:b.y,width:b.width,height:b.height}]})))
}
try{
  await dev.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
  const base=dev.resolvedUrls.local[0]+'src/features/avatar/whole-character/__tests__/preview.html'
  for(const width of [360,390,430]){
    await diskGuard();saved=structuredClone(initial);mode='ok';const a=assets.length,w=writes.length
    const context=await browser.newContext({viewport:{width,height:Math.round(width*530/298)+20},isMobile:true,hasTouch:true,deviceScaleFactor:1}),page=await context.newPage()
    page.on('pageerror',e=>errors.push(e.message));await page.goto(base);await ready(page)
    const g=await geometry(page);assert.equal((await state(page)).runtime.decodedAtlases,1);assert.equal((await state(page)).runtime.leases,4)
    assert.equal(await page.getByRole('tab').count(),0);assert.equal(await page.getByRole('combobox').count(),0)
    assert((await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.pixels())).every(c=>c.equal));await page.screenshot({path:out+`characters-${width}.png`})
    const main=await page.locator('.mcr-whole-preview canvas').elementHandle()
    const next=page.getByRole('button',{name:'Next characters',exact:true});await next.focus();await page.keyboard.press('Enter');await ready(page);await expect(next).toBeFocused()
    await expect(next).toHaveAttribute('aria-disabled','true');await page.keyboard.press('Enter');await expect(page.locator('.mcr-whole-choices header')).toContainText('4–6 / 6')
    assert(await main.evaluate(c=>c.isConnected));assert.equal(writes.length,w)
    await page.getByRole('button',{name:'Choose Black hair / teal jacket',exact:true}).tap();await ready(page)
    assert.equal((await state(page)).editor.draft.preset_id,'photo01_front_06');assert.equal(writes.length,w)
    assert((await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.pixels())).every(c=>c.equal))
    if(width===390)await page.screenshot({path:out+'sixth-character-390.png'})
    await page.getByRole('button',{name:'Reset draft',exact:true}).tap();await ready(page);assert.deepEqual((await state(page)).editor.draft,initial.config)
    await page.getByRole('button',{name:'Random character',exact:true}).tap();await ready(page);assert.notEqual((await state(page)).editor.draft.preset_id,initial.config.preset_id);assert.equal(writes.length,w)
    await page.getByRole('button',{name:'Save character',exact:true}).tap();await expect(status(page)).toContainText('Character saved.');assert.equal(writes.length,w+1)
    if(width===390){
      await page.getByRole('button',{name:'Random character',exact:true}).tap();await ready(page);await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.mode('uncertain'))
      await page.getByRole('button',{name:'Save character',exact:true}).tap();await expect(status(page)).toContainText('save result is unknown')
      await page.getByRole('button',{name:'Reset draft',exact:true}).tap();assert((await state(page)).editor.needsReconciliation);await expect(page.getByRole('button',{name:'Save character',exact:true})).toBeDisabled()
      await geometry(page);await page.screenshot({path:out+'uncertain-reset-390.png'});assert.equal(writes.length,w+2)
      await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.mode('rebase'));await page.getByRole('button',{name:'Check latest saved character',exact:true}).tap()
      await expect(page.getByRole('button',{name:'Keep my draft',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Reset draft',exact:true})).toBeDisabled()
      await page.getByRole('button',{name:'Keep my draft',exact:true}).tap();assert.equal(writes.length,w+2)
    }
    for(let n=0;n<12;n++){const prev=page.getByRole('button',{name:'Previous characters',exact:true});await prev.tap();await next.tap()}
    await ready(page);assert.equal((await state(page)).runtime.leases,4);assert.equal((await state(page)).runtime.decodedAtlases,1)
    assert.equal(assets.slice(a).filter(r=>r.path.endsWith('.png')).length,1);assert.equal(assets.slice(a).filter(r=>r.path.endsWith('.json')).length,1)
    await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.stop());await expect(page.locator('.mcr-whole-editor canvas')).toHaveCount(0)
    assert.equal((await state(page)).runtime.disposed,false);assert.equal((await state(page)).runtime.leases,0)
    await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.start());await ready(page)
    assert.equal((await state(page)).editor.candidate,null);assert.equal((await state(page)).editor.needsReconciliation,false)
    await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.suspend());await expect(page.locator('.mcr-whole-editor canvas')).toHaveCount(0)
    assert.equal((await state(page)).runtime.leases,0);assert.equal((await state(page)).runtime.decodedAtlases,0);assert.equal((await state(page)).editor.draft,null)
    await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.dispose());await context.close()
    checks.push({width,geometry:g,genuineRGBA:true,sixChoicesPaged:true,keyboardTouch:true,draftOnlyRandomReset:true,revisionSave:true,noLayerControls:true,oneMetadataOneAtlas:true,boundedPaging:true,borrowedOwnerSurvivesStop:true,privacyCleanup:true})
  }
  await diskGuard();saved=structuredClone(initial);mode='ok'
  const context=await browser.newContext({viewport:{width:816,height:748},deviceScaleFactor:1}),page=await context.newPage();page.on('pageerror',e=>errors.push(e.message));await page.goto(base);await ready(page)
  await page.evaluate(()=>window.__MCR_WHOLE_EDITOR__.dispose())
  await page.evaluate(base=>{const h=390*530/298;document.body.innerHTML=`<div style="display:flex;gap:12px;padding:8px"><section><p>Exact target · screen5</p><div style="width:390px;height:${h}px;overflow:hidden;position:relative"><img src="/__reference.png" style="position:absolute;max-width:none;width:${1536*390/298}px;left:${-1238*390/298}px;top:0"></div></section><section><p>Whole-character UI · genuine art / test HTTP</p><iframe title="Whole-character editor" src="${base}" style="border:0;width:390px;height:${h}px"></iframe></section></div>`},base)
  await ready(page.frameLocator('iframe'));await page.screenshot({path:out+'comparison-390.png'});await context.close()
  assert.deepEqual(errors,[]);assert(assets.every(a=>a.authorization===null&&a.cookie===null&&a.referer===null))
  await writeFile(out+'report.json',JSON.stringify({result:'PASS',browser:browser.version(),canonicalContract:'3398d8331863775ecc0180e5eccebddd179fedd5',sourceHashes,referenceSha256:hash(reference),manifestPin:manifest.content_hash,art:'Genuine reviewed selected-source photo_01 six whole front-idle figures',http:'Deterministic HTTP, not live backend',checks,assets,writes,errors,temporaryDirectory:temp},null,2)+'\n')
}catch(error){await writeFile(out+'report.json',JSON.stringify({result:'FAIL',failure:String(error),checks,errors},null,2)+'\n');throw error}
finally{await browser?.close();await dev.close();await rm(temp,{recursive:true,force:true})}
