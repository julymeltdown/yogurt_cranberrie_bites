import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../../..'),out=resolve(import.meta.dirname,'evidence/browser')
await mkdir(out,{recursive:true})
const server=await createServer({root:resolve(root,'frontend'),logLevel:'error',server:{host:'127.0.0.1',port:5197,strictPort:true}})
const report={scope:'FE22 isolated UI with deterministic C04 fetch fixtures through canonical Auth/privacy. No real backend/world/multiuser/device acceptance.',result:'FAIL',widths:[],checks:[]}
report.base=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim()
report.sourceHashes=Object.fromEntries(await Promise.all(['model/time.ts','model/controller.ts','model/contract.ts','ui/PartyAuthoring.tsx','ui/authoring.css','__tests__/fixtures/preview.tsx','__tests__/fixtures/harness.ts'].map(async p=>[p,createHash('sha256').update(await readFile(resolve(import.meta.dirname,'..',p))).digest('hex')])))
let browser
try { await server.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
for(const width of [360,390,430,1280]){
 const page=await browser.newPage({viewport:{width,height:900}}),errors=[];page.on('pageerror',e=>errors.push(e.message))
 await page.goto('http://127.0.0.1:5197/src/features/party-authoring/__tests__/fixtures/preview.html')
 const create=page.getByRole('button',{name:'Create party',exact:true})
 await create.click();await expect(page.getByRole('alert').first()).toBeVisible();assert.equal(await page.evaluate(()=>window.__FE22__.requests().length),0)
 await page.getByRole('textbox',{name:'Party title',exact:true}).fill('Encore friends')
 await page.getByRole('spinbutton',{name:'Capacity',exact:true}).fill('5')
 await page.getByLabel('Start time (KST)',{exact:true}).fill('2026-11-07T17:00')
 await page.getByLabel('End time (KST)',{exact:true}).fill('2026-11-07T18:00')
 await page.locator('#party-place').selectOption('00000000-0000-4000-8000-000000000030')
 await page.getByRole('button',{name:'Save session draft'}).click();await page.getByRole('textbox',{name:'Party title',exact:true}).fill('Changed')
 await page.getByRole('button',{name:'Restore draft'}).click();await expect(page.getByRole('textbox',{name:'Party title',exact:true})).toHaveValue('Encore friends')
 await page.screenshot({path:resolve(out,`${width}-form.png`),fullPage:true})
 assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'horizontal overflow')
 await page.evaluate(()=>window.__FE22__.mode('invalid'));await create.click();await expect(page.getByRole('status')).toContainText('server rejected')
 await expect(page.getByRole('textbox',{name:'Party title',exact:true})).toHaveValue('Encore friends')
 await page.evaluate(()=>window.__FE22__.mode('lost'));await create.click();await expect(page.getByRole('status')).toContainText('Outcome unknown');await expect(create).toBeDisabled()
 await page.screenshot({path:resolve(out,`${width}-uncertain.png`),fullPage:true})
 const count=await page.evaluate(()=>window.__FE22__.requests().filter(r=>r.method==='POST').length)
 await page.getByRole('button',{name:'Check original command'}).click();await expect(page.getByRole('status')).toContainText('server confirmed')
 assert.equal(await page.evaluate(()=>window.__FE22__.requests().filter(r=>r.method==='POST').length),count)
 await page.evaluate(()=>window.__FE22__.logout());await expect(page.getByRole('heading',{name:'Party authoring unavailable'})).toBeVisible();await expect(page.getByRole('textbox')).toHaveCount(0)
 assert.deepEqual(errors,[]);report.widths.push(width);await page.close()
}
report.checks=['RHF invalid input sends no request','session draft explicit restore','server error preserves draft','unknown write locks submit','receipt recovery never replays POST','logout scrubs input','no horizontal overflow at all widths','no browser errors'];report.result='PASS'
}finally{await browser?.close();await server.close();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n')}
console.log(JSON.stringify(report,null,2))
