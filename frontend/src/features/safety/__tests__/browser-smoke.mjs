import assert from 'node:assert/strict'
import {mkdir,writeFile,readFile} from 'node:fs/promises'
import {createHash} from 'node:crypto'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const evidence=process.env.MCR_FE36_EVIDENCE??'/tmp/mcr-fe36-evidence'
await mkdir(evidence,{recursive:true})
const ids={actor:'11111111-1111-4111-8111-111111111111',event:'22222222-2222-4222-8222-222222222222',target:'33333333-3333-4333-8333-333333333333',report:'55555555-5555-4555-8555-555555555555',receipt:'66666666-6666-4666-8666-666666666666'}
const safeReport={report_id:ids.report,event_id:ids.event,target_type:'POST',target_id:ids.target,state:'OPEN',revision:1,created_at:'2026-11-04T00:59:00Z'}
let network='normal',key=null,browser
const requests=[],errors=[],results=[]
const dev=await createServer({server:{host:'127.0.0.1',port:0,strictPort:false},plugins:[{name:'FE36-explicit-synthetic-middleware',configureServer(server){server.middlewares.use(async(req,res,next)=>{
 if(!req.url?.startsWith('/api/'))return next()
 let body='';for await(const chunk of req)body+=chunk
 requests.push({path:req.url,method:req.method,body:body?JSON.parse(body):null,authorization:req.headers.authorization??null,intent:req.headers['x-mcr-intent']??null})
 res.setHeader('Cache-Control','no-store');res.setHeader('Content-Type','application/json')
 if(req.url==='/api/auth/logout'){res.statusCode=network==='logout-failure'?503:204;return res.end()}
 let value
 if(req.method==='POST'&&req.url.endsWith('/reports')){
  key=req.headers['idempotency-key'];if(network==='pending'){res.statusCode=202;value={receipt_id:ids.receipt,event_id:ids.event,actor_id:ids.actor,operation_id:'createConcertReport',idempotency_key:key,state:'PENDING',resource_id:null,resource_revision:null,failure_reason:null,created_at:new Date(Date.now()-1000).toISOString(),expires_at:new Date(Date.now()+60000).toISOString(),result_status:null}}else value=safeReport
 }else if(req.url.includes('/commands?'))value={receipt_id:ids.receipt,event_id:ids.event,actor_id:ids.actor,operation_id:'createConcertReport',idempotency_key:key,state:'SUCCEEDED',resource_id:ids.report,resource_revision:1,failure_reason:null,created_at:new Date(Date.now()-1000).toISOString(),expires_at:new Date(Date.now()+60000).toISOString(),result_status:200}
 else if(req.url.includes('/me/reports'))value={items:[safeReport],next_cursor:null}
 else if(['PUT','DELETE'].includes(req.method)&&req.url.includes('/me/blocks/'))value={target_user_id:ids.target,blocked_by_me:req.method==='PUT',revision:2}
 else if(req.url.includes('/me/blocks'))value={items:[{target_user_id:ids.target,blocked_by_me:true,revision:1}],next_cursor:null}
 else{res.statusCode=404;value={}}
 res.end(JSON.stringify(value))
})}}]})
try{
 await dev.listen();browser=await chromium.launch({executablePath:process.env.MCR_CHROME_EXECUTABLE??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 const url=`http://127.0.0.1:${dev.httpServer.address().port}/src/features/safety/__tests__/preview.html`
 for(const width of [360,390,430]){
  const context=await browser.newContext({viewport:{width,height:900},reducedMotion:'reduce'}),page=await context.newPage();page.on('pageerror',e=>errors.push(e.message))
  const open=async(query='')=>{await page.goto(url+query);await expect(page.getByRole('button',{name:'Open report',exact:true})).toBeVisible()}
  network='pending';await open();await page.getByRole('button',{name:'Open report',exact:true}).click();await expect(page.getByRole('combobox',{name:'Reason',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Close report'})).toBeFocused()
  await page.keyboard.press('Shift+Tab');assert(await page.evaluate(()=>document.querySelector('dialog').contains(document.activeElement)),'focus left dialog')
  await page.keyboard.press('Escape');await expect(page.getByRole('dialog')).toHaveCount(0);await expect(page.getByRole('button',{name:'Open report',exact:true})).toBeFocused()
  await page.getByRole('button',{name:'Open report',exact:true}).click();await page.getByRole('combobox',{name:'Reason',exact:true}).selectOption('PRIVACY');await page.getByLabel('What happened?',{exact:false}).fill('Native private explanation');await page.getByLabel('My ready report attachment').check();await page.getByRole('button',{name:'Submit report'}).click();await expect(page.getByRole('dialog').getByRole('status').filter({hasText:'Request pending'})).toBeVisible();await page.getByRole('dialog').getByRole('button',{name:'Check original request status'}).click();await expect(page.getByRole('dialog').getByRole('status').filter({hasText:'current status verified'})).toBeVisible();await page.screenshot({path:`${evidence}/report-${width}.png`,fullPage:true});await page.getByRole('button',{name:'Close report'}).click()
  const reports=requests.filter(r=>r.method==='POST'&&r.path.endsWith('/reports'));assert(reports.length>0);assert.deepEqual(reports.at(-1).body,{target_type:'POST',target_id:ids.target,reason:'PRIVACY',text:'Native private explanation',evidence_media_ids:['77777777-7777-4777-8777-777777777777']})
  await open();await page.getByRole('button',{name:'Blocked users',exact:true}).click();await page.getByRole('button',{name:'Unblock user',exact:true}).click();await expect(page.getByRole('button',{name:'Keep blocked'})).toBeFocused();await page.getByRole('button',{name:'Confirm unblock'}).click();await expect(page.getByRole('status').filter({hasText:'Private views were cleared'})).toBeVisible();await expect(page.getByRole('button',{name:'Unblock user'})).toHaveCount(0)
  await open();await page.getByRole('button',{name:'Block user',exact:true}).click();await expect(page.getByRole('button',{name:'Cancel block'})).toBeFocused();await page.getByRole('button',{name:'Confirm block'}).click();await expect(page.getByRole('status').filter({hasText:'Private views were cleared'})).toBeVisible()
  await open('?denied');await page.getByRole('button',{name:'Open report',exact:true}).click();await expect(page.getByRole('dialog').getByRole('status').filter({hasText:'target is unavailable'})).toBeVisible();await expect(page.getByRole('combobox',{name:'Reason',exact:true})).toHaveCount(0)
  await open('?unavailable');await page.getByRole('button',{name:'Account',exact:true}).click();await expect(page.getByRole('button',{name:'Request account deletion'})).toBeDisabled()
  await open();await page.getByRole('button',{name:'Account',exact:true}).click();await page.getByRole('button',{name:'Request account deletion'}).click();await expect(page.getByRole('button',{name:'Keep account'})).toBeFocused();await expect(page.getByRole('button',{name:'Confirm deletion request'})).toBeDisabled();await page.getByLabel('Deletion confirmation').fill('DELETE_MY_ACCOUNT');await page.getByRole('button',{name:'Confirm deletion request'}).click();await expect(page.getByRole('status').filter({hasText:'Deletion is pending'})).toBeVisible();await expect(page.getByRole('button',{name:'Request account deletion'})).toBeDisabled();await page.evaluate(()=>window.__FE36__.mode('failure'));await page.getByRole('button',{name:'Check deletion progress'}).click();await expect(page.getByRole('status').filter({hasText:'Deletion needs recovery'})).toBeVisible();assert(!(await page.locator('body').innerText()).includes('SECRET'));assert.equal(await page.evaluate(()=>window.__FE36__.deletionCalls()),1);await page.screenshot({path:`${evidence}/account-${width}.png`,fullPage:true});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'horizontal overflow')
  await page.evaluate(()=>window.__FE36__.mode('complete'));await page.getByRole('button',{name:'Check deletion progress'}).click();await expect(page.getByRole('status').filter({hasText:'Account deletion confirmed'})).toBeVisible()
  await open();await page.getByRole('button',{name:'Account',exact:true}).click();await page.evaluate(()=>window.__FE36__.mode('slow'));await page.getByRole('button',{name:'Request account deletion'}).click();await page.getByLabel('Deletion confirmation').fill('DELETE_MY_ACCOUNT');await page.getByRole('button',{name:'Confirm deletion request'}).click();await page.evaluate(()=>{window.__FE36__.changeIdentity();window.__FE36__.release()});await expect(page.getByRole('status').filter({hasText:'Private account views were cleared'})).toBeVisible();await expect(page.getByRole('list',{name:'Deletion progress'})).toHaveCount(0)
  network='logout-failure';await open();await page.getByRole('button',{name:'Account',exact:true}).click();await page.getByRole('button',{name:'Sign out',exact:true}).click();await expect(page.getByRole('button',{name:'Stay signed in'})).toBeFocused();await page.getByRole('button',{name:'Confirm sign out'}).click();await expect(page.getByRole('status').filter({hasText:'Server logout could not be confirmed'})).toBeVisible()
  results.push({width,reportReceiptReread:true,typedEvidence:true,deniedTarget:true,blockUnblockCleanup:true,nativeModalFocusEscapeRestore:true,deletionUnavailable:true,pendingPartialFailureComplete:true,deletionMutationOnce:true,foreignLateResponseFenced:true,unconfirmedLogout:true,overflow:false});await context.close()
 }
 assert.deepEqual(errors,[])
 const paths=['src/features/safety/contract.ts','src/features/safety/controller.ts','src/features/safety/panels.tsx','src/features/safety/Dialog.tsx','src/features/safety/safety.css','src/features/account/controller.ts','src/features/account/contract.ts','src/features/account/panel.tsx','src/features/safety/__tests__/preview.tsx','src/features/safety/__tests__/browser-smoke.mjs']
 const hashes=Object.fromEntries(await Promise.all(paths.map(async path=>[path,createHash('sha256').update(await readFile(path)).digest('hex')])))
 await writeFile(`${evidence}/result.json`,JSON.stringify({status:'PASS',synthetic:true,rootIntegrated:false,liveProvider:false,results,errors,requests,hashes},null,2)+'\n');console.log(JSON.stringify({status:'PASS',evidence,viewports:results.length,requests:requests.length,errors}))
}finally{await browser?.close();await dev.close()}
