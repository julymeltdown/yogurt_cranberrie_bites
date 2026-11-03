import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir,readFile,writeFile } from 'node:fs/promises'
import { createServer,preview } from 'vite'
import { chromium,expect } from '@playwright/test'
const evidence=process.env.MCR_ACCOUNT_EVIDENCE??'/tmp/mcr-account-entry-evidence'
await mkdir(evidence,{recursive:true})
const original=JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/core/bootstrap-open.json',import.meta.url)))
const login=JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/core/auth-success.json',import.meta.url)))
const capabilities=JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/core/auth-capabilities-disabled.json',import.meta.url)))
capabilities.providers=capabilities.providers.map(p=>p.provider==='EMAIL'?{...p,enabled:true,disabled_reason:null}:p)
let participant=structuredClone(original.participant),mode='valid',browser,production
const requests=[],errors=[],results=[]
const dev=await createServer({server:{host:'127.0.0.1',port:0,strictPort:false},plugins:[{name:'account-native-fixtures',configureServer(server){
  server.middlewares.use(async(req,res,next)=>{
    if(!req.url?.startsWith('/api/'))return next()
    let text='';for await(const chunk of req)text+=chunk
    const body=text?JSON.parse(text):null;requests.push({path:req.url,method:req.method,body,authorization:req.headers.authorization??null})
    res.setHeader('Content-Type','application/json');res.setHeader('Cache-Control','no-store')
    let value
    if(req.url==='/api/auth/capabilities')value=capabilities
    else if(req.url==='/api/auth/login')value=login
    else if(req.url.endsWith('/bootstrap')){value=structuredClone(original);value.participant=participant;value.avatar=participant.profile.avatar;value.capabilities.world={available:true,reason:null}}
    else if(req.url.endsWith('/me')){
      if(mode==='slow')await new Promise(resolve=>setTimeout(resolve,250))
      if(req.method==='PATCH'){assert.equal(body.expected_revision,participant.revision);participant={...participant,revision:participant.revision+1,...(body.preferences?{preferences:body.preferences}:{profile:{...participant.profile,display_name:body.display_name,bio:body.bio}})}}
      value=participant
    }else throw new Error('Unexpected synthetic route '+req.url)
    res.end(JSON.stringify(value))
  })
}}]})
try{
  await dev.listen();browser=await chromium.launch({executablePath:process.env.MCR_CHROME_EXECUTABLE??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
  const url=`http://127.0.0.1:${dev.httpServer.address().port}/src/app/account-entry/__tests__/preview.html`
  for(const width of [390,1280]){
    participant=structuredClone(original.participant);mode='valid';const start=requests.length,scripts=[]
    const context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce'}),page=await context.newPage()
    page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(r.resourceType()==='script')scripts.push(new URL(r.url()).pathname)})
    await page.goto(url);await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeVisible()
    assert.equal(requests.slice(start).filter(r=>r.path.endsWith('/me')).length,0)
    await page.evaluate(()=>{window.__ACCOUNT_HOST__=document.querySelector('[data-world-host]')})
    const mounts=await page.evaluate(()=>window.__ACCOUNT__.stats.mounts)
    await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Me',exact:true}).click();await expect(page.getByRole('heading',{name:'Fixture fan',exact:true})).toBeVisible()
    await expect(page.getByText(/Avatar preview unavailable. Your saved outfit/)).toBeVisible()
    await page.getByRole('navigation',{name:'Your profile'}).getByRole('link',{name:'Edit profile',exact:true}).click();await page.getByLabel('Bio (optional)').fill('Private unsaved root draft')
    await page.getByRole('link',{name:'Back to world',exact:true}).click();await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Me',exact:true}).click()
    await page.getByRole('navigation',{name:'Your profile'}).getByRole('link',{name:'Edit profile',exact:true}).click();await expect(page.getByLabel('Bio (optional)')).toHaveValue('Private unsaved root draft')
    await page.evaluate(()=>window.__ACCOUNT__.navigate('/me/settings'));await expect(page.getByLabel('Enable sound')).not.toBeChecked()
    await page.getByLabel('Low power',{exact:true}).check();await page.getByRole('button',{name:'Save preferences',exact:true}).click();await expect(page.getByRole('status').first()).toContainText('Preferences saved')
    await expect(page.getByText(/Live preference controls are unavailable/)).toBeVisible()
    await page.evaluate(()=>window.__ACCOUNT__.navigate('/me/avatar'));await expect(page.getByText(/Reviewed avatar artwork is unavailable/)).toBeVisible();await expect(page.getByRole('button',{name:'Save outfit'})).toBeDisabled();assert.equal(await page.locator('canvas').count(),0)
    await page.getByRole('button',{name:'Expand panel'}).click();await page.screenshot({path:`${evidence}/avatar-${width}.png`,fullPage:true})
    await page.getByRole('button',{name:'Back to previous panel'}).click();await expect(page.getByRole('heading',{name:'Fixture fan',exact:true})).toBeVisible()
    await page.getByRole('navigation',{name:'Your profile'}).getByRole('link',{name:'Edit profile',exact:true}).click();await expect(page.getByLabel('Bio (optional)')).toHaveValue('Private unsaved root draft')
    const before=await page.evaluate(()=>({same:document.querySelector('[data-world-host]')===window.__ACCOUNT_HOST__,stats:window.__ACCOUNT__.stats}))
    assert(before.same);assert.equal(before.stats.mounts,mounts);assert.equal(before.stats.peakGames,1);assert.equal(before.stats.peakConnections,1)
    assert.equal(requests.slice(start).filter(r=>r.path.endsWith('/bootstrap')).length,1)
    assert(!scripts.some(p=>/node_modules.*phaser/.test(p)),'account composition fetched Phaser')
    assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'horizontal overflow')
    await page.getByRole('button',{name:'Discard drafts and reload saved version'}).focus();await expect(page.getByRole('button',{name:'Discard drafts and reload saved version'})).toBeFocused()
    await page.screenshot({path:`${evidence}/profile-${width}.png`,fullPage:true})
    await page.evaluate(()=>window.__ACCOUNT__.revoke());await expect(page.getByLabel('Bio (optional)')).toHaveValue('');await expect(page.getByLabel('Bio (optional)')).toBeEnabled()
    mode='slow';await page.getByRole('button',{name:'Discard drafts and reload saved version'}).click();await page.evaluate(()=>window.__ACCOUNT__.changeIdentity());await page.waitForTimeout(350)
    await expect(page.getByLabel('Bio (optional)')).toHaveCount(0);assert(!(await page.locator('body').innerText()).includes('Private unsaved root draft'))
    assert.equal(await page.evaluate(()=>window.__ACCOUNT__.stats.games),0)
    results.push({width,existingRoutes:true,draftSurvivesWorld:true,oneInstrumentedLease:true,oneBootstrapThroughNavigation:true,productionArtUnavailable:true,privacyCleared:true,foreignLateResponseFenced:true,phaserLoaded:false})
    await context.close()
    // A new document starts directly at the existing profile deep link.
    participant=structuredClone(original.participant);mode='valid';const direct=await browser.newPage({viewport:{width,height:844}})
    direct.on('pageerror',e=>errors.push(e.message));await direct.goto(url+'?route=/me/profile');await expect(direct.getByLabel('Display name')).toHaveValue('Fixture fan');await direct.close()
  }
  production=await preview({preview:{host:'127.0.0.1',port:0,strictPort:false}})
  const publicPage=await browser.newPage(),publicScripts=[];publicPage.on('request',r=>{if(r.resourceType()==='script')publicScripts.push(new URL(r.url()).pathname)})
  await publicPage.goto(`http://127.0.0.1:${production.httpServer.address().port}/login`);await expect(publicPage.getByRole('heading',{name:'Sign in',exact:true})).toBeVisible()
  assert.equal(await publicPage.locator('[data-world-host],canvas').count(),0)
  assert(!publicScripts.some(p=>/account-entry|provider-|panels-|phaser|CommunityFeatures/.test(p)),'public route loaded account or world chunks')
  assert.deepEqual(errors,[])
  const paths=['src/app/router.tsx','src/app/ports.ts','src/app/WorldShell.tsx','src/app/account-entry/assets.ts','src/app/account-entry/owner.ts','src/app/account-entry/provider.tsx','src/app/account-entry/panels.tsx','src/app/account-entry/__tests__/browser-smoke.mjs','src/app/account-entry/__tests__/preview.tsx']
  const hashes=Object.fromEntries(await Promise.all(paths.map(async path=>[path,createHash('sha256').update(await readFile(path)).digest('hex')])))
  await writeFile(`${evidence}/result.json`,JSON.stringify({status:'PASS',synthetic:true,results,publicScripts,requests,errors,hashes},null,2)+'\n')
  console.log(JSON.stringify({status:'PASS',evidence,viewports:results.length,requests:requests.length,errors}))
}finally{await browser?.close();await production?.close();await dev.close()}
