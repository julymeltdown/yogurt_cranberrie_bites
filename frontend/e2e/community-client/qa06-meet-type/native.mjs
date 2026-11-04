// Bounded title/place/action qualification. Actual root; synthetic HTTP/C05, no device/live claim.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {execFileSync} from 'node:child_process'
import {mkdir,readFile,writeFile,statfs,rm,readdir,stat} from 'node:fs/promises'
import {resolve,extname} from 'node:path'
import {createServer as netServer} from 'node:net'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const slot=process.env.MCR_NATIVE_SLOT;assert(slot?.startsWith('QA06')&&slot.includes('MEET')&&slot.includes('TYPE'),'Explicit named allocation required')
const repo=resolve(import.meta.dirname,'../../../..'),frontend=resolve(repo,'frontend'),out=resolve(repo,'docs/mcr/evidence/qa06-meet-type/native')
const base='4af1c86863698e311d286039a0702d5b2517c1c3',css='frontend/src/features/party-discovery/ui/party-discovery.css'
const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:64*1024*1024}),hash=x=>createHash('sha256').update(x).digest('hex')
const baseline=git('show',`${base}:${css}`).toString(),reference=resolve(repo,'docs/mcr/evidence/visual/qa06/human-reference-20261104/reference-human.jpg')
// The durable reference may not yet be in this sparse checkout; read the authorized QA packet instead.
const referenceBytes=await readFile(reference).catch(()=>readFile('/Users/lhs/Desktop/mcr-worktrees/fe-ref-public/docs/mcr/evidence/visual/qa06/human-reference-20261104/reference-human.jpg'))
assert.equal(hash(referenceBytes),'2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23')
async function disk(start=false){const d=await statfs(repo);assert(d.bavail*d.bsize>=(start?16:8)*2**30,'Disk floor');return d.bavail*d.bsize/2**30}
async function bytes(path){let sum=0;for(const e of await readdir(path,{withFileTypes:true}).catch(()=>[])){const p=resolve(path,e.name);sum+=e.isDirectory()?await bytes(p):(await stat(p)).size}return sum}
const report={base,candidate:git('rev-parse','HEAD').toString().trim(),slot,startFreeGiB:await disk(true),referenceSha256:hash(referenceBytes),
  qualification:'ACTUAL_ROOT_SELECTED_ART_SYNTHETIC_HTTP_C05',physicalIOS:'NOT_RUN',physicalAndroid:'NOT_RUN',sourceHashes:{},phases:[],assetReads:[]}
for(const p of git('ls-files','frontend/src/features/party-discovery','frontend/src/app/router.tsx','frontend/src/app/WorldShell.tsx','frontend/src/app/shell.css','frontend/src/app/shell-chrome','frontend/src/shared/ui/concert-window','frontend/e2e/reference-integrated/whole/data.ts','frontend/e2e/reference-integrated/data.ts','contracts/domain/v1/party.schema.json','contracts/generated/mcr-domain.openapi.json').toString().trim().split('\n').filter(p=>/\.(tsx?|css|json)$/.test(p)&&!p.includes('/__tests__/')&&!p.includes('/evidence/')))report.sourceHashes[p]=hash(await readFile(resolve(repo,p)))
for(const p of ['native.mjs','preview.tsx','preview.html'])report.sourceHashes['frontend/e2e/community-client/qa06-meet-type/'+p]=hash(await readFile(resolve(import.meta.dirname,p)))
for(const p of ['preview.tsx','preview.html']){const path='frontend/src/features/party-discovery/__tests__/qa06-meet-cta/'+p;report.sourceHashes[path]=hash(await readFile(resolve(repo,path)))}
const tracked=new Set(git('ls-files','frontend/public').toString().trim().split('\n')),assets=new Map()
async function metrics(page){return page.evaluate(()=>{
 const box=e=>{if(!e)return null;const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right}}
 const feature=document.querySelector('.party-browse'),body=document.querySelector('.shell-panel-body'),discovery=document.querySelector('.meet-discovery'),create=document.querySelector('.meet-create__action'),copy=document.querySelector('.meet-create__unavailable'),rect=box(body)
 const within=e=>{const b=box(e);return b&&{...b,visible:b.y>=rect.y-1&&b.bottom<=rect.bottom+1,hit:e.contains(document.elementFromPoint(b.x+b.width/2,b.y+b.height/2))}}
 const ink=e=>{const r=document.createRange();r.selectNodeContents(e);return box(r)}
 return {overflow:document.documentElement.scrollWidth>innerWidth,body:{...rect,scrollTop:body.scrollTop},discovery:{...box(discovery),scrollHeight:discovery.scrollHeight,clientHeight:discovery.clientHeight,scrollTop:discovery.scrollTop},
  action:within(create),copy:within(copy),disabled:create.matches(':disabled'),footer:box(document.querySelector('.mcr-chrome-parade-footer')),
  controls:[...feature.querySelectorAll('button,a,summary,input,select,textarea')].filter(e=>e.getClientRects().length).map(e=>({label:e.getAttribute('aria-label')??e.textContent.trim(),...box(e.matches('input[type=checkbox]')?e.labels[0]:e)})),
  rows:[...feature.querySelectorAll('.meet-party')].map(row=>{const a=row.querySelector('h3 a'),where=row.querySelector('.meet-party__where'),join=row.querySelector('.meet-party__join');return {title:a.textContent,capacity:row.querySelector('.meet-party__capacity').textContent,box:box(row),titleBox:box(a),titleInk:ink(a),where:box(where),whereText:where.textContent,
   titleSize:parseFloat(getComputedStyle(a).fontSize),placeSize:parseFloat(getComputedStyle(where).fontSize),placeWeight:getComputedStyle(where).fontWeight,actionSize:parseFloat(getComputedStyle(join).fontSize),titleFamily:getComputedStyle(a).fontFamily,titleTracking:getComputedStyle(a).letterSpacing,join:box(join)}}),state:window.__QA06__.state()}
 })}
let browser
try{
 await mkdir(out,{recursive:true});browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version();report.node=process.version
 for(const phase of ['before','after']){
  const cacheDir=`/tmp/mcr-qa06-meet-type-${process.pid}-${phase}`,reservation=netServer();await new Promise(r=>reservation.listen(0,'127.0.0.1',r));const port=reservation.address().port;await new Promise(r=>reservation.close(r))
  const phaseReport={phase,cssSha256:hash(phase==='before'?baseline:await readFile(resolve(repo,css))),baselineOverride:phase==='before',cases:[]};report.phases.push(phaseReport)
  const server=await createServer({root:frontend,cacheDir,configLoader:'runner',logLevel:'error',plugins:[{name:'immutable-owned-baseline-css',enforce:'pre',transform(code,id){if(phase==='before'&&id.split('?')[0]===resolve(repo,css))return {code:baseline,map:null}}},{name:'readonly-selected-assets',configureServer(s){s.middlewares.use((req,res,next)=>{
   const path='frontend/public'+decodeURIComponent(new URL(req.url,'http://localhost').pathname);if(!tracked.has(path)||!['.png','.jpg','.webp','.ttf'].includes(extname(path)))return next()
   try{if(!assets.has(path)){const data=git('show',`HEAD:${path}`);assets.set(path,data);report.assetReads.push({path,sha256:hash(data),bytes:data.length})}res.setHeader('Content-Type',extname(path)==='.ttf'?'font/ttf':`image/${extname(path).slice(1)}`);res.end(assets.get(path))}catch(e){next(e)}
  })}}],server:{host:'127.0.0.1',port,strictPort:true,hmr:false}})
  try{
   await server.listen()
   const sizes=phase==='before'?[[390,637]]:[[390,637],[360,844],[390,844],[430,844],[360,420],[390,420],[430,420]]
   for(const [width,height] of sizes){
    await disk();assert(await bytes(out)+await bytes(cacheDir)<0.2*2**30,'Hard 0.2GiB new output ceiling')
    const context=await browser.newContext({viewport:{width,height},isMobile:true,hasTouch:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),entry={width,height,errors:[],checks:[]};phaseReport.cases.push(entry)
    page.on('pageerror',e=>entry.errors.push(e.message));page.setDefaultTimeout(12000);await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'))
    try{
     await page.goto(`http://127.0.0.1:${port}/e2e/community-client/qa06-meet-type/preview.html?entry=%2Fmeet&era=PARADE`);await page.waitForFunction(()=>window.__QA06__?.state().published);await expect(page.locator('.meet-party')).toHaveCount(3);await page.evaluate(()=>document.fonts.ready)
     entry.initial=await metrics(page);const m=entry.initial
     assert.deepEqual(m.rows.map(r=>r.title),['Area A Queue Group','After Show Hangout','Photo Walk (Parade ver.)']);assert.deepEqual(m.rows.map(r=>r.capacity.trim()),['12/20 members','8/12 members','5/8 members'])
     assert.equal(m.overflow,false);assert.equal(m.disabled,true);assert(m.action.visible&&m.copy.visible);assert.equal(m.body.scrollTop,0);assert.equal(m.state.canvases,1);assert(m.state.socketPeak<=1)
     assert(m.controls.every(c=>c.width>=43.5&&c.height>=43.5),'44px feature controls');assert.equal(m.rows[0].titleSize,phase==='before'?15:18);assert.equal(m.rows[0].placeSize,phase==='before'?12:14)
     if(phase==='after')for(const row of m.rows)assert(row.where.y>=row.titleInk.bottom-0.5,'Place must not overlap wrapped title')
     await page.screenshot({path:resolve(out,`${phase}-${width}x${height}.png`)})
     if(phase==='before')continue
     entry.checks.push('truthful same cohort; larger title/place/action hierarchy; no text overlap; 44px controls; Create Party/copy initially reachable')
     if(m.discovery.scrollHeight>m.discovery.clientHeight){const cdp=await context.newCDPSession(page);await cdp.send('Input.synthesizeScrollGesture',{x:Math.round(m.discovery.x+m.discovery.width/2),y:Math.round(m.discovery.y+m.discovery.height/2),yDistance:-180,gestureSourceType:'touch',speed:600});await cdp.detach();entry.scrolled=await metrics(page);assert(entry.scrolled.discovery.scrollTop>0);assert(entry.scrolled.action.visible&&entry.scrolled.copy.visible)}
     const title=page.locator('.meet-party h3 a').first();await title.scrollIntoViewIfNeeded();await title.tap();await expect.poll(()=>page.evaluate(()=>window.__QA06__.state().path)).toMatch(/^\/meet\/parties\//);await page.goBack();await expect(page.locator('.meet-party')).toHaveCount(3)
     const join=page.locator('.meet-party__join').last();await join.scrollIntoViewIfNeeded();await join.tap();await expect.poll(()=>page.evaluate(()=>window.__QA06__.state().path)).toMatch(/^\/meet\/parties\//);await page.goBack();await expect(page.locator('.meet-party')).toHaveCount(3)
     await page.locator('.party-filters summary').scrollIntoViewIfNeeded();await page.locator('.party-filters summary').tap();const field=page.getByLabel('Ticket section',{exact:true});await field.fill('Area A');await page.getByRole('button',{name:'Show parties',exact:true}).tap();await page.locator('.party-filters summary').tap()
     await page.locator('#party-people').tap();await expect(page.getByRole('heading',{name:'People',exact:true}).first()).toBeVisible();await page.goBack();await expect(page.locator('.meet-party')).toHaveCount(3)
     await page.getByRole('button',{name:'MY PLAN',exact:true}).tap();await page.getByRole('button',{name:'Parties',exact:true}).tap()
     await page.locator('.meet-party h3 a').first().focus();await page.keyboard.press('Tab');entry.focus=await page.evaluate(()=>{const e=document.activeElement,s=getComputedStyle(e),r=e.getBoundingClientRect();return {label:e.textContent,outline:s.outlineStyle,width:parseFloat(s.outlineWidth),visible:r.y>=0&&r.bottom<=innerHeight}});assert(entry.focus.visible&&entry.focus.width>=2&&entry.focus.outline!=='none')
     entry.final=await metrics(page);assert.equal(entry.final.overflow,false);assert.equal(entry.final.disabled,true);assert.equal(entry.final.state.canvases,1);assert(entry.final.state.socketPeak<=1);assert.deepEqual(entry.errors,[])
     entry.checks.push('actual title and JOIN touch then Back; filters/input/submit; People/Back; MY PLAN; visible keyboard focus; singleton world retained')
    }finally{await page.evaluate(()=>window.__QA06__?.dispose()).catch(()=>{});await context.close()}
   }
  }finally{await server.close();await rm(cacheDir,{recursive:true,force:true})}
 }
 report.result='PASS'
}catch(e){report.result='FAIL';report.error=e.stack;throw e}
finally{await browser?.close();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n')}
console.log(JSON.stringify({result:report.result,candidate:report.candidate,phases:report.phases.map(p=>({phase:p.phase,cssSha256:p.cssSha256,cases:p.cases.map(c=>({width:c.width,height:c.height,rows:c.initial?.rows,checks:c.checks,errors:c.errors}))}))},null,2))
