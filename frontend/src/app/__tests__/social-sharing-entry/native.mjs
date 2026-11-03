import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const frontend = resolve(import.meta.dirname, '../../../..'), evidence = resolve(import.meta.dirname, 'evidence/native')
await mkdir(evidence, { recursive: true })
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: '/tmp/mcr-sharing-entry-vite', server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
await server.listen()
const base = `http://127.0.0.1:${server.httpServer.address().port}/src/app/__tests__/social-sharing-entry/native.html`
const report = { result: 'FAIL', qualification: 'Actual default routes, WorldShell, canonical Auth/HTTP/FE31 and one production world/Phaser owner. Explicit TEST ONLY C04/readiness, MockSocket, geometric terrain. Live provider, physical device, two-user IN03 and load NOT_RUN.', cases: [], screenshots: [], errors: [] }
let browser, page, stage = 'start'
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[360,800], [390,844], [430,932]]) {
    page = await browser.newPage({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce' })
    page.on('pageerror', e => report.errors.push(e.message))
    const state = () => page.evaluate(() => window.__SHARING_ENTRY__?.state())
    const action = (name, value) => page.evaluate(([n,v]) => window.__SHARING_ENTRY__[n](v), [name,value])
    const open = async query => { await page.goto(`${base}?${query}`); await expect.poll(async () => (await state())?.published).toBe(true); await expect.poll(async () => (await state()).room).toBeTruthy() }
    const shot = async name => { const file = `${name}-${width}.png`; await page.screenshot({ path: resolve(evidence,file), fullPage: true }); report.screenshots.push(file) }
    const detail = async () => { await page.getByRole('button',{name:'Open request',exact:true}).click(); await expect(page.getByRole('button',{name:'Reveal Instagram',exact:true})).toBeVisible() }
    const mutations = async () => (await state()).requests.filter(r => /social-handles|social-requests/.test(r.path) && r.method !== 'GET')
    const stable = async original => { const s = await state(); assert.equal(s.games,1); assert.equal(s.mounts,original.mounts); assert.equal(s.peak,1); assert.equal(s.sockets,original.sockets); assert.equal(s.activeSockets,await page.locator('[data-panel-size=full]').count() ? 0 : 1); assert.equal(s.sameCamera,true); assert.equal(s.socialFactories,original.socialFactories); assert.equal(s.geometry,original.geometry); assert.equal(s.queryEntries,0) }
    stage = `${width}/handles`; await open('path=/me/social')
    const original = await state(), host = await page.locator('[data-world-host]').elementHandle()
    await shot('handles-shell')
    await page.getByRole('button',{name:'Expand panel',exact:true}).click()
    await page.getByLabel('Replacement Instagram account name',{exact:true}).fill('private_entry_draft')
    await page.getByLabel('Replacement Instagram account name',{exact:true}).focus(); await shot('handles-keyboard')
    await page.getByRole('link',{name:'Requests',exact:true}).click(); await detail()
    assert.equal((await state()).requests.filter(r => r.path.includes('/reveal')).length,0)
    await page.getByRole('button',{name:'Reveal Instagram',exact:true}).click(); await page.getByText('@synthetic_private_entry_name',{exact:true}).scrollIntoViewIfNeeded(); await expect(page.getByText('@synthetic_private_entry_name',{exact:true})).toBeInViewport(); await shot('detail-revealed')
    await page.getByRole('button',{name:'Hide account name',exact:true}).click(); await expect(page.getByText('@synthetic_private_entry_name',{exact:true})).toHaveCount(0)
    await page.getByRole('button',{name:'Back to requests',exact:true}).click(); await expect(page.getByRole('button',{name:'Open request',exact:true})).toBeVisible(); await shot('requests')
    await page.getByRole('link',{name:'Social links',exact:true}).click(); await expect(page.getByLabel('Replacement Instagram account name',{exact:true})).toHaveValue('private_entry_draft')
    await stable(original); assert.equal(await host.evaluate(el => el === document.querySelector('[data-world-host]')),true)
    // Canonical handle write invalidates the old privacy lease; the fixture denies unreviewed policy reentry.
    await page.getByRole('button',{name:'Replace privately',exact:true}).click(); await expect(page.getByRole('heading',{name:'Your event is unavailable',exact:true})).toBeVisible(); assert.equal((await mutations()).length,1); assert.equal((await state()).games,0)
    await action('dispose'); assert.equal((await state()).activeSockets,0)

    stage = `${width}/uncertain-handle`; await open('path=/me/social'); await action('mode','uncertain'); await page.getByLabel('Replacement Instagram account name',{exact:true}).fill('uncertain_name'); await page.getByRole('button',{name:'Replace privately',exact:true}).click(); await expect(page.getByRole('button',{name:'Check change status',exact:true})).toBeVisible()
    const handleCommand=await mutations(); assert.equal(handleCommand.length,1); assert(handleCommand[0].key)
    await page.getByRole('link',{name:'Requests',exact:true}).click(); await expect(page.getByRole('button',{name:'Stay with current action',exact:true})).toBeInViewport(); assert.equal((await state()).path,'/me/social'); await page.getByRole('button',{name:'Stay with current action',exact:true}).click()
    await action('mode','valid'); await page.getByRole('button',{name:'Check change status',exact:true}).click(); await expect.poll(async()=>(await state()).requests.filter(r=>r.path.endsWith('/commands')).length).toBe(1); await expect(page.getByLabel('Replacement Instagram account name',{exact:true})).toHaveValue('uncertain_name'); assert.deepEqual(await mutations(),handleCommand); await action('dispose')

    stage = `${width}/uncertain`; await open('path=/me/requests'); await action('pending'); await action('mode','uncertain')
    await page.getByRole('button',{name:'Open request',exact:true}).click(); await page.getByRole('button',{name:'Reject request',exact:true}).click(); await expect(page.getByRole('button',{name:'Check change status',exact:true})).toBeVisible()
    const before = await state(), sent = await mutations(); assert.equal(sent.length,1); assert(sent[0].key)
    await expect(page.getByRole('button',{name:'Back to requests',exact:true})).toBeDisabled()
    for (const navigation of ['link','back','pop']) {
      if (navigation === 'link') await page.getByRole('link',{name:'Social links',exact:true}).click()
      if (navigation === 'back') await page.getByRole('button',{name:'Back to previous panel',exact:true}).click()
      if (navigation === 'pop') await action('back')
      await expect(page.getByRole('button',{name:'Stay with current action',exact:true})).toBeInViewport(); await expect(page.getByRole('button',{name:'Continue navigation',exact:true})).toBeDisabled(); assert.equal((await state()).path,'/me/requests')
      await page.getByRole('button',{name:'Stay with current action',exact:true}).click()
    }
    await stable(before)
    await page.getByRole('link',{name:'Social links',exact:true}).click(); await expect(page.getByRole('button',{name:'Stay with current action',exact:true})).toBeInViewport(); await shot('uncertain'); await page.getByRole('button',{name:'Stay with current action',exact:true}).click()
    await action('mode','foreign-receipt'); await page.getByRole('button',{name:'Check change status',exact:true}).click(); await expect(page.getByRole('button',{name:'Check change status',exact:true})).toBeEnabled(); await expect(page.getByRole('button',{name:'Back to requests',exact:true})).toBeDisabled()
    await action('mode','resolved'); await page.getByRole('button',{name:'Check change status',exact:true}).click(); await expect(page.getByText('The server confirmed this change.',{exact:true})).toBeVisible(); assert.deepEqual(await mutations(),sent); await stable(before)
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth),false)
    await action('dispose'); assert.equal((await state()).games,0); assert.equal((await state()).activeSockets,0)

    for (const transition of ['policy','identity','read-only','dispose']) {
      stage = `${width}/uncertain-${transition}`; await open('path=/me/requests'); await action('pending'); await action('mode','uncertain')
      await page.getByRole('button',{name:'Open request',exact:true}).click(); await page.getByRole('button',{name:'Reject request',exact:true}).click(); await expect(page.getByRole('button',{name:'Check change status',exact:true})).toBeVisible()
      const originalCommand=await mutations(); await action(transition === 'read-only' ? 'readOnly' : transition)
      await expect(page.getByRole('button',{name:'Check change status',exact:true})).toHaveCount(0)
      if(transition === 'read-only') { await page.getByRole('button',{name:'Open request',exact:true}).click(); await expect(page.getByRole('button',{name:'Reject request',exact:true})).toBeDisabled() }
      assert.deepEqual(await mutations(),originalCommand); assert.equal((await state()).peak,1); await action('dispose'); assert.equal((await state()).activeSockets,0)
    }

    for (const transition of ['policy','identity','readiness','detail-back','route','dispose','401']) {
      stage = `${width}/${transition}`; await open('path=/me/requests'); await action('mode',transition === '401' ? 'denied-reveal' : 'late-reveal'); await detail(); await page.getByRole('button',{name:'Reveal Instagram',exact:true}).click()
      await expect.poll(async () => (await state()).requests.filter(r => r.path.includes('/reveal')).length).toBe(1)
      if (transition === 'policy') await action('policy')
      if (transition === 'identity') await action('identity')
      if (transition === 'readiness') await action('sharingReady',false)
      if (transition === 'detail-back') await page.getByRole('button',{name:'Back to requests',exact:true}).click()
      if (transition === 'route') await action('navigate','/me/social')
      if (transition === 'dispose') await action('dispose')
      await action('releaseReveal'); await expect(page.getByText('@synthetic_private_entry_name',{exact:true})).toHaveCount(0)
      if (transition === '401') await expect(page.getByRole('heading',{name:'Sign in to continue',exact:true})).toBeVisible()
      const end = await state(); assert.equal(end.requests.filter(r => r.path.includes('/reveal')).length,1); assert.equal(end.requests.filter(r => r.path.includes('/auth/refresh')).length,0); assert.equal(end.queryEntries,0)
      await action('dispose'); assert.equal((await state()).activeSockets,0)
    }
    stage = `${width}/read-only`; await open('path=/me/social&readonly=1'); await expect(page.getByLabel('Replacement Instagram account name',{exact:true})).toBeDisabled(); await expect(page.getByRole('button',{name:'Remove Instagram',exact:true})).toBeEnabled(); await shot('read-only'); assert.equal((await mutations()).length,0); await action('dispose')
    stage = `${width}/unavailable`; await open('path=/me/social&ready=0'); await expect(page.getByText('This section is unavailable for the current account or event.',{exact:true})).toBeVisible(); assert.equal((await state()).requests.filter(r=>/social-handles|social-requests/.test(r.path)).length,0); await shot('unavailable')
    const unready=await state(); await action('sharingReady',true); await expect(page.getByLabel('Replacement Instagram account name',{exact:true})).toBeEnabled(); await stable(unready); await action('sharingReady',false); await expect(page.getByText('This section is unavailable for the current account or event.',{exact:true})).toBeVisible(); await action('sharingReady',true); await expect(page.getByLabel('Replacement Instagram account name',{exact:true})).toHaveValue(''); await action('dispose')
    stage = `${width}/empty`; await open('path=/me/requests'); await action('mode','empty'); await page.getByRole('button',{name:'Refresh requests',exact:true}).click(); await expect(page.getByText('No sharing requests.',{exact:true})).toBeVisible(); await shot('empty'); await action('dispose')
    stage = `${width}/error`; await open('path=/me/requests'); await action('mode','error'); await page.getByRole('button',{name:'Refresh requests',exact:true}).click(); await expect(page.getByRole('button',{name:'Retry read',exact:true})).toBeVisible(); await shot('error'); await action('dispose')
    report.cases.push({width,height,uncertainKey:sent[0].key,uncertainWrites:1,uncertainWithdrawalTransitions:4,lateRevealTransitions:7,peakGames:1,queryEntries:0,disposal:'no active sockets/game'}); await page.close()
  }
  assert.deepEqual(report.errors,[]); report.result='PASS'
} catch (error) { report.failure={stage,error:String(error),state:await page?.evaluate(()=>window.__SHARING_ENTRY__?.state()),text:await page?.locator('body').innerText()}; await page?.screenshot({path:resolve(evidence,'failure.png')}); throw error }
finally { await writeFile(resolve(evidence,'report.json'),JSON.stringify(report,null,2)+'\n'); await browser?.close(); await server.close() }
