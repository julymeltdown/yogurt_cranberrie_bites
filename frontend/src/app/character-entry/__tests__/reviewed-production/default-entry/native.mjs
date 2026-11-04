import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const frontend = resolve(import.meta.dirname, '../../../../../..')
const evidence = resolve(process.env.MCR_REVIEWED_NATIVE_OUTPUT ?? resolve(frontend, '../docs/mcr/evidence/customize-root-adoption/native-default-round01'))
const scratch = process.env.MCR_REVIEWED_NATIVE_SCRATCH
if (!scratch) throw Error('MCR_REVIEWED_NATIVE_SCRATCH must name the allocated private scratch directory')
await mkdir(evidence, { recursive: true })
const fixture = '/src/app/character-entry/__tests__/reviewed-production/default-entry/preview.html'
const persistenceMotion = process.env.MCR_REVIEWED_PERSISTENCE_MOTION === '1'
const reviewedSchema = JSON.parse(await readFile(resolve(frontend, '../contracts/assets/v2/reviewed/avatar-manifest.schema.json'), 'utf8'))
const initialParticipant = JSON.parse(await readFile(resolve(frontend, '../contracts/fixtures/core/bootstrap-open.json'), 'utf8')).participant
const durable = { avatar: { revision: 7, config: reviewedSchema.$defs.ReviewedMotionAvatarManifest.const.presets[0].config },
  participantRevision: initialParticipant.revision, reads: 0, writes: [] }
const run = new Date().toISOString().replaceAll(/[:.]/g, '-')
const input = { revision: 'TEST_ONLY_REVIEWED_ROOT_not_C05_authority', reviewedBy: 'Synthetic default-entry fixture',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
const server = await createServer({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner',
  cacheDir: resolve(scratch, 'vite'), logLevel: 'warn',
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
    'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)) },
  plugins: [{ name: 'reviewed-actual-main-fixture', configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (request.url === '/__reviewed_persistence_motion/state') {
        response.setHeader('Content-Type', 'application/json'); response.setHeader('Cache-Control', 'no-store')
        if (request.method === 'GET') { durable.reads++; response.end(JSON.stringify(durable)); return }
        if (request.method !== 'PUT') { response.statusCode=405;response.end('{}');return }
        try {
          let text='';for await (const chunk of request) { text+=chunk; if(text.length>32768)throw Error('Fixture request too large') }
          const body=JSON.parse(text)
          assert.equal(body.expected_revision,durable.avatar.revision)
          assert.equal(body.config.manifest_id,'mcr_selected_motion_reviewed_20261104')
          assert.equal(body.config.manifest_version,2)
          assert(['motion_layered','motion_preset'].includes(body.config.representation))
          durable.avatar={revision:durable.avatar.revision+1,config:body.config};durable.participantRevision++
          durable.writes.push({expectedRevision:body.expected_revision,acceptedRevision:durable.avatar.revision})
          response.end(JSON.stringify(durable))
        } catch(error) { response.statusCode=409;response.end(JSON.stringify({code:'CONFLICT',message:String(error),details:{reason:'VERSION_CONFLICT'}})) }
        return
      }
      if (!request.headers.accept?.includes('text/html')) return next()
      const html = await readFile(resolve(frontend, `.${fixture}`), 'utf8')
      response.setHeader('Content-Type', 'text/html'); response.end(await server.transformIndexHtml(request.url, html))
    })
  } }], server: { host: '127.0.0.1', port: 0, strictPort: false, watch: null } })
const report = { run, result: 'FAIL', qualification: 'ACTUAL_MAIN_SYNTHETIC_HTTP_C05_REVIEWED_C06_NOT_LIVE', widths: [], screenshots: [], errors: [] }
let browser, stage = 'start', diagnosticPage
const networkFailures=[]
try {
  await server.listen(); const base = `http://127.0.0.1:${server.httpServer.address().port}`; report.server = base
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const [width, height] of (persistenceMotion ? [[390,844]] : [[360,800], [390,844], [430,932]])) {
    // Preserve the historical full matrix's fresh fixture per context; persistence spans only the narrow reload.
    if (!persistenceMotion) { durable.avatar={revision:7,config:reviewedSchema.$defs.ReviewedMotionAvatarManifest.const.presets[0].config};durable.participantRevision=initialParticipant.revision;durable.reads=0;durable.writes=[] }
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    const page = await context.newPage(); diagnosticPage=page; page.on('pageerror', error => report.errors.push(error.message))
    page.on('response',response=>{if(response.status()>=400)networkFailures.push({url:response.url(),status:response.status()})})
    // The new narrow journey runs natural Date/performance/RAF. Retain the historical full mode's wall-date fixture.
    if (!persistenceMotion) await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
    const state = () => page.evaluate(() => window.__REVIEWED_ROOT_ENTRY__.state())
    const shot = async label => { const file = `${run}-${label}-${width}.png`; await page.screenshot({ path: resolve(evidence,file), fullPage:true }); report.screenshots.push(file); assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)) }
    const nav = page.getByRole('navigation', { name: 'Main navigation' })
    stage = `${width}/login`
    await page.goto(`${base}/login`)
    await page.getByLabel('Email', { exact: true }).fill('fixture@example.test'); await page.getByLabel('Password', { exact: true }).fill('fixture-password')
    await page.getByRole('button', { name: 'Sign in with email' }).click()
    await expect.poll(async () => (await state()).canvases).toBe(1)
    await expect.poll(async () => (await state()).active).toBe(1)
    await expect(page.getByRole('region', { name: 'World connection status' })).toHaveCount(0)
    await expect.poll(async()=>{try{return (await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof())).actors.filter(actor=>actor.visible&&actor.cell.alpha>0).length}catch{return 0}},{timeout:10000}).toBeGreaterThan(1)
    const worldBefore=await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof())
    for(const actor of worldBefore.actors){assert(actor.visible);assert(actor.cell.alpha>0);assert.equal(actor.sourceWidth,320);assert.equal(actor.sourceHeight,256)}
    assert(worldBefore.actors.find(actor=>actor.self)?.onCamera)
    assert(worldBefore.actors.some(actor=>!actor.self&&actor.onCamera))
    await shot('reviewed-world-before')
    if (persistenceMotion) {
      stage = '390/save-before-hard-reload'
      const oldDocument = (await state()).documentId
      await nav.getByRole('link',{name:'Me',exact:true}).click()
      await page.getByRole('link',{name:'Dress up',exact:true}).click()
      const editor=page.getByRole('region',{name:'Avatar editor'})
      await editor.getByRole('button',{name:'Dress up',exact:true}).click()
      await editor.getByRole('button',{name:'Skin / body',exact:true}).click()
      await editor.getByRole('button',{name:'Skin tone 8',exact:true}).click()
      const beforeRevision=(await state()).avatar.revision
      await editor.getByRole('button',{name:'Save outfit',exact:true}).click()
      await expect.poll(async()=>(await state()).avatar.revision).toBe(beforeRevision+1)
      await expect(editor.getByRole('button',{name:'Save outfit',exact:true})).toBeDisabled()
      const saved=structuredClone(durable.avatar)
      assert.equal(durable.writes.length,1);assert.equal(saved.config.item_ids.body,'motion_v2_body_7')
      await page.getByRole('button',{name:'Back to previous panel',exact:true}).click()
      await expect(page.locator('.me-avatar--presentation canvas')).toBeVisible()
      const savedProfile=(await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof())).profile
      assert(savedProfile?.alpha>0);await shot('persisted-profile-before-reload')
      stage='390/explicit-hard-reload-and-real-ui-login'
      await page.reload({waitUntil:'domcontentloaded'})
      // Tokens are memory-only. A new document legitimately signs in through the real UI again.
      await page.getByRole('link',{name:'Sign in',exact:true}).click()
      await expect(page.getByLabel('Email',{exact:true})).toBeVisible()
      await page.getByLabel('Email',{exact:true}).fill('fixture@example.test')
      await page.getByLabel('Password',{exact:true}).fill('fixture-password')
      await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
      await expect.poll(async()=>(await state()).canvases).toBe(1)
      // A reloaded full Profile intentionally pauses World networking until it closes.
      assert.notEqual((await state()).documentId,oldDocument);assert(durable.reads>=2)
      assert.deepEqual((await state()).avatar,saved);assert.equal(durable.writes.length,1)
      if (!await page.locator('.me-avatar--presentation canvas').isVisible()) await nav.getByRole('link',{name:'Me',exact:true}).click()
      await expect(page.locator('.me-avatar--presentation canvas')).toBeVisible()
      const reloadedProfile=(await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof())).profile
      assert.deepEqual(reloadedProfile,savedProfile);await shot('persisted-profile-after-hard-reload')
      await page.getByRole('link',{name:'Back to world',exact:true}).last().click()
      await expect.poll(async()=>(await state()).active).toBe(1)
      const proof=()=>page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof())
      await expect.poll(async()=>{try{return (await proof()).actors.find(a=>a.self)?.cell.sha256}catch{return null}}).toBe(savedProfile.sha256)
      const reloadedWorld=await proof(),selfBefore=reloadedWorld.actors.find(a=>a.self)
      stage='390/actual-world-keyboard-input-and-incumbent-frame-clock'
      const joystick=page.getByRole('button',{name:'Movement joystick. Arrow keys or W A S D when focused',exact:true})
      await expect(joystick).toBeEnabled();await joystick.focus();await page.keyboard.down('ArrowRight')
      await expect.poll(async()=>(await state()).inputs.filter(v=>v.axisX>0).length).toBeGreaterThan(0)
      const selfSamples=[]
      for(let i=0;i<18;i++){selfSamples.push((await proof()).actors.find(a=>a.self));await page.waitForTimeout(40)}
      await page.keyboard.up('ArrowRight')
      assert(selfSamples.some(a=>a.x>selfBefore.x+2))
      const selfFrames=[...new Set(selfSamples.filter(a=>a.authoritative.moving).map(a=>Number(a.frame)))].filter(f=>f>=11&&f<=14)
      assert(selfFrames.length>=2,'Actual self walk must advance on the incumbent frame clock')
      await expect.poll(async()=>{const a=(await proof()).actors.find(a=>a.self);return !a.authoritative.moving&&Number(a.frame)===10}).toBe(true)
      const selfStopped=(await proof()).actors.find(a=>a.self)
      stage='390/controlled-authoritative-peer-delta-through-real-transport'
      const peerId=await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.peerMotion(true))
      const peerBefore=reloadedWorld.actors.find(a=>a.userId===peerId),peerSamples=[]
      for(let i=0;i<18;i++){peerSamples.push((await proof()).actors.find(a=>a.userId===peerId));await page.waitForTimeout(40)}
      const peerFrames=[...new Set(peerSamples.map(a=>Number(a.frame)))].filter(f=>f>=6&&f<=9)
      assert(peerFrames.length>=2,'Actual peer walk must advance on the incumbent frame clock')
      assert(peerSamples.some(a=>a.x>peerBefore.x+4));assert(peerSamples.some(a=>a.texture!==peerBefore.texture))
      assert(peerSamples.some(a=>a.authoritative.avatarRevision===peerBefore.authoritative.avatarRevision+1))
      await shot('actual-world-peer-walk-after-reload')
      await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.peerMotion(false))
      await expect.poll(async()=>{const a=(await proof()).actors.find(a=>a.userId===peerId);return !a.authoritative.moving&&Number(a.frame)===5}).toBe(true)
      const finalWorld=await proof(),finalSelf=finalWorld.actors.find(a=>a.self)
      assert.deepEqual(finalSelf.cell,selfStopped.cell);assert.equal(finalWorld.canvasCount,1)
      assert.equal((await state()).peak,1);assert.equal((await state()).active,1)
      assert((await state()).serverUpdates.some(v=>v.ackInputSeq>0))
      const inputProof=(await state()).inputs,updates=(await state()).serverUpdates
      stage='390/canonical-signout-private-cleanup'
      await nav.getByRole('link',{name:'Me',exact:true}).click()
      await page.getByText('Profile options',{exact:true}).click()
      await page.getByRole('link',{name:'Account',exact:true}).click()
      await page.getByRole('button',{name:'Sign out',exact:true}).click()
      await page.getByRole('button',{name:'Confirm sign out',exact:true}).click()
      await expect.poll(async()=>(await state()).canvases).toBe(0)
      await expect.poll(async()=>(await state()).active).toBe(0)
      await expect.poll(async()=>(await state()).blobURLs).toBe(0)
      report.widths.push({width,height,result:'PASS',actualMain:true,qualification:'NARROW_PERSISTENCE_MOTION_CONTROLLED_HTTP_C05_NOT_LIVE',
        explicitHardReload:true,newDocument:true,canonicalUILoginAfterReload:true,externalHTTPAuthority:{reads:durable.reads,writes:durable.writes,saved},
        savedProfile,reloadedProfile,reloadedWorld,actualInput:inputProof,controlledAuthoritativeUpdates:updates,
        selfMotion:{before:selfBefore,samples:selfSamples,walkFrames:selfFrames,stopped:selfStopped},
        peerMotion:{before:peerBefore,samples:peerSamples,walkFrames:peerFrames,after:finalWorld},
        actualAnimationClock:'Unmodified incumbent Phaser scene; no fake Date/performance/RAF or injected frame sampling',
        peakActiveSocketPerDocument:1,finalOwnedResources:0,
        reusedEvidence:'Published5da64426 reviewed-round01 all3width catalog/BODY/Map/FE41; not rerun or relabeled'})
      await context.close();continue
    }
    stage = `${width}/editor`
    await nav.getByRole('link', { name: 'Me', exact: true }).click()
    await page.getByRole('link', { name: 'Dress up', exact: true }).click()
    const editor = page.getByRole('region', { name: 'Avatar editor' })
    // The editor is an aria-labelled section. Do not replace its controller or mount it separately.
    await expect(editor.getByRole('button', { name: 'Random character', exact: true })).toBeEnabled()
    await expect(editor.getByRole('button', { name: /^Choose / })).toHaveCount(6)
    await editor.getByRole('button', { name: 'Dress up', exact: true }).click()
    const categories = [ ['Hair',6], ['Outfit',6], ['Accessory',4], ['Skin / body',8], ['Eyes',4] ]
    for (const [name,count] of categories) {
      await editor.getByRole('button', { name, exact:true }).click()
      const options = editor.locator('.mcr-motion-select select')
      await expect(options.locator('option')).toHaveCount(count + (name==='Outfit'||name==='Accessory'?1:0))
      await options.selectOption({ index: count - 1 })
    }
    await editor.getByRole('button', { name: 'Skin / body', exact:true }).click()
    const palette = editor.getByRole('group', { name:'Skin tones', exact:true })
    await expect(palette.getByRole('button')).toHaveCount(8)
    await palette.getByRole('button', { name:'Skin tone 8', exact:true }).click()
    await expect(palette.getByRole('button', { name:'Skin tone 8', exact:true })).toHaveAttribute('aria-pressed','true')
    await editor.getByLabel('View', { exact:true }).selectOption('left')
    await editor.getByRole('button', { name:'Preview walk', exact:true }).click()
    await editor.getByRole('button', { name:'Stop walking', exact:true }).click()
    await shot('reviewed-all-layer-draft')
    await editor.getByRole('button', { name:'Random character', exact:true }).click()
    await editor.getByRole('button', { name:'Reset draft', exact:true }).click()
    await expect(editor.getByRole('button', { name:'Save outfit', exact:true })).toBeDisabled()
    await editor.getByRole('button', { name:'Dress up', exact:true }).click()
    await editor.getByRole('button', { name:'Skin / body', exact:true }).click()
    await palette.getByRole('button', { name:'Skin tone 8', exact:true }).click()
    const before = await state()
    await editor.getByRole('button', { name:'Save outfit', exact:true }).click()
    await expect.poll(async () => (await state()).avatar.revision).toBe(before.avatar.revision+1)
    await expect(editor.getByRole('button', { name:'Save outfit', exact:true })).toBeDisabled()
    await shot('reviewed-saved')
    stage = `${width}/profile-and-world-readback`
    // Mobile full panels intentionally hide the bottom navigation. Use their real parent action.
    await page.getByRole('button', { name:'Back to previous panel',exact:true }).click()
    await expect(page.locator('#panel-title')).toHaveText('My profile')
    await expect(page.locator('.me-avatar--presentation canvas')).toBeVisible()
    assert.equal((await state()).avatar.config.representation,'motion_layered')
    const profileProof=await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof())
    assert(profileProof.profile&&profileProof.profile.alpha>0)
    await shot('reviewed-profile-readback')
    await page.getByRole('button', { name:'Back to previous panel',exact:true }).click()
    await expect.poll(async () => (await state()).canvases).toBe(1)
    assert.equal((await state()).peak,1); assert.equal((await state()).active,1)
    await expect.poll(async()=>{try{const value=await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof());return value.actors.find(actor=>actor.self)?.cell.sha256}catch{return null}}).toBe(profileProof.profile.sha256)
    const worldAfter=await page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof())
    const beforeSelf=worldBefore.actors.find(actor=>actor.self),afterSelf=worldAfter.actors.find(actor=>actor.self)
    assert.notEqual(afterSelf.cell.sha256,beforeSelf.cell.sha256)
    for(const peer of worldBefore.actors.filter(actor=>!actor.self)){const next=worldAfter.actors.find(actor=>actor.userId===peer.userId);assert(next?.visible);assert.deepEqual(next.cell,peer.cell)}
    await shot('reviewed-world-after')
    // The registry deliberately retains scoped artwork across routes while signed in.
    // Canonical sign-out is the privacy boundary that retires its assets and URLs.
    stage = `${width}/canonical-ui-signout-cleanup`
    await nav.getByRole('link', { name:'Me',exact:true }).click()
    await page.getByText('Profile options', { exact:true }).click()
    await page.getByRole('link', { name:'Account',exact:true }).click()
    await page.getByRole('button', { name:'Sign out',exact:true }).click()
    await page.getByRole('button', { name:'Confirm sign out',exact:true }).click()
    await expect.poll(async () => (await state()).canvases).toBe(0)
    await expect.poll(async () => (await state()).active).toBe(0)
    await expect.poll(async () => (await state()).blobURLs).toBe(0)
    report.widths.push({ width,height,result:'PASS', actualMain:true, uiLogin:true, allCatalogLayers:true, eightQualifiedBodyPaletteChoices:true, directionAndWalk:true, randomReset:true, saveAndProfileReadback:true, actualIncumbentWorldSelfAndPeer:{before:worldBefore,profile:profileProof.profile,after:worldAfter}, peakActiveSocket:1, cleanupBoundary:'CANONICAL_UI_SIGNOUT', finalOwnedResources:0 })
    await context.close()
  }
  assert.deepEqual(report.errors,[]);report.result='PASS'
} catch(error) {
  report.errors.push(`${stage}: ${error.stack??error}`);report.networkFailures=networkFailures
  if(diagnosticPage&&!diagnosticPage.isClosed()){
    report.failureState=await diagnosticPage.evaluate(()=>({url:location.href,text:document.body.innerText,state:window.__REVIEWED_ROOT_ENTRY__?.state()})).catch(()=>null)
    const file=`${run}-failure.png`;await diagnosticPage.screenshot({path:resolve(evidence,file),fullPage:true}).then(()=>report.screenshots.push(file)).catch(()=>{})
  }
  throw error
}
finally { await browser?.close();await server.close();await writeFile(resolve(evidence,`${run}-native.json`),JSON.stringify(report,null,2)+'\n') }
console.log(JSON.stringify(report))
