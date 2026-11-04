// Actual default-router / selected-source art; explicitly deterministic Auth/HTTP/C05.
// Test-only fixture transform adds revision-aware save/failure responses; product code is untouched.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile, writeFile, mkdir, rm, statfs, readdir, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
const repo=resolve(import.meta.dirname,'../../../..'),frontend=resolve(repo,'frontend')
const require=createRequire(frontend+'/package.json')
const {createServer}=await import(pathToFileURL(require.resolve('vite')))
const {chromium,expect}=require('@playwright/test')
const node='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
assert.equal(process.execPath,node);assert.equal(process.env.PATH.split(':')[0],node.slice(0,-5))
assert(process.env.MCR_CUSTOMIZE_JOB,'Named accepted resource allocation required')
const round=process.env.MCR_CUSTOMIZE_ROUND??'native-01';assert(/^native-\d+$/.test(round))
const output=resolve(import.meta.dirname,round),temp='/private/tmp/mcr-css-customize-'+round
const css='frontend/src/features/avatar/whole-character/whole-character.css'
const fixture='frontend/src/app/__tests__/cozy-backgrounds-v1/preview.tsx'
const referencePath='/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/AB983BDC-49A0-4F5E-A56F-9F45A5560B6D/1-Photo-1.jpg'
const hash=b=>createHash('sha256').update(b).digest('hex'),git=(...args)=>execFileSync('git',args,{cwd:repo})
const baseline=process.env.MCR_CUSTOMIZE_BASE??'54320d71c4ae0bff3524d1b500cea772e0ba42e0'
const oldCss=git('show',baseline+':'+css).toString(),newCss=await readFile(resolve(repo,css),'utf8')
const reference=await readFile(referencePath)
const report={job:process.env.MCR_CUSTOMIZE_JOB,baseline,candidate:git('rev-parse','HEAD').toString().trim(),qualification:'MOCK_READY actual default router/components/selected-source pixels; synthetic HTTP/Auth/C05, no physical device/live backend or full visual acceptance',reference:{originalPath:referencePath,sha256:hash(reference),panel:{x:1034,y:0,width:246,height:440}},sources:{[css]:{baseline:hash(oldCss),candidate:hash(newCss)},[fixture]:hash(await readFile(resolve(repo,fixture))),driver:hash(await readFile(import.meta.filename))},cases:[],errors:[],caps:{temp:100*1024**2,durable:20*1024**2,whole:140*1024**2},peaks:{temp:0,durable:0,whole:0},startedAt:new Date().toISOString()}
let browser,server,mode,cache
async function bytes(path){let n=0;for(const e of await readdir(path,{withFileTypes:true}).catch(()=>[])){const p=resolve(path,e.name);n+=e.isDirectory()?await bytes(p):e.isFile()?(await stat(p)).size:0}return n}
async function guard(start=false){const d=await statfs(repo);assert(d.bavail*d.bsize>=(start?16:8)*1024**3,'Disk floor');const t=await bytes(temp),o=await bytes(output);for(const[k,v]of Object.entries({temp:t,durable:o,whole:t+o})){report.peaks[k]=Math.max(report.peaks[k],v);assert(v<=report.caps[k],k+' cap')}}
const fixturePatch=`if (path.endsWith('/me/avatar') && request.method === 'PUT') {
 const body = await request.json();
 if (params.get('saveMode') === 'denied') return Response.json({code:'DENIED',message:'Synthetic save denial',details:{reason:'PERMISSION_DENIED'}},{status:403});
 if (body.expected_revision !== data.avatar.revision) throw Error('Fixture revision mismatch');
 data.avatar = { revision:data.avatar.revision+1,config:body.config };
 return Response.json(data.avatar);
 }
 if (path.endsWith('/me') && params.get('loadMode') === 'denied') return new Response(null,{status:503});
 `
async function start(which){mode=which;cache=temp+'/cache-'+which;server=await createServer({root:frontend,configLoader:'runner',cacheDir:cache,logLevel:'error',resolve:{dedupe:['react','react-dom']},server:{host:'127.0.0.1',port:5299,strictPort:true,hmr:false},plugins:[{name:'bounded-customize-foreground-fixture',enforce:'pre',load(id){if(id.split('?')[0]===resolve(repo,css))return mode==='baseline'?oldCss:newCss},transform(code,id){if(id.split('?')[0]!==resolve(repo,fixture))return;const needle="if (path.endsWith('/me')) return Response.json(participant(data, data.avatar))";assert(code.includes(needle),'Pinned save-fixture insertion seam');return code.replace(needle,fixturePatch+needle)}}]});await server.listen();return server.resolvedUrls.local[0]}
async function stop(){await server?.close();server=null;await rm(cache,{force:true,recursive:true})}
async function ready(page){await expect.poll(()=>page.evaluate(()=>window.__COZY_PANEL__?.state().published),{timeout:25000}).toBe(true);await expect(page.locator('.mcr-whole-editor [data-whole-character-state=ready]')).toHaveCount(4);await page.evaluate(()=>document.fonts.ready);await page.waitForLoadState('networkidle')}
async function metrics(page){return page.evaluate(async()=>{
const r=e=>{const b=e.getBoundingClientRect();return{x:b.x,y:b.y,width:b.width,height:b.height}},h=async b=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',b))].map(x=>x.toString(16).padStart(2,'0')).join('')
const chars=await Promise.all([...document.querySelectorAll('.mcr-whole-editor canvas')].map(async e=>({rect:r(e),size:[e.width,e.height],rgba:await h(e.getContext('2d').getImageData(0,0,e.width,e.height).data)})))
const controls=[...document.querySelectorAll('.mcr-whole-editor button')].filter(e=>e.checkVisibility()).map(e=>({label:e.getAttribute('aria-label'),rect:r(e),font:getComputedStyle(e).font,disabled:e.disabled,ariaDisabled:e.getAttribute('aria-disabled')}))
return{overflow:document.documentElement.scrollWidth>innerWidth,chars,controls,heading:r(document.querySelector('.mcr-whole-choices header')),preview:r(document.querySelector('.mcr-whole-preview')),actions:r(document.querySelector('.mcr-whole-actions')),state:window.__COZY_PANEL__.state()}
})}
async function screenshot(page,file){const b=await page.screenshot({path:resolve(output,file)});return{file,bytes:b.length,sha256:hash(b)}}
async function capture(url,width){const context=await browser.newContext({viewport:{width,height:({360:800,390:844,430:932})[width]},deviceScaleFactor:1,hasTouch:true,isMobile:true,reducedMotion:'reduce',serviceWorkers:'block'}),page=await context.newPage(),entry={mode,width,errors:[]};page.setDefaultTimeout(15000);page.on('pageerror',e=>entry.errors.push(e.message))
try{await page.clock.setFixedTime(new Date('2026-11-07T07:00:00Z'));const base=url+'src/app/__tests__/cozy-backgrounds-v1/preview.html?entry=/me/avatar&era=PARADE';await page.goto(base);await ready(page);entry.metrics=await metrics(page);assert(!entry.metrics.overflow);assert(entry.metrics.state.socketPeak<=1);assert.equal(entry.metrics.state.canvases,1);assert.equal(entry.metrics.chars.length,4)
for(const c of entry.metrics.controls)assert(c.rect.width>=44&&c.rect.height>=44,'44px target '+c.label)
entry.screenshot=await screenshot(page,mode+'-'+width+'.png')
if(mode==='candidate'){
const prior=report.cases.find(c=>c.mode==='baseline'&&c.width===width);assert(prior);assert.deepEqual(entry.metrics.chars,prior.metrics.chars,'Character pixel or geometry drift');assert.deepEqual(entry.metrics.preview,prior.metrics.preview,'Preview rect drift');entry.characterPixelsAndGeometryPreserved=true
const next=page.getByRole('button',{name:'Next characters',exact:true});await next.focus();await page.keyboard.press('Enter');await expect(page.locator('.mcr-whole-choices header')).toContainText('4–6 / 6');await expect(next).toBeFocused();entry.focus=await next.evaluate(e=>({style:getComputedStyle(e).outlineStyle,width:getComputedStyle(e).outlineWidth,offset:getComputedStyle(e).outlineOffset}));assert.notEqual(entry.focus.style,'none');assert(parseFloat(entry.focus.width)>=3)
if(width===390)entry.focusScreenshot=await screenshot(page,'focus-390.png')
if(width===390){await page.emulateMedia({forcedColors:'active'});await next.focus();entry.forcedFocus=await next.evaluate(e=>({style:getComputedStyle(e).outlineStyle,width:getComputedStyle(e).outlineWidth,active:matchMedia('(forced-colors:active)').matches}));assert(entry.forcedFocus.active);assert.notEqual(entry.forcedFocus.style,'none');assert(parseFloat(entry.forcedFocus.width)>=3);entry.forcedScreenshot=await screenshot(page,'forced-focus-390.png');await page.emulateMedia({forcedColors:'none'})}
await page.getByRole('button',{name:'Previous characters',exact:true}).click();await ready(page)
const choices=page.locator('.mcr-whole-choice');await choices.nth(1).click();await ready(page);await expect(choices.nth(1)).toHaveAttribute('aria-pressed','true');await expect(page.getByRole('button',{name:'Save character',exact:true})).toBeEnabled();const putCount=async()=>page.evaluate(()=>window.__COZY_PANEL__.state().requests.filter(r=>r.method==='PUT'&&r.path.endsWith('/me/avatar')).length);assert.equal(await putCount(),0)
await page.getByRole('button',{name:'Reset draft',exact:true}).click();await ready(page);await expect(choices.nth(0)).toHaveAttribute('aria-pressed','true');await expect(page.getByRole('button',{name:'Save character',exact:true})).toBeDisabled();assert.equal(await putCount(),0)
await page.getByRole('button',{name:'Random character',exact:true}).click();await ready(page);await expect(page.getByRole('button',{name:'Save character',exact:true})).toBeEnabled();assert.equal(await putCount(),0)
await page.getByRole('button',{name:'Save character',exact:true}).click();await expect(page.locator('.mcr-whole-status [role=status]')).toContainText('Character saved.');assert.equal(await putCount(),1);entry.interactions={pagingKeyboard:true,choose:true,resetDraftOnly:true,randomDraftOnly:true,explicitRevisionSave:true}
if(width===390){await page.goto(base+'&saveMode=denied');await ready(page);await page.getByRole('button',{name:'Random character',exact:true}).click();await ready(page);await page.getByRole('button',{name:'Save character',exact:true}).click();await expect(page.locator('.mcr-whole-status [role=status]')).toContainText('Your character was not saved.');entry.deniedScreenshot=await screenshot(page,'save-denied-390.png');await page.goto(base+'&loadMode=denied');await expect(page.locator('.mcr-whole-status [role=status]')).toContainText('The saved character could not be verified.');for(const label of ['Random character','Reset draft','Save character'])await expect(page.getByRole('button',{name:label,exact:true})).toBeDisabled();entry.unavailableScreenshot=await screenshot(page,'load-denied-390.png')}
}
assert.deepEqual(entry.errors,[]);entry.result='PASS'
}catch(e){entry.result='FAIL';entry.failure=String(e);report.errors.push({mode,width,error:String(e)})}
finally{await page.evaluate(()=>window.__COZY_PANEL__?.dispose()).catch(()=>{});await context.close();report.cases.push(entry);await guard();await writeFile(resolve(output,'report.partial.json'),JSON.stringify(report,null,2)+'\n')}
}
await mkdir(output,{recursive:false});await mkdir(temp,{recursive:false});await guard(true);process.env.TMPDIR=temp
const cutoff=setTimeout(()=>{report.errors.push({error:'210s work cutoff; cleanup reserve'});browser?.close().catch(()=>{});server?.close().catch(()=>{})},210000)
try{browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version();let url=await start('baseline');for(const width of [390,360,430])await capture(url,width);await stop();url=await start('candidate');for(const width of [390,360,430])await capture(url,width);await stop()
// Static evidence composition only: new reference panel beside actual native captures.
const context=await browser.newContext({viewport:{width:1206,height:770},deviceScaleFactor:1}),page=await context.newPage();const before=await readFile(resolve(output,'baseline-390.png')),after=await readFile(resolve(output,'candidate-390.png'))
await page.setContent(`<html><body style="margin:0;display:flex;gap:8px;font:12px Arial;background:#ddd"><section><p>Human reference · Customize · 390px</p><div style="width:390px;height:715px;overflow:hidden;position:relative"><img src="data:image/jpeg;base64,${reference.toString('base64')}" style="position:absolute;width:${1280*390/246}px;max-width:none;left:${-1034*390/246}px;top:0"></div></section><section><p>Before · actual router · selected characters</p><img style="width:390px" src="data:image/png;base64,${before.toString('base64')}"></section><section><p>After · foreground CSS only · gaps remain</p><img style="width:390px" src="data:image/png;base64,${after.toString('base64')}"></section></body></html>`);await page.evaluate(()=>Promise.all([...document.images].map(i=>i.decode())));report.comparison=await screenshot(page,'comparison-390.png');await context.close();report.result=report.errors.length?'FAIL':'PASS'}catch(e){report.result='FAIL';report.failure=String(e);throw e}finally{clearTimeout(cutoff);await browser?.close();if(server)await stop();await rm(temp,{recursive:true,force:true});report.cleanup={browserClosed:true,viteClosed:true,tempRemoved:true};report.finishedAt=new Date().toISOString();await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n');await rm(resolve(output,'report.partial.json'),{force:true})}
if(report.errors.length)process.exitCode=1
