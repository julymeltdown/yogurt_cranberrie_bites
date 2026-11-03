import {createRequire} from 'node:module'
import {readFile,writeFile} from 'node:fs/promises'
import {createServer as createNetServer} from 'node:net'
import assert from 'node:assert/strict'
const root='<worktrees>/fe-places-review/frontend', require=createRequire(root+'/package.json')
const {createServer}=await import(require.resolve('vite')), {chromium,expect}=require('@playwright/test')
const reservation=createNetServer();await new Promise(r=>reservation.listen(0,'127.0.0.1',r));const port=reservation.address().port;await new Promise(r=>reservation.close(r))
const fixture=root+'/src/features/places/__tests__/fixtures/preview.html'
const server=await createServer({root,logLevel:'error',server:{host:'127.0.0.1',port,strictPort:true},plugins:[{name:'independent-reload-host',configureServer(v){v.middlewares.use(async(req,res,next)=>{if(new URL(req.url,'http://localhost').pathname!=='/world/map')return next();try{res.setHeader('Content-Type','text/html');res.end(await v.transformIndexHtml(req.url,await readFile(fixture,'utf8')))}catch(e){next(e)}})}}]})
let browser;const report={candidate:'5a56ee9354e483b43094cceed11b63abd1aa4639',scope:'independent R2 390x844 synthetic fixture on integration review candidate',result:'FAIL',errors:[]}
try{
 await server.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
 const context=await browser.newContext({viewport:{width:390,height:844},hasTouch:true,reducedMotion:'reduce'}),page=await context.newPage();page.on('pageerror',e=>report.errors.push(String(e)))
 const state=()=>page.evaluate(()=>window.__FE19__.state())
 await page.goto('http://127.0.0.1:'+port+'/src/features/places/__tests__/fixtures/preview.html')
 await expect.poll(async()=>(await state()).runtime?.phase).toBe('ready');await expect.poll(async()=>(await state()).places.list.phase).toBe('ready')
 const initial=await state();assert.equal(initial.canvases,1);assert.equal(initial.places.mapMismatch,false)
 await page.getByRole('button',{name:'Expand panel'}).click()
 await page.evaluate(()=>window.__FE19__.mode('mismatch'))
 await expect(page.getByRole('heading',{name:'Your map and places do not match'})).toBeVisible();await expect(page.getByRole('button',{name:'Show Synthetic virtual stage on virtual map'})).toBeDisabled()
 const rejected=await state();assert.deepEqual(rejected.foot,initial.foot);assert.equal(rejected.documentId,initial.documentId)
 await page.screenshot({path:'<tmp>/mcr-fe19-review-final/reload-before.png',fullPage:true})
 await Promise.all([page.waitForEvent('load'),page.getByRole('button',{name:'Reload event',exact:true}).click()])
 await page.waitForFunction(()=>Boolean(window.__FE19__))
 await expect.poll(async()=>(await state()).runtime?.phase).toBe('ready');await expect.poll(async()=>(await state()).places.list.phase).toBe('ready')
 const recovered=await state();assert.notEqual(recovered.documentId,initial.documentId);assert.equal(recovered.canvases,1);assert.equal(recovered.mounts,1);assert.equal(recovered.places.mapMismatch,false)
 await expect(page.getByRole('button',{name:'Show Synthetic virtual stage on virtual map'})).toBeEnabled()
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true)
 await page.screenshot({path:'<tmp>/mcr-fe19-review-final/reload-after.png',fullPage:true})
 await page.evaluate(()=>window.__FE19__.dispose());assert.equal(await page.locator('canvas').count(),0);assert.equal(await page.locator('.world-controls').count(),0)
 assert.deepEqual(report.errors,[]);Object.assign(report,{result:'PASS',documentChanged:true,rejectedPinRecovered:true,currentCanvases:1,avatarUnchangedBeforeReload:true,cleanup:'PASS'})
 await context.close()
}catch(e){report.failure=String(e);process.exitCode=1}
finally{await browser?.close();await server.close();await writeFile('<tmp>/mcr-fe19-review-final/reload.json',JSON.stringify(report,null,2)+'\n')}
console.log(JSON.stringify(report))
