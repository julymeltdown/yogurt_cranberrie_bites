import {execFileSync} from 'node:child_process'
import {createRequire} from 'node:module'
import {readFile,writeFile} from 'node:fs/promises'
import {resolve} from 'node:path'
const root=process.cwd(),out=resolve(root,'docs/mcr/evidence/ui-origin'),req=createRequire(resolve(process.env.QA02_NODE_MODULES,'../package.json'))
const {createServer}=req('vite'),{chromium}=req('@playwright/test')
const server=await createServer({root:resolve(root,'frontend'),configLoader:'runner',cacheDir:'/tmp/mcr-qa02-supplement-vite',logLevel:'error',server:{host:'127.0.0.1',port:0,strictPort:false,fs:{allow:[root]}}});await server.listen()
const base=`http://127.0.0.1:${server.httpServer.address().port}`,browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true}),rows=[]
async function shot(id,path,action){const page=await browser.newPage({viewport:{width:390,height:844}}),errors=[];page.on('pageerror',e=>errors.push(e.message));try{await page.goto(base+path,{waitUntil:'networkidle'});if(action)await action(page);await page.waitForTimeout(150);const text=await page.locator('body').innerText();if(!text.trim())throw Error('Empty render');const file=`snapshots/${id}.jpg`;await page.screenshot({path:resolve(out,file),type:'jpeg',quality:80,fullPage:true});rows.push({id,path,file,text,errors})}catch(e){rows.push({id,path,errors,error:e.message})}finally{await page.close()}}
try{
 const probe=`/@fs${out}/route-probe.html`
 await shot('route-error',`${probe}?route=/world/missing&anonymous=0`)
 await shot('report-query',`${probe}?route=${encodeURIComponent('/report?targetType=USER&targetId=00000000-0000-4000-8000-000000000002')}&anonymous=0`)
 await shot('primitives','/src/shared/ui/__tests__/fixtures/preview.html')
 await shot('controls-layout','/src/world/__tests__/controls-layout/preview.html')
 for(const doc of ['terms','privacy','safety','contact'])await shot(`document-${doc}`,`/src/features/public/__tests__/reference-target/preview.html?route=/legal/${doc}`)
 for(const surface of ['summary','places','audit'])await shot(`operations-${surface}`,`/src/features/admin/event/__tests__/fixtures/preview.html?surface=${surface}`)
 for(const lane of ['members','content'])await shot(`moderation-${lane}`,`/src/features/admin/reports/__tests__/fixtures/preview.html?lane=${lane}`)
 for(const state of ['offline','reconnecting','resync','suspended','unavailable','maintenance','version'])await shot(`degraded-${state}`,`${probe}?notice=${state}`)
 await shot('reference-scene-current','/src/app/world-entry/__tests__/reference-scene-root/preview.html')
 // Real default-loader world in each configured era; no alternate artwork or engine.
 for(const era of ['REVENGE','PARADE','DANGER'])await shot(`world-${era.toLowerCase()}`,'/e2e/reference-integrated/whole/preview.html',async p=>{await p.waitForFunction(()=>window.__MCR_WHOLE_ROOT__?.state().path==='/world');await p.getByRole('button',{name:new RegExp('^'+era+'$','i')}).click();await p.waitForTimeout(300)})
}finally{await writeFile(resolve(out,'states.json'),JSON.stringify({candidate:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),rows},null,2)+'\n');await browser.close();await server.close()}
console.log(JSON.stringify(rows.map(r=>({id:r.id,errors:r.errors,error:r.error}))))
