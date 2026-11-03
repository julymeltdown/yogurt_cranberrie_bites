import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_FE34_EVIDENCE ?? '/tmp/mcr-fe34-evidence'
await mkdir(evidence, { recursive: true })
const avatar = JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/assets/avatar-config.json', import.meta.url)))
const user = '11111111-1111-4111-8111-111111111111', event = '22222222-2222-4222-8222-222222222222'
const initial = () => ({ event_id:event, user_id:user, revision:1, profile:{ user_id:user,display_name:'Synthetic listener',bio:null,avatar:{revision:1,config:structuredClone(avatar)} },preferences:{theme:'REVENGE',low_power:false,reduced_motion:false,sound_enabled:false},age_status:'UNKNOWN' })
let saved = initial(), mode = 'valid', browser
const requests = [], errors = [], results = []
const dev = await createServer({ server:{host:'127.0.0.1',port:0,strictPort:false}, plugins:[{ name:'fe34-native-fixture', configureServer(server) {
  server.middlewares.use(async (req,res,next) => {
    if (req.url !== `/api/concert/events/${event}/me`) return next()
    let bytes=''; for await(const chunk of req) bytes+=chunk
    const body=bytes ? JSON.parse(bytes) : null
    requests.push({method:req.method,body,authorization:req.headers.authorization,cache:req.headers['cache-control']})
    res.setHeader('Content-Type','application/json');res.setHeader('Cache-Control','no-store')
    if(mode==='slow') await new Promise(resolve=>setTimeout(resolve,250))
    if(req.method==='PATCH') {
      if(mode==='conflict'){res.statusCode=409;res.end(JSON.stringify({code:'CONFLICT',message:'Fixture changed',details:{reason:'VERSION_CONFLICT',current_revision:saved.revision+1}}));return}
      if(mode==='failure'){res.statusCode=503;res.end(JSON.stringify({code:'UNAVAILABLE',message:'Fixture unavailable',details:{reason:'DEPENDENCY_UNAVAILABLE'}}));return}
      assert.equal(body.expected_revision,saved.revision)
      assert.deepEqual(Object.keys(body).sort(),body.preferences?['expected_revision','preferences']:['bio','display_name','expected_revision'])
      if(body.preferences)saved.preferences=body.preferences;else{saved.profile.display_name=body.display_name;saved.profile.bio=body.bio}
      saved.revision++
    }
    res.end(JSON.stringify(saved))
  })
} }] })
try {
  await dev.listen(); const url=`http://127.0.0.1:${dev.httpServer.address().port}/src/features/me/__tests__/preview.html`
  browser=await chromium.launch({executablePath:process.env.MCR_CHROME_EXECUTABLE??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
  for(const width of [390,1280]) {
    saved=initial();mode='valid'
    const context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce'}),page=await context.newPage()
    page.on('pageerror',error=>errors.push(error.message));await page.goto(url)
    await expect(page.getByRole('heading',{name:'Synthetic listener'})).toBeVisible()
    await expect(page.getByText('Avatar preview unavailable.',{exact:false})).toBeVisible();assert.equal(await page.locator('canvas').count(),0)
    await page.screenshot({path:`${evidence}/hub-${width}.png`,fullPage:true})
    assert.equal(await page.getByRole('link',{name:'Dress up'}).getAttribute('href'),'/me/avatar')
    assert.equal(await page.getByRole('link',{name:'My gifts',exact:true}).getAttribute('href'),'/gifts/mine')
    await page.getByRole('link',{name:'Edit profile',exact:true}).click()
    await page.getByLabel('Display name').fill('Updated listener');await page.getByLabel('Bio (optional)').fill('Public synthetic bio')
    await page.getByRole('button',{name:'Cancel profile changes'}).click();await expect(page.getByLabel('Display name')).toHaveValue('Synthetic listener');await expect(page.getByLabel('Bio (optional)')).toHaveValue('')
    await page.getByLabel('Display name').fill('Updated listener');await page.getByRole('button',{name:'Save profile',exact:true}).click();await expect(page.getByRole('status')).toContainText('Profile saved')
    mode='conflict';await page.getByLabel('Bio (optional)').fill('Keep this draft');const writes=requests.filter(r=>r.method==='PATCH').length
    await page.getByRole('button',{name:'Save profile',exact:true}).click();await expect(page.getByRole('status')).toContainText('changed elsewhere');await expect(page.getByLabel('Bio (optional)')).toHaveValue('Keep this draft');await expect(page.getByRole('button',{name:'Save profile',exact:true})).toBeDisabled();assert.equal(requests.filter(r=>r.method==='PATCH').length,writes+1)
    mode='valid';await page.getByRole('button',{name:'Discard drafts and reload saved version'}).click();await expect(page.getByLabel('Bio (optional)')).toHaveValue('')
    mode='failure';await page.getByLabel('Bio (optional)').fill('Retry draft');await page.getByRole('button',{name:'Save profile',exact:true}).click();await expect(page.getByRole('status')).toContainText('save result is unknown');await expect(page.getByLabel('Bio (optional)')).toHaveValue('Retry draft');await expect(page.getByRole('button',{name:'Save profile',exact:true})).toBeDisabled()
    mode='valid';await page.getByRole('button',{name:'Discard drafts and reload saved version'}).click();await expect(page.getByLabel('Bio (optional)')).toHaveValue('');await page.getByLabel('Bio (optional)').fill('Retry draft');await page.getByRole('button',{name:'Save profile',exact:true}).focus();await page.keyboard.press('Enter');await expect(page.getByRole('status')).toContainText('Profile saved');await page.screenshot({path:`${evidence}/profile-${width}.png`,fullPage:true})
    // Direct route entry uses a new document/controller and only the canonical participant.
    await page.goto(url);await page.getByRole('link',{name:'Settings',exact:true}).click();await expect(page.getByLabel('Enable sound')).not.toBeChecked()
    await page.getByLabel('Low power', {exact:true}).check();await page.getByLabel('Reduced motion',{exact:true}).check();await page.getByLabel('Enable sound').check();await page.getByLabel('Preferred era').selectOption('DANGER')
    await page.getByRole('button',{name:'Save preferences',exact:true}).click();await expect(page.getByRole('status').first()).toContainText('Preferences saved');assert.deepEqual(await page.evaluate(()=>window.__FE34__.runtime()),{calls:0,activeSound:false,aborted:[]})
    await page.getByRole('button',{name:'Apply saved preferences in this session'}).click();await expect(page.getByRole('status').last()).toContainText('applied in this session');assert.equal((await page.evaluate(()=>window.__FE34__.runtime())).activeSound,true)
    assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'horizontal overflow')
    for(const button of await page.getByRole('button').all()){const b=await button.boundingBox();assert(b.height>=44,'small touch target')}
    await page.screenshot({path:`${evidence}/settings-${width}.png`,fullPage:true})
    await page.evaluate(()=>window.__FE34__.revoke());await expect(page.getByRole('status').first()).toContainText('permissions changed');await expect(page.getByLabel('Enable sound')).not.toBeChecked();assert.equal((await page.evaluate(()=>window.__FE34__.runtime())).activeSound,false)
    assert.equal(await page.evaluate(()=>localStorage.length),0);assert.equal(await page.evaluate(()=>sessionStorage.length),0)
    // A native late read cannot repopulate data after an identity switch.
    mode='slow';await page.goto(url);await page.evaluate(()=>window.__FE34__.switchIdentity());await page.waitForTimeout(350);assert.equal(await page.evaluate(()=>window.__FE34__.state().participant),null)
    results.push({width,profileSaveCancel:true,conflictNoReplay:true,preferencesSavedSoundOffUntilApply:true,privacyCleared:true,lateIdentityFenced:true,productionArtUnavailable:true})
    await context.close()
  }
  assert.deepEqual(errors,[]);assert(requests.every(r=>r.authorization==='Bearer SYNTHETIC_ONLY'))
  await writeFile(`${evidence}/result.json`,JSON.stringify({status:'PASS',fixtureOnly:true,results,requests,errors},null,2)+'\n')
  console.log(JSON.stringify({status:'PASS',evidence,viewports:results.length,requests:requests.length,errors}))
} finally {await browser?.close();await dev.close()}
