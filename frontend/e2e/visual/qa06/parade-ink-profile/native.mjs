// PI06 actual main journey; controlled strict wire inputs, never a live/device claim.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFile, writeFile, mkdir, stat } from 'node:fs/promises'
import { resolve, extname } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { frozenSource } from './frozen-source.mjs'
const repo = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(repo, 'frontend')
const output = process.env.MCR_PI06_OUTPUT, temporary = process.env.MCR_PI06_TMP
const job = process.env.MCR_PI06_JOB, port = Number(process.env.MCR_PI06_PORT)
assert(output?.startsWith(repo + '/docs/mcr/evidence/parade-ink-profile/native') && temporary?.startsWith('/private/tmp/mcr-pi06-') && job && port > 1023)
const source = frozenSource(repo, process.env.MCR_PI06_SOURCE)
const reusedPublic = !source.staticFrame
const publicBaseline = 'f71a25633c65f2ddbfcf76fb45bfb5169b49c8c5'
const publicReuse = []
if (reusedPublic) {
  for (const path of ['frontend/src/features/profile/PublicProfile.tsx','frontend/src/features/profile/profile.css','frontend/src/features/profile/reference-presentation/activity.css','frontend/src/app/social-composition/panels.tsx','frontend/e2e/visual/qa06/parade-ink-profile/preview.ts']) {
    const before=execFileSync('git',['show',`${publicBaseline}:${path}`],{cwd:repo}),after=execFileSync('git',['show',`${source.revision}:${path}`],{cwd:repo})
    assert(before.equals(after),'Unchanged public state reuse: '+path);publicReuse.push(path)
  }
}
const input = JSON.parse(source.bytes('frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'))
const localInput = JSON.parse(source.bytes('frontend/src/app/world-entry/__tests__/map-presentation-v1/source-inputs/local-input.json'))
const report = { job, status: 'IN_PROGRESS', candidate: source.revision, foundation: source.foundation, shellCorrection: source.shellCorrection, portrait: source.portrait, portraitPaths: source.portraitPaths, staticFrame:source.staticFrame,
  harnessHead: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim(),
  qualification: 'Frozen actual main + explicit PI01 presentation and PI07 unqualified frame checkpoints; controlled canonical HTTP/C05, not live or physical device',
  publicReuse: reusedPublic ? { source:publicBaseline, files:publicReuse, evidence:'native02/screens/report.json public-profile capture+assertions at360/390/430; old overall FAIL retained' } : null,
  cases: [], pngBytes: 0, startedAt: new Date().toISOString() }
let server, browser, context
const state = page => page.evaluate(() => window.__PROFILE_DEFAULT_FLOW__.state())
async function navigate(page, label) {
  const nav = page.getByRole('navigation', { name: 'Main navigation' })
  if (!await nav.isVisible()) {
    await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
    await expect(nav).toBeVisible()
    await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))))
  }
  await nav.getByRole('link', { name: label, exact: true }).click()
}
async function shot(page, name) {
  const path = resolve(output, name)
  await page.screenshot({ path })
  report.pngBytes += (await stat(path)).size
  assert(report.pngBytes <= 6 * 1024 ** 2, 'Bounded 13 PNG cohort')
  return name
}
async function geometry(page) {
  await page.evaluate(() => document.fonts.ready)
  return page.evaluate(() => {
    const selectors = ['.me-hub--reference .concert-window__titlebar', '.pi-public-profile .concert-window__titlebar', '.pi-portrait-frame', '.me-profile-header', '.concert-window', '.concert-window__titlebar', '.me-identity', '.me-avatar--presentation', '.me-identity__copy h3', '.me-bio', '.pi-profile-customize', '.me-activity-navigation', '.reference-activity__list li', '.public-profile-identity', '.public-profile-copy h2', '.public-profile-actions', '.pi-settings fieldset', '.pi-profile-form fieldset', '.pi-profile-footer', '.me-hub--reference', '.pi-profile-form .concert-action--red']
    const data = {}
    for (const selector of selectors) {
      const e = document.querySelector(selector); if (!e) continue
      const r = e.getBoundingClientRect(), s = getComputedStyle(e)
      data[selector] = { x:r.x,y:r.y,width:r.width,height:r.height,font:s.fontFamily,fontSize:s.fontSize,lineHeight:s.lineHeight,color:s.color,background:s.backgroundColor,overflow:s.overflow, borderLeft:s.borderLeft, backgroundImage:s.backgroundImage }
    }
    return { viewport:{width:innerWidth,height:innerHeight}, horizontalOverflow:document.documentElement.scrollWidth>innerWidth, data }
  })
}
async function capture(page, entry, name) {
  const measures = await geometry(page); assert.equal(measures.horizontalOverflow, false)
  entry.screens.push({ name, screenshot: await shot(page, name + '-' + entry.width + '.png'), ...measures })
}
try {
  await mkdir(output, { recursive: false })
  server = await createServer({ root:frontend, configLoader:'runner', cacheDir:resolve(temporary,'vite'), publicDir:false, logLevel:'error',
    define:{ 'import.meta.env.VITE_MCR_EVENT_ID':JSON.stringify(input.event.event_id), 'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG':JSON.stringify(JSON.stringify(localInput)) },
    plugins:[source.plugin,{name:'pi06-frozen-public-and-entry',configureServer(s){
      s.middlewares.use(async(req,res,next)=>{
        const url = decodeURIComponent(new URL(req.url,'http://localhost').pathname)
        const path = url.startsWith('/__pi06_frozen_asset__/') ? url.slice('/__pi06_frozen_asset__/'.length) : 'frontend/public'+url
        if(source.files.has(path)) {
          const mime={'.svg':'image/svg+xml','.json':'application/json','.png':'image/png','.ttf':'font/ttf','.woff2':'font/woff2'}[extname(path)]
          if(mime){res.setHeader('Content-Type',mime);res.end(source.bytes(path));return}
        }
        if(!req.headers.accept?.includes('text/html')) return next()
        res.setHeader('Content-Type','text/html')
        res.end(await s.transformIndexHtml(req.url,await readFile(resolve(import.meta.dirname,'preview.html'),'utf8')))
      })
    }}], server:{host:'127.0.0.1',port,strictPort:true,hmr:false,watch:null} })
  await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
  for(const width of [360,390,430]) {
    context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce',serviceWorkers:'block'})
    const page=await context.newPage(),entry={width,status:'IN_PROGRESS',errors:[],screens:[]};report.cases.push(entry)
    page.setDefaultTimeout(10000);page.on('pageerror',error=>entry.errors.push(error.message))
    try {
    await page.goto(`http://127.0.0.1:${port}/login?mode=loaded`)
    await page.getByLabel('Email',{exact:true}).fill('profile-controlled@example.test')
    await page.getByLabel('Password',{exact:true}).fill('controlled-password')
    await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
    await expect(page.locator('.shell-world-host canvas')).toHaveCount(1)
    await expect.poll(async()=>(await state(page)).admittedStreams).toBe(1)
    const host=page.locator('[data-world-host]');await host.evaluate(e=>Reflect.set(e,'__pi06Incumbent','same'))
    await navigate(page,'Feed')
    await expect(page.getByText('This section is unavailable for the current account or event.',{exact:true})).toBeVisible()
    assert.equal((await state(page)).feedReads,0)
    await page.evaluate(()=>window.__PROFILE_DEFAULT_FLOW__.baseline())
    await expect(page.locator('.feed-posts > li')).toHaveCount(2)
    for(let i=0;i<2;i++) {
      const image=page.locator('.feed-post').nth(i).locator('.feed-image').first()
      await image.scrollIntoViewIfNeeded();await expect.poll(()=>image.evaluate(e=>e.complete&&e.naturalWidth===197)).toBe(true)
    }
    const before=await state(page);assert.equal(before.feedReads,1)
    if (!reusedPublic) {
    // Feed has no author route. Follow actual Meet -> People -> authorized introduction link.
    await navigate(page,'Meet')
    await page.getByRole('link',{name:'People',exact:true}).click()
    await page.locator(`a[href="/people/${before.userId}"]`).first().click()
    await expect(page.locator('.pi-public-profile .public-profile-identity')).toBeVisible()
    await expect(page.locator('.public-profile-identity canvas')).toHaveCount(1)
    assert.equal(await page.locator('.public-profile-identity canvas').evaluate(e=>e.getBoundingClientRect().width),128,'Default public-profile caller supplies the native128 portrait')
    await expect(page.getByRole('button',{name:'Request sharing',exact:true})).toBeDisabled()
    await expect(page.getByRole('button',{name:'Block user',exact:true})).toBeDisabled()
    await expect(page.getByRole('link',{name:'Report profile',exact:true})).toHaveAttribute('href',`/report?targetType=USER&targetId=${before.userId}`)
    await expect(page.locator('.public-profile-identity .pi-portrait-frame')).toHaveCount(1)
    await capture(page,entry,'public-profile')
    }
    await navigate(page,'Me');await expect(page.locator('.me-hub')).toBeVisible()
    await expect(page.getByRole('link',{name:/Posted to the feed/})).toHaveCount(2)
    await expect(page.getByText('Preferred era · PARADE',{exact:true})).toBeVisible()
    assert.equal(await page.locator('.me-identity canvas').evaluate(e=>e.getBoundingClientRect().width),128)
    entry.profile=await state(page);assert.equal(entry.profile.feedReads,before.feedReads);assert.equal(entry.profile.mediaReads,before.mediaReads)
    await expect(page.locator('.me-identity .pi-portrait-frame')).toHaveCount(1)
    await capture(page,entry,'own-profile')
    const edit=page.getByRole('link',{name:'Edit profile',exact:true});await page.keyboard.press('Tab');await edit.focus();await expect(edit).toBeFocused()
    assert.equal(await edit.evaluate(e=>e.matches(':focus-visible')),true,'Actual keyboard modality before checking the visible outline')
    assert.notEqual(await edit.evaluate(e=>getComputedStyle(e).outlineStyle),'none')
    await edit.click();await expect(page.locator('.pi-profile-form')).toBeVisible()
    const name=page.getByLabel('Display name',{exact:true}),saved=await name.inputValue()
    await name.fill('Controlled temporary draft')
    await expect(page.getByRole('button',{name:'Save profile',exact:true})).toBeEnabled()
    await capture(page,entry,'edit-profile')
    await page.getByRole('button',{name:'Cancel profile changes',exact:true}).click();await expect(name).toHaveValue(saved)
    await expect(page.getByRole('button',{name:'Save profile',exact:true})).toBeDisabled()
    await navigate(page,'Me');await page.getByText('Profile options',{exact:true}).click()
    await page.getByRole('link',{name:'Settings',exact:true}).click();await expect(page.locator('.pi-settings')).toBeVisible()
    const era=page.getByRole('combobox',{name:/^Preferred era/});await expect(era).toHaveValue('PARADE')
    await expect(page.getByLabel('Enable sound',{exact:true})).not.toBeChecked()
    const activeWorldEra=await page.locator('.mcr-theme-selector input:checked').inputValue();entry.activeWorldEra=activeWorldEra
    await era.selectOption('DANGER');await page.getByLabel('Low power',{exact:true}).check()
    assert.equal(await page.locator('.mcr-theme-selector input:checked').inputValue(),activeWorldEra,'Unsaved preferred era does not switch the World')
    await capture(page,entry,'settings-draft')
    await page.getByRole('button',{name:'Cancel preference changes',exact:true}).click();await expect(era).toHaveValue('PARADE')
    await expect(page.getByRole('button',{name:'Save preferences',exact:true})).toBeDisabled()
    if(width===390){await page.setViewportSize({width,height:420});await capture(page,entry,'settings-short');await page.setViewportSize({width,height:844})}
    assert.equal((await state(page)).requests.filter(r=>r.method==='PATCH').length,0,'Draft/cancel sends no saved mutation')
    assert.equal(await host.evaluate(e=>Reflect.get(e,'__pi06Incumbent')),'same')
    entry.final=await state(page);assert.equal(entry.final.socketsCreated,1);assert.equal(entry.final.peakCanvases,1)
    await navigate(page,'Me');await page.getByText('Profile options',{exact:true}).click()
    await page.getByRole('link',{name:'Account',exact:true}).click()
    await page.getByRole('button',{name:'Sign out',exact:true}).click();await page.getByRole('button',{name:'Confirm sign out',exact:true}).click()
    await expect.poll(async()=>{const s=await state(page);return s.activeSockets===0&&s.canvases===0&&s.urls.created.every(u=>s.urls.revoked.includes(u))}).toBe(true)
    entry.signedOut=await state(page);assert.deepEqual(entry.errors,[]);entry.status='PASS'
    } catch(error) {
      entry.status='FAIL';entry.error=String(error.stack??error)
      entry.failure=await page.evaluate(()=>({state:window.__PROFILE_DEFAULT_FLOW__?.state(),text:document.body.innerText})).catch(()=>null)
    } finally { await context.close();context=null }
    await writeFile(resolve(output,'report.partial.json'),JSON.stringify(report,null,2)+'\n')
  }
  report.status=report.cases.length===3&&report.cases.every(entry=>entry.status==='PASS')?'PASS':'FAIL'
  if(report.status==='FAIL')process.exitCode=1
}catch(error){report.status='FAIL';report.error=String(error.stack??error);process.exitCode=1
  report.failure=await context?.pages()[0]?.evaluate(()=>({state:window.__PROFILE_DEFAULT_FLOW__?.state(),text:document.body.innerText})).catch(()=>null)
}finally{
  await context?.close();await browser?.close();await server?.close()
  report.sourceReceipts=source.receipts();report.transformations=source.transformations();report.finishedAt=new Date().toISOString()
  await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n')
}
