import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const frontend=resolve(import.meta.dirname,'../../../..'),evidence=resolve(import.meta.dirname,'evidence')
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:'/tmp/mcr-default-social-cache',server:{host:'127.0.0.1',port:0},logLevel:'warn'})
await server.listen()
const {post,publicIntro}=await server.ssrLoadModule('/src/app/social-composition/__tests__/fixtures.ts')
const intro=publicIntro(),report={result:'FAIL',qualification:'Actual default createAppRoutes/WorldShell/SocialFeatures, canonical Auth/HTTP, existing one Phaser/production world owner and C05 MockSocket. Deterministic in-process C04 fixture/readiness and geometric terrain; no live backend, approved avatar/media/provider, sharing, two-user, device or load acceptance.',cases:[],screenshots:[],errors:[]}
let browser,currentPage,stage='start'
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 const base=`http://127.0.0.1:${server.httpServer.address().port}`
 for(const [width,height]of [[360,800],[390,844],[430,932]]){
  const page=await browser.newPage({viewport:{width,height},hasTouch:true,reducedMotion:'reduce'});currentPage=page;page.on('pageerror',e=>report.errors.push(e.message))
  const state=()=>page.evaluate(()=>window.__DEFAULT_QUALIFIED__?.state())
  const nav=path=>page.evaluate(p=>window.__DEFAULT_QUALIFIED__.navigate(p),path)
  const shot=async name=>{const file=`social-default-${name}-${width}.png`;await page.screenshot({path:resolve(evidence,file),fullPage:true});report.screenshots.push(file)}
  stage=`${width}/ready`;await page.goto(`${base}/src/app/__tests__/default-shell/qualified.html?social=1&path=/feed`);await expect.poll(async()=>(await state())?.published).toBe(true);await expect.poll(async()=>(await state()).room).toBeTruthy();await expect(page.getByText(post.text,{exact:true})).toBeVisible()
  const initial=await state(),host=await page.locator('[data-world-host]').elementHandle()
  const stable=async()=>{const s=await state();assert.equal(s.mounts,1);assert.equal(s.games,1);assert.equal(s.peak,1);assert.equal(s.sockets,1);assert.equal(s.activeSockets,1);assert.equal(s.sameCamera,true);assert.equal(s.socialFactories,initial.socialFactories);assert.equal(s.geometry,initial.geometry);assert.equal(await host.evaluate(el=>el===document.querySelector('[data-world-host]')),true)}
  await page.getByRole('button',{name:'Write a post',exact:true}).click();await page.getByLabel('Post text',{exact:true}).fill('Retained default application draft')
  await page.getByRole('link',{name:'Inbox',exact:true}).click();await expect(page.getByText('37 unread in this event',{exact:true})).toBeVisible();await shot('inbox')
  await page.getByRole('button',{name:'Open update',exact:true}).click();await expect(page.getByRole('heading',{name:'Thread',exact:true,level:1})).toBeVisible();assert((await state()).requests.filter(r=>r.path.endsWith(`/posts/${post.post_id}`)).length>=2)
  await nav('/meet/people');await expect(page.getByRole('heading',{name:'PEOPLE',exact:true})).toBeVisible();await page.getByRole('link',{name:intro.author.display_name,exact:true}).click();await expect(page.getByRole('heading',{name:intro.author.display_name,exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Request sharing'})).toBeDisabled();await shot('profile')
  await nav('/meet/intro/edit');await expect(page.getByLabel('Introduction',{exact:true})).toBeEnabled();await page.getByLabel('Introduction',{exact:true}).fill('Keep introduction draft')
  await nav('/me/age');await expect(page.getByRole('button',{name:'Start verification',exact:true})).toBeDisabled();await nav('/meet/intro/edit');await expect(page.getByLabel('Introduction',{exact:true})).toHaveValue('Keep introduction draft')
  await nav('/feed/new');await expect(page.getByLabel('Post text',{exact:true})).toHaveValue('Retained default application draft');await stable()
  stage=`${width}/uncertain`;await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.mode('uncertain'));await page.getByRole('button',{name:'Publish post',exact:true}).click();await expect(page.getByRole('button',{name:'Check command status',exact:true})).toBeVisible()
  const writes=()=>state().then(s=>s.requests.filter(r=>r.method==='POST'&&!r.path.includes('/auth/')&&!r.path.endsWith('/world-tickets')))
  const sent=await writes();assert.equal(sent.length,1);assert(sent[0].key)
  await page.getByRole('link',{name:'Inbox',exact:true}).click();await expect(page.getByRole('button',{name:'Stay with current action',exact:true})).toBeInViewport();await expect(page.getByRole('button',{name:'Continue navigation',exact:true})).toBeDisabled();assert.equal((await state()).path,'/feed/new');await expect(page.getByLabel('Post text',{exact:true})).toHaveValue('Retained default application draft')
  await page.getByRole('button',{name:'Stay with current action',exact:true}).click();await page.evaluate(()=>{void window.__DEFAULT_QUALIFIED__.back()});await expect(page.getByRole('button',{name:'Stay with current action',exact:true})).toBeInViewport();assert.equal((await state()).path,'/feed/new');assert.deepEqual(await writes(),sent);await stable();await shot('pending')
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport()
  await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.policy());await expect(page.getByLabel('Post text',{exact:true})).toHaveCount(0);await expect.poll(async()=>(await state()).games).toBe(0);assert.deepEqual(await writes(),sent)
  await page.evaluate(()=>window.__DEFAULT_QUALIFIED__.dispose());const end=await state();assert.equal(end.activeSockets,0);report.cases.push({width,height,mounts:end.mounts,peak:end.peak,posts:sent.length,retainedKey:sent[0].key,disposals:end.disposals,sceneClosed:end.sceneClosed});await page.close()
 }
 assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(error){report.failure={stage,error:String(error),state:await currentPage?.evaluate(()=>window.__DEFAULT_QUALIFIED__?.state()),text:await currentPage?.locator('body').innerText()};await currentPage?.screenshot({path:resolve(evidence,'social-default-failure.png')});throw error}finally{await writeFile(resolve(evidence,'social-native.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close()}
