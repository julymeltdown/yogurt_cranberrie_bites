// Owner qualification only: actual default app routes; synthetic HTTP/C05, no physical-device acceptance.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {execFileSync} from 'node:child_process'
import {mkdir,readFile,writeFile,statfs,rm} from 'node:fs/promises'
import {resolve,extname} from 'node:path'
import {createServer as netServer} from 'node:net'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const allocation=process.env.MCR_NATIVE_SLOT;assert(allocation?.startsWith('FE21-FE24-NATIVE-'),'Requires explicit named coordinator slot')
const repo=resolve(import.meta.dirname,'../../../..'),frontend=resolve(repo,'frontend'),out=resolve(repo,'docs/mcr/evidence/fe21-fe24-client/native')
const disk=await statfs(repo);assert(disk.bavail*disk.bsize>=16*2**30,'Expanded slot requires >=16 GiB at launch; stop floor 8 GiB')
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:64*1024*1024}),hash=x=>createHash('sha256').update(x).digest('hex')
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n')),assets=new Map(),assetReads=[]
const reservation=netServer();await new Promise(r=>reservation.listen(0,'127.0.0.1',r));const port=reservation.address().port;await new Promise(r=>reservation.close(r))
const cacheDir=`/tmp/mcr-fe21-fe24-204-${process.pid}`
const server=await createServer({root:frontend,cacheDir,configLoader:'runner',logLevel:'error',plugins:[{name:'readonly-sparse-assets',configureServer(s){s.middlewares.use((req,res,next)=>{
 const path='frontend/public'+decodeURIComponent(new URL(req.url,'http://localhost').pathname)
 if(!tracked.has(path)||!['.png','.jpg','.webp','.ttf'].includes(extname(path)))return next()
 try{if(!assets.has(path)){const bytes=git('show',`HEAD:${path}`);assets.set(path,bytes);assetReads.push({path,sha256:hash(bytes),bytes:bytes.length})}res.setHeader('Content-Type',extname(path)==='.ttf'?'font/ttf':`image/${extname(path).slice(1)}`);res.end(assets.get(path))}catch(e){next(e)}
 })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false}})
const report={base:git('rev-parse','HEAD').toString().trim(),clock:'2026-11-07T07:00:00.000Z',slot:allocation,diskFreeGiB:disk.bavail*disk.bsize/2**30,
 qualification:'ACTUAL_DEFAULT_ROOT_WITH_SYNTHETIC_HTTP_C05',physicalIOS:'NOT_RUN',physicalAndroid:'NOT_RUN',sourceHashes:{},cases:[],assetReads}
for(const p of git('ls-files','frontend/src/features/party-discovery','frontend/src/features/gift-discovery','frontend/src/features/my-gifts','frontend/src/app/CommunityFeatures.tsx','frontend/src/app/router.tsx','contracts/generated/mcr-domain.openapi.json','contracts/domain/v1/party.schema.json','contracts/domain/v1/giveaway.schema.json').toString().trim().split('\n').filter(p=>/\.(tsx?|css|json)$/.test(p)&&!p.includes('/evidence/')))report.sourceHashes[p]=hash(await readFile(resolve(repo,p)))
for(const p of ['data.ts','preview.tsx','preview.html','native.mjs','fixture.test.ts'])report.sourceHashes['frontend/e2e/community-client/fe21-fe24/'+p]=hash(await readFile(resolve(import.meta.dirname,p)))
let browser
const P='00000000-0000-4000-8000-000000000010',N='00000000-0000-4000-8000-000000000011',G='00000000-0000-4000-8000-000000000064'
const mode=(p,m)=>p.evaluate(m=>window.__COMMUNITY_CLIENT__.mode(m),m)
const status=(p,m)=>p.evaluate(m=>window.__COMMUNITY_CLIENT__.status(m),m)
const navigate=(p,path)=>p.evaluate(path=>window.__COMMUNITY_CLIENT__.navigate(path),path)
const calls=p=>p.evaluate(()=>window.__COMMUNITY_CLIENT__.calls())
async function layout(page){return page.evaluate(()=>{
 const roots=[...document.querySelectorAll('.party-view,.gifts-reference,.gifts-view')],controls=roots.flatMap(root=>[...root.querySelectorAll('a,button,input,select,textarea,summary')]).filter(e=>e.getClientRects().length)
 return {overflow:document.documentElement.scrollWidth>innerWidth,smallControls:controls.map(e=>{const r=(e.matches('input[type=checkbox]')?e.labels[0]:e).getBoundingClientRect();return {label:(e.getAttribute('aria-label')??e.textContent).trim().slice(0,80),width:r.width,height:r.height}}).filter(r=>r.width<43.5||r.height<43.5),state:window.__COMMUNITY_CLIENT__.state()}
})}
try{
 await mkdir(out,{recursive:true});await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version();report.node=process.version
 for(const width of [360,390,430]){
  const currentDisk=await statfs(repo);assert(currentDisk.bavail*currentDisk.bsize>=8*2**30,'Stop floor reached')
  const context=await browser.newContext({viewport:{width,height:844},isMobile:true,hasTouch:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),errors=[],entry={width,checks:[],captures:[],errors};report.cases.push(entry)
  page.on('pageerror',e=>errors.push(e.message));page.setDefaultTimeout(12000);await page.clock.setFixedTime(new Date(report.clock))
  const open=async(route='/meet',scenario='normal')=>{await page.evaluate(()=>window.__COMMUNITY_CLIENT__?.dispose()).catch(()=>{});await page.goto(`http://127.0.0.1:${port}/e2e/community-client/fe21-fe24/preview.html?`+new URLSearchParams({entry:route,mode:scenario}));await page.waitForFunction(()=>window.__COMMUNITY_CLIENT__?.state().published);await page.evaluate(()=>document.fonts.ready)}
  const capture=async name=>{const metrics=await layout(page);assert.equal(metrics.overflow,false);assert.deepEqual(metrics.smallControls,[]);assert.equal(metrics.state.canvases,1);assert(metrics.state.socketPeak<=1);const file=`${width}-${name}.png`;await page.screenshot({path:resolve(out,file)});entry.captures.push({file,...metrics})}
  try{
   await open();await expect(page.locator('.meet-party')).toHaveCount(2);await capture('parties')
   await page.locator('.party-filters summary').tap();await page.getByLabel('Ticket section',{exact:true}).fill('No match');await page.getByRole('button',{name:'Show parties',exact:true}).tap();await expect(page.getByText(/No parties match this page/)).toBeVisible();await page.getByLabel('Ticket section',{exact:true}).fill('');await page.getByRole('button',{name:'Show parties',exact:true}).tap()
   await page.getByRole('link',{name:'Before the encore',exact:true}).tap();const apply=page.getByRole('button',{name:'Apply to party',exact:true});await expect(apply).toBeEnabled();await page.getByRole('textbox',{name:/Note to the host/}).fill('Private owner qualification note')
   await mode(page,'lost');await apply.tap();await expect(page.getByText('The response was lost. The outcome is unknown.')).toBeVisible();await expect(apply).toBeDisabled();await capture('uncertain-application')
   await status(page,'404');await page.getByRole('button',{name:'Check original command status',exact:true}).tap();await expect(page.getByText('Command status is unavailable. This does not prove whether it ran.')).toBeVisible()
   await status(page,'PENDING');await page.getByRole('button',{name:'Check original command status',exact:true}).tap();await expect(page.getByText('The server is still processing this command.')).toBeVisible()
   await status(page,'SUCCEEDED');await page.getByRole('button',{name:'Check original command status',exact:true}).tap();await expect(page.getByText('Application state:')).toContainText('pending')
   const applied=await calls(page),posts=applied.filter(r=>r.method==='POST');assert.equal(posts.length,1);assert(applied.filter(r=>r.path.endsWith('/commands')).every(r=>new URLSearchParams(r.query).get('idempotency_key')===posts[0].key));entry.checks.push('FE21 lost response -> 404 -> PENDING -> SUCCEEDED + fresh MY PLAN; one POST, original key, no replay')
   await mode(page,'normal');await page.locator('#party-plan').tap();await page.getByRole('button',{name:'Cancel application',exact:true}).tap();await expect(page.getByText('Application state:')).toContainText('cancelled');assert.equal((await calls(page)).filter(r=>r.method==='DELETE').length,1);entry.checks.push('FE21 actual MY PLAN link and cancellation')
   await page.getByRole('button',{name:'Parties',exact:true}).tap();await mode(page,'full');await navigate(page,`/meet/parties/${N}`);await expect(page.getByText('This party is full.',{exact:true})).toBeVisible();await expect(apply).toBeDisabled()
   await mode(page,'cancelled');await navigate(page,`/meet/parties/${P}`);await expect(page.getByText('This party is cancelled.',{exact:true})).toBeVisible();await expect(apply).toBeDisabled()
   await mode(page,'private');await navigate(page,`/meet/parties/${N}`);await expect(page.getByText('Your current access does not allow this view.')).toBeVisible();await expect(page.getByRole('heading',{name:'After the encore',exact:true})).toHaveCount(0);entry.checks.push('FE21 empty/full/cancelled/private states')
   await mode(page,'normal');await navigate(page,'/gifts');await expect(page.locator('.gift-reference-row')).toHaveCount(3);await capture('gifts')
   await page.getByText('Filter giveaways',{exact:true}).tap();await page.getByRole('combobox',{name:'Status',exact:true}).selectOption('OPEN');await page.getByRole('combobox',{name:'Stock',exact:true}).selectOption('true');await page.getByLabel('Pickup starts from (KST)').fill('2026-11-07T15:00');await page.getByLabel('Pickup starts before (KST)').fill('2026-11-07T16:00');await page.getByRole('button',{name:'Apply filters',exact:true}).tap();await expect(page.locator('.gift-reference-row')).toHaveCount(1)
   const filtered=(await calls(page)).filter(r=>r.path.endsWith('/giveaways')&&!r.path.endsWith('/me/giveaways')).at(-1);assert.equal(new URLSearchParams(filtered.query).get('pickup_after'),'2026-11-07T06:00:00.000Z');entry.checks.push('FE24 current stock/status/KST time filters')
   await page.getByRole('link',{name:'Fan badges',exact:true}).tap();await expect(page.getByRole('heading',{name:'Fan badges',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Reserve unavailable',exact:true})).toBeDisabled();await expect(page.getByRole('link',{name:'Review reservation',exact:true})).toHaveAttribute('href',`/gifts/${G}/reservation`);await capture('gift-detail')
   await page.locator('#gift-detail-own').tap();await expect(page.getByText('Only you can see this draft.')).toBeVisible();await page.getByRole('button',{name:'Next giving',exact:true}).tap();await expect(page.getByRole('link',{name:'Your published badges',exact:true})).toBeVisible();await page.getByRole('button',{name:'RECEIVING',exact:true}).tap();await expect(page.getByText('Held',{exact:true})).toBeVisible();await page.getByRole('button',{name:'Next receiving',exact:true}).tap();await expect(page.getByRole('button',{name:'Next receiving',exact:true})).toBeDisabled();await capture('receiving')
   const activity=(await calls(page)).filter(r=>r.path.endsWith('/me/giveaways')).at(-1);assert.equal(new URLSearchParams(activity.query).get('giving_cursor'),'giving_next');assert.equal(new URLSearchParams(activity.query).get('receiving_cursor'),'receiving_next');entry.checks.push('FE24 real GIVING/RECEIVING route, own draft, independent cursor requests')
   await page.locator('#my-gifts-browse').tap();await page.getByText('Filter giveaways',{exact:true}).tap();await page.getByRole('button',{name:'Clear filters',exact:true}).tap();await expect(page.locator('.gift-reference-row')).toHaveCount(3)
   for(const [name,label] of [['Paused badges','Paused'],['Last badges','Sold out']]){await page.getByRole('link',{name,exact:true}).tap();await expect(page.locator('.gift-eyebrow')).toHaveText(label);await expect(page.getByRole('link',{name:'Review reservation',exact:true})).toHaveCount(0);await page.locator('#gift-back').tap()}
   await open('/gifts','ended');await page.getByRole('link',{name:'Fan badges',exact:true}).tap();await expect(page.locator('.gift-eyebrow')).toHaveText('Ended');await expect(page.getByRole('link',{name:'Review reservation',exact:true})).toHaveCount(0);await expect(page.getByRole('button',{name:'Reserve unavailable',exact:true})).toBeDisabled();entry.checks.push('FE24 paused/sold-out/ended prohibit reservation action')
   await open('/gifts','private-projection');await page.getByRole('link',{name:'Fan badges',exact:true}).tap();await expect(page.getByRole('heading',{name:'Gift information could not be verified',exact:true})).toBeVisible();await expect(page.getByText('MUST_NOT_RENDER_PRIVATE_RECIPIENT')).toHaveCount(0);entry.checks.push('FE24 public recipient projection rejected')
   await open('/gifts/mine');await expect(page.getByText('Only you can see this draft.')).toBeVisible();await page.evaluate(()=>window.__COMMUNITY_CLIENT__.revoke());await expect(page.getByText('Only you can see this draft.')).toHaveCount(0);await expect(page.getByText('Private owner qualification note')).toHaveCount(0);entry.checks.push('Identity/policy cleanup withdraws private activity and drafts')
   assert.equal(errors.length,0);entry.result='PASS'
  }finally{await page.evaluate(()=>window.__COMMUNITY_CLIENT__?.dispose()).catch(()=>{});await context.close();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n')}
  console.log(JSON.stringify({width,result:entry.result,checks:entry.checks.length,captures:entry.captures.length}))
 }
 report.result='PASS'
}catch(e){report.result='FAIL';report.error=e.stack;throw e}
finally{await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await rm(cacheDir,{recursive:true,force:true})}
