// Run from repository root with QA02_NODE_MODULES pointing at retained dependencies.
import { createRequire } from 'node:module'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
const require = createRequire(resolve(process.env.QA02_NODE_MODULES, '../package.json'))
const { createServer } = require('vite'), { chromium } = require('@playwright/test')
const root = process.cwd(), out = resolve(root, 'docs/mcr/evidence/ui-origin')
await mkdir(resolve(out,'snapshots'), {recursive:true})
const server = await createServer({root:resolve(root,'frontend'),configLoader:'runner',cacheDir:'/tmp/mcr-qa02-vite',logLevel:'error',server:{host:'127.0.0.1',port:0,strictPort:false,fs:{allow:[root]}}})
await server.listen()
const base = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
const report={candidate:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),capturedAt:new Date().toISOString(),qualification:'Native Chrome; actual production components. Deterministic fixture data/readiness only; route probe uses instrumented host. Feature previews separately exercise present components. No live/backend/device/QA06/fidelity acceptance.',routes:[],features:[],errors:[]}
const page=await browser.newPage({viewport:{width:390,height:844}})
page.on('pageerror',e=>report.errors.push(e.message))
async function capture(url,id){
 const errors=[];const listener=e=>errors.push(e.message);page.on('pageerror',listener)
 try{
 await page.goto(url,{waitUntil:'networkidle',timeout:20000})
 await page.locator('#root').waitFor({timeout:8000});await page.waitForTimeout(180)
 const text=await page.locator('body').innerText(),file=`snapshots/${id}.jpg`
 await page.screenshot({path:resolve(out,file),type:'jpeg',quality:78,fullPage:true})
 const layout=await page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth,headings:[...document.querySelectorAll('h1,h2,h3')].map(e=>e.textContent),canvases:document.querySelectorAll('canvas').length}))
 return {id,url:url.replace(base,''),file,text,layout,errors}
 }catch(e){return {id,url:url.replace(base,''),error:e.message,errors}}
 finally{page.off('pageerror',listener)}
}
try{
 const probe=`${base}/@fs${out}/route-probe.html`
 await page.goto(probe);await page.waitForFunction(()=>window.__QA02__)
 const paths=await page.evaluate(()=>window.__QA02__.paths)
 let n=0
 for(const pattern of paths){
 const route=pattern.replace(/:document/g,'privacy').replace(/:[A-Za-z]+/g,'00000000-0000-4000-8000-000000000002')
 const anonymous=['/','/login','/signup','/verify-email','/reset-password','/auth/callback','/legal/:document','/account/deleted','/archive'].includes(pattern)
 report.routes.push({pattern,route,...await capture(`${probe}?route=${encodeURIComponent(route)}&anonymous=${anonymous?1:0}`,`route-${String(n++).padStart(2,'0')}`)})
 console.log('route',n,pattern)
 }
 const all=execFileSync('rg',['--files','frontend/src','frontend/e2e'],{encoding:'utf8'}).trim().split('\n')
 const previews=process.env.QA02_ROUTES_ONLY ? [] : all.filter(p=>p.endsWith('preview.html')&&!p.includes('/world/network/')&&!p.includes('/world/__tests__/fixtures/'))
 n=0
 for(const file of previews){report.features.push({source:file,...await capture(`${base}/${file.replace(/^frontend\//,'')}`,`feature-${String(n++).padStart(2,'0')}`)});console.log('feature',n,file)}
 for(const width of [360,430]){await page.setViewportSize({width,height:844});report.features.push({source:'public mobile boundary',...await capture(`${base}/src/features/public/__tests__/reference-target/preview.html`,`public-${width}`)});report.features.push({source:'auth mobile boundary',...await capture(`${base}/src/features/auth/__tests__/reference-target/preview.html`,`auth-${width}`)})}
 // A review index is authored from fresh captures, not adopted as a golden.
 const allRows=[...report.routes,...report.features].filter(r=>r.file)
 const html=`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#ddd;font:13px monospace}.grid{display:grid;grid-template-columns:repeat(8,195px);gap:6px}.item{background:white;overflow:hidden}.item p{height:52px;margin:3px;overflow:hidden}.item img{width:195px;height:422px;object-fit:cover;object-position:top}</style>`
 for(let start=0;start<allRows.length;start+=24){const rows=allRows.slice(start,start+24);const name=`${process.env.QA02_ROUTES_ONLY?'current-':''}review-${String(start/24).padStart(2,'0')}`;await writeFile(resolve(out,`${name}.html`),html+`<div class="grid">`+rows.map(r=>`<div class="item"><p>${r.id}: ${r.pattern??r.source}</p><img src="${r.file}"></div>`).join('')+'</div>');await page.setViewportSize({width:1610,height:1445});await page.goto(`${base}/@fs${out}/${name}.html`,{waitUntil:'networkidle'});await page.screenshot({path:resolve(out,`${name}.jpg`),type:'jpeg',quality:85,fullPage:true})}
}finally{await writeFile(resolve(out,process.env.QA02_ROUTES_ONLY ? 'browser-current.json' : 'browser.json'),JSON.stringify(report,null,2)+'\n');await browser.close();await server.close()}
console.log(JSON.stringify({routes:report.routes.length,features:report.features.length,errors:report.errors.length,failed:[...report.routes,...report.features].filter(r=>r.error).map(r=>({id:r.id,error:r.error}))}))
