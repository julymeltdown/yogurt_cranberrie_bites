import assert from 'node:assert/strict'
import {mkdir,readFile,writeFile,readdir,rm,statfs} from 'node:fs/promises'
import {createHash} from 'node:crypto'
import {createServer} from 'vite'
import react from '@vitejs/plugin-react'
import {chromium,expect} from '@playwright/test'
const own='src/features/me/__tests__/reference-target',evidence=`${own}/evidence`,fs=await statfs('.')
assert(fs.bavail*fs.bsize>8*1024**3+100*1024**2,'disk needs >8GiB plus100MiB headroom')
await mkdir(evidence,{recursive:true})
const server=await createServer({configFile:false,plugins:[react()],cacheDir:`${own}/.cache/browser`,server:{host:'127.0.0.1',port:0}})
let browser;const errors=[],results=[],captures=[],hash=b=>createHash('sha256').update(b).digest('hex')
try {
 await server.listen();const base=`http://127.0.0.1:${server.httpServer.address().port}/src/features/me/__tests__/reference-target/preview.html`
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 for(const width of [360,390,430]){
  const context=await browser.newContext({viewport:{width,height:Math.round(width*480/307)+34},reducedMotion:'reduce'}),page=await context.newPage();page.on('pageerror',e=>errors.push(e.message))
  const open=async(q='')=>{await page.goto(base+q);await expect(page.locator('.concert-window')).toBeVisible()}
  const capture=async(name)=>{const path=`${evidence}/${name}-${width}.png`;await page.screenshot({path,fullPage:true});captures.push(path)}
  async function targets(){
   assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'document overflow')
   for(const el of await page.locator('button,a,input,textarea,summary').all()) {if(!await el.isVisible())continue;const b=await el.boundingBox();assert(b&&b.width>=44&&b.height>=44,'44px control '+await el.textContent())}
  }
  await open();await expect(page.getByRole('heading',{name:'Synthetic listener'})).toBeVisible();await expect(page.getByRole('link',{name:/Posted to the feed/})).toHaveCount(2)
  await expect(page.getByText(/This is a partial view of activity/)).toBeVisible();await targets()
  const geometry=await page.evaluate(()=>{const box=s=>{const r=document.querySelector(s).getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height}};return{frame:box('.concert-window'),header:box('.me-profile-header'),portrait:box('.me-avatar'),bio:box('.me-bio'),navigation:box('.me-activity-navigation'),activity:box('.reference-activity'),options:box('.me-profile-options'),footerReservation:box('.fixture-root-footer')}})
  assert(Math.abs(geometry.portrait.width-geometry.portrait.height)<1,'square large portrait region')
  assert(geometry.portrait.width>=112&&geometry.portrait.width<=148)
  assert.equal(await page.locator('img,canvas').count(),0,'no substitute character art')
  await capture('me-partial-activity')
  const requests=await page.evaluate(()=>window.__REF_PROFILE__.requests());await page.getByText('Activity',{exact:true}).click();assert.equal(await page.evaluate(()=>window.__REF_PROFILE__.requests()),requests,'local presentation adds no request')
  await page.getByRole('link',{name:'Edit profile',exact:true}).focus();await page.keyboard.press('Tab');await expect(page.getByRole('link',{name:'Dress up'})).toBeFocused()
  assert.notEqual(await page.getByRole('link',{name:'Dress up'}).evaluate(el=>getComputedStyle(el).outlineStyle),'none')
  await page.getByText('Profile options',{exact:true}).click();await expect(page.getByRole('link',{name:'Settings',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Discard drafts and reload saved version'})).toBeVisible()
  await page.getByRole('link',{name:'Edit profile',exact:true}).click();await expect(page.getByRole('heading',{name:'Edit profile'})).toBeVisible()
  await page.getByLabel('Bio (optional)').fill('Local draft');await page.getByRole('button',{name:'Cancel profile changes'}).click();await expect(page.getByLabel('Bio (optional)')).toHaveValue('mcr / live music / the black parade\nSee you at the show.')
  await page.getByLabel('Bio (optional)').fill('Saved by explicit submit');await page.getByRole('button',{name:'Save profile'}).click();await expect(page.getByText('Profile saved.',{exact:true})).toBeVisible()
  await page.evaluate(()=>window.__REF_PROFILE__.mode('conflict'));await page.getByLabel('Bio (optional)').fill('Preserved conflict draft');await page.getByRole('button',{name:'Save profile'}).click();await expect(page.getByText('Your profile changed elsewhere. Your draft is kept. Discard it and reload before saving.')).toBeVisible();await expect(page.getByLabel('Bio (optional)')).toHaveValue('Preserved conflict draft');await expect(page.getByRole('button',{name:'Save profile'})).toBeDisabled();await targets();if(width===390)await capture('edit-conflict')
  await page.evaluate(()=>window.__REF_PROFILE__.revoke());await expect(page.getByLabel('Display name')).toHaveValue('');await expect(page.getByLabel('Bio (optional)')).toHaveValue('');await expect(page.getByRole('button',{name:'Save profile'})).toBeDisabled()
  await open('?route=edit');await page.evaluate(()=>window.__REF_PROFILE__.mode('pending'));await page.getByLabel('Bio (optional)').fill('Pending draft');await page.getByRole('button',{name:'Save profile'}).click();await expect(page.getByRole('button',{name:'Save profile'})).toBeDisabled();await page.evaluate(()=>window.__REF_PROFILE__.revoke());await page.evaluate(()=>window.__REF_PROFILE__.settle());await expect(page.getByLabel('Bio (optional)')).toHaveValue('')
  await open('?route=public&mode=actions');await expect(page.getByRole('heading',{name:'Fixture fan'})).toBeVisible();await expect(page.getByRole('link',{name:/Posted to the feed/})).toHaveCount(2);await expect(page.getByRole('button',{name:'Parties',exact:true})).toBeDisabled();await expect(page.getByRole('button',{name:'Gifts',exact:true})).toBeDisabled();await targets()
  await page.getByRole('button',{name:'Request sharing'}).click();await page.getByRole('button',{name:'Block user'}).click();assert.deepEqual(await page.evaluate(()=>{const c=window.__REF_PROFILE__.publicCounts();return{sharing:c.sharing,blocking:c.blocking}}),{sharing:1,blocking:1})
  assert(!(await page.locator('body').innerText()).includes('PRIVATE_RAW_ERROR'));assert.equal(await page.locator('img,canvas').count(),0)
  if(width===390)await capture('public-partial-activity')
  await page.evaluate(()=>window.__REF_PROFILE__.revokePublic());await expect(page.getByRole('heading',{name:'Fixture fan'})).toHaveCount(0);await expect(page.getByRole('link',{name:/Posted to the feed/})).toHaveCount(0)
  await open('?route=public&mode=no-activity');await expect(page.getByRole('heading',{name:'Fixture fan'})).toBeVisible();await expect(page.getByRole('button',{name:'Request sharing'})).toBeDisabled();await expect(page.getByRole('button',{name:'Block user'})).toBeDisabled();await expect(page.getByText('Recent activity is unavailable in this connection.')).toBeVisible()
  await open('?mode=no-activity');await expect(page.getByText('Recent activity is unavailable in this connection.')).toBeVisible();if(width===390)await capture('me-unavailable-activity');await targets()
  for(const mode of ['long','empty','error','loading','expired']){await open(`?mode=${mode}`);await targets();if(mode==='expired'){await expect(page.getByRole('heading',{name:'Synthetic listener'})).toHaveCount(0);await expect(page.getByRole('link',{name:/Posted to the feed/})).toHaveCount(0)}}
  await page.evaluate(()=>window.__REF_PROFILE__.dispose());await context.close()
  results.push({width,geometry,layoutTargetsNoSubstituteArt:'PASS',activityScopeNoExtraFetch:'PASS',navigationKeyboard:'PASS',saveCancelConflict:'PASS',pendingPrivacyScrub:'PASS',publicConsentAndUnavailable:'PASS',longEmptyErrorLoadingExpired:'PASS'})
 }
 assert.deepEqual(errors,[])
 const sources={};async function scan(dir){for(const e of await readdir(dir,{withFileTypes:true})){if(e.name==='evidence'||e.name==='.cache')continue;const p=`${dir}/${e.name}`;if(e.isDirectory())await scan(p);else if(/\.(tsx?|mjs|css|html)$/.test(e.name))sources[`frontend/${p}`]=hash(await readFile(p))}}
 for(const dir of [own,'src/features/profile/__tests__/reference-target','src/features/me/ui/reference-profile','src/features/profile/reference-presentation'])await scan(dir)
 for(const p of ['src/features/me/ui/MePages.tsx','src/features/me/ui/me.css','src/features/profile/PublicProfile.tsx','src/features/profile/profile.css','src/features/me/__tests__/visual-adoption/presentation.test.tsx','src/features/me/ui/AvatarThumbnail.tsx','src/features/profile/Avatar.tsx','src/shared/ui/concert-window/index.tsx','src/shared/ui/concert-window/concert-window.css','src/features/event-feed/contract.ts'])sources[`frontend/${p}`]=hash(await readFile(p))
 await writeFile(`${evidence}/report.json`,JSON.stringify({status:'MOCK_READY',browser:browser.version(),referenceSha256:'d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617',results,errors,sources,captures:Object.fromEntries(await Promise.all(captures.map(async p=>[`frontend/${p}`,hash(await readFile(p))]))),limitations:['Actual production MeHub/ProfileForm/PublicProfile with canonical controllers and synthetic public-schema data','Missing current-user reviewed AvatarConfig resource binding: exact large square renders honest unavailable state; no selected-static decoration becomes identity','Root7201b875 embedded window/footer not present at source base; fixture is standalone feature and64px nonillustrated reservation only','Activity is two own-author posts from one authorized page, not joined parties/reservations or complete activity history','No real backend/device/root all10route acceptance']},null,2)+'\n')
 console.log(JSON.stringify({results:results.map(r=>({width:r.width,result:'PASS'})),errors}))
}finally{await browser?.close();await server.close();await rm(`${own}/.cache/browser`,{recursive:true,force:true})}
