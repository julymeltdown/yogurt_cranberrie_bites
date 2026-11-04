import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {mkdir,readFile,writeFile,mkdtemp,rm} from 'node:fs/promises'
import {resolve} from 'node:path'
import {tmpdir} from 'node:os'
import {execFileSync} from 'node:child_process'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const frontend=resolve(import.meta.dirname,'../../../..'),repo=resolve(frontend,'..'),out=resolve(import.meta.dirname,'evidence')
const hash=b=>createHash('sha256').update(b).digest('hex'),cache=await mkdtemp(resolve(tmpdir(),'mcr-fe15-scroll-'))
await mkdir(out,{recursive:true})
const server=await createServer({root:frontend,cacheDir:cache,logLevel:'warn',server:{host:'127.0.0.1',port:0}})
const report={result:'FAIL',candidate:execFileSync('git',['rev-parse','HEAD'],{cwd:repo,encoding:'utf8'}).trim(),qualification:'Actual browser router, native layout/scroll/history, canonical Auth, default production composition/Phaser; explicit HTTP/C05 test fixture. Original FE07 owner retains complete task qualification.',pins:{},cases:[]}
for(const p of ['src/app/WorldShell.tsx',...['scroll-preview.tsx','scroll-preview.html','scroll-native.mjs'].map(p=>'src/world/__tests__/fe15-application-input/'+p)])report.pins['frontend/'+p]=hash(await readFile(resolve(frontend,p)))
let browser,page
try{
 await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const width of (process.env.MCR_FE15_WIDTH?[+process.env.MCR_FE15_WIDTH]:[360,390,430])){
  const context=await browser.newContext({viewport:{width,height:844},isMobile:true,hasTouch:true});page=await context.newPage();const errors=[]
  page.on('pageerror',e=>errors.push(e.message));await page.goto(server.resolvedUrls.local[0]+'src/world/__tests__/fe15-application-input/scroll-preview.html')
  await expect.poll(()=>page.evaluate(()=>window.__FE15_SCROLL__?.state().then(s=>s.phase)),{timeout:20000}).toBe('live')
  await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Gifts',exact:true}).click()
  const opener=page.getByRole('link',{name:'Synthetic shell gift 18',exact:true}),body=page.locator('.shell-panel-body')
  await opener.scrollIntoViewIfNeeded();const before=await body.evaluate(e=>e.scrollTop);assert(before>100)
  // Flush the browser's user-scroll event before navigating to a shorter DOM tree.
  await page.waitForTimeout(100);await opener.click();await expect(page.getByRole('heading',{name:'Synthetic shell gift 18',exact:true})).toBeVisible()
  await page.waitForTimeout(150);const detailScroll=await body.evaluate(e=>e.scrollTop)
  await page.getByRole('button',{name:'Back to previous panel',exact:true}).click();await expect(opener).toBeFocused()
  const after=await body.evaluate(e=>e.scrollTop),state=await page.evaluate(()=>window.__FE15_SCROLL__.state())
  report.cases.push({width,before,detailScroll,after,state,errors});assert.equal(state.action,'POP');assert(Math.abs(after-before)<3,`scroll restoration ${before} -> ${after}`)
  assert.equal(state.knownGames,1);assert.equal(state.peakGames,1);assert.equal(state.sockets,1);assert.equal(state.sameCamera,true)
  await page.goForward();await expect(page.getByRole('heading',{name:'Synthetic shell gift 18',exact:true})).toBeVisible();await expect.poll(()=>body.evaluate(e=>e.scrollTop)).toBe(detailScroll)
  await page.goBack();await expect(opener).toBeFocused();assert(Math.abs(await body.evaluate(e=>e.scrollTop)-before)<3)
  const image=`scroll-return-${width}.png`;await page.screenshot({path:resolve(out,image)});report.cases.at(-1).capture={file:image,sha256:hash(await readFile(resolve(out,image)))}
  await page.evaluate(()=>window.__FE15_SCROLL__.dispose());const disposed=await page.evaluate(()=>window.__FE15_SCROLL__.state());assert.equal(disposed.games,0);assert.equal(disposed.activeSockets,0);assert.deepEqual(errors,[]);await context.close()
 }
 report.result='PASS'
}catch(e){report.error=String(e.stack);throw e}
finally{await writeFile(resolve(out,process.env.MCR_FE15_BEFORE?'before-scroll.json':'scroll-native.json'),JSON.stringify(report,null,2)+'\n');await page?.evaluate(()=>window.__FE15_SCROLL__?.dispose()).catch(()=>{});await browser?.close();await server.close();await rm(cache,{recursive:true,force:true})}
console.log(JSON.stringify({result:report.result,widths:report.cases.map(c=>c.width)}))
