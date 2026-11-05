// Controlled HTTP/browser fixture. It does not implement or qualify Gateway OAuth state cryptography.
import assert from 'node:assert/strict'
import { copyFile,mkdir,readFile,writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { pathToFileURL,fileURLToPath } from 'node:url'

const root=fileURLToPath(new URL('../../../',import.meta.url))
const deps=process.env.MCR_OAUTH_READONLY_DEPS
const runtimeDeps=Object.keys(JSON.parse(await readFile(resolve(root,'frontend/package.json'),'utf8')).dependencies)
const packageRoot=deps??resolve(root,'frontend/node_modules')
const vite=await import(deps?pathToFileURL(resolve(deps,'vite/dist/node/index.js')).href:'vite')
const {chromium,expect}=await import(deps?pathToFileURL(resolve(deps,'@playwright/test/index.mjs')).href:'@playwright/test')
const evidence=process.env.MCR_OAUTH_EVIDENCE
assert(evidence,'Explicit owned evidence output is required');await mkdir(evidence,{recursive:true})
const auth=JSON.parse(await readFile(resolve(root,'contracts/fixtures/core/auth-success.json'),'utf8'))
const capabilities=JSON.parse(await readFile(resolve(root,'contracts/fixtures/core/auth-capabilities-disabled.json'),'utf8'))
const bootstrap=JSON.parse(await readFile(resolve(root,'contracts/fixtures/core/bootstrap-open.json'),'utf8'))
const html=await readFile(new URL('./index.html',import.meta.url),'utf8')
const errors=[],results=[],consoleErrors=[],requestFailures=[],calls={refresh:0,admission:0,logout:0,cancelled:0}
const scrub=value=>{let text=JSON.stringify(value);for(const secret of [auth.accessToken,'SYNTHETIC_HTTP_ONLY','PRIVATE_CODE','PRIVATE_RAW_ERROR'])text=text.replaceAll(secret,'[redacted]');return text}
let progressWrite=Promise.resolve()
function checkpoint(phase){
  const snapshot=scrub({phase,at:new Date().toISOString(),errors,consoleErrors,requestFailures,completedResults:results,calls})+'\n'
  progressWrite=progressWrite.then(()=>writeFile(resolve(evidence,'progress.json'),snapshot))
  return progressWrite
}
function observe(page){
  page.on('pageerror',error=>{errors.push(error.message);void checkpoint('pageerror')})
  page.on('console',message=>{if(message.type()==='error'){consoleErrors.push(message.text());void checkpoint('console-error')}})
  page.on('requestfailed',request=>{requestFailures.push({url:request.url(),failure:request.failure()?.errorText});void checkpoint('requestfailed')})
}
let mode='success',browser,origin
const config=process.env.MCR_OAUTH_VITE_CONFIG
const extra=config?(await import(pathToFileURL(config).href)).default:{}
const retained=new Set()
if(process.env.MCR_OAUTH_RETAINED_CASES){
  const packet=JSON.parse(await readFile(process.env.MCR_OAUTH_RETAINED_CASES,'utf8'))
  assert.equal(packet.status,'PARTIAL_CONTROLLED_CASES_RESOURCE_FAIL')
  assert.equal(packet.failedRound,'NATIVE04')
  assert.equal(createHash('sha256').update(await readFile(config)).digest('hex'),packet.runtimeConfigSHA)
  for(const [path,digest]of Object.entries(packet.sourcePins))assert.equal(createHash('sha256').update(await readFile(path)).digest('hex'),digest,'Retained case source changed')
  assert.deepEqual(packet.results.map(item=>[item.width,item.height,item.case,item.status]),[[360,800,'denied','PASS'],[360,800,'foreign','PASS'],[360,800,'expired','PASS']])
  for(const item of packet.results){
    const capture=packet.captures[item.case]
    assert.equal(createHash('sha256').update(await readFile(capture.path)).digest('hex'),capture.sha256)
    await copyFile(capture.path,resolve(evidence,item.case+'-360.png'))
    results.push({...item,provenance:'RETAINED_NATIVE04_CANDIDATE2',captureWindowUTC:packet.captureWindowUTC})
    retained.add(item.case+'-360')
  }
}
const server=await vite.createServer({publicDir:resolve(root,'frontend/public'),oxc:{jsx:{runtime:'automatic'}},
  resolve:{alias:Object.fromEntries(runtimeDeps.map(name=>[name,resolve(packageRoot,name)]))},
  optimizeDeps:{noDiscovery:true,include:['react','react/jsx-runtime','react/jsx-dev-runtime','react-dom','react-dom/client','react-router-dom','@tanstack/react-query','ky','zod','phaser','react-hook-form','@hookform/resolvers/zod','zustand','browser-image-compression']},
  ...extra,root,configFile:false,cacheDir:resolve(process.env.TMPDIR??evidence,'oauth-vite-cache'),
  server:{host:'localhost',port:5187,strictPort:true,fs:{allow:[root,...(deps?[deps]:[])]}},
  plugins:[...(extra.plugins??[]),{name:'controlled-cookie-handoff-counterpart',configureServer(dev){dev.middlewares.use(async(req,res,next)=>{
    const url=new URL(req.url??'/',origin??'http://localhost:5187')
    if(url.pathname==='/__oauth_fixture/callback'){
      mode=url.searchParams.get('mode')??'success'
      res.statusCode=303;res.setHeader('Cache-Control','no-store')
      res.setHeader('Set-Cookie','mcr_oauth_fixture_refresh=SYNTHETIC_HTTP_ONLY; Path=/; Secure; HttpOnly; SameSite=Lax')
      const status=mode==='denied'?'denied':mode==='error'?'error':'success'
      const requested=url.searchParams.get('target'),target=['/world','/feed','/me','/meet/people'].includes(requested)?requested:'/world'
      res.setHeader('Location','/auth/callback?status='+status+'&returnTo='+encodeURIComponent(target)+(mode==='foreign'?'&code=PRIVATE_CODE':''));return res.end()
    }
    if(url.pathname.startsWith('/api/')){
      res.setHeader('Content-Type','application/json');res.setHeader('Cache-Control','no-store')
      const send=(status,body)=>{if(!res.destroyed){res.statusCode=status;res.end(body===undefined?undefined:JSON.stringify(body))}}
      if(url.pathname==='/api/auth/capabilities')return send(200,capabilities)
      if(url.pathname==='/api/auth/refresh'){
        ++calls.refresh
        assert.equal(req.method,'POST');assert.equal(req.headers.origin,origin);assert.equal(req.headers['x-mcr-intent'],'browser');assert.equal(req.headers.referer,undefined)
        assert.match(req.headers.cookie??'',/mcr_oauth_fixture_refresh=SYNTHETIC_HTTP_ONLY/)
        res.on('close',()=>{if(!res.writableEnded)++calls.cancelled})
        if(mode==='delay')await new Promise(resolve=>setTimeout(resolve,700))
        if(mode==='expired')return send(401,{code:'EXPIRED',message:'PRIVATE_RAW_ERROR'})
        return send(200,auth)
      }
      if(url.pathname==='/api/auth/enrollment/admission'){
        ++calls.admission;assert.equal(req.headers.authorization,`Bearer ${auth.accessToken}`)
        return send(200,{decision:mode==='restricted'?'ACCOUNT_INACTIVE':'ADMITTED',policyRevision:'00000000-0000-4000-8000-000000000123',acceptedRevision:mode==='restricted'?null:'00000000-0000-4000-8000-000000000123',serverTime:'2026-11-05T00:00:00Z'})
      }
      if(url.pathname==='/api/auth/logout'){++calls.logout;res.setHeader('Set-Cookie','mcr_oauth_fixture_refresh=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0');return send(204)}
      if(url.pathname.endsWith('/bootstrap'))return send(200,bootstrap)
      return send(503,{code:'UNAVAILABLE',message:'Controlled fixture has no feature service'})
    }
    if(!url.pathname.includes('.')&&!url.pathname.startsWith('/@')&&!url.pathname.startsWith('/frontend/')){res.setHeader('Content-Type','text/html');res.setHeader('Cache-Control','no-store');return res.end(html)}
    next()
  })}}],
})
async function hygiene(page){const values=await page.evaluate(()=>[document.body.innerHTML,location.href,JSON.stringify(history.state),JSON.stringify(localStorage),JSON.stringify(sessionStorage),document.cookie]);for(const value of values)for(const secret of [auth.accessToken,'SYNTHETIC_HTTP_ONLY','PRIVATE_CODE','PRIVATE_RAW_ERROR'])assert(!value.includes(secret),'Credential/error reflection');assert.equal(new URL(page.url()).search,'');assert.equal(new URL(page.url()).hash,'')}
try{
  await checkpoint('before-server-listen')
  await server.listen();origin='http://localhost:5187'
  await checkpoint('before-browser-launch')
  browser=await chromium.launch({executablePath:process.env.MCR_CHROME_EXECUTABLE??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,timeout:20000})
  await checkpoint('browser-launched')
  for(const [width,height]of [[360,800],[390,480],[430,932]]){
    const context=await browser.newContext({viewport:{width,height},reducedMotion:'reduce'}),page=await context.newPage();observe(page)
    for(const selected of ['denied','foreign','expired','restricted','success']){
      if(retained.has(selected+'-'+width))continue
      const before=calls.refresh;await checkpoint('before-navigation-'+selected+'-'+width);await page.goto(origin+'/__oauth_fixture/callback?mode='+selected,{timeout:15000});await checkpoint('after-navigation-'+selected+'-'+width)
      if(selected==='success'){
        await expect(page).toHaveURL(origin+'/world');await expect.poll(()=>page.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.read().admission)).toBe('ADMITTED')
        const state=await page.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.read());assert(state.authenticated&&state.cleanups>=1);assert.equal(calls.refresh,before+1)
      }else{
        const title=selected==='denied'?'Sign-in cancelled':selected==='foreign'?'Sign-in could not be verified':selected==='restricted'?'Account entry unavailable':'Provider sign-in is unavailable'
        if(selected==='restricted'){
          await expect(page.getByRole('heading',{name:title,exact:true})).toBeVisible()
          await expect(page.getByRole('button',{name:'Sign out',exact:true})).toBeEnabled()
          assert.equal(await page.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.read().authenticated),true)
        }else await expect(page.getByRole('heading',{name:title,exact:true})).toBeFocused()
        assert.equal(calls.refresh,before+(selected==='expired'||selected==='restricted'?1:0))
        await page.evaluate(async()=>{await document.fonts.ready});await page.screenshot({path:resolve(evidence,selected+'-'+width+'.png'),fullPage:true})
        if(selected==='expired'){await page.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.go('/auth/callback?status=success'));await expect(page.getByRole('heading',{name:title,exact:true})).toBeFocused();assert.equal(calls.refresh,before+1)}
      }
      await hygiene(page);assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'Horizontal overflow')
      const cookies=await context.cookies();const refresh=cookies.find(cookie=>cookie.name==='mcr_oauth_fixture_refresh');assert(refresh?.httpOnly&&refresh.secure&&refresh.sameSite==='Lax')
      results.push({width,height,case:selected,status:'PASS'})
    }
    const logoutBefore=calls.logout;assert.equal(await page.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.logout()),true)
    assert.equal(calls.logout,logoutBefore+1);assert.equal(await page.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.read().authenticated),false)
    assert(!(await context.cookies()).some(cookie=>cookie.name==='mcr_oauth_fixture_refresh'))
    await context.close()
  }
  const targetContext=await browser.newContext({viewport:{width:390,height:844}}),targetPage=await targetContext.newPage()
  observe(targetPage)
  for(const target of ['/feed','/me','/meet/people']){
    const before=calls.refresh;await targetPage.goto(origin+'/__oauth_fixture/callback?mode=success&target='+encodeURIComponent(target))
    await expect(targetPage).toHaveURL(origin+target)
    await expect.poll(()=>targetPage.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.read().admission)).toBe('ADMITTED')
    assert.equal(calls.refresh,before+1);assert(await targetPage.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.read().authenticated))
    await hygiene(targetPage);results.push({width:390,height:844,case:'canonical-app-target-'+target,status:'PASS'})
  }
  await targetContext.close()
  const context=await browser.newContext(),page=await context.newPage();observe(page);const before=calls.refresh,cancelledBefore=calls.cancelled
  await page.goto(origin+'/__oauth_fixture/callback?mode=delay');await expect.poll(()=>calls.refresh).toBe(before+1)
  await page.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.go('/legal/privacy'));await page.waitForTimeout(800)
  assert.equal(await page.evaluate(()=>window.__OAUTH_HANDOFF_FIXTURE__.read().authenticated),false);assert.equal(calls.refresh,before+1);assert(calls.cancelled>cancelledBefore,'Route exit must cancel the actual refresh connection');await hygiene(page);await context.close()
  results.push({case:'route-exit-cancels-late-adoption',status:'PASS'});assert.deepEqual(errors,[])
  await writeFile(resolve(evidence,'report.json'),JSON.stringify({status:'PASS_CONTROLLED_CLIENT',browserVersion:browser.version(),results,calls,errors,retainedCases:retained.size,freshCases:results.length-retained.size,limitations:['Synthetic Node HTTP cookie/303 counterpart; actual Gateway OAuth/state cryptography/provider exchange NOT_RUN','Canonical App/router/Auth/admission/cookie/privacy implementation exercised; no production capability enabled','Retained cases keep their failed outer resource round; their provenance does not qualify that round','No device/live service/whole BE03 acceptance']},null,2)+'\n')
}catch(error){
  const pages=[]
  for(const context of browser?.contexts()??[])for(const page of context.pages()){
    try{pages.push(await page.evaluate(()=>({url:location.href,title:document.title,body:document.body.innerText.slice(0,6000),scripts:[...document.scripts].map(script=>script.src)})))}catch{pages.push({unavailable:true})}
  }
  await checkpoint('driver-failed')
  await writeFile(resolve(evidence,'failure-diagnostics.json'),scrub({errors,consoleErrors,requestFailures,pages,completedResults:results,calls})+'\n')
  await writeFile(resolve(evidence,'failure.txt'),String(error.stack??error));throw error
}
finally{await browser?.close();await server.close()}
